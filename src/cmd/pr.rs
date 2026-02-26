use anyhow::Result;
use clap::Subcommand;

use crate::{
    client,
    output::{output_created, output_list, output_one},
    pagination::{DEFAULT_LIST_PAGE_SIZE, collect_all_pages},
};

#[derive(Subcommand)]
pub enum PrAction {
    /// List pull requests in a repository
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

pub async fn handle(client: &client::Client, action: PrAction, json: bool) -> Result<()> {
    match action {
        PrAction::List {
            repo,
            page,
            page_size,
            all,
        } => {
            if all {
                let collected = collect_all_pages(page, page_size, DEFAULT_LIST_PAGE_SIZE, |p, ps| {
                    client.list_prs(&repo, Some(p), Some(ps))
                })
                .await?;
                output_list(&collected, json)
            } else {
                let prs = client.list_prs(&repo, page, page_size).await?;
                output_list(&prs, json)
            }
        }
        PrAction::Get { repo, number } => {
            let pr = client.get_pr(&repo, number).await?;
            output_one(&pr, json)
        }
        PrAction::Create {
            repo,
            title,
            source,
            target,
            body,
        } => {
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
