//! `yukti case list | create | show` against a mock API, through the library entry point.

mod support;

use clap::Parser;
use serde_json::{json, Value};
use support::{fixtures, unused_base_url, MockServer, Route};
use yukti_cli::cli::Cli;
use yukti_cli::config::EnvVars;
use yukti_cli::output::Rendered;

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

fn list_api(cases_body: &str, readiness: &str) -> MockServer {
    MockServer::start(vec![
        Route::json("/api/v1/cases", 200, cases_body).method("GET"),
        Route::json("/health/ready", 200, readiness),
    ])
}

/* ------------------------------------------------------------------------------- case list */

#[tokio::test]
async fn list_shows_every_case_in_server_order() {
    let body = format!("[{},{}]", fixtures::CASE_B, fixtures::CASE_A);
    let server = list_api(&body, fixtures::READY);
    let r = yukti(&server.base_url, &["case", "list"]).await;
    assert_eq!(r.exit_code, 0, "{}{}", r.stdout, r.stderr);
    assert!(
        r.stderr.is_empty(),
        "no warnings with PostgreSQL connected: {}",
        r.stderr
    );

    let b = r.stdout.find(fixtures::CASE_B_ID).expect("case B listed");
    let a = r.stdout.find(fixtures::CASE_A_ID).expect("case A listed");
    assert!(b < a, "server order preserved");
    assert!(r.stdout.starts_with("CASE ID"));
    assert!(r.stdout.contains("ATM skimming"));
    assert!(r.stdout.contains("DRAFT"));
    assert!(r.stdout.contains("2026-11-21T09:15:00Z"));
    assert!(r
        .stdout
        .contains("2 case(s), in the order returned by the API."));

    let v = parse(&yukti(&server.base_url, &["case", "list", "--json"]).await);
    assert_eq!(v["schema"], "yukti.cli.case_list/v1");
    assert_eq!(v["count"], 2);
    assert_eq!(v["cases"][0]["case_id"], fixtures::CASE_B_ID);
    let expected_a: Value = serde_json::from_str(fixtures::CASE_A).unwrap();
    assert_eq!(
        v["cases"][1], expected_a,
        "records are passed through unchanged"
    );
    assert_eq!(v["storage"]["database"], "connected");
    assert_eq!(v["warnings"], json!([]));
}

#[tokio::test]
async fn empty_list_is_success() {
    let server = list_api("[]", fixtures::READY);
    let r = yukti(&server.base_url, &["case", "list"]).await;
    assert_eq!(r.exit_code, 0);
    assert_eq!(r.stdout, "No cases returned by the API.\n");

    let v = parse(&yukti(&server.base_url, &["case", "list", "--json"]).await);
    assert_eq!(v["count"], 0);
    assert_eq!(v["cases"], json!([]));
}

#[tokio::test]
async fn filesystem_mode_and_placeholders_are_flagged() {
    let body = format!("[{},{}]", fixtures::CASE_A, fixtures::FS_PLACEHOLDER);
    let server = list_api(&body, fixtures::DEGRADED);
    let r = yukti(&server.base_url, &["case", "list"]).await;
    assert_eq!(r.exit_code, 0);
    assert!(r.stdout.contains("[filesystem placeholder]"));
    assert!(r
        .stderr
        .contains("warning: The API is running without PostgreSQL (filesystem mode)"));
    assert!(r.stderr.contains("9d8c7b6a-5f4e-4d3c-8b2a-1f0e9d8c7b6a"));
    // The record itself is shown as returned, not altered.
    assert!(r
        .stdout
        .contains("Case 9d8c7b6a-5f4e-4d3c-8b2a-1f0e9d8c7b6a"));

    let r = yukti(&server.base_url, &["--json", "case", "list"]).await;
    let v: Value = serde_json::from_str(&r.stdout).unwrap();
    assert_eq!(v["storage"]["database"], "unconfigured_filesystem_mode");
    assert_eq!(v["storage"]["readiness_status"], "degraded");
    assert_eq!(
        v["placeholder_case_ids"],
        json!(["9d8c7b6a-5f4e-4d3c-8b2a-1f0e9d8c7b6a"])
    );
    assert_eq!(v["warnings"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn degraded_only_because_of_the_graph_does_not_warn_about_storage() {
    let server = list_api(
        &format!("[{}]", fixtures::CASE_A),
        fixtures::DEGRADED_DB_CONNECTED,
    );
    let r = yukti(&server.base_url, &["case", "list"]).await;
    assert_eq!(r.exit_code, 0);
    assert!(r.stderr.is_empty(), "{}", r.stderr);
}

#[tokio::test]
async fn unknown_storage_mode_warns_but_does_not_fail() {
    // No readiness route: the probe returns 404.
    let server = MockServer::start(vec![Route::json(
        "/api/v1/cases",
        200,
        &format!("[{}]", fixtures::CASE_A),
    )]);
    let r = yukti(&server.base_url, &["case", "list"]).await;
    assert_eq!(r.exit_code, 0);
    assert!(r
        .stderr
        .contains("Could not determine the API's storage mode"));
}

#[tokio::test]
async fn non_uuid_case_ids_are_shown_but_flagged() {
    let folder = r#"{"case_id":"legacy-folder","title":"Case legacy-folder","description":"Filesystem evidence case container","fir_number":null,"status":"ACTIVE","investigator_id":null,"metadata":{"storage":"filesystem"},"created_at":"2026-11-21T10:00:00Z","updated_at":"2026-11-21T10:00:00Z"}"#;
    let server = list_api(&format!("[{folder}]"), fixtures::DEGRADED);
    let r = yukti(&server.base_url, &["case", "list"]).await;
    assert_eq!(r.exit_code, 0);
    assert!(r.stdout.contains("legacy-folder"));
    assert!(r.stderr.contains("not a UUID"));
}

#[tokio::test]
async fn list_http_failures_map_to_exit_codes() {
    let server = MockServer::start(vec![Route::json(
        "/api/v1/cases",
        500,
        r#"{"error":{"code":"INTERNAL_SERVER_ERROR","message":"An internal server error occurred while processing the request."}}"#,
    )]);
    let r = yukti(&server.base_url, &["case", "list"]).await;
    assert_eq!(r.exit_code, 5);
    assert!(r.stdout.is_empty());
    assert!(r.stderr.contains("HTTP 500 (INTERNAL_SERVER_ERROR)"));

    let r = yukti(&server.base_url, &["case", "list", "--json"]).await;
    assert_eq!(r.exit_code, 5);
    let v = parse(&r);
    assert_eq!(v["schema"], "yukti.cli.error/v1");
    assert_eq!(v["error"]["kind"], "http_server_error");
    assert_eq!(v["error"]["http_status"], 500);

    let r = yukti(&unused_base_url(), &["case", "list"]).await;
    assert_eq!(r.exit_code, 3);
}

#[tokio::test]
async fn malformed_list_responses_are_exit_six() {
    for body in [r#"{"cases":[]}"#, r#"[{"id":1}]"#, "not json"] {
        let server = list_api(body, fixtures::READY);
        let r = yukti(&server.base_url, &["case", "list"]).await;
        assert_eq!(r.exit_code, 6, "{body}");
    }
}

/* ----------------------------------------------------------------------------- case create */

fn create_api(status: u16, body: &str) -> MockServer {
    MockServer::start(vec![
        Route::json("/api/v1/cases", status, body).method("POST")
    ])
}

#[tokio::test]
async fn create_sends_only_the_title_and_prints_the_returned_uuid() {
    let server = create_api(201, fixtures::CASE_B);
    let r = yukti(
        &server.base_url,
        &["case", "create", "--title", "ATM skimming"],
    )
    .await;
    assert_eq!(r.exit_code, 0, "{}{}", r.stdout, r.stderr);
    assert!(r
        .stdout
        .starts_with(&format!("Created case {}\n", fixtures::CASE_B_ID)));
    assert!(r.stdout.contains("Description   (none)"));

    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].path, "/api/v1/cases");
    assert_eq!(
        requests[0].content_type.as_deref(),
        Some("application/json")
    );
    let sent: Value = serde_json::from_str(&requests[0].body).unwrap();
    assert_eq!(sent, json!({"title": "ATM skimming"}));
}

#[tokio::test]
async fn create_serializes_every_supplied_optional_field() {
    let server = create_api(201, fixtures::CASE_A);
    let r = yukti(
        &server.base_url,
        &[
            "case",
            "create",
            "--title",
            "Warehouse burglary",
            "--description",
            "Night-time break-in",
            "--fir-number",
            "FIR-112/2026",
            "--investigator-id",
            "INV-7",
            "--metadata",
            r#"{"station":"Saket","priority":2}"#,
            "--case-id",
            "3F2B8C1E-7A4D-4E1B-9C3A-5D6E7F8A9B0C",
        ],
    )
    .await;
    assert_eq!(r.exit_code, 0, "{}{}", r.stdout, r.stderr);
    assert!(r.stderr.is_empty(), "{}", r.stderr);
    let sent: Value = serde_json::from_str(&server.requests()[0].body).unwrap();
    assert_eq!(
        sent,
        json!({
            "title": "Warehouse burglary",
            "description": "Night-time break-in",
            "fir_number": "FIR-112/2026",
            "investigator_id": "INV-7",
            "metadata": {"station": "Saket", "priority": 2},
            "case_id": "3f2b8c1e-7a4d-4e1b-9c3a-5d6e7f8a9b0c"
        })
    );
}

#[tokio::test]
async fn create_json_output_passes_the_case_through() {
    let server = create_api(201, fixtures::CASE_B);
    let r = yukti(
        &server.base_url,
        &["case", "create", "--title", "ATM skimming", "--json"],
    )
    .await;
    assert_eq!(r.exit_code, 0);
    let v = parse(&r);
    assert_eq!(v["schema"], "yukti.cli.case/v1");
    assert_eq!(v["action"], "create");
    let expected: Value = serde_json::from_str(fixtures::CASE_B).unwrap();
    assert_eq!(v["case"], expected);
    assert_eq!(v["warnings"], json!([]));
}

#[tokio::test]
async fn invalid_create_input_never_reaches_the_api() {
    let server = create_api(201, fixtures::CASE_B);
    for args in [
        vec!["case", "create", "--title", "   "],
        vec!["case", "create", "--title", "T", "--metadata", "[1,2]"],
        vec!["case", "create", "--title", "T", "--metadata", "{bad json"],
        vec!["case", "create", "--title", "T", "--case-id", "CASE-026"],
        vec!["case", "create", "--title", "T", "--fir-number", ""],
    ] {
        let r = yukti(&server.base_url, &args).await;
        assert_eq!(r.exit_code, 2, "{args:?}");
        assert!(
            r.stderr.starts_with("error: invalid argument:"),
            "{args:?}: {}",
            r.stderr
        );
    }
    let v = parse(
        &yukti(
            &server.base_url,
            &["--json", "case", "create", "--title", ""],
        )
        .await,
    );
    assert_eq!(v["error"]["kind"], "invalid_argument");
    assert!(
        server.requests().is_empty(),
        "validation happens before any request"
    );
}

#[tokio::test]
async fn create_rejections_from_the_api_are_reported() {
    let server = create_api(
        400,
        r#"{"error":{"code":"BAD_REQUEST","message":"Field 'title' cannot be empty"}}"#,
    );
    let r = yukti(&server.base_url, &["case", "create", "--title", "T"]).await;
    assert_eq!(r.exit_code, 4);
    assert!(r
        .stderr
        .contains("HTTP 400 (BAD_REQUEST): Field 'title' cannot be empty"));
}

#[tokio::test]
async fn create_warns_when_the_api_ignores_the_requested_id() {
    let server = create_api(201, fixtures::CASE_B);
    let r = yukti(
        &server.base_url,
        &[
            "case",
            "create",
            "--title",
            "T",
            "--case-id",
            fixtures::CASE_A_ID,
        ],
    )
    .await;
    assert_eq!(r.exit_code, 0);
    assert!(r
        .stdout
        .contains(&format!("Created case {}", fixtures::CASE_B_ID)));
    assert!(r.stderr.contains("instead of the requested"));
}

/* ------------------------------------------------------------------------------- case show */

const CASE_A_PATH: &str = "/api/v1/cases/3f2b8c1e-7a4d-4e1b-9c3a-5d6e7f8a9b0c";

#[tokio::test]
async fn show_displays_the_returned_fields() {
    let server = MockServer::start(vec![
        Route::json(CASE_A_PATH, 200, fixtures::CASE_A).method("GET")
    ]);
    let r = yukti(
        &server.base_url,
        &["case", "show", "--case", fixtures::CASE_A_ID],
    )
    .await;
    assert_eq!(r.exit_code, 0, "{}{}", r.stdout, r.stderr);
    for expected in [
        "Case ID       3f2b8c1e-7a4d-4e1b-9c3a-5d6e7f8a9b0c",
        "Title         Warehouse burglary",
        "Status        ACTIVE",
        "Description   Night-time break-in",
        "FIR number    FIR-112/2026",
        "Investigator  INV-7",
        "Created       2026-11-21T09:15:00Z",
        r#"Metadata      {"station":"Saket"}"#,
    ] {
        assert!(
            r.stdout.contains(expected),
            "missing {expected:?} in\n{}",
            r.stdout
        );
    }

    let v = parse(
        &yukti(
            &server.base_url,
            &["case", "show", "--case", fixtures::CASE_A_ID, "--json"],
        )
        .await,
    );
    assert_eq!(v["action"], "show");
    let expected: Value = serde_json::from_str(fixtures::CASE_A).unwrap();
    assert_eq!(v["case"], expected);
}

#[tokio::test]
async fn show_uses_the_canonical_lowercase_uuid_in_the_path() {
    let server = MockServer::start(vec![Route::json(CASE_A_PATH, 200, fixtures::CASE_A)]);
    let r = yukti(
        &server.base_url,
        &[
            "case",
            "show",
            "--case",
            "3F2B8C1E-7A4D-4E1B-9C3A-5D6E7F8A9B0C",
        ],
    )
    .await;
    assert_eq!(r.exit_code, 0);
    assert_eq!(server.requests()[0].path, CASE_A_PATH);
}

#[tokio::test]
async fn show_404_is_exit_four() {
    let server = MockServer::start(vec![Route::json(CASE_A_PATH, 404, fixtures::NOT_FOUND)]);
    let r = yukti(
        &server.base_url,
        &["case", "show", "--case", fixtures::CASE_A_ID],
    )
    .await;
    assert_eq!(r.exit_code, 4);
    assert!(r.stdout.is_empty());
    assert!(r.stderr.contains("HTTP 404 (NOT_FOUND): Case not found"));

    let v = parse(
        &yukti(
            &server.base_url,
            &["case", "show", "--case", fixtures::CASE_A_ID, "--json"],
        )
        .await,
    );
    assert_eq!(v["exit_code"], 4);
    assert_eq!(v["error"]["api_code"], "NOT_FOUND");
}

#[tokio::test]
async fn show_rejects_names_and_malformed_ids_locally() {
    let server = MockServer::start(vec![]);
    for bad in [
        "CASE-026",
        "26",
        "3f2b8c1e7a4d4e1b9c3a5d6e7f8a9b0c",
        "../etc/passwd",
    ] {
        let r = yukti(&server.base_url, &["case", "show", "--case", bad]).await;
        assert_eq!(r.exit_code, 2, "{bad}");
        assert!(r.stderr.contains("--case must be a case UUID"), "{bad}");
    }
    assert!(
        server.requests().is_empty(),
        "no request is ever sent for a malformed ID"
    );
}

#[tokio::test]
async fn show_flags_placeholder_records() {
    let path = "/api/v1/cases/9d8c7b6a-5f4e-4d3c-8b2a-1f0e9d8c7b6a";
    let server = MockServer::start(vec![Route::json(path, 200, fixtures::FS_PLACEHOLDER)]);
    let r = yukti(
        &server.base_url,
        &[
            "case",
            "show",
            "--case",
            "9d8c7b6a-5f4e-4d3c-8b2a-1f0e9d8c7b6a",
        ],
    )
    .await;
    assert_eq!(r.exit_code, 0);
    assert!(r.stderr.contains("metadata.storage = \"filesystem\""));
}
