use anyhow::Result;
use clap::Subcommand;
use std::io::{self, Write};

use crate::{
    client,
    output::{output_created, output_json, output_list, output_one},
    pagination::{DEFAULT_REPO_PAGE_SIZE, collect_all_pages},
};

#[derive(Subcommand)]
pub enum RepoAction {
    /// List repositories
    List {
        /// Group slug
        #[arg(short, long)]
        group: Option<String>,

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

pub async fn handle(client: &client::Client, action: RepoAction, json: bool) -> Result<()> {
    match action {
        RepoAction::List {
            group,
            page,
            page_size,
            all,
        } => {
            if all {
                let collected = collect_all_pages(page, page_size, DEFAULT_REPO_PAGE_SIZE, |p, ps| {
                    client.list_repos(group.as_deref(), Some(p), Some(ps))
                })
                .await?;
                output_list(&collected, json)
            } else {
                let repos = client.list_repos(group.as_deref(), page, page_size).await?;
                output_list(&repos, json)
            }
        }
        RepoAction::Get { slug } => {
            let repo = client.get_repo(&slug).await?;
            output_one(&repo, json)
        }
        RepoAction::Create {
            group,
            name,
            description,
            private,
        } => {
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
                    if json {
                        output_json(&serde_json::json!({ "aborted": true }))?;
                    } else {
                        println!("Aborted.");
                    }
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
