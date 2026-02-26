use anyhow::Result;
use clap::Subcommand;

use crate::{client, output::output_one};

#[derive(Subcommand)]
pub enum UserAction {
    /// Get current user info
    Info,
}

pub async fn handle(client: &client::Client, action: UserAction, json: bool) -> Result<()> {
    match action {
        UserAction::Info => {
            let user = client.get_current_user().await?;
            output_one(&user, json)
        }
    }
}
