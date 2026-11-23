//! Command handlers. Each command lives in its own module and returns a [`Rendered`] result.

pub mod case;
pub mod doctor;
pub mod evidence;
pub mod reconstruction;
pub mod status;

use crate::cli::Command;
use crate::client::ApiClient;
use crate::error::CliError;
use crate::output::{self, Rendered};

/// Runs one command against the API.
///
/// `doctor` is not dispatched here: it needs the unresolved configuration so it can report an
/// invalid one as a failed check, so [`crate::execute`] runs it before resolving anything.
pub async fn dispatch(command: &Command, client: &ApiClient, json: bool) -> Rendered {
    match command {
        Command::Status => status::run(client, json).await,
        Command::Doctor => output::render_error(
            &CliError::Internal(
                "`doctor` must be run through yukti_cli::execute, not commands::dispatch".into(),
            ),
            json,
        ),
        Command::Case(case_command) => case::run(case_command, client, json).await,
        Command::Evidence(evidence_command) => evidence::run(evidence_command, client, json).await,
        Command::Reconstruction(reconstruction_command) => {
            reconstruction::run(reconstruction_command, client, json).await
        }
    }
}
