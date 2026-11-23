//! `yukti doctor` against mock APIs, through the library entry point.

mod support;

use clap::Parser;
use serde_json::Value;
use support::{fixtures, unused_base_url, MockServer, Route};
use yukti_cli::cli::Cli;
use yukti_cli::config::EnvVars;
use yukti_cli::output::Rendered;

async fn doctor_with_env(args: &[&str], env: &EnvVars) -> Rendered {
    let mut argv = vec!["yukti"];
    argv.extend_from_slice(args);
    argv.push("doctor");
    let cli = Cli::try_parse_from(argv).expect("valid arguments");
    yukti_cli::execute(&cli, env).await
}

async fn doctor(base_url: &str, extra: &[&str]) -> Rendered {
    let mut args = vec!["--api-url", base_url];
    args.extend_from_slice(extra);
    doctor_with_env(&args, &EnvVars::default()).await
}

async fn doctor_json(base_url: &str) -> (Value, u8) {
    let r = doctor(base_url, &["--json"]).await;
    assert!(
        r.stderr.is_empty(),
        "JSON mode writes nothing to stderr: {}",
        r.stderr
    );
    let v: Value = serde_json::from_str(&r.stdout).expect("stdout is one JSON document");
    assert_eq!(v["schema"], "yukti.cli.doctor/v1");
    assert_eq!(v["exit_code"], r.exit_code);
    (v, r.exit_code)
}

fn check<'a>(v: &'a Value, id: &str) -> &'a Value {
    v["checks"]
        .as_array()
        .expect("checks array")
        .iter()
        .find(|c| c["id"] == id)
        .unwrap_or_else(|| panic!("check {id} missing: {v}"))
}

fn api(ready_status: u16, ready_body: &str, cases: Route) -> MockServer {
    MockServer::start(vec![
        Route::json("/health", 200, fixtures::HEALTH_OK),
        Route::json("/health/ready", ready_status, ready_body),
        cases,
    ])
}

fn cases_ok() -> Route {
    Route::json(
        "/api/v1/cases",
        200,
        &format!("[{},{}]", fixtures::CASE_A, fixtures::CASE_B),
    )
}

#[tokio::test]
async fn fully_ready_api_passes_every_check() {
    let server = api(200, fixtures::READY, cases_ok());
    let r = doctor(&server.base_url, &[]).await;
    assert_eq!(r.exit_code, 0, "{}", r.stdout);
    assert!(r.stderr.is_empty());
    assert!(r.stdout.contains("[PASS] CLI"));
    assert!(r.stdout.contains("[PASS] Config      API URL"));
    assert!(r.stdout.contains("(from --api-url)"));
    assert!(r.stdout.contains("[PASS] API         reachable at"));
    assert!(r
        .stdout
        .contains("[PASS] Readiness   ready (GET /health/ready -> HTTP 200)"));
    assert!(r.stdout.contains("[PASS] Database    connected"));
    assert!(r.stdout.contains("[PASS] Graph       connected"));
    assert!(r
        .stdout
        .contains("[PASS] Cases API   GET /api/v1/cases -> 2 case(s)"));
    assert!(r
        .stdout
        .contains("7 passed, 0 warning(s), 0 failed, 0 skipped"));
    assert!(r.stdout.contains("Result  OK: yukti is set up"));
    assert!(!r.stdout.contains("[WARN]"));
}

#[tokio::test]
async fn degraded_api_warns_but_exits_zero() {
    let server = api(200, fixtures::DEGRADED, cases_ok());
    let r = doctor(&server.base_url, &[]).await;
    assert_eq!(r.exit_code, 0, "{}", r.stdout);
    assert!(r.stdout.contains("[WARN] Readiness   degraded"));
    assert!(r
        .stdout
        .contains("[WARN] Database    not configured (filesystem mode, no PostgreSQL)"));
    assert!(r
        .stdout
        .contains("[WARN] Graph       disabled (Neo4j not configured)"));
    assert!(r.stdout.contains("hint: Fine for local use"));
    assert!(r
        .stdout
        .contains("4 passed, 3 warning(s), 0 failed, 0 skipped"));
    assert!(r.stdout.contains("Result  OK with warnings"));
}

#[tokio::test]
async fn doctor_only_sends_read_only_requests() {
    let server = api(200, fixtures::DEGRADED, cases_ok());
    let _ = doctor(&server.base_url, &[]).await;
    let requests = server.requests();
    assert!(!requests.is_empty());
    for req in &requests {
        assert_eq!(
            req.method, "GET",
            "doctor must not change anything: {req:?}"
        );
    }
    let paths: Vec<&str> = requests.iter().map(|r| r.path.as_str()).collect();
    assert_eq!(paths, ["/health", "/health/ready", "/api/v1/cases"]);
}

#[tokio::test]
async fn unreachable_api_fails_with_exit_three_and_skips_the_rest() {
    let url = unused_base_url();
    let r = doctor(&url, &[]).await;
    assert_eq!(r.exit_code, 3, "{}", r.stdout);
    assert!(r
        .stdout
        .contains("[FAIL] API         GET /health -> no response"));
    assert!(r
        .stdout
        .contains("hint: Start the API in another terminal: cd backend-rust; cargo run --bin sutram-backend-rust"));
    for label in ["Readiness", "Database", "Graph", "Cases API"] {
        assert!(
            r.stdout
                .contains(&format!("[SKIP] {label:<11} the API is not usable")),
            "{label} skipped: {}",
            r.stdout
        );
    }
    assert!(r
        .stdout
        .contains("2 passed, 0 warning(s), 1 failed, 4 skipped"));
    assert!(r.stdout.contains("Result  FAILED"));
}

#[tokio::test]
async fn not_ready_api_fails_with_exit_five_and_shows_the_api_error() {
    let server = api(503, fixtures::NOT_READY, cases_ok());
    let r = doctor(&server.base_url, &[]).await;
    assert_eq!(r.exit_code, 5, "{}", r.stdout);
    assert!(r.stdout.contains(
        "[FAIL] Readiness   NOT READY (GET /health/ready -> HTTP 503): Database error: pool timed out"
    ));
    assert!(r.stdout.contains("[FAIL] Database    DISCONNECTED"));
    assert!(r.stdout.contains("[WARN] Graph       disabled"));
    assert!(r.stdout.contains("[PASS] Cases API"));
}

#[tokio::test]
async fn failing_case_api_fails_with_its_exit_code() {
    let server = api(
        200,
        fixtures::DEGRADED,
        Route::json(
            "/api/v1/cases",
            500,
            r#"{"error":{"code":"INTERNAL","message":"boom"}}"#,
        ),
    );
    let r = doctor(&server.base_url, &[]).await;
    assert_eq!(r.exit_code, 5, "{}", r.stdout);
    assert!(r.stdout.contains("[FAIL] Cases API   GET /api/v1/cases ->"));
    assert!(r.stdout.contains("Result  FAILED"));
}

#[tokio::test]
async fn non_yukti_server_fails_the_api_check() {
    let server = MockServer::start(vec![Route::text(
        "/health",
        200,
        "text/html",
        "<html>hello</html>",
    )]);
    let r = doctor(&server.base_url, &[]).await;
    assert_eq!(r.exit_code, 6, "{}", r.stdout);
    assert!(r.stdout.contains("[FAIL] API"));
    assert!(r.stdout.contains("but not like the YUKTI Rust API"));
}

#[tokio::test]
async fn invalid_configuration_is_a_failed_check_with_exit_two() {
    let env = EnvVars {
        api_url: Some("ftp://example.com".into()),
        timeout_secs: None,
        ..EnvVars::default()
    };
    let r = doctor_with_env(&[], &env).await;
    assert_eq!(r.exit_code, 2, "{}", r.stdout);
    assert!(r.stderr.is_empty());
    assert!(r.stdout.contains("[PASS] CLI"));
    assert!(r.stdout.contains("[FAIL] Config"));
    assert!(r.stdout.contains("YUKTI_API_URL"));
    assert!(r
        .stdout
        .contains("[SKIP] API         configuration is invalid"));
}

#[tokio::test]
async fn environment_url_is_reported_as_the_source() {
    let server = api(200, fixtures::READY, cases_ok());
    let env = EnvVars {
        api_url: Some(server.base_url.clone()),
        timeout_secs: Some("7".into()),
        ..EnvVars::default()
    };
    let r = doctor_with_env(&[], &env).await;
    assert_eq!(r.exit_code, 0, "{}", r.stdout);
    assert!(r
        .stdout
        .contains("(from YUKTI_API_URL), timeout 7 s (from YUKTI_TIMEOUT_SECS)"));
}

#[tokio::test]
async fn json_document_lists_every_check() {
    let server = api(200, fixtures::DEGRADED, cases_ok());
    let (v, code) = doctor_json(&server.base_url).await;
    assert_eq!(code, 0);
    assert_eq!(v["result"], "ok_with_warnings");
    assert_eq!(v["api_url"], server.base_url);
    assert_eq!(v["cli_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(
        (v["passed"].as_u64(), v["warnings"].as_u64()),
        (Some(4), Some(3))
    );
    let ids: Vec<&str> = v["checks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        [
            "cli",
            "config",
            "api",
            "readiness",
            "database",
            "graph",
            "cases_api"
        ]
    );
    assert_eq!(check(&v, "database")["status"], "warn");
    assert!(check(&v, "database")["hint"].is_string());
    assert_eq!(check(&v, "cases_api")["status"], "pass");
}

#[tokio::test]
async fn json_unreachable_reports_the_error() {
    let (v, code) = doctor_json(&unused_base_url()).await;
    assert_eq!(code, 3);
    assert_eq!(v["result"], "failed");
    assert_eq!(check(&v, "api")["status"], "fail");
    assert_eq!(check(&v, "api")["error"]["kind"], "unreachable");
    assert_eq!(check(&v, "graph")["status"], "skipped");
}
