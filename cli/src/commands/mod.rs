//! Command handlers. Each command lives in its own module and returns a [`Rendered`] result.

pub mod analysis;
pub mod ask;
pub mod case;
pub mod challenge;
pub mod completions;
pub mod config;
pub mod custody;
pub mod demo;
pub mod doctor;
pub mod evidence;
pub mod graph;
pub mod reconstruction;
pub mod replay;
pub mod robustness;
pub mod status;
pub mod whatif;

use serde::Serialize;
use serde_json::Value;

use crate::cli::Command;
use crate::client::ApiClient;
use crate::dto::{CaseId, CaseRecord};
use crate::error::CliError;
use crate::output::{render_error_with_notes, Rendered};
use crate::ui::Ui;

/// Runs one command against the API.
pub async fn dispatch(command: &Command, client: &ApiClient, json: bool, ui: &Ui) -> Rendered {
    match command {
        Command::Status => status::run(client, json, ui).await,
        // Doctor needs the unresolved configuration, so `crate::execute` runs it.
        Command::Case(c) => case::run(c, client, json, ui).await,
        Command::Evidence(c) => evidence::run(c, client, json, ui).await,
        Command::Reconstruction(c) => reconstruction::run(c, client, json, ui).await,
        Command::Timeline(a) => analysis::timeline(a, client, json, ui).await,
        Command::Spatial(a) => analysis::spatial(a, client, json, ui).await,
        Command::Contradictions(a) => analysis::contradictions(a, client, json, ui).await,
        Command::Gaps(a) => analysis::gaps(a, client, json, ui).await,
        Command::Provenance(a) => analysis::provenance(a, client, json, ui).await,
        Command::Report(a) => analysis::report(a, client, json, ui).await,
        Command::Graph(c) => graph::run(c, client, json, ui).await,
        Command::WhatIf(a) => whatif::run(a, client, json, ui).await,
        Command::Robustness(a) => robustness::run(a, client, json, ui).await,
        Command::Custody(c) => custody::run(c, client, json, ui).await,
        Command::Certificate(a) => custody::certificate(a, client, json, ui).await,
        Command::Challenge(a) => challenge::run(a, client, json, ui).await,
        Command::Ask(a) => ask::run(a, client, json, ui).await,
        Command::Replay(a) => replay::run(a, client, json, ui).await,
        // Handled before a client exists (see `crate::execute`).
        Command::Doctor
        | Command::Config
        | Command::Completions(_)
        | Command::Shell
        | Command::Demo(_)
        | Command::Server(_) => {
            let err = CliError::Internal(format!("{command:?} cannot be dispatched to the API"));
            crate::output::render_error(&err, json)
        }
    }
}

/// Validates `--case` and checks that the case exists, before anything else is requested.
/// `action` completes the sentence "Nothing was ...".
pub async fn require_case(
    raw_case_id: &str,
    client: &ApiClient,
    json: bool,
    action: &str,
) -> Result<(CaseId, CaseRecord), Rendered> {
    let case_id =
        CaseId::parse(raw_case_id, "--case").map_err(|e| render_error_with_notes(&e, json, &[]))?;
    match client.get_case(&case_id).await {
        Ok(case) => Ok((case_id, case)),
        Err(err) => {
            let note = format!(
                "Nothing was {action}: the case check for {} failed.",
                case_id.as_str()
            );
            Err(render_error_with_notes(&err, json, &[note]))
        }
    }
}

/// The request a document describes.
#[derive(Debug, Serialize)]
pub struct RequestDoc<'a> {
    pub method: &'static str,
    pub path: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<&'a Value>,
}

/// Generic JSON document of a read-only case view: the response exactly as returned.
#[derive(Debug, Serialize)]
pub struct CaseViewDocument<'a> {
    pub schema: &'static str,
    pub api_url: &'a str,
    pub case_id: &'a str,
    pub request: RequestDoc<'a>,
    pub response: &'a Value,
    pub warnings: &'a [String],
}

/// Warning when the requested case is a filesystem placeholder.
pub fn placeholder_warning(case_id: &CaseId, case: &CaseRecord) -> Option<String> {
    case.dto.has_filesystem_placeholder_marker().then(|| {
        format!(
            "Case {} is a filesystem placeholder record (metadata.storage = \"filesystem\"), not a registered case.",
            case_id.as_str()
        )
    })
}

/// Parses a typed view of `raw`, reporting a shape mismatch as an unexpected response.
pub fn typed<T: serde::de::DeserializeOwned>(
    url: &str,
    raw: &Value,
    shape: &str,
) -> Result<T, CliError> {
    serde_json::from_value::<T>(raw.clone()).map_err(|e| CliError::BadResponse {
        url: url.to_string(),
        reason: format!("response does not match {shape} ({e})"),
    })
}
