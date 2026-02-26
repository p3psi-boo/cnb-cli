use anyhow::{anyhow, bail, Context, Result};
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

const MAX_TEXT_BODY_BYTES: u64 = 8 * 1024 * 1024;

// Keep error messages actionable and consistent across endpoints.
fn auth_hint(status: reqwest::StatusCode) -> Option<&'static str> {
    match status {
        reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN => Some(
            "Hint: set CNB_TOKEN (env/--token) or configure $XDG_CONFIG_HOME/cnb/auth.json",
        ),
        _ => None,
    }
}

fn summarize_body(body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    const LIMIT: usize = 2048;
    if trimmed.len() <= LIMIT {
        return trimmed.to_string();
    }
    format!("{}…(truncated)", &trimmed[..LIMIT])
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
            .with_context(|| format!("Failed to {action}"))?;

        let status = resp.status();
        let url = resp.url().clone();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            let body = summarize_body(&body);
            let mut msg = format!("{action} failed: {status} {url}");
            if let Some(hint) = auth_hint(status) {
                msg.push('\n');
                msg.push_str(hint);
            }
            if !body.is_empty() {
                msg.push('\n');
                msg.push_str(&body);
            }
            return Err(anyhow!(msg));
        }

        resp.json()
            .await
            .with_context(|| format!("Failed to decode {action} response"))
    }

    async fn send_ok(&self, req: reqwest::RequestBuilder, action: &'static str) -> Result<()> {
        let resp = req
            .send()
            .await
            .with_context(|| format!("Failed to {action}"))?;

        let status = resp.status();
        let url = resp.url().clone();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            let body = summarize_body(&body);
            let mut msg = format!("{action} failed: {status} {url}");
            if let Some(hint) = auth_hint(status) {
                msg.push('\n');
                msg.push_str(hint);
            }
            if !body.is_empty() {
                msg.push('\n');
                msg.push_str(&body);
            }
            return Err(anyhow!(msg));
        }
        Ok(())
    }

    async fn send_text(&self, req: reqwest::RequestBuilder, action: &'static str) -> Result<String> {
        let resp = req
            .send()
            .await
            .with_context(|| format!("Failed to {action}"))?;

        let status = resp.status();
        let url = resp.url().clone();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            let body = summarize_body(&body);
            let mut msg = format!("{action} failed: {status} {url}");
            if let Some(hint) = auth_hint(status) {
                msg.push('\n');
                msg.push_str(hint);
            }
            if !body.is_empty() {
                msg.push('\n');
                msg.push_str(&body);
            }
            return Err(anyhow!(msg));
        }

        if let Some(content_length) = resp.content_length() {
            if content_length > MAX_TEXT_BODY_BYTES {
                bail!(
                    "{action} response too large: {} bytes (limit: {} bytes)",
                    content_length,
                    MAX_TEXT_BODY_BYTES
                );
            }
        }

        resp.text()
            .await
            .with_context(|| format!("Failed to decode {action} response"))
    }

    pub async fn get_current_user(&self) -> Result<User> {
        let url = self.endpoint(vec!["user".to_string()])?;
        let req = self.http.get(url);
        self.send_json(req, "get current user").await
    }

    pub async fn list_repos(
        &self,
        group: Option<&str>,
        page: Option<u32>,
        page_size: Option<u32>,
    ) -> Result<Vec<Repo>> {
        let segments = match group {
            Some(g) => vec!["groups".to_string(), g.to_string(), "repos".to_string()],
            None => vec!["user".to_string(), "repos".to_string()],
        };
        let url = self.endpoint(segments)?;
        let req = with_pagination(self.http.get(url), page, page_size);
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
    pub async fn list_issues(&self, repo: &str, page: Option<u32>, page_size: Option<u32>) -> Result<Vec<Issue>> {
        let mut segments = repo_resource_segments(repo)?;
        segments.push("issues".to_string());
        let url = self.endpoint(segments)?;
        let req = with_pagination(self.http.get(url), page, page_size);
        self.send_json(req, "list issues").await
    }

    /// Get a specific issue by number
    pub async fn get_issue(&self, repo: &str, number: i64) -> Result<Issue> {
        let mut segments = repo_resource_segments(repo)?;
        segments.push("issues".to_string());
        segments.push(number.to_string());
        let url = self.endpoint(segments)?;
        let req = self.http.get(url);
        self.send_json(req, "get issue").await
    }

    /// Create a new issue
    pub async fn create_issue(&self, repo: &str, title: &str, body: Option<&str>) -> Result<Issue> {
        let mut segments = repo_resource_segments(repo)?;
        segments.push("issues".to_string());
        let url = self.endpoint(segments)?;
        let req = CreateIssueRequest { title, body };
        let req = self.http.post(url).json(&req);
        self.send_json(req, "create issue").await
    }

    /// List pull requests for a repository
    pub async fn list_prs(&self, repo: &str, page: Option<u32>, page_size: Option<u32>) -> Result<Vec<PullRequest>> {
        let mut segments = repo_resource_segments(repo)?;
        segments.push("pulls".to_string());
        let url = self.endpoint(segments)?;
        let req = with_pagination(self.http.get(url), page, page_size);
        self.send_json(req, "list pull requests").await
    }

    /// Get a specific pull request by number
    pub async fn get_pr(&self, repo: &str, number: i64) -> Result<PullRequest> {
        let mut segments = repo_resource_segments(repo)?;
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
        let mut segments = repo_resource_segments(repo)?;
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
        let mut segments = repo_resource_segments(repo)?;
        segments.push("pulls".to_string());
        segments.push(number.to_string());
        segments.push("merge".to_string());
        let url = self.endpoint(segments)?;
        let req = self.http.put(url);
        self.send_json(req, "merge pull request").await
    }

    /// List pipeline builds for a repository (OpenAPI: GET /{repo}/-/build/logs)
    pub async fn list_builds(
        &self,
        repo: &str,
        page: Option<u32>,
        page_size: Option<u32>,
    ) -> Result<Vec<LogInfo>> {
        let mut segments = repo_resource_segments(repo)?;
        segments.push("build".to_string());
        segments.push("logs".to_string());
        let url = self.endpoint(segments)?;
        let req = with_pagination(self.http.get(url), page, page_size);
        let result: BuildLogsResult = self.send_json(req, "list builds").await?;
        Ok(result.data)
    }

    /// Get build status by build serial number (OpenAPI: GET /{repo}/-/build/status/{sn})
    pub async fn get_build_status(&self, repo: &str, sn: &str) -> Result<BuildStatusResult> {
        let mut segments = repo_resource_segments(repo)?;
        segments.push("build".to_string());
        segments.push("status".to_string());
        segments.push(sn.to_string());
        let url = self.endpoint(segments)?;
        let req = self.http.get(url);
        self.send_json(req, "get build status").await
    }

    /// Trigger a new build (OpenAPI: POST /{repo}/-/build/start)
    pub async fn trigger_build(&self, repo: &str, branch: &str, sha: Option<&str>) -> Result<BuildResult> {
        let mut segments = repo_resource_segments(repo)?;
        segments.push("build".to_string());
        segments.push("start".to_string());
        let url = self.endpoint(segments)?;

        let req = StartBuildRequest {
            branch,
            sha,
            // Keep the request minimal; callers can extend later.
            tag: None,
            event: None,
            config: None,
            env: None,
            sync: None,
        };
        let req = self.http.post(url).json(&req);
        self.send_json(req, "trigger build").await
    }

    /// Cancel/stop a build by build serial number (OpenAPI: POST /{repo}/-/build/stop/{sn})
    pub async fn cancel_build(&self, repo: &str, sn: &str) -> Result<BuildResult> {
        let mut segments = repo_resource_segments(repo)?;
        segments.push("build".to_string());
        segments.push("stop".to_string());
        segments.push(sn.to_string());
        let url = self.endpoint(segments)?;
        let req = self.http.post(url);
        self.send_json(req, "cancel build").await
    }

    /// Get build logs (OpenAPI: GET /{repo}/-/build/logs/{sn})
    pub async fn get_build_logs(&self, repo: &str, sn: &str) -> Result<String> {
        let mut segments = repo_resource_segments(repo)?;
        segments.push("build".to_string());
        segments.push("logs".to_string());
        segments.push(sn.to_string());
        let url = self.endpoint(segments)?;
        let req = self.http.get(url);

        // The API returns JSON for build logs; render it as joined lines for human output.
        // If decoding fails (server change), fall back to raw text.
        let text = self.send_text(req, "get build logs").await?;
        if let Ok(stage) = serde_json::from_str::<BuildStageResult>(&text) {
            return Ok(stage.content.join("\n"));
        }
        Ok(text)
    }
}

/// Request body for triggering a build (OpenAPI: dto.StartBuildReq)
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct StartBuildRequest<'a> {
    branch: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    sha: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tag: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    event: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    config: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    env: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sync: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildResult {
    #[serde(default)]
    pub build_log_url: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub sn: Option<String>,
    #[serde(default)]
    pub success: Option<bool>,
}

impl fmt::Display for BuildResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sn = self.sn.as_deref().unwrap_or("-");
        let success = self.success.unwrap_or(false);
        write!(f, "sn={} success={}", sn, success)?;
        if let Some(msg) = &self.message {
            if !msg.trim().is_empty() {
                write!(f, " {msg}")?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BuildLogsResult {
    #[serde(default)]
    data: Vec<LogInfo>,
    #[serde(default)]
    total: Option<i64>,
    #[serde(default)]
    timestamp: Option<i64>,
    #[serde(default)]
    init: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogInfo {
    #[serde(default)]
    pub sn: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub sha: Option<String>,
    #[serde(default)]
    pub create_time: Option<i64>,
    #[serde(default)]
    pub duration: Option<i64>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub commit_title: Option<String>,
    #[serde(default)]
    pub build_log_url: Option<String>,

    // Keep forward-compatible with additional fields.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl fmt::Display for LogInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sn = self.sn.as_deref().unwrap_or("-");
        let status = self.status.as_deref().unwrap_or("-");
        let sha = self
            .sha
            .as_deref()
            .map(|s| &s[..8.min(s.len())])
            .unwrap_or("-");
        let title = self
            .title
            .as_deref()
            .or(self.commit_title.as_deref())
            .unwrap_or("");
        write!(f, "{sn:<18} [{status:<10}] {sha} {title}")
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildStatusResult {
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub pipelines_status: Option<serde_json::Value>,
}

impl fmt::Display for BuildStatusResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.status.as_deref().unwrap_or("-"))
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BuildStageResult {
    #[serde(default)]
    content: Vec<String>,
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

// OpenAPI uses /{repo}/-/... for most repository-scoped resources.
fn repo_resource_segments(repo: &str) -> Result<Vec<String>> {
    let mut segments = repo_segments(repo)?;
    segments.push("-".to_string());
    Ok(segments)
}

fn with_pagination(
    req: reqwest::RequestBuilder,
    page: Option<u32>,
    page_size: Option<u32>,
) -> reqwest::RequestBuilder {
    let mut params: Vec<(&str, u32)> = Vec::with_capacity(2);
    if let Some(p) = page {
        params.push(("page", p));
    }
    if let Some(ps) = page_size {
        params.push(("page_size", ps));
    }

    if params.is_empty() {
        req
    } else {
        req.query(&params)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_segments_requires_owner_and_repo() {
        assert!(slug_segments("a/b").is_ok());
        assert!(slug_segments("a").is_err());
        assert!(slug_segments("/a/b/").is_ok());
    }

    #[test]
    fn normalize_base_url_appends_slash() {
        let url = normalize_base_url("https://api.cnb.cool").unwrap();
        assert_eq!(url.as_str(), "https://api.cnb.cool/");

        let url = normalize_base_url("https://api.cnb.cool/").unwrap();
        assert_eq!(url.as_str(), "https://api.cnb.cool/");
    }

    #[test]
    fn repo_resource_segments_inserts_dash() {
        let segs = repo_resource_segments("owner/repo").unwrap();
        assert_eq!(segs, vec!["owner", "repo", "-"]);
    }
}
