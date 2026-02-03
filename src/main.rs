use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde::Serialize;
use std::io::{self, Write};

mod client;
mod config;

#[derive(Parser)]
#[command(name = "cnb")]
#[command(about = "CNB API command line interface")]
struct Cli {
    /// API base URL
    #[arg(long, env = "CNB_API_URL")]
    api_url: Option<String>,

    /// API token
    #[arg(long, env = "CNB_TOKEN")]
    token: Option<String>,

    /// Output in JSON format
    #[arg(long)]
    json: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// User operations
    User {
        #[command(subcommand)]
        action: UserAction,
    },
    /// Repository operations
    Repo {
        #[command(subcommand)]
        action: RepoAction,
    },
    /// Issue operations
    Issue {
        #[command(subcommand)]
        action: IssueAction,
    },
    /// Pull request operations
    Pr {
        #[command(subcommand)]
        action: PrAction,
    },
    /// Build operations (CI/CD)
    Build {
        #[command(subcommand)]
        action: BuildAction,
    },
}

#[derive(Subcommand)]
enum UserAction {
    /// Get current user info
    Info,
}

#[derive(Subcommand)]
enum RepoAction {
    /// List repositories
    List {
        /// Group slug
        #[arg(short, long)]
        group: Option<String>,
    },
    /// Get repository details
    Get {
        /// Repository slug (e.g., owner/repo)
        slug: String,
    },
    /// Create a new repository
    Create {
        /// Group slug to create the repository in
        group: String,
        /// Repository name
        name: String,
        /// Repository description
        #[arg(short, long)]
        description: Option<String>,
        /// Make repository private
        #[arg(long)]
        private: bool,
    },
    /// Delete a repository
    Delete {
        /// Repository slug (e.g., owner/repo)
        slug: String,
        /// Skip confirmation prompt
        #[arg(short, long)]
        force: bool,
    },
}

#[derive(Subcommand)]
enum IssueAction {
    /// List issues in a repository
    List {
        /// Repository slug (e.g., owner/repo)
        repo: String,
    },
    /// Get a specific issue
    Get {
        /// Repository slug (e.g., owner/repo)
        repo: String,
        /// Issue number
        number: i64,
    },
    /// Create a new issue
    Create {
        /// Repository slug (e.g., owner/repo)
        repo: String,
        /// Issue title
        #[arg(short, long)]
        title: String,
        /// Issue body
        #[arg(short, long)]
        body: Option<String>,
    },
}

#[derive(Subcommand)]
enum PrAction {
    /// List pull requests in a repository
    List {
        /// Repository slug (e.g., owner/repo)
        repo: String,
    },
    /// Get a specific pull request
    Get {
        /// Repository slug (e.g., owner/repo)
        repo: String,
        /// Pull request number
        number: i64,
    },
    /// Create a new pull request
    Create {
        /// Repository slug (e.g., owner/repo)
        repo: String,
        /// Pull request title
        #[arg(short, long)]
        title: String,
        /// Source branch
        #[arg(short, long)]
        source: String,
        /// Target branch
        #[arg(long)]
        target: String,
        /// Pull request body/description
        #[arg(short, long)]
        body: Option<String>,
    },
    /// Merge a pull request
    Merge {
        /// Repository slug (e.g., owner/repo)
        repo: String,
        /// Pull request number
        number: i64,
    },
}

#[derive(Subcommand)]
enum BuildAction {
    /// List builds in a repository
    List {
        /// Repository slug (e.g., owner/repo)
        repo: String,
    },
    /// Get a specific build
    Get {
        /// Repository slug (e.g., owner/repo)
        repo: String,
        /// Build number
        number: i64,
    },
    /// Trigger a new build
    Trigger {
        /// Repository slug (e.g., owner/repo)
        repo: String,
        /// Branch to build
        #[arg(short, long)]
        branch: String,
        /// Specific commit SHA (optional)
        #[arg(short, long)]
        commit: Option<String>,
    },
    /// Cancel a running build
    Cancel {
        /// Repository slug (e.g., owner/repo)
        repo: String,
        /// Build number
        number: i64,
    },
    /// Get build logs
    Logs {
        /// Repository slug (e.g., owner/repo)
        repo: String,
        /// Build number
        number: i64,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let file_config = config::AuthConfig::load().context("Failed to load auth config")?;
    let resolved = config::AuthConfig::resolve(
        cli.api_url.as_deref(),
        cli.token.as_deref(),
        file_config,
    );

    let client = client::Client::new(&resolved.api_url, resolved.token.as_deref())
        .context("Failed to initialize HTTP client")?;

    match cli.command {
        Commands::User { action } => handle_user(&client, action, cli.json).await?,
        Commands::Repo { action } => handle_repo(&client, action, cli.json).await?,
        Commands::Issue { action } => handle_issue(&client, action, cli.json).await?,
        Commands::Pr { action } => handle_pr(&client, action, cli.json).await?,
        Commands::Build { action } => handle_build(&client, action, cli.json).await?,
    }

    Ok(())
}

async fn handle_user(client: &client::Client, action: UserAction, json: bool) -> Result<()> {
    match action {
        UserAction::Info => {
            let user = client.get_current_user().await?;
            output_one(&user, json)
        }
    }
}

async fn handle_repo(client: &client::Client, action: RepoAction, json: bool) -> Result<()> {
    match action {
        RepoAction::List { group } => {
            let repos = client.list_repos(group.as_deref()).await?;
            output_list(&repos, json)
        }
        RepoAction::Get { slug } => {
            let repo = client.get_repo(&slug).await?;
            output_one(&repo, json)
        }
        RepoAction::Create { group, name, description, private } => {
            let repo = client
                .create_repo(&group, &name, description.as_deref(), private)
                .await?;
            output_created("Created", &repo, json)
        }
        RepoAction::Delete { slug, force } => {
            if !force {
                eprint!("Delete repository '{}'? [y/N] ", slug);
                io::stderr().flush()?;
                let mut input = String::new();
                io::stdin().read_line(&mut input)?;
                if !input.trim().eq_ignore_ascii_case("y") {
                    println!("Aborted.");
                    return Ok(());
                }
            }
            client.delete_repo(&slug).await?;
            if json {
                output_json(&serde_json::json!({ "deleted": slug }))
            } else {
                println!("Deleted: {}", slug);
                Ok(())
            }
        }
    }
}

async fn handle_issue(client: &client::Client, action: IssueAction, json: bool) -> Result<()> {
    match action {
        IssueAction::List { repo } => {
            let issues = client.list_issues(&repo).await?;
            output_list(&issues, json)
        }
        IssueAction::Get { repo, number } => {
            let issue = client.get_issue(&repo, number).await?;
            output_one(&issue, json)
        }
        IssueAction::Create { repo, title, body } => {
            let issue = client.create_issue(&repo, &title, body.as_deref()).await?;
            output_created("Created", &issue, json)
        }
    }
}

async fn handle_pr(client: &client::Client, action: PrAction, json: bool) -> Result<()> {
    match action {
        PrAction::List { repo } => {
            let prs = client.list_prs(&repo).await?;
            output_list(&prs, json)
        }
        PrAction::Get { repo, number } => {
            let pr = client.get_pr(&repo, number).await?;
            output_one(&pr, json)
        }
        PrAction::Create { repo, title, source, target, body } => {
            let pr = client
                .create_pr(&repo, &title, &source, &target, body.as_deref())
                .await?;
            output_created("Created", &pr, json)
        }
        PrAction::Merge { repo, number } => {
            let pr = client.merge_pr(&repo, number).await?;
            output_created("Merged", &pr, json)
        }
    }
}

async fn handle_build(client: &client::Client, action: BuildAction, json: bool) -> Result<()> {
    match action {
        BuildAction::List { repo } => {
            let builds = client.list_builds(&repo).await?;
            output_list(&builds, json)
        }
        BuildAction::Get { repo, number } => {
            let build = client.get_build(&repo, number).await?;
            output_one(&build, json)
        }
        BuildAction::Trigger { repo, branch, commit } => {
            let build = client.trigger_build(&repo, &branch, commit.as_deref()).await?;
            output_created("Triggered", &build, json)
        }
        BuildAction::Cancel { repo, number } => {
            let build = client.cancel_build(&repo, number).await?;
            output_created("Cancelled", &build, json)
        }
        BuildAction::Logs { repo, number } => {
            let logs = client.get_build_logs(&repo, number).await?;
            if json {
                output_json(&serde_json::json!({ "logs": logs }))
            } else {
                println!("{}", logs);
                Ok(())
            }
        }
    }
}

fn output_one<T>(value: &T, json: bool) -> Result<()>
where
    T: Serialize + std::fmt::Display,
{
    if json {
        output_json(value)
    } else {
        println!("{}", value);
        Ok(())
    }
}

fn output_list<T>(values: &[T], json: bool) -> Result<()>
where
    T: Serialize + std::fmt::Display,
{
    if json {
        output_json(values)
    } else {
        for value in values {
            println!("{}", value);
        }
        Ok(())
    }
}

fn output_created<T>(label: &str, value: &T, json: bool) -> Result<()>
where
    T: Serialize + std::fmt::Display,
{
    if json {
        output_json(value)
    } else {
        println!("{label}: {value}");
        Ok(())
    }
}

fn output_json<T: Serialize + ?Sized>(value: &T) -> Result<()> {
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    serde_json::to_writer_pretty(&mut handle, value)?;
    writeln!(handle)?;
    Ok(())
}
