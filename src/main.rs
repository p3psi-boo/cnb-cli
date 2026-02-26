use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

mod client;
mod cmd;
mod config;
mod output;
mod pagination;

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
        action: cmd::user::UserAction,
    },
    /// Repository operations
    Repo {
        #[command(subcommand)]
        action: cmd::repo::RepoAction,
    },
    /// Issue operations
    Issue {
        #[command(subcommand)]
        action: cmd::issue::IssueAction,
    },
    /// Pull request operations
    Pr {
        #[command(subcommand)]
        action: cmd::pr::PrAction,
    },
    /// Build operations (CI/CD)
    Build {
        #[command(subcommand)]
        action: cmd::build::BuildAction,
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
        Commands::User { action } => cmd::user::handle(&client, action, cli.json).await?,
        Commands::Repo { action } => cmd::repo::handle(&client, action, cli.json).await?,
        Commands::Issue { action } => cmd::issue::handle(&client, action, cli.json).await?,
        Commands::Pr { action } => cmd::pr::handle(&client, action, cli.json).await?,
        Commands::Build { action } => cmd::build::handle(&client, action, cli.json).await?,
    }

    Ok(())
}
