use anyhow::Result;
use clap::Subcommand;

use crate::{
    client,
    output::{output_created, output_json, output_list, output_one},
    pagination::{DEFAULT_LIST_PAGE_SIZE, collect_all_pages},
};

#[derive(Subcommand)]
pub enum BuildAction {
    /// List builds in a repository
    List {
        /// Repository slug (e.g., owner/repo)
        repo: String,

        /// Pagination page number
        #[arg(long)]
        page: Option<u32>,

        /// Pagination page size
        #[arg(long, value_name = "N")]
        page_size: Option<u32>,

        /// Fetch all pages (auto-paginate)
        #[arg(long)]
        all: bool,
    },
    /// Get a specific build
    Get {
        /// Repository slug (e.g., owner/repo)
        repo: String,
        /// Build serial number (sn)
        sn: String,
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
        /// Build serial number (sn)
        sn: String,
    },
    /// Get build logs
    Logs {
        /// Repository slug (e.g., owner/repo)
        repo: String,
        /// Build serial number (sn)
        sn: String,
    },
}

pub async fn handle(client: &client::Client, action: BuildAction, json: bool) -> Result<()> {
    match action {
        BuildAction::List {
            repo,
            page,
            page_size,
            all,
        } => {
            if all {
                let collected = collect_all_pages(page, page_size, DEFAULT_LIST_PAGE_SIZE, |p, ps| {
                    client.list_builds(&repo, Some(p), Some(ps))
                })
                .await?;
                output_list(&collected, json)
            } else {
                let builds = client.list_builds(&repo, page, page_size).await?;
                output_list(&builds, json)
            }
        }
        BuildAction::Get { repo, sn } => {
            let status = client.get_build_status(&repo, &sn).await?;
            output_one(&status, json)
        }
        BuildAction::Trigger {
            repo,
            branch,
            commit,
        } => {
            let build = client.trigger_build(&repo, &branch, commit.as_deref()).await?;
            output_created("Triggered", &build, json)
        }
        BuildAction::Cancel { repo, sn } => {
            let build = client.cancel_build(&repo, &sn).await?;
            output_created("Cancelled", &build, json)
        }
        BuildAction::Logs { repo, sn } => {
            let logs = client.get_build_logs(&repo, &sn).await?;
            if json {
                output_json(&serde_json::json!({ "logs": logs }))
            } else {
                println!("{}", logs);
                Ok(())
            }
        }
    }
}
