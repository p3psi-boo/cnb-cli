use anyhow::Result;
use clap::Subcommand;

use crate::{
    client,
    output::{output_created, output_list, output_one},
    pagination::{DEFAULT_LIST_PAGE_SIZE, collect_all_pages},
};

#[derive(Subcommand)]
pub enum IssueAction {
    /// List issues in a repository
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

pub async fn handle(client: &client::Client, action: IssueAction, json: bool) -> Result<()> {
    match action {
        IssueAction::List {
            repo,
            page,
            page_size,
            all,
        } => {
            if all {
                let collected = collect_all_pages(page, page_size, DEFAULT_LIST_PAGE_SIZE, |p, ps| {
                    client.list_issues(&repo, Some(p), Some(ps))
                })
                .await?;
                output_list(&collected, json)
            } else {
                let issues = client.list_issues(&repo, page, page_size).await?;
                output_list(&issues, json)
            }
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
