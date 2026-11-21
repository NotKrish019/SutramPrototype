//! Command handlers. Each command lives in its own module and returns a [`Rendered`] result.

pub mod case;
pub mod evidence;
pub mod reconstruction;
pub mod status;

use crate::cli::Command;
use crate::client::ApiClient;
use crate::output::Rendered;

/// Runs one command against the API.
pub async fn dispatch(command: &Command, client: &ApiClient, json: bool) -> Rendered {
    match command {
        Command::Status => status::run(client, json).await,
        Command::Case(case_command) => case::run(case_command, client, json).await,
        Command::Evidence(evidence_command) => evidence::run(evidence_command, client, json).await,
        Command::Reconstruction(reconstruction_command) => {
            reconstruction::run(reconstruction_command, client, json).await
        }
    }
}
