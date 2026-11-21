//! `yukti case list | create | show`.
//!
//! API contracts (`backend-rust/src/api/routes/cases.rs`, registered in `backend-rust/src/app.rs`):
//! * `GET  /api/v1/cases` returns `Vec<CaseDto>`.
//! * `POST /api/v1/cases` with `CreateCaseRequest` returns `201` + `CaseDto`.
//! * `GET  /api/v1/cases/:case_id` returns `CaseDto` or `404`.
//!
//! Storage caveat: without PostgreSQL the API lists evidence-storage folders. A folder without
//! `case.json` is returned as a placeholder whose title and timestamps the API generates at
//! request time (marked `metadata.storage = "filesystem"`). The CLI shows records exactly as
//! returned and warns about this instead of hiding or "fixing" them.

use serde::Serialize;
use serde_json::{Map, Value};

use crate::cli::{CaseCommand, CreateCaseArgs, ShowCaseArgs};
use crate::client::ApiClient;
use crate::dto::{
    is_hyphenated_uuid, CaseDto, CaseId, CaseRecord, CreateCaseRequest, ReadinessResponse,
};
use crate::error::{exit, CliError};
use crate::output::{compact_json, render_error, render_warnings, to_json, Rendered};

/// Schema of the `case list` JSON document.
pub const CASE_LIST_SCHEMA: &str = "yukti.cli.case_list/v1";
/// Schema of the `case create` / `case show` JSON document.
pub const CASE_SCHEMA: &str = "yukti.cli.case/v1";

/// Runs a `case` subcommand.
pub async fn run(command: &CaseCommand, client: &ApiClient, json: bool) -> Rendered {
    let result = match command {
        CaseCommand::List => list(client, json).await,
        CaseCommand::Create(args) => create(args, client, json).await,
        CaseCommand::Show(args) => show(args, client, json).await,
    };
    result.unwrap_or_else(|err| render_error(&err, json))
}

/* ------------------------------------------------------------------------------------ list */

/// Storage mode as reported by `GET /health/ready`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StorageInfo {
    /// Readiness `status` (`ready` / `degraded` / `not_ready`), `null` if unknown.
    pub readiness_status: Option<String>,
    /// Readiness `database` value exactly as reported, `null` if unknown.
    pub database: Option<String>,
    /// Why the storage mode could not be determined, if it could not.
    pub error: Option<String>,
}

#[derive(Serialize)]
struct CaseListDocument<'a> {
    schema: &'static str,
    api_url: &'a str,
    count: usize,
    /// Records exactly as returned, in the API's order.
    cases: Vec<&'a Value>,
    storage: &'a StorageInfo,
    /// IDs of records carrying the filesystem-placeholder marker.
    placeholder_case_ids: Vec<&'a str>,
    warnings: &'a [String],
}

async fn list(client: &ApiClient, json: bool) -> Result<Rendered, CliError> {
    let cases = client.list_cases().await?;
    let storage = storage_info(client).await;

    let mut warnings = Vec::new();
    warnings.extend(storage_warning(&storage));
    warnings.extend(record_warnings(&cases));

    if json {
        let doc = CaseListDocument {
            schema: CASE_LIST_SCHEMA,
            api_url: client.base_url(),
            count: cases.len(),
            cases: cases.iter().map(|c| &c.raw).collect(),
            storage: &storage,
            placeholder_case_ids: cases
                .iter()
                .filter(|c| c.dto.has_filesystem_placeholder_marker())
                .map(|c| c.dto.case_id.as_str())
                .collect(),
            warnings: &warnings,
        };
        return Ok(Rendered::stdout(to_json(&doc), exit::SUCCESS));
    }

    Ok(Rendered {
        stdout: render_case_table(&cases),
        stderr: render_warnings(&warnings),
        exit_code: exit::SUCCESS,
    })
}

/// Reads the storage mode from the readiness probe. Failure here never fails `case list`.
pub async fn storage_info(client: &ApiClient) -> StorageInfo {
    let unknown = |error: String| StorageInfo {
        readiness_status: None,
        database: None,
        error: Some(error),
    };
    match client.readiness().await {
        Err(err) => unknown(err.to_string()),
        Ok(raw) => match serde_json::from_str::<ReadinessResponse>(&raw.body) {
            Ok(body) => StorageInfo {
                readiness_status: Some(body.status),
                database: Some(body.database),
                error: None,
            },
            Err(_) => unknown(format!(
                "{} returned HTTP {} without a ReadinessResponse body",
                raw.url, raw.status
            )),
        },
    }
}

/// Warning about where the case list comes from, or `None` when the API reports PostgreSQL
/// as connected.
pub fn storage_warning(storage: &StorageInfo) -> Option<String> {
    match storage.database.as_deref() {
        Some("connected") => None,
        Some("unconfigured_filesystem_mode") => Some(
            "The API is running without PostgreSQL (filesystem mode). Cases are read from evidence-storage folders, not a database; folders without case.json appear as placeholder records whose title and timestamps the API generates when listing.".to_string(),
        ),
        Some(other) => Some(format!(
            "The API reports its database as {other:?}. It falls back to listing evidence-storage folders, so this list may be incomplete or include placeholder records."
        )),
        None => Some(format!(
            "Could not determine the API's storage mode ({}). This list may come from the filesystem fallback and include placeholder records.",
            storage.error.as_deref().unwrap_or("unknown reason")
        )),
    }
}

fn record_warnings(cases: &[CaseRecord]) -> Vec<String> {
    let mut warnings = Vec::new();
    let placeholders: Vec<&str> = cases
        .iter()
        .filter(|c| c.dto.has_filesystem_placeholder_marker())
        .map(|c| c.dto.case_id.as_str())
        .collect();
    if !placeholders.is_empty() {
        warnings.push(format!(
            "{} record(s) carry metadata.storage = \"filesystem\", the marker the API puts on storage folders without case.json; their titles and timestamps were generated by the API, not recorded: {}",
            placeholders.len(),
            placeholders.join(", ")
        ));
    }
    let non_uuid: Vec<&str> = cases
        .iter()
        .filter(|c| !is_hyphenated_uuid(&c.dto.case_id))
        .map(|c| c.dto.case_id.as_str())
        .collect();
    if !non_uuid.is_empty() {
        warnings.push(format!(
            "{} record(s) have a case_id that is not a UUID (likely a storage folder name); `yukti case show` only accepts UUIDs: {}",
            non_uuid.len(),
            non_uuid.join(", ")
        ));
    }
    warnings
}

/// Plain-text table of cases in the given order.
pub fn render_case_table(cases: &[CaseRecord]) -> String {
    if cases.is_empty() {
        return "No cases returned by the API.\n".to_string();
    }
    let status_width = cases
        .iter()
        .map(|c| c.dto.status.len())
        .max()
        .unwrap_or(0)
        .max("STATUS".len());
    let created_width = cases
        .iter()
        .map(|c| c.dto.created_at.len())
        .max()
        .unwrap_or(0)
        .max("CREATED".len());

    let mut out = format!(
        "{:<36}  {:<status_width$}  {:<created_width$}  TITLE\n",
        "CASE ID", "STATUS", "CREATED"
    );
    for case in cases {
        let marker = if case.dto.has_filesystem_placeholder_marker() {
            "  [filesystem placeholder]"
        } else {
            ""
        };
        out.push_str(&format!(
            "{:<36}  {:<status_width$}  {:<created_width$}  {}{marker}\n",
            case.dto.case_id, case.dto.status, case.dto.created_at, case.dto.title
        ));
    }
    out.push_str(&format!(
        "\n{} case(s), in the order returned by the API.\n",
        cases.len()
    ));
    out
}

/* ---------------------------------------------------------------------------- create, show */

#[derive(Serialize)]
struct CaseDocument<'a> {
    schema: &'static str,
    api_url: &'a str,
    /// `create` or `show`.
    action: &'static str,
    /// The case exactly as returned by the API.
    case: &'a Value,
    warnings: &'a [String],
}

/// Validates `case create` arguments into the request body, before any network call.
pub fn build_create_request(args: &CreateCaseArgs) -> Result<CreateCaseRequest, CliError> {
    if args.title.trim().is_empty() {
        return Err(CliError::InvalidArgument(
            "--title must not be blank".to_string(),
        ));
    }
    let optional = |value: &Option<String>, option: &str| -> Result<Option<String>, CliError> {
        match value {
            Some(v) if v.trim().is_empty() => Err(CliError::InvalidArgument(format!(
                "{option} must not be blank; omit the option instead"
            ))),
            other => Ok(other.clone()),
        }
    };
    let metadata = match &args.metadata {
        None => None,
        Some(raw) => Some(parse_metadata(raw)?),
    };
    let case_id = match &args.case_id {
        None => None,
        Some(raw) => Some(CaseId::parse(raw, "--case-id")?.as_str().to_string()),
    };
    Ok(CreateCaseRequest {
        title: args.title.clone(),
        description: optional(&args.description, "--description")?,
        fir_number: optional(&args.fir_number, "--fir-number")?,
        investigator_id: optional(&args.investigator_id, "--investigator-id")?,
        metadata,
        case_id,
    })
}

/// Parses `--metadata`, which must be a JSON object.
pub fn parse_metadata(raw: &str) -> Result<Map<String, Value>, CliError> {
    match serde_json::from_str::<Value>(raw) {
        Ok(Value::Object(map)) => Ok(map),
        Ok(other) => Err(CliError::InvalidArgument(format!(
            "--metadata must be a JSON object such as '{{\"station\":\"Saket\"}}', got {}",
            match other {
                Value::Array(_) => "an array",
                Value::String(_) => "a string",
                Value::Number(_) => "a number",
                Value::Bool(_) => "a boolean",
                _ => "null",
            }
        ))),
        Err(e) => Err(CliError::InvalidArgument(format!(
            "--metadata is not valid JSON ({e})"
        ))),
    }
}

async fn create(
    args: &CreateCaseArgs,
    client: &ApiClient,
    json: bool,
) -> Result<Rendered, CliError> {
    let request = build_create_request(args)?;
    let record = client.create_case(&request).await?;

    let mut warnings = Vec::new();
    if let Some(requested) = &request.case_id {
        if !record.dto.case_id.eq_ignore_ascii_case(requested) {
            warnings.push(format!(
                "The API returned case_id {} instead of the requested {requested}.",
                record.dto.case_id
            ));
        }
    }
    if !is_hyphenated_uuid(&record.dto.case_id) {
        warnings.push(format!(
            "The API returned a case_id that is not a UUID: {:?}.",
            record.dto.case_id
        ));
    }

    render_case(client, "create", &record, &warnings, json)
}

async fn show(args: &ShowCaseArgs, client: &ApiClient, json: bool) -> Result<Rendered, CliError> {
    let case_id = CaseId::parse(&args.case_id, "--case")?;
    let record = client.get_case(&case_id).await?;

    let mut warnings = Vec::new();
    if !record.dto.case_id.eq_ignore_ascii_case(case_id.as_str()) {
        warnings.push(format!(
            "The API returned case_id {} for the requested {}.",
            record.dto.case_id,
            case_id.as_str()
        ));
    }
    if record.dto.has_filesystem_placeholder_marker() {
        warnings.push(
            "This record carries metadata.storage = \"filesystem\", the marker the API puts on storage folders without case.json; its title and timestamps were generated by the API, not recorded.".to_string(),
        );
    }

    render_case(client, "show", &record, &warnings, json)
}

fn render_case(
    client: &ApiClient,
    action: &'static str,
    record: &CaseRecord,
    warnings: &[String],
    json: bool,
) -> Result<Rendered, CliError> {
    if json {
        let doc = CaseDocument {
            schema: CASE_SCHEMA,
            api_url: client.base_url(),
            action,
            case: &record.raw,
            warnings,
        };
        return Ok(Rendered::stdout(to_json(&doc), exit::SUCCESS));
    }
    let mut stdout = String::new();
    if action == "create" {
        stdout.push_str(&format!("Created case {}\n\n", record.dto.case_id));
    }
    stdout.push_str(&render_case_details(&record.dto));
    Ok(Rendered {
        stdout,
        stderr: render_warnings(warnings),
        exit_code: exit::SUCCESS,
    })
}

/// Plain-text field list of one case. Absent values are shown as `(none)`.
pub fn render_case_details(case: &CaseDto) -> String {
    let none = || "(none)".to_string();
    let rows: [(&str, String); 9] = [
        ("Case ID", case.case_id.clone()),
        ("Title", case.title.clone()),
        ("Status", case.status.clone()),
        ("Description", case.description.clone().unwrap_or_else(none)),
        ("FIR number", case.fir_number.clone().unwrap_or_else(none)),
        (
            "Investigator",
            case.investigator_id.clone().unwrap_or_else(none),
        ),
        ("Created", case.created_at.clone()),
        ("Updated", case.updated_at.clone()),
        ("Metadata", compact_json(&case.metadata)),
    ];
    rows.iter()
        .map(|(label, value)| format!("{label:<14}{value}\n"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(title: &str) -> CreateCaseArgs {
        CreateCaseArgs {
            title: title.to_string(),
            description: None,
            fir_number: None,
            investigator_id: None,
            metadata: None,
            case_id: None,
        }
    }

    #[test]
    fn blank_title_and_blank_options_are_rejected() {
        for title in ["", "   ", "\t\n"] {
            let err = build_create_request(&args(title)).unwrap_err();
            assert_eq!(err.exit_code(), 2);
            assert!(err.to_string().contains("--title must not be blank"));
        }
        let mut a = args("T");
        a.description = Some("  ".into());
        assert!(build_create_request(&a)
            .unwrap_err()
            .to_string()
            .contains("--description"));
    }

    #[test]
    fn metadata_must_be_a_json_object() {
        assert!(parse_metadata(r#"{"a":1,"b":{"c":[1,2]}}"#).is_ok());
        assert!(parse_metadata("{}").is_ok());
        for (raw, expected) in [
            ("[1,2]", "an array"),
            ("\"x\"", "a string"),
            ("42", "a number"),
            ("null", "null"),
            ("{not json", "not valid JSON"),
            ("", "not valid JSON"),
        ] {
            let err = parse_metadata(raw).unwrap_err();
            assert_eq!(err.exit_code(), 2, "{raw}");
            assert!(err.to_string().contains(expected), "{raw}: {err}");
        }
    }

    #[test]
    fn case_id_option_is_validated_and_canonicalized() {
        let mut a = args("T");
        a.case_id = Some("CASE-026".into());
        assert!(build_create_request(&a)
            .unwrap_err()
            .to_string()
            .contains("--case-id"));
        a.case_id = Some("3F2B8C1E-7A4D-4E1B-9C3A-5D6E7F8A9B0C".into());
        assert_eq!(
            build_create_request(&a).unwrap().case_id.as_deref(),
            Some("3f2b8c1e-7a4d-4e1b-9c3a-5d6e7f8a9b0c")
        );
    }

    #[test]
    fn storage_warning_follows_the_reported_database() {
        let info = |db: Option<&str>| StorageInfo {
            readiness_status: Some("degraded".into()),
            database: db.map(str::to_string),
            error: None,
        };
        assert_eq!(storage_warning(&info(Some("connected"))), None);
        assert!(storage_warning(&info(Some("unconfigured_filesystem_mode")))
            .unwrap()
            .contains("filesystem mode"));
        assert!(storage_warning(&info(Some("disconnected")))
            .unwrap()
            .contains("\"disconnected\""));
        assert!(storage_warning(&info(None))
            .unwrap()
            .contains("Could not determine"));
    }
}
