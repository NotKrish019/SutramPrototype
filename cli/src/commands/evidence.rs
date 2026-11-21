//! `yukti evidence ingest | list`.
//!
//! API contracts (`backend-rust/src/api/routes/evidence.rs`, registered in `backend-rust/src/app.rs`):
//! * `GET  /api/v1/cases/:case_id` is the case preflight. A 404 stops the command before upload.
//! * `POST /api/v1/evidence/upload` (multipart) returns `201` with an `EvidenceUploadReceipt`,
//!   or with an `EvidenceUploadAndExtractResponse` when `auto_extract` was requested.
//! * `GET  /api/v1/cases/:case_id/evidence` returns `Vec<EvidenceUploadReceipt>`. The API returns
//!   `[]` for an unknown case, so the CLI checks the case first to tell "no evidence" from
//!   "no such case".
//!
//! Forensic rules:
//! * Optional fields you don't pass are not sent. In particular, timezone and clock offset are
//!   never defaulted to UTC or zero, so the API can tell declared values from missing ones.
//! * The file is only opened for reading. Its bytes are uploaded unmodified.
//! * The SHA-256 shown is the one the API computed from the bytes it stored. The CLI does not
//!   compute or verify a hash itself.
//! * No extraction or forensic logic runs in the CLI. `--auto-extract` asks the API to run it.

use std::path::Path;

use serde::Serialize;
use serde_json::Value;

use crate::cli::{EvidenceCommand, IngestEvidenceArgs, ListEvidenceArgs};
use crate::client::ApiClient;
use crate::commands::case::parse_metadata;
use crate::dto::{
    CaseId, CaseRecord, EvidenceReceipt, EvidenceRecord, EvidenceUpload, ExtractionRunSummary,
    UploadOutcome,
};
use crate::error::{exit, CliError};
use crate::output::{compact_json, render_error_with_notes, render_warnings, to_json, Rendered};

/// Schema of the `evidence ingest` JSON document.
pub const EVIDENCE_INGEST_SCHEMA: &str = "yukti.cli.evidence_ingest/v1";
/// Schema of the `evidence list` JSON document.
pub const EVIDENCE_LIST_SCHEMA: &str = "yukti.cli.evidence_list/v1";

/// Metadata keys that the dedicated `--source-timezone` / `--clock-offset-seconds` options own.
/// The API merges `metadata` after those fields, so these keys would silently override them.
pub const RESERVED_METADATA_KEYS: [&str; 2] = ["source_timezone", "clock_offset_seconds"];

/// Runs an `evidence` subcommand.
pub async fn run(command: &EvidenceCommand, client: &ApiClient, json: bool) -> Rendered {
    match command {
        EvidenceCommand::Ingest(args) => ingest(args, client, json).await,
        EvidenceCommand::List(args) => list(args, client, json).await,
    }
}

/* ---------------------------------------------------------------------------------- ingest */

/// Everything validated locally, before any network call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedUpload {
    pub upload: EvidenceUpload,
    /// The path as given on the command line.
    pub path: String,
}

/// The multipart fields that were sent, exactly; `null` means "not sent".
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct SentFields<'a> {
    case_id: &'a str,
    file_name: &'a str,
    file_size_bytes: usize,
    source_type: Option<&'a str>,
    source_timezone: Option<&'a str>,
    clock_offset_seconds: Option<&'a str>,
    metadata: Option<&'a str>,
    auto_extract: Option<&'a str>,
}

#[derive(Serialize)]
struct ExtractionStatusDoc<'a> {
    status: &'a str,
    succeeded: bool,
}

#[derive(Serialize)]
struct IngestDocument<'a> {
    schema: &'static str,
    api_url: &'a str,
    case_id: &'a str,
    file_path: &'a str,
    /// Multipart fields as sent (`null` = not sent).
    request: SentFields<'a>,
    /// The complete response body, exactly as returned.
    response: &'a Value,
    evidence_id: &'a str,
    /// SHA-256 computed by the API (not verified by the CLI).
    sha256_hash: &'a str,
    auto_extract_requested: bool,
    /// `null` when the API returned no extraction run.
    extraction: Option<ExtractionStatusDoc<'a>>,
    warnings: &'a [String],
}

/// Validates arguments and reads the file. No network access.
pub fn prepare_upload(args: &IngestEvidenceArgs) -> Result<PreparedUpload, CliError> {
    let case_id = CaseId::parse(&args.case_id, "--case")?;
    let source_type = non_blank(&args.source_type, "--source-type")?;
    let source_timezone = non_blank(&args.source_timezone, "--source-timezone")?;
    let clock_offset_seconds = match &args.clock_offset_seconds {
        None => None,
        Some(raw) => Some(parse_clock_offset(raw)?),
    };
    let metadata = match &args.metadata {
        None => None,
        Some(raw) => {
            let map = parse_metadata(raw)?;
            if let Some(key) = RESERVED_METADATA_KEYS
                .iter()
                .find(|k| map.contains_key(**k))
            {
                let option = key.replace('_', "-");
                return Err(CliError::InvalidArgument(format!(
                    "--metadata must not contain {key:?}; use --{option} so the value is declared explicitly"
                )));
            }
            Some(Value::Object(map).to_string())
        }
    };
    let (file_name, bytes) = read_evidence_file(&args.file)?;
    Ok(PreparedUpload {
        upload: EvidenceUpload {
            case_id,
            file_name,
            bytes,
            source_type,
            source_timezone,
            clock_offset_seconds,
            metadata,
            auto_extract: args.auto_extract,
        },
        path: args.file.display().to_string(),
    })
}

fn non_blank(value: &Option<String>, option: &str) -> Result<Option<String>, CliError> {
    match value {
        Some(v) if v.trim().is_empty() => Err(CliError::InvalidArgument(format!(
            "{option} must not be blank; omit the option to leave it undeclared"
        ))),
        Some(v) => Ok(Some(v.trim().to_string())),
        None => Ok(None),
    }
}

/// Validates `--clock-offset-seconds`. The API reads this field as an `f64` and silently drops
/// values it cannot parse or that are not finite, so the CLI rejects those up front. The
/// trimmed text is sent as typed.
pub fn parse_clock_offset(raw: &str) -> Result<String, CliError> {
    let text = raw.trim();
    match text.parse::<f64>() {
        Ok(v) if v.is_finite() => Ok(text.to_string()),
        _ => Err(CliError::InvalidArgument(format!(
            "--clock-offset-seconds must be a finite number of seconds such as -30 or 12.5, got {raw:?}"
        ))),
    }
}

/// Reads a regular, non-empty file. It is opened read-only and never modified.
pub fn read_evidence_file(path: &Path) -> Result<(String, Vec<u8>), CliError> {
    let shown = path.display();
    let meta = std::fs::metadata(path).map_err(|e| {
        CliError::InvalidArgument(match e.kind() {
            std::io::ErrorKind::NotFound => format!("evidence file {shown} does not exist"),
            _ => format!("cannot access evidence file {shown}: {e}"),
        })
    })?;
    if !meta.is_file() {
        return Err(CliError::InvalidArgument(format!(
            "evidence path {shown} is not a regular file"
        )));
    }
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .filter(|n| !n.is_empty())
        .ok_or_else(|| {
            CliError::InvalidArgument(format!("evidence path {shown} has no file name"))
        })?;
    let bytes = std::fs::read(path).map_err(|e| {
        CliError::InvalidArgument(format!("cannot read evidence file {shown}: {e}"))
    })?;
    if bytes.is_empty() {
        return Err(CliError::InvalidArgument(format!(
            "evidence file {shown} is empty (0 bytes); the API rejects empty uploads"
        )));
    }
    Ok((file_name, bytes))
}

async fn ingest(args: &IngestEvidenceArgs, client: &ApiClient, json: bool) -> Rendered {
    let prepared = match prepare_upload(args) {
        Ok(p) => p,
        Err(err) => return render_error_with_notes(&err, json, &[]),
    };
    let upload = &prepared.upload;
    let case_id = upload.case_id.as_str();

    // Preflight: the upload endpoint does not check that the case exists.
    let case = match client.get_case(&upload.case_id).await {
        Ok(case) => case,
        Err(err) => {
            let note = format!("Nothing was uploaded: the case check for {case_id} failed.");
            return render_error_with_notes(&err, json, &[note]);
        }
    };

    let outcome = match client.upload_evidence(upload).await {
        Ok(outcome) => outcome,
        Err(err) => {
            let notes = upload_failure_notes(&err, upload);
            return render_error_with_notes(&err, json, &notes);
        }
    };

    let warnings = ingest_warnings(&prepared, &case, &outcome);
    if json {
        render_ingest_json(client, &prepared, &outcome, &warnings)
    } else {
        Rendered {
            stdout: render_ingest_human(&prepared, &outcome),
            stderr: render_warnings(&warnings),
            exit_code: exit::SUCCESS,
        }
    }
}

/// Size above which the API, as currently deployed, rejects uploads: its multipart extractor
/// keeps the web framework's default 2 MiB request-body limit, whatever `MAX_UPLOAD_SIZE_BYTES`
/// says. Observed as HTTP 400 "Streaming read error ... multipart/form-data" (or 413).
pub const OBSERVED_API_BODY_LIMIT_BYTES: usize = 2 * 1024 * 1024;

/// Statuses the API returns while validating the upload, before anything is stored.
const INGESTION_STAGE_STATUSES: [u16; 5] = [400, 409, 413, 415, 422];

/// Extra guidance for a failed upload (human mode only).
pub fn upload_failure_notes(err: &CliError, upload: &EvidenceUpload) -> Vec<String> {
    let case_id = upload.case_id.as_str();
    let mut notes = Vec::new();
    match err {
        CliError::Http { status, .. } => {
            if matches!(status, 400 | 413) && upload.bytes.len() > OBSERVED_API_BODY_LIMIT_BYTES {
                notes.push(format!(
                    "This file is {} bytes. The API currently rejects uploads larger than about 2 MiB (its request-body limit), regardless of its configured maximum evidence size.",
                    upload.bytes.len()
                ));
            }
            if upload.auto_extract && !INGESTION_STAGE_STATUSES.contains(status) {
                notes.push(format!(
                    "With --auto-extract the API stores the evidence before extracting, so if this error came from the extraction step the evidence may already be ingested. Check with: yukti evidence list --case {case_id}"
                ));
            }
        }
        CliError::Timeout { .. } => notes.push(format!(
            "The upload may still have completed on the server. Check with: yukti evidence list --case {case_id}; a larger --timeout may be needed."
        )),
        _ => {}
    }
    notes
}

fn ingest_warnings(
    prepared: &PreparedUpload,
    case: &CaseRecord,
    outcome: &UploadOutcome,
) -> Vec<String> {
    let upload = &prepared.upload;
    let receipt = &outcome.receipt;
    let mut warnings = Vec::new();
    if case.dto.has_filesystem_placeholder_marker() {
        warnings.push(format!(
            "Case {} is a filesystem placeholder record (metadata.storage = \"filesystem\"): the API has a storage folder for it but no case.json, so the case was never registered with `case create`.",
            upload.case_id.as_str()
        ));
    }
    if !receipt
        .case_id
        .eq_ignore_ascii_case(upload.case_id.as_str())
    {
        warnings.push(format!(
            "The API recorded the evidence under case {} instead of the requested {}.",
            receipt.case_id,
            upload.case_id.as_str()
        ));
    }
    if receipt.original_filename != upload.file_name {
        warnings.push(format!(
            "The API stored the file name as {:?} (sent as {:?}).",
            receipt.original_filename, upload.file_name
        ));
    }
    if receipt.file_size_bytes != upload.bytes.len() as u64 {
        warnings.push(format!(
            "The API reports {} bytes stored, but {} bytes were sent.",
            receipt.file_size_bytes,
            upload.bytes.len()
        ));
    }
    if let Some(source_type) = &upload.source_type {
        warnings.push(format!(
            "--source-type replaces the type the API detects from the file content; it was stored exactly as given ({source_type:?}). The API's extractor recognises only call-detail-record and chat-export types."
        ));
    }
    match (upload.auto_extract, &outcome.extraction) {
        (true, None) => warnings.push(
            "Auto-extraction was requested, but the API returned no extraction run.".to_string(),
        ),
        (false, Some(_)) => warnings.push(
            "The API ran an extraction although auto-extraction was not requested.".to_string(),
        ),
        _ => {}
    }
    warnings
}

fn render_ingest_json(
    client: &ApiClient,
    prepared: &PreparedUpload,
    outcome: &UploadOutcome,
    warnings: &[String],
) -> Rendered {
    let upload = &prepared.upload;
    let doc = IngestDocument {
        schema: EVIDENCE_INGEST_SCHEMA,
        api_url: client.base_url(),
        case_id: upload.case_id.as_str(),
        file_path: &prepared.path,
        request: SentFields {
            case_id: upload.case_id.as_str(),
            file_name: &upload.file_name,
            file_size_bytes: upload.bytes.len(),
            source_type: upload.source_type.as_deref(),
            source_timezone: upload.source_timezone.as_deref(),
            clock_offset_seconds: upload.clock_offset_seconds.as_deref(),
            metadata: upload.metadata.as_deref(),
            auto_extract: upload.auto_extract.then_some("true"),
        },
        response: &outcome.raw,
        evidence_id: &outcome.receipt.evidence_id,
        sha256_hash: &outcome.receipt.sha256_hash,
        auto_extract_requested: upload.auto_extract,
        extraction: outcome.extraction.as_ref().map(|e| ExtractionStatusDoc {
            status: &e.status,
            succeeded: e.succeeded(),
        }),
        warnings,
    };
    Rendered::stdout(to_json(&doc), exit::SUCCESS)
}

fn render_ingest_human(prepared: &PreparedUpload, outcome: &UploadOutcome) -> String {
    let receipt = &outcome.receipt;
    let mut out = format!("Ingested evidence {}\n\n", receipt.evidence_id);
    out.push_str(&render_receipt(receipt));
    out.push('\n');
    out.push_str(&render_extraction(
        prepared.upload.auto_extract,
        outcome.extraction.as_ref(),
    ));
    out
}

/// Field list of one receipt. The timezone and offset lines read the receipt's metadata,
/// i.e. what the API recorded, and say "not declared" when absent.
pub fn render_receipt(receipt: &EvidenceReceipt) -> String {
    let declared = |key: &str| match receipt.metadata.get(key) {
        None | Some(Value::Null) => "not declared".to_string(),
        Some(Value::String(s)) => format!("{s} (declared)"),
        Some(other) => format!("{other} (declared)"),
    };
    let rows: [(&str, String); 13] = [
        ("Evidence ID", receipt.evidence_id.clone()),
        ("Case ID", receipt.case_id.clone()),
        ("File", receipt.original_filename.clone()),
        ("Source type", receipt.source_type.clone()),
        ("MIME type", receipt.mime_type.clone()),
        ("Size", format!("{} bytes", receipt.file_size_bytes)),
        (
            "SHA-256",
            format!("{} (computed by the API)", receipt.sha256_hash),
        ),
        ("Status", receipt.status.clone()),
        ("Storage URI", receipt.storage_uri.clone()),
        ("Created", receipt.created_at.clone()),
        ("Timezone", declared("source_timezone")),
        ("Clock offset", declared("clock_offset_seconds")),
        ("Metadata", compact_json(&receipt.metadata)),
    ];
    rows.iter()
        .map(|(label, value)| format!("{label:<16}{value}\n"))
        .collect()
}

fn render_extraction(requested: bool, extraction: Option<&ExtractionRunSummary>) -> String {
    let Some(run) = extraction else {
        return if requested {
            format!(
                "{:<16}requested, but no extraction run was returned\n",
                "Auto-extract"
            )
        } else {
            format!("{:<16}not requested\n", "Auto-extract")
        };
    };
    let outcome = if run.succeeded() {
        "succeeded"
    } else {
        "did NOT succeed"
    };
    let mut out = format!(
        "{:<16}{}; extraction {outcome} (status {})\n",
        "Auto-extract",
        if requested {
            "requested"
        } else {
            "not requested"
        },
        run.status
    );
    let mut row = |label: &str, value: String| out.push_str(&format!("  {label:<14}{value}\n"));
    row("Run ID", run.extraction_run_id.clone());
    row(
        "Records",
        format!(
            "{} processed, {} valid, {} rejected, {} validation issue(s)",
            run.records_processed,
            run.valid_records_count,
            run.rejected_records_count,
            run.validation_issue_count
        ),
    );
    if let Some(ms) = run.duration_ms {
        row("Duration", format!("{ms} ms"));
    }
    row("Config hash", run.configuration_hash.clone());
    if let Some(done) = &run.completed_at {
        row("Completed", done.clone());
    }
    if let Some(message) = &run.error_message {
        row("Error", message.clone());
    }
    out
}

/* ------------------------------------------------------------------------------------ list */

#[derive(Serialize)]
struct ListDocument<'a> {
    schema: &'static str,
    api_url: &'a str,
    case_id: &'a str,
    count: usize,
    /// Receipts exactly as returned, in the API's order.
    evidence: Vec<&'a Value>,
    warnings: &'a [String],
}

async fn list(args: &ListEvidenceArgs, client: &ApiClient, json: bool) -> Rendered {
    let case_id = match CaseId::parse(&args.case_id, "--case") {
        Ok(id) => id,
        Err(err) => return render_error_with_notes(&err, json, &[]),
    };
    // The evidence endpoint returns [] for unknown cases; check the case exists first.
    let case = match client.get_case(&case_id).await {
        Ok(case) => case,
        Err(err) => return render_error_with_notes(&err, json, &[]),
    };
    let records = match client.list_case_evidence(&case_id).await {
        Ok(records) => records,
        Err(err) => return render_error_with_notes(&err, json, &[]),
    };

    let mut warnings = Vec::new();
    if case.dto.has_filesystem_placeholder_marker() {
        warnings.push(format!(
            "Case {} is a filesystem placeholder record (metadata.storage = \"filesystem\"), not a registered case.",
            case_id.as_str()
        ));
    }
    let foreign: Vec<&str> = records
        .iter()
        .filter(|r| !r.receipt.case_id.eq_ignore_ascii_case(case_id.as_str()))
        .map(|r| r.receipt.evidence_id.as_str())
        .collect();
    if !foreign.is_empty() {
        warnings.push(format!(
            "{} record(s) name a different case_id than the one requested: {}",
            foreign.len(),
            foreign.join(", ")
        ));
    }

    if json {
        let doc = ListDocument {
            schema: EVIDENCE_LIST_SCHEMA,
            api_url: client.base_url(),
            case_id: case_id.as_str(),
            count: records.len(),
            evidence: records.iter().map(|r| &r.raw).collect(),
            warnings: &warnings,
        };
        return Rendered::stdout(to_json(&doc), exit::SUCCESS);
    }
    Rendered {
        stdout: render_evidence_list(case_id.as_str(), &records),
        stderr: render_warnings(&warnings),
        exit_code: exit::SUCCESS,
    }
}

/// One block per receipt (a SHA-256 is too wide for a table row), in the given order.
pub fn render_evidence_list(case_id: &str, records: &[EvidenceRecord]) -> String {
    if records.is_empty() {
        return format!("No evidence returned by the API for case {case_id}.\n");
    }
    let mut out = String::new();
    for record in records {
        let r = &record.receipt;
        out.push_str(&format!(
            "{}  {}  {} bytes  {}  {}\n",
            r.evidence_id, r.source_type, r.file_size_bytes, r.created_at, r.status
        ));
        out.push_str(&format!("  file    {}\n", r.original_filename));
        out.push_str(&format!("  mime    {}\n", r.mime_type));
        out.push_str(&format!("  sha256  {}\n\n", r.sha256_hash));
    }
    out.push_str(&format!(
        "{} evidence item(s) for case {case_id}, in the order returned by the API. SHA-256 values are as computed by the API.\n",
        records.len()
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_offset_accepts_finite_numbers_only() {
        for ok in ["0", "-30", "12.5", " 3600 ", "+5", "-0.25"] {
            assert_eq!(parse_clock_offset(ok).unwrap(), ok.trim(), "{ok}");
        }
        for bad in ["", "  ", "ten", "30s", "NaN", "inf", "-infinity", "1,5"] {
            let err = parse_clock_offset(bad).unwrap_err();
            assert_eq!(err.exit_code(), 2, "{bad}");
            assert!(err.to_string().contains("--clock-offset-seconds"), "{bad}");
        }
    }

    #[test]
    fn blank_optional_values_are_rejected_not_defaulted() {
        assert!(non_blank(&Some("  ".into()), "--source-timezone").is_err());
        assert_eq!(non_blank(&None, "--source-timezone").unwrap(), None);
        assert_eq!(
            non_blank(&Some(" Asia/Kolkata ".into()), "--source-timezone").unwrap(),
            Some("Asia/Kolkata".into())
        );
    }

    #[test]
    fn missing_and_non_regular_files_are_rejected() {
        let missing = read_evidence_file(Path::new("definitely/not/here.csv")).unwrap_err();
        assert!(missing.to_string().contains("does not exist"));
        assert_eq!(missing.exit_code(), 2);
        let dir = read_evidence_file(Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap_err();
        assert!(dir.to_string().contains("not a regular file"));
    }
}
