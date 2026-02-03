use anyhow::{bail, Context, Result};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use reqwest::Url;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::Duration;

pub struct Client {
    http: reqwest::Client,
    base_url: Url,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub nickname: Option<String>,
    pub email: Option<String>,
}

impl fmt::Display for User {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.username, self.nickname.as_deref().unwrap_or("-"))?;
        if let Some(email) = &self.email {
            write!(f, " <{}>", email)?;
        }
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Repo {
    pub id: i64,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub private: bool,
}

impl fmt::Display for Repo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let visibility = if self.private { "[private]" } else { "[public]" };
        write!(f, "{:<30} {:<10} {}", self.slug, visibility, self.description.as_deref().unwrap_or(""))
    }
}

/// Request body for creating a repository
#[derive(Debug, Serialize)]
struct CreateRepoRequest<'a> {
    name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<&'a str>,
    private: bool,
}

/// Issue in a repository
#[derive(Debug, Serialize, Deserialize)]
pub struct Issue {
    pub id: i64,
    pub number: i64,
    pub title: String,
    pub state: String,
    pub author: User,
    pub created_at: String,
}

impl fmt::Display for Issue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "#{:<6} [{:<6}] {:<50} @{}",
            self.number,
            self.state,
            self.title,
            self.author.username
        )
    }
}

#[derive(Debug, Serialize)]
struct CreateIssueRequest<'a> {
    title: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<&'a str>,
}

/// Pull request state
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PrState {
    Open,
    Closed,
    Merged,
}

impl fmt::Display for PrState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PrState::Open => write!(f, "open"),
            PrState::Closed => write!(f, "closed"),
            PrState::Merged => write!(f, "merged"),
        }
    }
}

/// Pull request in a repository
#[derive(Debug, Serialize, Deserialize)]
pub struct PullRequest {
    pub id: i64,
    pub number: i64,
    pub title: String,
    pub state: PrState,
    pub source_branch: String,
    pub target_branch: String,
    pub author: String,
    pub created_at: String,
}

impl fmt::Display for PullRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "#{:<6} [{:<6}] {:<50} ({} -> {}) @{}",
            self.number,
            self.state,
            self.title,
            self.source_branch,
            self.target_branch,
            self.author
        )
    }
}

#[derive(Debug, Serialize)]
struct CreatePrRequest<'a> {
    title: &'a str,
    source_branch: &'a str,
    target_branch: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<&'a str>,
}

/// Build status
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BuildStatus {
    Pending,
    Running,
    Success,
    Failed,
    Cancelled,
}

impl fmt::Display for BuildStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BuildStatus::Pending => write!(f, "pending"),
            BuildStatus::Running => write!(f, "running"),
            BuildStatus::Success => write!(f, "success"),
            BuildStatus::Failed => write!(f, "failed"),
            BuildStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// Build record
#[derive(Debug, Serialize, Deserialize)]
pub struct Build {
    pub id: i64,
    pub number: i64,
    pub status: BuildStatus,
    pub branch: String,
    pub commit_sha: String,
    pub created_at: String,
    pub finished_at: Option<String>,
}

impl fmt::Display for Build {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "#{:<6} [{:<9}] {:<20} {}",
            self.number,
            self.status,
            self.branch,
            &self.commit_sha[..8.min(self.commit_sha.len())]
        )
    }
}

/// Request body for triggering a build
#[derive(Debug, Serialize)]
struct TriggerBuildRequest<'a> {
    branch: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    commit_sha: Option<&'a str>,
}

impl Client {
    pub fn new(base_url: &str, token: Option<&str>) -> Result<Self> {
        let mut headers = HeaderMap::new();
        if let Some(t) = token {
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {t}"))
                    .context("Invalid token value for Authorization header")?,
            );
        }

        let http = reqwest::Client::builder()
            .default_headers(headers)
            .user_agent(format!("cnb-cli/{}", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build()
            .context("Failed to build HTTP client")?;

        let base_url = normalize_base_url(base_url)?;

        Ok(Self { http, base_url })
    }

    fn endpoint(&self, segments: Vec<String>) -> Result<Url> {
        let mut url = self.base_url.clone();
        {
            let mut path = url
                .path_segments_mut()
                .map_err(|_| anyhow::anyhow!("base URL cannot be a base"))?;
            for segment in segments {
                path.push(&segment);
            }
        }
        Ok(url)
    }

    async fn send_json<T: DeserializeOwned>(
        &self,
        req: reqwest::RequestBuilder,
        action: &'static str,
    ) -> Result<T> {
        let resp = req
            .send()
            .await
            .with_context(|| format!("Failed to {action}"))?
            .error_for_status()
            .with_context(|| format!("{action} returned error status"))?;
        resp.json()
            .await
            .with_context(|| format!("Failed to decode {action} response"))
    }

    async fn send_ok(&self, req: reqwest::RequestBuilder, action: &'static str) -> Result<()> {
        req.send()
            .await
            .with_context(|| format!("Failed to {action}"))?
            .error_for_status()
            .with_context(|| format!("{action} returned error status"))?;
        Ok(())
    }

    async fn send_text(&self, req: reqwest::RequestBuilder, action: &'static str) -> Result<String> {
        let resp = req
            .send()
            .await
            .with_context(|| format!("Failed to {action}"))?
            .error_for_status()
            .with_context(|| format!("{action} returned error status"))?;
        resp.text()
            .await
            .with_context(|| format!("Failed to decode {action} response"))
    }

    pub async fn get_current_user(&self) -> Result<User> {
        let url = self.endpoint(vec!["user".to_string()])?;
        let req = self.http.get(url);
        self.send_json(req, "get current user").await
    }

    pub async fn list_repos(&self, group: Option<&str>) -> Result<Vec<Repo>> {
        let segments = match group {
            Some(g) => vec!["groups".to_string(), g.to_string(), "repos".to_string()],
            None => vec!["user".to_string(), "repos".to_string()],
        };
        let url = self.endpoint(segments)?;
        let req = self.http.get(url);
        self.send_json(req, "list repos").await
    }

    /// Get repository details by slug
    pub async fn get_repo(&self, slug: &str) -> Result<Repo> {
        let url = self.endpoint(slug_segments(slug)?)?;
        let req = self.http.get(url);
        self.send_json(req, "get repo").await
    }

    /// Create a new repository in a group
    pub async fn create_repo(
        &self,
        group: &str,
        name: &str,
        description: Option<&str>,
        private: bool,
    ) -> Result<Repo> {
        let url = self.endpoint(vec!["groups".to_string(), group.to_string(), "repos".to_string()])?;
        let req = CreateRepoRequest { name, description, private };
        let req = self.http.post(url).json(&req);
        self.send_json(req, "create repo").await
    }

    /// Delete a repository by slug
    pub async fn delete_repo(&self, slug: &str) -> Result<()> {
        let url = self.endpoint(slug_segments(slug)?)?;
        let req = self.http.delete(url);
        self.send_ok(req, "delete repo").await
    }

    /// List issues for a repository
    pub async fn list_issues(&self, repo: &str) -> Result<Vec<Issue>> {
        let mut segments = repo_segments(repo)?;
        segments.push("issues".to_string());
        let url = self.endpoint(segments)?;
        let req = self.http.get(url);
        self.send_json(req, "list issues").await
    }

    /// Get a specific issue by number
    pub async fn get_issue(&self, repo: &str, number: i64) -> Result<Issue> {
        let mut segments = repo_segments(repo)?;
        segments.push("issues".to_string());
        segments.push(number.to_string());
        let url = self.endpoint(segments)?;
        let req = self.http.get(url);
        self.send_json(req, "get issue").await
    }

    /// Create a new issue
    pub async fn create_issue(&self, repo: &str, title: &str, body: Option<&str>) -> Result<Issue> {
        let mut segments = repo_segments(repo)?;
        segments.push("issues".to_string());
        let url = self.endpoint(segments)?;
        let req = CreateIssueRequest { title, body };
        let req = self.http.post(url).json(&req);
        self.send_json(req, "create issue").await
    }

    /// List pull requests for a repository
    pub async fn list_prs(&self, repo: &str) -> Result<Vec<PullRequest>> {
        let mut segments = repo_segments(repo)?;
        segments.push("pulls".to_string());
        let url = self.endpoint(segments)?;
        let req = self.http.get(url);
        self.send_json(req, "list pull requests").await
    }

    /// Get a specific pull request by number
    pub async fn get_pr(&self, repo: &str, number: i64) -> Result<PullRequest> {
        let mut segments = repo_segments(repo)?;
        segments.push("pulls".to_string());
        segments.push(number.to_string());
        let url = self.endpoint(segments)?;
        let req = self.http.get(url);
        self.send_json(req, "get pull request").await
    }

    /// Create a new pull request
    pub async fn create_pr(
        &self,
        repo: &str,
        title: &str,
        source: &str,
        target: &str,
        body: Option<&str>,
    ) -> Result<PullRequest> {
        let mut segments = repo_segments(repo)?;
        segments.push("pulls".to_string());
        let url = self.endpoint(segments)?;
        let req = CreatePrRequest {
            title,
            source_branch: source,
            target_branch: target,
            body,
        };
        let req = self.http.post(url).json(&req);
        self.send_json(req, "create pull request").await
    }

    /// Merge a pull request
    pub async fn merge_pr(&self, repo: &str, number: i64) -> Result<PullRequest> {
        let mut segments = repo_segments(repo)?;
        segments.push("pulls".to_string());
        segments.push(number.to_string());
        segments.push("merge".to_string());
        let url = self.endpoint(segments)?;
        let req = self.http.put(url);
        self.send_json(req, "merge pull request").await
    }

    /// List builds for a repository
    pub async fn list_builds(&self, repo: &str) -> Result<Vec<Build>> {
        let mut segments = repo_segments(repo)?;
        segments.push("builds".to_string());
        let url = self.endpoint(segments)?;
        let req = self.http.get(url);
        self.send_json(req, "list builds").await
    }

    /// Get a specific build by number
    pub async fn get_build(&self, repo: &str, number: i64) -> Result<Build> {
        let mut segments = repo_segments(repo)?;
        segments.push("builds".to_string());
        segments.push(number.to_string());
        let url = self.endpoint(segments)?;
        let req = self.http.get(url);
        self.send_json(req, "get build").await
    }

    /// Trigger a new build
    pub async fn trigger_build(&self, repo: &str, branch: &str, commit: Option<&str>) -> Result<Build> {
        let mut segments = repo_segments(repo)?;
        segments.push("builds".to_string());
        let url = self.endpoint(segments)?;
        let req = TriggerBuildRequest { branch, commit_sha: commit };
        let req = self.http.post(url).json(&req);
        self.send_json(req, "trigger build").await
    }

    /// Cancel a build
    pub async fn cancel_build(&self, repo: &str, number: i64) -> Result<Build> {
        let mut segments = repo_segments(repo)?;
        segments.push("builds".to_string());
        segments.push(number.to_string());
        segments.push("cancel".to_string());
        let url = self.endpoint(segments)?;
        let req = self.http.post(url);
        self.send_json(req, "cancel build").await
    }

    /// Get build logs
    pub async fn get_build_logs(&self, repo: &str, number: i64) -> Result<String> {
        let mut segments = repo_segments(repo)?;
        segments.push("builds".to_string());
        segments.push(number.to_string());
        segments.push("logs".to_string());
        let url = self.endpoint(segments)?;
        let req = self.http.get(url);
        self.send_text(req, "get build logs").await
    }
}

fn normalize_base_url(base_url: &str) -> Result<Url> {
    let mut url = Url::parse(base_url).context("Invalid API base URL")?;
    if !url.path().ends_with('/') {
        let trimmed = url.path().trim_end_matches('/');
        url.set_path(&format!("{trimmed}/"));
    }
    Ok(url)
}

fn slug_segments(slug: &str) -> Result<Vec<String>> {
    let segments: Vec<String> = slug
        .split('/')
        .filter(|part| !part.is_empty())
        .map(|part| part.to_string())
        .collect();
    if segments.len() < 2 {
        bail!("invalid repository slug: {slug}");
    }
    Ok(segments)
}

fn repo_segments(repo: &str) -> Result<Vec<String>> {
    slug_segments(repo)
}
