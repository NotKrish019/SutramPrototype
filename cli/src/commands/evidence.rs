//! `yukti evidence ingest | list | show | extract | reprocess | extraction | runs | verify`.
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
//! * The SHA-256 shown is the one the API computed from the bytes it stored. The CLI also
//!   hashes the bytes it sends and reports any difference (exit 7): a transfer-integrity check,
//!   not a replacement for the API's own record. `evidence verify` compares a local copy with
//!   the recorded hash the same way.
//! * No extraction or forensic logic runs in the CLI. `--auto-extract` asks the API to run it.

use std::path::Path;

use serde::Serialize;
use serde_json::Value;

use sha2::{Digest, Sha256};

use crate::cli::{
    EvidenceArgs, EvidenceCommand, IngestEvidenceArgs, ListEvidenceArgs, ReprocessArgs,
    VerifyEvidenceArgs,
};
use crate::client::{evidence_path, ApiClient};
use crate::commands::case::parse_metadata;
use crate::dto::{
    CaseId, CaseRecord, EvidenceReceipt, EvidenceRecord, EvidenceUpload, ExtractionRunSummary,
    UploadOutcome,
};
use crate::error::{exit, CliError};
use crate::output::{compact_json, render_error_with_notes, render_warnings, to_json, Rendered};
use crate::ui::{
    human_bytes, short_id, status_badge, Badge, Cell, Column, Glyph, Tone, Ui, MARGIN,
};

/// Schema of the `evidence ingest` JSON document.
pub const EVIDENCE_INGEST_SCHEMA: &str = "yukti.cli.evidence_ingest/v1";
/// Schema of the `evidence list` JSON document.
pub const EVIDENCE_LIST_SCHEMA: &str = "yukti.cli.evidence_list/v1";
/// Schema of the `evidence show` JSON document.
pub const EVIDENCE_SCHEMA: &str = "yukti.cli.evidence/v1";
/// Schema of the `evidence extract` / `reprocess` JSON document.
pub const EXTRACTION_RUN_SCHEMA: &str = "yukti.cli.extraction_run/v1";
/// Schema of the `evidence extraction` JSON document.
pub const EXTRACTION_RESULT_SCHEMA: &str = "yukti.cli.extraction_result/v1";
/// Schema of the `evidence runs` JSON document.
pub const EXTRACTION_RUNS_SCHEMA: &str = "yukti.cli.extraction_runs/v1";
/// Schema of the `evidence verify` JSON document.
pub const EVIDENCE_VERIFY_SCHEMA: &str = "yukti.cli.evidence_verify/v1";

/// Metadata keys that the dedicated `--source-timezone` / `--clock-offset-seconds` options own.
/// The API merges `metadata` after those fields, so these keys would silently override them.
pub const RESERVED_METADATA_KEYS: [&str; 2] = ["source_timezone", "clock_offset_seconds"];

/// Runs an `evidence` subcommand.
pub async fn run(command: &EvidenceCommand, client: &ApiClient, json: bool, ui: &Ui) -> Rendered {
    match command {
        EvidenceCommand::Ingest(args) => ingest(args, client, json, ui).await,
        EvidenceCommand::List(args) => list(args, client, json, ui).await,
        EvidenceCommand::Show(args) => show(args, client, json, ui).await,
        EvidenceCommand::Extract(args) => extract(args, client, json, ui).await,
        EvidenceCommand::Reprocess(args) => reprocess(args, client, json, ui).await,
        EvidenceCommand::Extraction(args) => extraction(args, client, json, ui).await,
        EvidenceCommand::Runs(args) => runs(args, client, json, ui).await,
        EvidenceCommand::Verify(args) => verify(args, client, json, ui).await,
    }
}

/// Lowercase hex SHA-256 of `bytes`.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|b| format!("{b:02x}")).collect()
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
    /// SHA-256 computed by the API from the bytes it stored.
    sha256_hash: &'a str,
    /// SHA-256 the CLI computed from the bytes it sent.
    local_sha256: &'a str,
    /// `true` when both hashes are equal (case-insensitive); exit 7 otherwise.
    hash_match: bool,
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

async fn ingest(args: &IngestEvidenceArgs, client: &ApiClient, json: bool, ui: &Ui) -> Rendered {
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

    let mut warnings = ingest_warnings(&prepared, &case, &outcome);
    let local = sha256_hex(&upload.bytes);
    let hash_match = local.eq_ignore_ascii_case(&outcome.receipt.sha256_hash);
    if !hash_match {
        warnings.push(format!(
            "INTEGRITY: the API recorded SHA-256 {} but the bytes sent hash to {local}. The stored exhibit may not be the file you uploaded.",
            outcome.receipt.sha256_hash
        ));
    }
    let exit_code = if hash_match {
        exit::SUCCESS
    } else {
        exit::VERIFICATION_FAILED
    };
    if json {
        let mut r = render_ingest_json(client, &prepared, &outcome, &local, hash_match, &warnings);
        r.exit_code = exit_code;
        r
    } else {
        let mut stdout = ui.header(&["evidence", "ingest"], client.base_url());
        stdout.push_str(&render_ingest_human(&prepared, &outcome, &local, ui));
        Rendered {
            stdout,
            stderr: render_warnings(&warnings),
            exit_code,
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
    local_sha256: &str,
    hash_match: bool,
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
        local_sha256,
        hash_match,
        auto_extract_requested: upload.auto_extract,
        extraction: outcome.extraction.as_ref().map(|e| ExtractionStatusDoc {
            status: &e.status,
            succeeded: e.succeeded(),
        }),
        warnings,
    };
    Rendered::stdout(to_json(&doc), exit::SUCCESS)
}

fn render_ingest_human(
    prepared: &PreparedUpload,
    outcome: &UploadOutcome,
    local_sha256: &str,
    ui: &Ui,
) -> String {
    let receipt = &outcome.receipt;
    let mut out = String::from("\n");
    out.push_str(&format!(
        "{MARGIN}{}  {}\n",
        ui.badge(Badge::Ok, "ingested"),
        ui.paint(
            Tone::Text,
            &format!("Ingested evidence {}", receipt.evidence_id)
        )
    ));
    out.push_str(&ui.para(
        MARGIN.len(),
        Tone::Subtle,
        "The original file was only read. Derived artifacts are written by the API to its own stores.",
    ));
    out.push_str(&render_custody(receipt, Some(local_sha256), ui));
    out.push_str(&render_receipt(receipt, ui));
    out.push_str(&render_extraction(
        prepared.upload.auto_extract,
        outcome.extraction.as_ref(),
        ui,
    ));
    out.push('\n');
    let id = &receipt.evidence_id;
    out.push_str(&ui.next("history", &format!("yukti evidence show --evidence {id}")));
    if outcome.extraction.is_none() {
        out.push_str(&ui.next(
            "extract",
            &format!("yukti evidence extract --evidence {id}"),
        ));
    }
    out.push_str(&ui.next(
        "reconstruct",
        &format!("yukti reconstruction run --case {}", receipt.case_id),
    ));
    out
}

/// Chain-of-custody block: the API's hash and, when known, the hash of the bytes sent or read.
pub fn render_custody(receipt: &EvidenceReceipt, local_sha256: Option<&str>, ui: &Ui) -> String {
    let mut out = ui.section("Chain of custody", "SHA-256");
    out.push_str(&ui.kv_painted(
        "Recorded by API",
        &format!(
            "{} {}",
            ui.mark(Glyph::Observed, Tone::Text),
            ui.paint(Tone::Text, &receipt.sha256_hash)
        ),
    ));
    if let Some(local) = local_sha256 {
        let matched = local.eq_ignore_ascii_case(&receipt.sha256_hash);
        let (glyph, tone) = if matched {
            (Glyph::Check, Tone::Ok)
        } else {
            (Glyph::Cross, Tone::Flag)
        };
        out.push_str(&ui.kv_painted(
            "Computed locally",
            &format!("{} {}", ui.mark(glyph, tone), ui.paint(Tone::Text, local)),
        ));
        out.push_str(&ui.kv_painted(
            "Integrity",
            &if matched {
                ui.badge(Badge::Ok, "verified match")
            } else {
                ui.badge(Badge::Flag, "hash mismatch")
            },
        ));
    }
    out
}

/// Field list of one receipt. The timezone and offset lines read the receipt's metadata,
/// i.e. what the API recorded, and say "not declared" when absent.
pub fn render_receipt(receipt: &EvidenceReceipt, ui: &Ui) -> String {
    let declared = |key: &str| match receipt.metadata.get(key) {
        None | Some(Value::Null) => (Tone::Warn, "not declared".to_string()),
        Some(Value::String(s)) => (Tone::Supplied, format!("{s} (declared)")),
        Some(other) => (Tone::Supplied, format!("{other} (declared)")),
    };
    let mut out = ui.section("Exhibit", &receipt.source_type);
    let rows: [(&str, Tone, String); 9] = [
        ("Evidence ID", Tone::Text, receipt.evidence_id.clone()),
        ("Case ID", Tone::Muted, receipt.case_id.clone()),
        ("File", Tone::Strong, receipt.original_filename.clone()),
        ("Source type", Tone::Text, receipt.source_type.clone()),
        ("MIME type", Tone::Muted, receipt.mime_type.clone()),
        ("Size", Tone::Text, human_bytes(receipt.file_size_bytes)),
        ("Storage URI", Tone::Subtle, receipt.storage_uri.clone()),
        ("Created", Tone::Muted, receipt.created_at.clone()),
        ("Metadata", Tone::Subtle, compact_json(&receipt.metadata)),
    ];
    for (label, tone, value) in rows.iter().take(6) {
        out.push_str(&ui.kv(label, *tone, value));
    }
    out.push_str(&ui.kv_painted(
        "Status",
        &ui.badge(status_badge(&receipt.status), &receipt.status),
    ));
    for (label, key) in [
        ("Timezone", "source_timezone"),
        ("Clock offset", "clock_offset_seconds"),
    ] {
        let (tone, value) = declared(key);
        let glyph = if tone == Tone::Warn {
            Glyph::Assumed
        } else {
            Glyph::Supplied
        };
        out.push_str(&ui.kv_painted(
            label,
            &format!("{} {}", ui.mark(glyph, tone), ui.paint(tone, &value)),
        ));
    }
    for (label, tone, value) in rows.iter().skip(6) {
        out.push_str(&ui.kv(label, *tone, value));
    }
    out
}

fn render_extraction(
    requested: bool,
    extraction: Option<&ExtractionRunSummary>,
    ui: &Ui,
) -> String {
    let mut out = ui.section(
        "Extraction",
        if requested {
            "auto-extract requested"
        } else {
            "auto-extract not requested"
        },
    );
    let Some(run) = extraction else {
        out.push_str(&ui.kv(
            "Auto-extract",
            Tone::Muted,
            if requested {
                "requested, but no extraction run was returned"
            } else {
                "not requested"
            },
        ));
        return out;
    };
    let outcome = if run.succeeded() {
        "succeeded"
    } else {
        "did NOT succeed"
    };
    out.push_str(&ui.kv_painted(
        "Auto-extract",
        &format!(
            "{}  {}",
            ui.badge(status_badge(&run.status), &run.status),
            ui.paint(Tone::Muted, &format!("extraction {outcome}"))
        ),
    ));
    out.push_str(&ui.kv("Run ID", Tone::Text, &run.extraction_run_id));
    out.push_str(&render_record_counts(
        run.records_processed,
        run.valid_records_count,
        run.rejected_records_count,
        run.validation_issue_count,
        ui,
    ));
    if let Some(ms) = run.duration_ms {
        out.push_str(&ui.kv("Duration", Tone::Muted, &format!("{ms} ms")));
    }
    out.push_str(&ui.kv("Config hash", Tone::Subtle, &run.configuration_hash));
    if let Some(done) = &run.completed_at {
        out.push_str(&ui.kv("Completed", Tone::Muted, done));
    }
    if let Some(message) = &run.error_message {
        out.push_str(&ui.kv("Error", Tone::Flag, message));
    }
    out
}

/// `Records  ████████░░  8 valid · 2 rejected of 10 · 1 validation issue(s)`.
fn render_record_counts(processed: u64, valid: u64, rejected: u64, issues: u64, ui: &Ui) -> String {
    let ratio = if processed == 0 {
        0.0
    } else {
        valid as f64 / processed as f64
    };
    let dot = ui.g(Glyph::Dot);
    ui.kv_painted(
        "Records",
        &format!(
            "{}  {}",
            ui.meter(ratio, 16, if rejected > 0 { Tone::Warn } else { Tone::Ok }),
            ui.paint(
                Tone::Muted,
                &format!(
                    "{processed} processed, {valid} valid {dot} {rejected} rejected {dot} {issues} validation issue(s)"
                )
            )
        ),
    )
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

async fn list(args: &ListEvidenceArgs, client: &ApiClient, json: bool, ui: &Ui) -> Rendered {
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
    let mut stdout = ui.header(&["evidence", "list"], client.base_url());
    stdout.push_str(&render_evidence_list(case_id.as_str(), &records, ui));
    Rendered {
        stdout,
        stderr: render_warnings(&warnings),
        exit_code: exit::SUCCESS,
    }
}

/// Exhibit table followed by the full SHA-256 of each item, in the given order.
pub fn render_evidence_list(case_id: &str, records: &[EvidenceRecord], ui: &Ui) -> String {
    if records.is_empty() {
        let mut out = format!(
            "\n{MARGIN}{}\n\n",
            ui.paint(
                Tone::Muted,
                &format!("No evidence returned by the API for case {case_id}.")
            )
        );
        out.push_str(&ui.next(
            "ingest",
            &format!("yukti evidence ingest <file> --case {case_id}"),
        ));
        return out;
    }
    let total: u64 = records.iter().map(|r| r.receipt.file_size_bytes).sum();
    let mut out = String::from("\n");
    out.push_str(
        &ui.stats(&[
            (records.len().to_string(), "exhibits", Tone::Strong),
            (
                human_bytes(total)
                    .split(" (")
                    .next()
                    .unwrap_or("")
                    .to_string(),
                "stored",
                Tone::Muted,
            ),
        ]),
    );
    out.push('\n');
    let rows: Vec<Vec<Cell>> = records
        .iter()
        .map(|record| {
            let r = &record.receipt;
            let (glyph, tone) = super::case::badge_mark(status_badge(&r.status));
            vec![
                Cell::new(r.evidence_id.clone(), Tone::Subtle),
                Cell::new(r.source_type.clone(), Tone::Text),
                Cell::new(r.status.clone(), Tone::Text).marked(glyph, tone),
                Cell::new(r.file_size_bytes.to_string(), Tone::Muted),
                Cell::new(r.created_at.clone(), Tone::Muted),
                Cell::new(r.original_filename.clone(), Tone::Strong),
            ]
        })
        .collect();
    out.push_str(&ui.table(
        &[
            Column::left("EVIDENCE ID"),
            Column::left("SOURCE"),
            Column::left("STATUS"),
            Column::right("BYTES"),
            Column::left("CREATED"),
            Column::left("FILE"),
        ],
        &rows,
    ));
    out.push_str(&ui.section("SHA-256", "as computed by the API"));
    for record in records {
        let r = &record.receipt;
        out.push_str(&format!(
            "{MARGIN}{}  {}  {}\n",
            ui.paint(Tone::Subtle, &short_id(&r.evidence_id, ui)),
            ui.paint(Tone::Text, &r.sha256_hash),
            ui.paint(Tone::Muted, &r.mime_type)
        ));
    }
    out.push_str(&ui.footnote(&format!(
        "{} evidence item(s) for case {case_id}, in the order returned by the API. SHA-256 values are as computed by the API.",
        records.len()
    )));
    out.push_str(&ui.next(
        "verify a copy",
        "yukti evidence verify --evidence <id> <file>",
    ));
    out
}

/* ------------------------------------------------------------------------- show, runs */

fn evidence_arg(raw: &str) -> Result<String, CliError> {
    let id = raw.trim();
    if id.is_empty() {
        return Err(CliError::InvalidArgument(
            "--evidence must not be blank".to_string(),
        ));
    }
    Ok(id.to_string())
}

#[derive(Serialize)]
struct EvidenceDocument<'a> {
    schema: &'static str,
    api_url: &'a str,
    evidence_id: &'a str,
    /// The receipt exactly as returned.
    evidence: &'a Value,
    /// Extraction runs exactly as returned, or `null` when they could not be listed.
    extraction_runs: Option<&'a Value>,
    warnings: &'a [String],
}

async fn show(args: &EvidenceArgs, client: &ApiClient, json: bool, ui: &Ui) -> Rendered {
    let id = match evidence_arg(&args.evidence_id) {
        Ok(id) => id,
        Err(err) => return render_error_with_notes(&err, json, &[]),
    };
    let path = evidence_path(&id, "");
    let url = client.url_for(&path);
    let record = match client
        .get_json(&path)
        .await
        .and_then(|v| EvidenceRecord::from_value(&url, v))
    {
        Ok(r) => r,
        Err(err) => return render_error_with_notes(&err, json, &[]),
    };
    let runs = client
        .get_json(&evidence_path(&id, "extraction/runs"))
        .await;
    let mut warnings = Vec::new();
    if let Err(err) = &runs {
        warnings.push(format!("Extraction runs could not be listed: {err}"));
    }
    if json {
        let doc = EvidenceDocument {
            schema: EVIDENCE_SCHEMA,
            api_url: client.base_url(),
            evidence_id: &id,
            evidence: &record.raw,
            extraction_runs: runs.as_ref().ok(),
            warnings: &warnings,
        };
        return Rendered::stdout(to_json(&doc), exit::SUCCESS);
    }
    let r = &record.receipt;
    let mut out = ui.header(&["evidence", "show"], client.base_url());
    out.push('\n');
    out.push_str(&format!(
        "{MARGIN}{}  {}\n",
        ui.bold(Tone::Strong, &r.original_filename),
        ui.badge(status_badge(&r.status), &r.status)
    ));
    out.push_str(&render_custody(r, None, ui));
    out.push_str(&render_receipt(r, ui));
    if let Ok(Value::Array(items)) = &runs {
        out.push_str(&ui.section(
            "Extraction runs",
            &format!("{} run(s), oldest first", items.len()),
        ));
        out.push_str(&render_runs_table(items, ui));
    }
    out.push('\n');
    out.push_str(&ui.next(
        "verify a copy",
        &format!("yukti evidence verify --evidence {id} <file>"),
    ));
    out.push_str(&ui.next(
        "active result",
        &format!("yukti evidence extraction --evidence {id}"),
    ));
    Rendered {
        stdout: out,
        stderr: render_warnings(&warnings),
        exit_code: exit::SUCCESS,
    }
}

/// Table of `ExtractionRunManifest` values.
pub fn render_runs_table(items: &[Value], ui: &Ui) -> String {
    if items.is_empty() {
        return format!(
            "{MARGIN}{}\n",
            ui.paint(Tone::Muted, "No extraction runs recorded.")
        );
    }
    let rows: Vec<Vec<Cell>> = items
        .iter()
        .map(|run| {
            let status = run["status"].as_str().unwrap_or("?");
            let (glyph, tone) = super::case::badge_mark(status_badge(status));
            let n = |k: &str| run[k].as_u64().map_or("?".to_string(), |v| v.to_string());
            vec![
                Cell::new(
                    run["extraction_run_id"].as_str().unwrap_or("?"),
                    Tone::Subtle,
                ),
                Cell::new(status, Tone::Text).marked(glyph, tone),
                Cell::new(run["started_at"].as_str().unwrap_or(""), Tone::Muted),
                Cell::new(n("valid_records_count"), Tone::Text),
                Cell::new(n("rejected_records_count"), Tone::Text),
                Cell::new(
                    run["duration_ms"]
                        .as_f64()
                        .map_or(String::new(), |d| format!("{d:.0} ms")),
                    Tone::Muted,
                ),
                Cell::new(
                    format!(
                        "{} {}",
                        run["extractor_name"].as_str().unwrap_or(""),
                        run["extractor_version"].as_str().unwrap_or("")
                    ),
                    Tone::Muted,
                ),
            ]
        })
        .collect();
    ui.table(
        &[
            Column::left("RUN ID"),
            Column::left("STATUS"),
            Column::left("STARTED"),
            Column::right("VALID"),
            Column::right("REJECTED"),
            Column::right("TIME"),
            Column::left("EXTRACTOR"),
        ],
        &rows,
    )
}

#[derive(Serialize)]
struct RunsDocument<'a> {
    schema: &'static str,
    api_url: &'a str,
    evidence_id: &'a str,
    count: usize,
    runs: &'a Value,
}

async fn runs(args: &EvidenceArgs, client: &ApiClient, json: bool, ui: &Ui) -> Rendered {
    let id = match evidence_arg(&args.evidence_id) {
        Ok(id) => id,
        Err(err) => return render_error_with_notes(&err, json, &[]),
    };
    let path = evidence_path(&id, "extraction/runs");
    let value = match client.get_json(&path).await {
        Ok(v) => v,
        Err(err) => return render_error_with_notes(&err, json, &[]),
    };
    let Value::Array(items) = &value else {
        let err = CliError::BadResponse {
            url: client.url_for(&path),
            reason: "expected a JSON array of extraction runs".to_string(),
        };
        return render_error_with_notes(&err, json, &[]);
    };
    if json {
        let doc = RunsDocument {
            schema: EXTRACTION_RUNS_SCHEMA,
            api_url: client.base_url(),
            evidence_id: &id,
            count: items.len(),
            runs: &value,
        };
        return Rendered::stdout(to_json(&doc), exit::SUCCESS);
    }
    let mut out = ui.header(&["evidence", "runs"], client.base_url());
    out.push_str(&ui.section(
        "Extraction runs",
        &format!("{} for {}", items.len(), short_id(&id, ui)),
    ));
    out.push_str(&render_runs_table(items, ui));
    out.push_str(&ui.footnote(
        "Runs are versioned and immutable; reprocessing adds a run, it never edits one.",
    ));
    Rendered::stdout(out, exit::SUCCESS)
}

/* --------------------------------------------------------------- extract, reprocess */

#[derive(Serialize)]
struct RunDocument<'a> {
    schema: &'static str,
    api_url: &'a str,
    evidence_id: &'a str,
    request: super::RequestDoc<'a>,
    run: &'a Value,
}

/// Body of `POST /api/v1/evidence/:id/reprocess` (`ReprocessRequest`); omitted values are not sent.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct ReprocessRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_timezone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clock_offset_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_country_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
}

/// Validates `evidence reprocess` options into the request body.
pub fn build_reprocess_request(args: &ReprocessArgs) -> Result<ReprocessRequest, CliError> {
    let source_timezone = non_blank(&args.source_timezone, "--source-timezone")?;
    let clock_offset_seconds = match &args.clock_offset_seconds {
        None => None,
        Some(raw) => Some(parse_clock_offset(raw)?.parse::<f64>().map_err(|_| {
            CliError::InvalidArgument(format!(
                "--clock-offset-seconds must be a finite number of seconds, got {raw:?}"
            ))
        })?),
    };
    let default_country_code =
        match non_blank(&args.default_country_code, "--default-country-code")? {
            Some(cc) if !cc.chars().all(|c| c.is_ascii_digit()) => {
                return Err(CliError::InvalidArgument(format!(
                    "--default-country-code must contain digits only, e.g. 91, got {cc:?}"
                )))
            }
            other => other,
        };
    let metadata = match &args.metadata {
        None => None,
        Some(raw) => Some(Value::Object(parse_metadata(raw)?)),
    };
    let request = ReprocessRequest {
        source_timezone,
        clock_offset_seconds,
        default_country_code,
        metadata,
    };
    if request == ReprocessRequest::default() {
        return Err(CliError::InvalidArgument(
            "reprocess needs at least one of --source-timezone, --clock-offset-seconds, --default-country-code or --metadata; use `yukti evidence extract` to re-run with defaults".to_string(),
        ));
    }
    Ok(request)
}

async fn extract(args: &EvidenceArgs, client: &ApiClient, json: bool, ui: &Ui) -> Rendered {
    let id = match evidence_arg(&args.evidence_id) {
        Ok(id) => id,
        Err(err) => return render_error_with_notes(&err, json, &[]),
    };
    let body = Value::Object(serde_json::Map::new());
    post_run(&id, "extract", &body, client, json, ui).await
}

async fn reprocess(args: &ReprocessArgs, client: &ApiClient, json: bool, ui: &Ui) -> Rendered {
    let prepared =
        evidence_arg(&args.evidence_id).and_then(|id| Ok((id, build_reprocess_request(args)?)));
    let (id, request) = match prepared {
        Ok(p) => p,
        Err(err) => return render_error_with_notes(&err, json, &[]),
    };
    let body = serde_json::to_value(&request).unwrap_or(Value::Null);
    post_run(&id, "reprocess", &body, client, json, ui).await
}

async fn post_run(
    id: &str,
    action: &'static str,
    body: &Value,
    client: &ApiClient,
    json: bool,
    ui: &Ui,
) -> Rendered {
    let path = evidence_path(id, action);
    let run = match client.post_json(&path, body).await {
        Ok(v) => v,
        Err(err) => {
            let note = "The original evidence file is never modified; a failed run leaves earlier runs in place.".to_string();
            return render_error_with_notes(&err, json, &[note]);
        }
    };
    let status = run["status"].as_str().unwrap_or("UNKNOWN");
    let exit_code = if matches!(status, "SUCCESS" | "PARTIAL_SUCCESS") {
        exit::SUCCESS
    } else {
        exit::SERVER_ERROR
    };
    if json {
        let doc = RunDocument {
            schema: EXTRACTION_RUN_SCHEMA,
            api_url: client.base_url(),
            evidence_id: id,
            request: super::RequestDoc {
                method: "POST",
                path: &path,
                body: Some(body),
            },
            run: &run,
        };
        return Rendered::stdout(to_json(&doc), exit_code);
    }
    let mut out = ui.header(&["evidence", action], client.base_url());
    out.push('\n');
    out.push_str(&format!(
        "{MARGIN}{}  {}\n",
        ui.badge(status_badge(status), status),
        ui.paint(Tone::Text, &format!("Extraction run for evidence {id}"))
    ));
    if action == "reprocess" && !body.as_object().is_none_or(|m| m.is_empty()) {
        out.push_str(&ui.section("Declared for this run", ""));
        for (k, v) in body.as_object().into_iter().flatten() {
            let text = v.as_str().map_or_else(|| compact_json(v), str::to_string);
            out.push_str(&ui.kv_painted(
                k,
                &format!(
                    "{} {}",
                    ui.mark(Glyph::Supplied, Tone::Supplied),
                    ui.paint(Tone::Supplied, &text)
                ),
            ));
        }
    }
    out.push_str(&render_run_manifest(&run, ui));
    out.push('\n');
    out.push_str(&ui.next(
        "result",
        &format!("yukti evidence extraction --evidence {id}"),
    ));
    out.push_str(&ui.next("history", &format!("yukti evidence runs --evidence {id}")));
    Rendered::stdout(out, exit_code)
}

/// Field list of one `ExtractionRunManifest`.
pub fn render_run_manifest(run: &Value, ui: &Ui) -> String {
    let s = |k: &str| run[k].as_str().unwrap_or("").to_string();
    let n = |k: &str| run[k].as_u64().unwrap_or(0);
    let mut out = ui.section("Run manifest", "");
    out.push_str(&ui.kv("Run ID", Tone::Text, &s("extraction_run_id")));
    out.push_str(&ui.kv(
        "Extractor",
        Tone::Text,
        &format!("{} {}", s("extractor_name"), s("extractor_version")),
    ));
    out.push_str(&render_record_counts(
        n("records_processed"),
        n("valid_records_count"),
        n("rejected_records_count"),
        n("validation_issue_count"),
        ui,
    ));
    out.push_str(&ui.kv("Started", Tone::Muted, &s("started_at")));
    if let Some(done) = run["completed_at"].as_str() {
        out.push_str(&ui.kv("Completed", Tone::Muted, done));
    }
    if let Some(ms) = run["duration_ms"].as_f64() {
        out.push_str(&ui.kv("Duration", Tone::Muted, &format!("{ms:.0} ms")));
    }
    out.push_str(&ui.kv("Source SHA-256", Tone::Subtle, &s("source_sha256")));
    if let Some(h) = run["result_sha256"].as_str() {
        out.push_str(&ui.kv("Result SHA-256", Tone::Subtle, h));
    }
    out.push_str(&ui.kv("Config hash", Tone::Subtle, &s("configuration_hash")));
    if let Some(err) = run["error_message"].as_str() {
        out.push_str(&ui.kv("Error", Tone::Flag, err));
    }
    out
}

/* ------------------------------------------------------------------------- extraction */

#[derive(Serialize)]
struct ExtractionDocument<'a> {
    schema: &'static str,
    api_url: &'a str,
    evidence_id: &'a str,
    result: &'a Value,
}

async fn extraction(args: &EvidenceArgs, client: &ApiClient, json: bool, ui: &Ui) -> Rendered {
    let id = match evidence_arg(&args.evidence_id) {
        Ok(id) => id,
        Err(err) => return render_error_with_notes(&err, json, &[]),
    };
    let result = match client.get_json(&evidence_path(&id, "extraction")).await {
        Ok(v) => v,
        Err(err) => {
            let notes = if matches!(err, CliError::Http { status: 404, .. }) {
                vec![format!(
                    "Run `yukti evidence extract --evidence {id}` to create an extraction run."
                )]
            } else {
                Vec::new()
            };
            return render_error_with_notes(&err, json, &notes);
        }
    };
    if json {
        let doc = ExtractionDocument {
            schema: EXTRACTION_RESULT_SCHEMA,
            api_url: client.base_url(),
            evidence_id: &id,
            result: &result,
        };
        return Rendered::stdout(to_json(&doc), exit::SUCCESS);
    }
    let mut out = ui.header(&["evidence", "extraction"], client.base_url());
    Rendered::stdout(
        {
            out.push_str(&render_extraction_result(&result, ui));
            out
        },
        exit::SUCCESS,
    )
}

/// `UnifiedExtractionResult`: counts, event candidates, identifiers, entities and issues.
pub fn render_extraction_result(r: &Value, ui: &Ui) -> String {
    let arr = |k: &str| r[k].as_array().map(Vec::as_slice).unwrap_or(&[]);
    let s = |v: &Value, k: &str| v[k].as_str().unwrap_or("").to_string();
    let status = r["status"].as_str().unwrap_or("UNKNOWN");
    let mut out = String::from("\n");
    out.push_str(&format!(
        "{MARGIN}{}  {}\n",
        ui.badge(status_badge(status), status),
        ui.paint(
            Tone::Muted,
            &format!(
                "{} {} evidence {}",
                s(r, "source_type"),
                ui.g(Glyph::Dot),
                s(r, "evidence_id")
            )
        )
    ));
    out.push('\n');
    out.push_str(&ui.stats(&[
        (
            arr("event_candidates").len().to_string(),
            "events",
            Tone::Strong,
        ),
        (
            arr("identifiers").len().to_string(),
            "identifiers",
            Tone::Muted,
        ),
        (
            arr("resolved_entities").len().to_string(),
            "entities",
            Tone::Inferred,
        ),
        (
            arr("relationship_candidates").len().to_string(),
            "relationships",
            Tone::Inferred,
        ),
        (
            arr("validation_issues").len().to_string(),
            "issues",
            if arr("validation_issues").is_empty() {
                Tone::Muted
            } else {
                Tone::Warn
            },
        ),
    ]));
    out.push_str(&render_record_counts(
        r["total_records_processed"].as_u64().unwrap_or(0),
        r["valid_records_count"].as_u64().unwrap_or(0),
        r["rejected_records_count"].as_u64().unwrap_or(0),
        arr("validation_issues").len() as u64,
        ui,
    ));

    let events = arr("event_candidates");
    out.push_str(&ui.section(
        "Event candidates",
        &format!("{} observed in the source", events.len()),
    ));
    if events.is_empty() {
        out.push_str(&format!("{MARGIN}{}\n", ui.paint(Tone::Muted, "none")));
    } else {
        let rows: Vec<Vec<Cell>> = events
            .iter()
            .take(25)
            .map(|e| {
                vec![
                    Cell::new(s(e, "raw_timestamp"), Tone::Text)
                        .marked(Glyph::Observed, Tone::Text),
                    Cell::new(s(e, "event_type"), Tone::Text),
                    Cell::new(s(e, "source_record_id"), Tone::Subtle),
                    Cell::new(
                        e["location_name"].as_str().unwrap_or("").to_string(),
                        Tone::Muted,
                    ),
                    Cell::new(
                        e["description"].as_str().unwrap_or("").to_string(),
                        Tone::Muted,
                    ),
                ]
            })
            .collect();
        out.push_str(&ui.table(
            &[
                Column::left("RAW TIMESTAMP"),
                Column::left("TYPE"),
                Column::left("RECORD").max(14),
                Column::left("LOCATION").max(22),
                Column::left("DESCRIPTION").max(40),
            ],
            &rows,
        ));
        if events.len() > 25 {
            out.push_str(&ui.hint(&format!("{} more in --json", events.len() - 25)));
        }
    }

    let entities = arr("resolved_entities");
    if !entities.is_empty() {
        out.push_str(&ui.section(
            "Resolved entity candidates",
            "inferred equivalence, not identity",
        ));
        for e in entities.iter().take(15) {
            let mentions = e["raw_mentions"].as_array().map_or(0, Vec::len);
            out.push_str(&format!(
                "{MARGIN}{} {}  {}\n",
                ui.mark(Glyph::Inferred, Tone::Inferred),
                ui.paint(Tone::Inferred, &s(e, "canonical_identifier")),
                ui.paint(
                    Tone::Subtle,
                    &format!(
                        "{} {} {mentions} mention(s)",
                        s(e, "identifier_type"),
                        ui.g(Glyph::Dot)
                    )
                )
            ));
        }
    }

    let issues = arr("validation_issues");
    if !issues.is_empty() {
        out.push_str(&ui.section("Validation issues", &format!("{}", issues.len())));
        for i in issues.iter().take(15) {
            let lead = format!(
                "{MARGIN}{} {}  ",
                ui.mark(Glyph::Assumed, Tone::Warn),
                ui.paint(
                    Tone::Warn,
                    &format!("{}.{}", s(i, "issue_type"), s(i, "field_name"))
                )
            );
            out.push_str(&lead);
            out.push_str(&ui.paint(Tone::Muted, &s(i, "message")));
            out.push('\n');
        }
    }
    out
}

/* ---------------------------------------------------------------------------- verify */

#[derive(Serialize)]
struct HashFacts<'a> {
    sha256: &'a str,
    size_bytes: u64,
}

#[derive(Serialize)]
struct VerifyDocument<'a> {
    schema: &'static str,
    api_url: &'a str,
    evidence_id: &'a str,
    file_path: &'a str,
    local: HashFacts<'a>,
    recorded: HashFacts<'a>,
    /// `true` when the SHA-256 values are equal; the exit code is 7 otherwise.
    hash_match: bool,
    size_match: bool,
}

/// Streams a file through SHA-256 without loading it whole. The file is only read.
pub fn hash_file(path: &Path) -> Result<(String, u64), CliError> {
    use std::io::Read;
    let shown = path.display();
    let meta = std::fs::metadata(path).map_err(|e| {
        CliError::InvalidArgument(match e.kind() {
            std::io::ErrorKind::NotFound => format!("file {shown} does not exist"),
            _ => format!("cannot access file {shown}: {e}"),
        })
    })?;
    if !meta.is_file() {
        return Err(CliError::InvalidArgument(format!(
            "{shown} is not a regular file"
        )));
    }
    let mut file = std::fs::File::open(path)
        .map_err(|e| CliError::InvalidArgument(format!("cannot open {shown}: {e}")))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    let mut size = 0u64;
    loop {
        let n = file
            .read(&mut buf)
            .map_err(|e| CliError::InvalidArgument(format!("cannot read {shown}: {e}")))?;
        if n == 0 {
            break;
        }
        size += n as u64;
        hasher.update(&buf[..n]);
    }
    let hex = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    Ok((hex, size))
}

async fn verify(args: &VerifyEvidenceArgs, client: &ApiClient, json: bool, ui: &Ui) -> Rendered {
    let id = match evidence_arg(&args.evidence_id) {
        Ok(id) => id,
        Err(err) => return render_error_with_notes(&err, json, &[]),
    };
    let (local, size) = match hash_file(&args.file) {
        Ok(h) => h,
        Err(err) => return render_error_with_notes(&err, json, &[]),
    };
    let path = evidence_path(&id, "");
    let url = client.url_for(&path);
    let record = match client
        .get_json(&path)
        .await
        .and_then(|v| EvidenceRecord::from_value(&url, v))
    {
        Ok(r) => r,
        Err(err) => return render_error_with_notes(&err, json, &[]),
    };
    let r = &record.receipt;
    let hash_match = local.eq_ignore_ascii_case(&r.sha256_hash);
    let size_match = size == r.file_size_bytes;
    let exit_code = if hash_match {
        exit::SUCCESS
    } else {
        exit::VERIFICATION_FAILED
    };
    let file_path = args.file.display().to_string();
    if json {
        let doc = VerifyDocument {
            schema: EVIDENCE_VERIFY_SCHEMA,
            api_url: client.base_url(),
            evidence_id: &id,
            file_path: &file_path,
            local: HashFacts {
                sha256: &local,
                size_bytes: size,
            },
            recorded: HashFacts {
                sha256: &r.sha256_hash,
                size_bytes: r.file_size_bytes,
            },
            hash_match,
            size_match,
        };
        return Rendered::stdout(to_json(&doc), exit_code);
    }
    let mut out = ui.header(&["evidence", "verify"], client.base_url());
    out.push('\n');
    out.push_str(&format!(
        "{MARGIN}{}  {}\n",
        if hash_match {
            ui.badge(Badge::Ok, "verified")
        } else {
            ui.badge(Badge::Flag, "mismatch")
        },
        ui.paint(
            Tone::Text,
            if hash_match {
                "The local file is byte-identical to the ingested exhibit."
            } else {
                "The local file does NOT match the ingested exhibit."
            }
        )
    ));
    out.push_str(&render_custody(r, Some(&local), ui));
    out.push_str(&ui.kv("Local file", Tone::Strong, &file_path));
    out.push_str(&ui.kv(
        "Exhibit",
        Tone::Text,
        &format!("{} ({})", r.original_filename, r.evidence_id),
    ));
    let size_line = format!(
        "local {} {} recorded {}",
        human_bytes(size),
        ui.g(Glyph::Dot),
        human_bytes(r.file_size_bytes)
    );
    out.push_str(&ui.kv(
        "Size",
        if size_match { Tone::Muted } else { Tone::Flag },
        &size_line,
    ));
    out.push_str(&ui.footnote("The hash is compared with the value the API recorded at ingestion; the CLI never modifies either file."));
    Rendered::stdout(out, exit_code)
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
