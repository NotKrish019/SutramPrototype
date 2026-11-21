//! `yukti evidence ingest | list` against a mock API, through the library entry point.

mod support;

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use clap::Parser;
use serde_json::{json, Value};
use support::{fixtures, MockServer, Route};
use yukti_cli::cli::Cli;
use yukti_cli::config::EnvVars;
use yukti_cli::output::Rendered;

const CASE_PATH: &str = "/api/v1/cases/3f2b8c1e-7a4d-4e1b-9c3a-5d6e7f8a9b0c";
const EVIDENCE_PATH: &str = "/api/v1/cases/3f2b8c1e-7a4d-4e1b-9c3a-5d6e7f8a9b0c/evidence";
const UPLOAD_PATH: &str = "/api/v1/evidence/upload";
const CDR_CSV: &[u8] = b"record_id,caller_phone,receiver_phone,call_timestamp,duration_seconds\r\ncdr-1,+919811002233,+919899887766,2026-11-10T21:45:12+05:30,184\r\n";
const CHAT_JSON: &[u8] = br#"{"chat_id":"grp-1","messages":[]}"#;

/// A scratch directory under the system temp dir, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        static N: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "yukti-cli-evidence-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).expect("create scratch dir");
        Self(dir)
    }

    fn file(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, bytes).expect("write scratch file");
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A fixture receipt with `file_size_bytes` set to the real size of the uploaded bytes.
fn receipt(fixture: &str, size: usize) -> Value {
    let mut v: Value = serde_json::from_str(fixture).unwrap();
    v["file_size_bytes"] = json!(size);
    v
}

fn extraction(status: &str, error: Option<&str>) -> Value {
    json!({
        "extraction_run_id": "run-0001",
        "evidence_id": fixtures::RECEIPT_CSV_ID,
        "case_id": fixtures::CASE_A_ID,
        "status": status,
        "records_processed": 2,
        "valid_records_count": if status == "FAILED" { 0 } else { 2 },
        "rejected_records_count": if status == "FAILED" { 2 } else { 0 },
        "validation_issue_count": if status == "FAILED" { 1 } else { 0 },
        "duration_ms": 4.25,
        "configuration_hash": "c0ffee",
        "completed_at": "2026-11-21T09:30:01Z",
        "error_message": error,
    })
}

fn upload_api(upload_status: u16, upload_body: &str) -> MockServer {
    MockServer::start(vec![
        Route::json(CASE_PATH, 200, fixtures::CASE_A).method("GET"),
        Route::json(UPLOAD_PATH, upload_status, upload_body).method("POST"),
    ])
}

async fn yukti(base_url: &str, args: &[&str]) -> Rendered {
    let mut argv = vec!["yukti", "--api-url", base_url];
    argv.extend_from_slice(args);
    let cli = Cli::try_parse_from(argv).expect("valid arguments");
    yukti_cli::execute(&cli, &EnvVars::default()).await
}

fn parse(r: &Rendered) -> Value {
    assert!(
        r.stderr.is_empty(),
        "JSON mode writes nothing to stderr: {}",
        r.stderr
    );
    serde_json::from_str(&r.stdout).expect("stdout is one JSON document")
}

fn part_names(server: &MockServer) -> Vec<String> {
    let upload = server
        .requests()
        .into_iter()
        .find(|r| r.path == UPLOAD_PATH)
        .expect("upload request");
    upload
        .multipart_parts()
        .into_iter()
        .map(|p| p.name)
        .collect()
}

/* ----------------------------------------------------------------------------- ingest: ok */

#[tokio::test]
async fn csv_upload_checks_the_case_then_sends_the_file_unmodified() {
    let scratch = Scratch::new();
    let path = scratch.file("cdr.csv", CDR_CSV);
    let before = std::fs::metadata(&path).unwrap().modified().unwrap();
    let server = upload_api(
        201,
        &receipt(fixtures::RECEIPT_CSV, CDR_CSV.len()).to_string(),
    );

    let r = yukti(
        &server.base_url,
        &[
            "evidence",
            "ingest",
            "--case",
            fixtures::CASE_A_ID,
            path.to_str().unwrap(),
            "--source-timezone",
            "Asia/Kolkata",
            "--clock-offset-seconds",
            "-30",
        ],
    )
    .await;
    assert_eq!(r.exit_code, 0, "{}{}", r.stdout, r.stderr);
    assert!(r.stderr.is_empty(), "{}", r.stderr);

    // Preflight first, then exactly one upload.
    let requests = server.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(
        (requests[0].method.as_str(), requests[0].path.as_str()),
        ("GET", CASE_PATH)
    );
    assert_eq!(
        (requests[1].method.as_str(), requests[1].path.as_str()),
        ("POST", UPLOAD_PATH)
    );
    assert!(requests[1]
        .content_type
        .as_deref()
        .unwrap()
        .starts_with("multipart/form-data; boundary="));

    let parts = requests[1].multipart_parts();
    let names: Vec<&str> = parts.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(
        names,
        ["case_id", "source_timezone", "clock_offset_seconds", "file"]
    );
    assert_eq!(parts[0].data, fixtures::CASE_A_ID.as_bytes());
    assert_eq!(parts[1].data, b"Asia/Kolkata");
    assert_eq!(parts[2].data, b"-30");
    let file = &parts[3];
    assert_eq!(file.filename.as_deref(), Some("cdr.csv"));
    assert_eq!(
        file.content_type.as_deref(),
        Some("application/octet-stream")
    );
    assert_eq!(file.data, CDR_CSV, "file bytes are sent exactly");

    // The original file is untouched.
    assert_eq!(std::fs::read(&path).unwrap(), CDR_CSV);
    assert_eq!(
        std::fs::metadata(&path).unwrap().modified().unwrap(),
        before
    );

    for expected in [
        format!("Ingested evidence {}", fixtures::RECEIPT_CSV_ID),
        format!(
            "SHA-256         {} (computed by the API)",
            fixtures::RECEIPT_CSV_SHA
        ),
        "Source type     TABULAR_CSV".to_string(),
        format!("Size            {} bytes", CDR_CSV.len()),
        "Timezone        Asia/Kolkata (declared)".to_string(),
        "Clock offset    -30.0 (declared)".to_string(),
        "Auto-extract    not requested".to_string(),
    ] {
        assert!(
            r.stdout.contains(&expected),
            "missing {expected:?} in\n{}",
            r.stdout
        );
    }
}

#[tokio::test]
async fn json_upload_without_options_sends_no_optional_fields() {
    let scratch = Scratch::new();
    let path = scratch.file("chat.json", CHAT_JSON);
    let server = upload_api(
        201,
        &receipt(fixtures::RECEIPT_CHAT, CHAT_JSON.len()).to_string(),
    );

    let r = yukti(
        &server.base_url,
        &[
            "evidence",
            "ingest",
            "--case",
            fixtures::CASE_A_ID,
            path.to_str().unwrap(),
        ],
    )
    .await;
    assert_eq!(r.exit_code, 0, "{}{}", r.stdout, r.stderr);
    // Nothing is defaulted: no timezone, no clock offset, no auto_extract, no metadata.
    assert_eq!(part_names(&server), ["case_id", "file"]);
    assert!(r.stdout.contains("Timezone        not declared"));
    assert!(r.stdout.contains("Clock offset    not declared"));
    assert!(r.stdout.contains("Source type     CHAT_EXPORT"));

    let v = parse(
        &yukti(
            &server.base_url,
            &[
                "--json",
                "evidence",
                "ingest",
                "--case",
                fixtures::CASE_A_ID,
                path.to_str().unwrap(),
            ],
        )
        .await,
    );
    assert_eq!(v["schema"], "yukti.cli.evidence_ingest/v1");
    assert_eq!(v["evidence_id"], fixtures::RECEIPT_CHAT_ID);
    assert_eq!(v["request"]["source_timezone"], Value::Null);
    assert_eq!(v["request"]["clock_offset_seconds"], Value::Null);
    assert_eq!(v["request"]["auto_extract"], Value::Null);
    assert_eq!(v["request"]["file_name"], "chat.json");
    assert_eq!(v["request"]["file_size_bytes"], CHAT_JSON.len());
    assert_eq!(v["auto_extract_requested"], false);
    assert_eq!(v["extraction"], Value::Null);
    assert_eq!(
        v["response"],
        receipt(fixtures::RECEIPT_CHAT, CHAT_JSON.len())
    );
}

#[tokio::test]
async fn every_supplied_option_is_sent_exactly_once() {
    let scratch = Scratch::new();
    let video: &[u8] = b"\x00\x00\x00\x18ftypmp42 fake video bytes";
    let path = scratch.file("recording.mp4", video);
    let mut body = receipt(fixtures::RECEIPT_CSV, video.len());
    body["source_type"] = json!("CCTV_VIDEO");
    body["original_filename"] = json!("recording.mp4");
    let response = json!({"receipt": body, "extraction": extraction("SUCCESS", None)});
    let server = upload_api(201, &response.to_string());

    let r = yukti(
        &server.base_url,
        &[
            "evidence",
            "ingest",
            "--case",
            fixtures::CASE_A_ID,
            path.to_str().unwrap(),
            "--source-type",
            "CCTV_VIDEO",
            "--source-timezone",
            "Asia/Kolkata",
            "--clock-offset-seconds=12.5",
            "--metadata",
            r#"{"camera":"dock-2", "operator": "INV-7"}"#,
            "--auto-extract",
        ],
    )
    .await;
    assert_eq!(r.exit_code, 0, "{}{}", r.stdout, r.stderr);
    let upload = server
        .requests()
        .into_iter()
        .find(|r| r.path == UPLOAD_PATH)
        .unwrap();
    let parts = upload.multipart_parts();
    let get = |name: &str| -> Vec<u8> {
        let matching: Vec<_> = parts.iter().filter(|p| p.name == name).collect();
        assert_eq!(matching.len(), 1, "{name} sent once");
        matching[0].data.clone()
    };
    assert_eq!(get("source_type"), b"CCTV_VIDEO");
    assert_eq!(get("source_timezone"), b"Asia/Kolkata");
    assert_eq!(get("clock_offset_seconds"), b"12.5");
    assert_eq!(get("auto_extract"), b"true");
    let metadata: Value = serde_json::from_slice(&get("metadata")).unwrap();
    assert_eq!(metadata, json!({"camera": "dock-2", "operator": "INV-7"}));
    assert_eq!(get("file"), video);
    // A --source-type override is called out.
    assert!(r
        .stderr
        .contains("--source-type replaces the type the API detects"));
}

#[tokio::test]
async fn auto_extract_success_is_reported() {
    let scratch = Scratch::new();
    let path = scratch.file("cdr.csv", CDR_CSV);
    let response = json!({
        "receipt": receipt(fixtures::RECEIPT_CSV, CDR_CSV.len()),
        "extraction": extraction("SUCCESS", None)
    });
    let server = upload_api(201, &response.to_string());
    let args = [
        "evidence",
        "ingest",
        "--case",
        fixtures::CASE_A_ID,
        path.to_str().unwrap(),
        "--auto-extract",
    ];

    let r = yukti(&server.base_url, &args).await;
    assert_eq!(r.exit_code, 0);
    assert!(r
        .stdout
        .contains("Auto-extract    requested; extraction succeeded (status SUCCESS)"));
    assert!(r.stdout.contains("Run ID        run-0001"));
    assert!(r
        .stdout
        .contains("2 processed, 2 valid, 0 rejected, 0 validation issue(s)"));

    let mut json_args = vec!["--json"];
    json_args.extend_from_slice(&args);
    let v = parse(&yukti(&server.base_url, &json_args).await);
    assert_eq!(v["auto_extract_requested"], true);
    assert_eq!(v["request"]["auto_extract"], "true");
    assert_eq!(
        v["extraction"],
        json!({"status": "SUCCESS", "succeeded": true})
    );
    assert_eq!(v["response"], response, "complete response passed through");
    assert_eq!(v["sha256_hash"], fixtures::RECEIPT_CSV_SHA);
}

#[tokio::test]
async fn failed_auto_extraction_is_explicit_but_ingestion_still_succeeded() {
    let scratch = Scratch::new();
    let path = scratch.file("cdr.csv", CDR_CSV);
    let response = json!({
        "receipt": receipt(fixtures::RECEIPT_CSV, CDR_CSV.len()),
        "extraction": extraction("FAILED", Some("Extraction failed with 1 validation issue(s)."))
    });
    let server = upload_api(201, &response.to_string());
    let args = [
        "evidence",
        "ingest",
        "--case",
        fixtures::CASE_A_ID,
        path.to_str().unwrap(),
        "--auto-extract",
    ];

    let r = yukti(&server.base_url, &args).await;
    assert_eq!(
        r.exit_code, 0,
        "the evidence was ingested; extraction status is reported"
    );
    assert!(r
        .stdout
        .contains("extraction did NOT succeed (status FAILED)"));
    assert!(r
        .stdout
        .contains("Error         Extraction failed with 1 validation issue(s)."));

    let mut json_args = vec!["--json"];
    json_args.extend_from_slice(&args);
    let v = parse(&yukti(&server.base_url, &json_args).await);
    assert_eq!(
        v["extraction"],
        json!({"status": "FAILED", "succeeded": false})
    );
}

#[tokio::test]
async fn missing_extraction_after_request_is_flagged() {
    let scratch = Scratch::new();
    let path = scratch.file("cdr.csv", CDR_CSV);
    let server = upload_api(
        201,
        &receipt(fixtures::RECEIPT_CSV, CDR_CSV.len()).to_string(),
    );
    let r = yukti(
        &server.base_url,
        &[
            "evidence",
            "ingest",
            "--case",
            fixtures::CASE_A_ID,
            path.to_str().unwrap(),
            "--auto-extract",
        ],
    )
    .await;
    assert_eq!(r.exit_code, 0);
    assert!(r
        .stdout
        .contains("requested, but no extraction run was returned"));
    assert!(r
        .stderr
        .contains("Auto-extraction was requested, but the API returned no extraction run."));
}

/* -------------------------------------------------------------- ingest: nothing is uploaded */

#[tokio::test]
async fn unknown_case_stops_before_upload() {
    let scratch = Scratch::new();
    let path = scratch.file("cdr.csv", CDR_CSV);
    let server = MockServer::start(vec![
        Route::json(CASE_PATH, 404, fixtures::NOT_FOUND),
        Route::json(UPLOAD_PATH, 201, fixtures::RECEIPT_CSV),
    ]);
    let r = yukti(
        &server.base_url,
        &[
            "evidence",
            "ingest",
            "--case",
            fixtures::CASE_A_ID,
            path.to_str().unwrap(),
        ],
    )
    .await;
    assert_eq!(r.exit_code, 4);
    assert!(r.stderr.contains("HTTP 404 (NOT_FOUND)"));
    assert!(r.stderr.contains("note: Nothing was uploaded"));
    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].path, CASE_PATH);
}

#[tokio::test]
async fn local_validation_failures_send_no_request() {
    let scratch = Scratch::new();
    let good = scratch.file("cdr.csv", CDR_CSV);
    let empty = scratch.file("empty.csv", b"");
    let good = good.to_str().unwrap();
    let missing = scratch.0.join("nope.csv");
    let missing = missing.to_str().unwrap();
    let dir = scratch.0.to_str().unwrap();
    let id = fixtures::CASE_A_ID;
    let server = upload_api(201, fixtures::RECEIPT_CSV);

    let cases: Vec<(Vec<&str>, &str)> = vec![
        (
            vec!["--case", "CASE-026", good],
            "--case must be a case UUID",
        ),
        (vec!["--case", id, missing], "does not exist"),
        (vec!["--case", id, dir], "not a regular file"),
        (
            vec!["--case", id, empty.to_str().unwrap()],
            "is empty (0 bytes)",
        ),
        (
            vec!["--case", id, good, "--metadata", "[1]"],
            "must be a JSON object",
        ),
        (
            vec!["--case", id, good, "--metadata", "{oops"],
            "not valid JSON",
        ),
        (
            vec![
                "--case",
                id,
                good,
                "--metadata",
                r#"{"source_timezone":"UTC"}"#,
            ],
            "use --source-timezone",
        ),
        (
            vec![
                "--case",
                id,
                good,
                "--metadata",
                r#"{"clock_offset_seconds":0}"#,
            ],
            "use --clock-offset-seconds",
        ),
        (
            vec!["--case", id, good, "--clock-offset-seconds", "ten"],
            "--clock-offset-seconds must be a finite number",
        ),
        (
            vec!["--case", id, good, "--clock-offset-seconds", "NaN"],
            "--clock-offset-seconds must be a finite number",
        ),
        (
            vec!["--case", id, good, "--source-timezone", " "],
            "--source-timezone must not be blank",
        ),
        (
            vec!["--case", id, good, "--source-type", ""],
            "--source-type must not be blank",
        ),
    ];
    for (extra, expected) in cases {
        let mut args = vec!["evidence", "ingest"];
        args.extend_from_slice(&extra);
        let r = yukti(&server.base_url, &args).await;
        assert_eq!(r.exit_code, 2, "{extra:?}: {}", r.stderr);
        assert!(r.stderr.contains(expected), "{extra:?}: {}", r.stderr);
    }
    let r = yukti(
        &server.base_url,
        &["--json", "evidence", "ingest", "--case", id, missing],
    )
    .await;
    let v = parse(&r);
    assert_eq!(v["error"]["kind"], "invalid_argument");
    assert!(server.requests().is_empty(), "no request for invalid input");
}

/* ---------------------------------------------------------------------- ingest: API errors */

#[tokio::test]
async fn upload_errors_map_to_exit_codes() {
    let scratch = Scratch::new();
    let path = scratch.file("cdr.csv", CDR_CSV);
    let path = path.to_str().unwrap();

    let too_large = upload_api(
        413,
        r#"{"error":{"code":"PAYLOAD_TOO_LARGE","message":"Upload exceeds maximum allowable evidence size of 52428800 bytes."}}"#,
    );
    let r = yukti(
        &too_large.base_url,
        &["evidence", "ingest", "--case", fixtures::CASE_A_ID, path],
    )
    .await;
    assert_eq!(r.exit_code, 4);
    assert!(r.stderr.contains("HTTP 413 (PAYLOAD_TOO_LARGE)"));
    let v = parse(
        &yukti(
            &too_large.base_url,
            &[
                "--json",
                "evidence",
                "ingest",
                "--case",
                fixtures::CASE_A_ID,
                path,
            ],
        )
        .await,
    );
    assert_eq!(v["error"]["http_status"], 413);
    assert_eq!(v["error"]["api_code"], "PAYLOAD_TOO_LARGE");

    // Axum's own body-limit rejection is plain text, not the API envelope.
    let framework = upload_api(413, "length limit exceeded");
    let r = yukti(
        &framework.base_url,
        &["evidence", "ingest", "--case", fixtures::CASE_A_ID, path],
    )
    .await;
    assert_eq!(r.exit_code, 4);
    assert!(r.stderr.contains("HTTP 413: length limit exceeded"));

    let unsupported = upload_api(
        415,
        r#"{"error":{"code":"UNSUPPORTED_MEDIA_TYPE","message":"Unsupported evidence format or media type"}}"#,
    );
    let r = yukti(
        &unsupported.base_url,
        &["evidence", "ingest", "--case", fixtures::CASE_A_ID, path],
    )
    .await;
    assert_eq!(r.exit_code, 4);
    assert!(
        !r.stderr.contains("may have been ingested"),
        "no auto-extract, no note"
    );

    let crash = upload_api(
        500,
        r#"{"error":{"code":"INTERNAL_SERVER_ERROR","message":"An internal server error occurred while processing the request."}}"#,
    );
    let r = yukti(
        &crash.base_url,
        &[
            "evidence",
            "ingest",
            "--case",
            fixtures::CASE_A_ID,
            path,
            "--auto-extract",
        ],
    )
    .await;
    assert_eq!(r.exit_code, 5);
    assert!(r
        .stderr
        .contains("note: With --auto-extract the API stores the evidence before extracting"));

    let garbage = upload_api(201, r#"{"evidence_id":"x"}"#);
    let r = yukti(
        &garbage.base_url,
        &["evidence", "ingest", "--case", fixtures::CASE_A_ID, path],
    )
    .await;
    assert_eq!(r.exit_code, 6);
    assert!(r.stderr.contains("does not match EvidenceUploadReceipt"));
}

#[tokio::test]
async fn ingestion_stage_rejections_do_not_claim_the_evidence_may_be_stored() {
    let scratch = Scratch::new();
    let path = scratch.file("recording.mp4", b"not really a video\n");
    let server = upload_api(
        422,
        r#"{"error":{"code":"UNPROCESSABLE_ENTITY","message":"Content validation rejected evidence 'recording.mp4': Unsupported or unrecognized evidence format."}}"#,
    );
    let r = yukti(
        &server.base_url,
        &[
            "evidence",
            "ingest",
            "--case",
            fixtures::CASE_A_ID,
            path.to_str().unwrap(),
            "--auto-extract",
        ],
    )
    .await;
    assert_eq!(r.exit_code, 4);
    assert!(r.stderr.contains("HTTP 422 (UNPROCESSABLE_ENTITY)"));
    assert!(!r.stderr.contains("note:"), "{}", r.stderr);
}

#[tokio::test]
async fn files_over_the_api_body_limit_get_an_explanation() {
    let scratch = Scratch::new();
    let big = vec![b'a'; 2 * 1024 * 1024 + 1];
    let path = scratch.file("big.txt", &big);
    // What the real API answers when its 2 MiB body limit cuts the multipart stream.
    let server = upload_api(
        400,
        r#"{"error":{"code":"BAD_REQUEST","message":"Streaming read error during upload: Error parsing `multipart/form-data` request"}}"#,
    );
    let r = yukti(
        &server.base_url,
        &[
            "evidence",
            "ingest",
            "--case",
            fixtures::CASE_A_ID,
            path.to_str().unwrap(),
        ],
    )
    .await;
    assert_eq!(r.exit_code, 4);
    assert!(r.stderr.contains("note: This file is 2097153 bytes"));
    assert!(r.stderr.contains("about 2 MiB"));
    let upload = server
        .requests()
        .into_iter()
        .find(|q| q.path == UPLOAD_PATH)
        .unwrap();
    let file = upload
        .multipart_parts()
        .into_iter()
        .find(|p| p.name == "file")
        .unwrap();
    assert_eq!(file.data.len(), big.len(), "the whole file was sent");
}

/* ------------------------------------------------------------------------------------ list */

fn list_api(evidence_status: u16, evidence_body: &str) -> MockServer {
    MockServer::start(vec![
        Route::json(CASE_PATH, 200, fixtures::CASE_A).method("GET"),
        Route::json(EVIDENCE_PATH, evidence_status, evidence_body).method("GET"),
    ])
}

#[tokio::test]
async fn list_shows_every_receipt_in_server_order() {
    let body = format!("[{},{}]", fixtures::RECEIPT_CHAT, fixtures::RECEIPT_CSV);
    let server = list_api(200, &body);
    let r = yukti(
        &server.base_url,
        &["evidence", "list", "--case", fixtures::CASE_A_ID],
    )
    .await;
    assert_eq!(r.exit_code, 0, "{}{}", r.stdout, r.stderr);
    assert!(r.stderr.is_empty(), "{}", r.stderr);

    let chat = r.stdout.find(fixtures::RECEIPT_CHAT_ID).unwrap();
    let csv = r.stdout.find(fixtures::RECEIPT_CSV_ID).unwrap();
    assert!(chat < csv, "server order preserved");
    assert!(r
        .stdout
        .contains("CHAT_EXPORT  37 bytes  2026-11-21T09:31:00Z  INGESTED"));
    assert!(r.stdout.contains("  file    cdr.csv"));
    assert!(r
        .stdout
        .contains(&format!("  sha256  {}", fixtures::RECEIPT_CSV_SHA)));
    assert!(r
        .stdout
        .contains("2 evidence item(s) for case 3f2b8c1e-7a4d-4e1b-9c3a-5d6e7f8a9b0c"));

    let requests = server.requests();
    assert_eq!(requests[0].path, CASE_PATH, "case is checked first");
    assert_eq!(requests[1].path, EVIDENCE_PATH);

    let v = parse(
        &yukti(
            &server.base_url,
            &["evidence", "list", "--case", fixtures::CASE_A_ID, "--json"],
        )
        .await,
    );
    assert_eq!(v["schema"], "yukti.cli.evidence_list/v1");
    assert_eq!(v["count"], 2);
    let expected: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["evidence"], expected, "complete server response");
}

#[tokio::test]
async fn empty_evidence_list_is_success() {
    let server = list_api(200, "[]");
    let r = yukti(
        &server.base_url,
        &["evidence", "list", "--case", fixtures::CASE_A_ID],
    )
    .await;
    assert_eq!(r.exit_code, 0);
    assert_eq!(
        r.stdout,
        "No evidence returned by the API for case 3f2b8c1e-7a4d-4e1b-9c3a-5d6e7f8a9b0c.\n"
    );
    let v = parse(
        &yukti(
            &server.base_url,
            &["--json", "evidence", "list", "--case", fixtures::CASE_A_ID],
        )
        .await,
    );
    assert_eq!(v["count"], 0);
    assert_eq!(v["evidence"], json!([]));
}

#[tokio::test]
async fn list_errors_map_to_exit_codes() {
    // Unknown case: the case check fails, the evidence endpoint is never called.
    let unknown = MockServer::start(vec![
        Route::json(CASE_PATH, 404, fixtures::NOT_FOUND),
        Route::json(EVIDENCE_PATH, 200, "[]"),
    ]);
    let r = yukti(
        &unknown.base_url,
        &["evidence", "list", "--case", fixtures::CASE_A_ID],
    )
    .await;
    assert_eq!(r.exit_code, 4);
    assert!(r.stderr.contains("Case not found"));
    assert_eq!(unknown.requests().len(), 1);

    // The evidence endpoint itself returning 404 or 5xx.
    let not_found = list_api(404, r#"{"error":{"code":"NOT_FOUND","message":"gone"}}"#);
    let r = yukti(
        &not_found.base_url,
        &["evidence", "list", "--case", fixtures::CASE_A_ID],
    )
    .await;
    assert_eq!(r.exit_code, 4);

    let broken = list_api(
        503,
        r#"{"error":{"code":"INTERNAL_SERVER_ERROR","message":"An internal server error occurred while processing the request."}}"#,
    );
    let r = yukti(
        &broken.base_url,
        &["evidence", "list", "--case", fixtures::CASE_A_ID],
    )
    .await;
    assert_eq!(r.exit_code, 5);
    let v = parse(
        &yukti(
            &broken.base_url,
            &["--json", "evidence", "list", "--case", fixtures::CASE_A_ID],
        )
        .await,
    );
    assert_eq!(v["error"]["kind"], "http_server_error");

    let malformed = list_api(200, r#"{"evidence":[]}"#);
    let r = yukti(
        &malformed.base_url,
        &["evidence", "list", "--case", fixtures::CASE_A_ID],
    )
    .await;
    assert_eq!(r.exit_code, 6);

    let bad_id = MockServer::start(vec![]);
    let r = yukti(
        &bad_id.base_url,
        &["evidence", "list", "--case", "CASE-026"],
    )
    .await;
    assert_eq!(r.exit_code, 2);
    assert!(bad_id.requests().is_empty());
}

#[tokio::test]
async fn list_flags_records_from_another_case() {
    let mut other: Value = serde_json::from_str(fixtures::RECEIPT_CHAT).unwrap();
    other["case_id"] = json!(fixtures::CASE_B_ID);
    let server = list_api(200, &json!([other]).to_string());
    let r = yukti(
        &server.base_url,
        &["evidence", "list", "--case", fixtures::CASE_A_ID],
    )
    .await;
    assert_eq!(r.exit_code, 0);
    assert!(r.stderr.contains("name a different case_id"));
}
