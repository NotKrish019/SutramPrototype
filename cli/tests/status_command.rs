//! `yukti status` against a mock API, through the library entry point.

mod support;

use clap::Parser;
use serde_json::Value;
use support::{fixtures, flat, unused_base_url, MockServer, Route};
use yukti_cli::cli::Cli;
use yukti_cli::config::EnvVars;
use yukti_cli::output::Rendered;

async fn status(base_url: &str, extra: &[&str]) -> Rendered {
    let mut args = vec!["yukti", "--api-url", base_url];
    args.extend_from_slice(extra);
    args.push("status");
    let cli = Cli::try_parse_from(args).expect("valid arguments");
    yukti_cli::execute(&cli, &EnvVars::default()).await
}

async fn status_json(base_url: &str) -> (Value, u8) {
    let r = status(base_url, &["--json"]).await;
    assert!(
        r.stderr.is_empty(),
        "JSON mode writes nothing to stderr: {}",
        r.stderr
    );
    let v: Value = serde_json::from_str(&r.stdout).expect("stdout is one JSON document");
    assert_eq!(v["schema"], "yukti.cli.status/v1");
    assert_eq!(v["exit_code"], r.exit_code);
    (v, r.exit_code)
}

fn api(health: Route, ready: Route) -> MockServer {
    MockServer::start(vec![health, ready])
}

#[tokio::test]
async fn ready_api_exits_zero_and_reports_connected_dependencies() {
    let server = api(
        Route::json("/health", 200, fixtures::HEALTH_OK),
        Route::json("/health/ready", 200, fixtures::READY),
    );
    let r = status(&server.base_url, &[]).await;
    assert_eq!(r.exit_code, 0, "{}", r.stdout);
    assert!(r
        .stdout
        .contains("✓ REACHABLE  GET /health → HTTP 200  service sutram-backend-rust"));
    assert!(r
        .stdout
        .contains("Readiness         ✓ READY  GET /health/ready → HTTP 200"));
    assert!(r.stdout.contains("  Database          ✓ connected"));
    assert!(r.stdout.contains("  Graph             ✓ connected"));
    assert!(r.stdout.contains("\n  exit 0\n"));
}

#[tokio::test]
async fn degraded_api_exits_zero_and_never_claims_a_connection() {
    let server = MockServer::degraded_api();
    let r = status(&server.base_url, &[]).await;
    assert_eq!(r.exit_code, 0);
    assert!(r.stdout.contains("DEGRADED"));
    assert!(r
        .stdout
        .contains("not configured (filesystem mode, no PostgreSQL)"));
    assert!(r.stdout.contains("disabled (Neo4j not configured)"));
    assert!(!r.stdout.contains("Database          ✓ connected"));
    assert!(!r.stdout.contains("Graph             ✓ connected"));

    let (v, code) = status_json(&server.base_url).await;
    assert_eq!(code, 0);
    assert_eq!(v["result"], "degraded");
    assert_eq!(v["api_reachable"], true);
    assert_eq!(
        v["dependencies"]["database"],
        "unconfigured_filesystem_mode"
    );
    assert_eq!(v["dependencies"]["graph"], "disabled");
    // Server data is preserved exactly.
    let expected: Value = serde_json::from_str(fixtures::DEGRADED).unwrap();
    assert_eq!(v["readiness"]["body"], expected);
    assert_eq!(v["readiness"]["http_status"], 200);
    assert_eq!(v["health"]["body"]["service"], "sutram-backend-rust");
}

#[tokio::test]
async fn not_ready_is_exit_five_and_shows_the_api_error() {
    let server = api(
        Route::json("/health", 200, fixtures::HEALTH_OK),
        Route::json("/health/ready", 503, fixtures::NOT_READY),
    );
    let r = status(&server.base_url, &[]).await;
    assert_eq!(r.exit_code, 5);
    assert!(r
        .stdout
        .contains("✕ NOT READY  GET /health/ready → HTTP 503"));
    assert!(r.stdout.contains("DISCONNECTED"));
    assert!(r.stdout.contains("Database error: pool timed out"));
    assert!(r
        .stdout
        .contains("API is up, but a configured dependency is not ready."));

    let (v, code) = status_json(&server.base_url).await;
    assert_eq!(code, 5);
    assert_eq!(v["result"], "not_ready");
    assert_eq!(v["api_reachable"], true);
    assert_eq!(v["dependencies"]["error"], "Database error: pool timed out");
}

#[tokio::test]
async fn a_5xx_readiness_is_never_ready_even_if_the_body_says_so() {
    let server = api(
        Route::json("/health", 200, fixtures::HEALTH_OK),
        Route::json("/health/ready", 503, fixtures::READY),
    );
    let (v, code) = status_json(&server.base_url).await;
    assert_eq!(code, 5);
    assert_eq!(v["result"], "not_ready");
}

#[tokio::test]
async fn unreachable_api_is_exit_three() {
    let url = unused_base_url();
    let r = status(&url, &[]).await;
    assert_eq!(r.exit_code, 3);
    assert!(r
        .stdout
        .contains("✕ UNREACHABLE  GET /health → no response"));
    assert!(r.stdout.contains("Readiness         not checked"));
    assert!(flat(&r.stdout).contains("cannot reach the YUKTI API"));

    let (v, code) = status_json(&url).await;
    assert_eq!(code, 3);
    assert_eq!(v["result"], "unreachable");
    assert_eq!(v["api_reachable"], false);
    assert_eq!(v["health"]["http_status"], Value::Null);
    assert_eq!(v["health"]["error"]["kind"], "unreachable");
    assert_eq!(v["readiness"], Value::Null);
    assert_eq!(v["dependencies"], Value::Null);
}

#[tokio::test]
async fn timeout_is_exit_three_with_timeout_kind() {
    let server = MockServer::hanging();
    let r = status(&server.base_url, &["--json", "--timeout", "1"]).await;
    assert_eq!(r.exit_code, 3);
    let v: Value = serde_json::from_str(&r.stdout).unwrap();
    assert_eq!(v["result"], "unreachable");
    assert_eq!(v["health"]["error"]["kind"], "timeout");
}

#[tokio::test]
async fn health_http_errors_map_to_exit_four_and_five() {
    let not_found = MockServer::start(vec![]);
    let r = status(&not_found.base_url, &[]).await;
    assert_eq!(r.exit_code, 4);
    assert!(r
        .stdout
        .contains("✕ HEALTH CHECK FAILED  GET /health → HTTP 404"));
    assert!(flat(&r.stdout).contains("HTTP 404 (NOT_FOUND): no such route in mock"));

    let broken = api(
        Route::json(
            "/health",
            500,
            r#"{"error":{"code":"INTERNAL_SERVER_ERROR","message":"An internal server error occurred while processing the request."}}"#,
        ),
        Route::json("/health/ready", 200, fixtures::READY),
    );
    let (v, code) = status_json(&broken.base_url).await;
    assert_eq!(code, 5);
    assert_eq!(v["result"], "health_failed");
    assert_eq!(v["api_reachable"], true);
    assert_eq!(v["health"]["error"]["api_code"], "INTERNAL_SERVER_ERROR");
    assert_eq!(
        v["readiness"],
        Value::Null,
        "readiness is not probed after a failed health check"
    );
}

#[tokio::test]
async fn html_from_the_web_app_is_exit_six_with_a_hint() {
    let web_app = MockServer::start(vec![Route::text(
        "/health",
        200,
        "text/html",
        "<!doctype html><html><body>YUKTI</body></html>",
    )]);
    let r = status(&web_app.base_url, &[]).await;
    assert_eq!(r.exit_code, 6);
    assert!(flat(&r.stdout).contains("not valid JSON"));
    assert!(flat(&r.stdout).contains("pointing at the web app"));
}

#[tokio::test]
async fn wrong_health_shape_or_status_is_exit_six() {
    let wrong_shape = api(
        Route::json("/health", 200, r#"{"alive":true}"#),
        Route::json("/health/ready", 200, fixtures::READY),
    );
    let (v, code) = status_json(&wrong_shape.base_url).await;
    assert_eq!(code, 6);
    assert_eq!(v["result"], "unexpected_response");

    let wrong_status = api(
        Route::json("/health", 200, r#"{"status":"starting","service":"x"}"#),
        Route::json("/health/ready", 200, fixtures::READY),
    );
    let (v, code) = status_json(&wrong_status.base_url).await;
    assert_eq!(code, 6);
    assert!(v["health"]["error"]["message"]
        .as_str()
        .unwrap()
        .contains("expected \"ok\""));
}

#[tokio::test]
async fn readiness_problems_are_distinguished() {
    // Endpoint missing: client error.
    let missing = MockServer::start(vec![Route::json("/health", 200, fixtures::HEALTH_OK)]);
    let (v, code) = status_json(&missing.base_url).await;
    assert_eq!(code, 4);
    assert_eq!(v["result"], "readiness_unavailable");
    assert_eq!(v["readiness"]["http_status"], 404);

    // Invalid JSON with 200: unexpected response.
    let garbage = api(
        Route::json("/health", 200, fixtures::HEALTH_OK),
        Route::text("/health/ready", 200, "application/json", "{not json"),
    );
    let (v, code) = status_json(&garbage.base_url).await;
    assert_eq!(code, 6);
    assert_eq!(v["result"], "unexpected_response");
    assert_eq!(v["readiness"]["body"], Value::Null);

    // Unknown status value: unexpected response, never assumed ready.
    let unknown = api(
        Route::json("/health", 200, fixtures::HEALTH_OK),
        Route::json(
            "/health/ready",
            200,
            r#"{"status":"warming_up","service":"s","database":"connected","graph":"connected"}"#,
        ),
    );
    let r = status(&unknown.base_url, &[]).await;
    assert_eq!(r.exit_code, 6);
    assert!(r.stdout.contains("UNKNOWN"));
    assert!(flat(&r.stdout).contains("unknown readiness status \"warming_up\""));

    // Plain-text 502 from a proxy: server error.
    let proxy = api(
        Route::json("/health", 200, fixtures::HEALTH_OK),
        Route::text("/health/ready", 502, "text/plain", "upstream connect error"),
    );
    let (v, code) = status_json(&proxy.base_url).await;
    assert_eq!(code, 5);
    assert_eq!(v["result"], "readiness_unavailable");
    assert!(v["readiness"]["error"]["message"]
        .as_str()
        .unwrap()
        .contains("upstream connect error"));
}

#[tokio::test]
async fn api_url_path_prefix_is_respected() {
    let server = MockServer::start(vec![
        Route::json("/yukti/health", 200, fixtures::HEALTH_OK),
        Route::json("/yukti/health/ready", 200, fixtures::DEGRADED),
    ]);
    let url = format!("{}/yukti/", server.base_url);
    let (v, code) = status_json(&url).await;
    assert_eq!(code, 0);
    assert_eq!(v["api_url"], format!("{}/yukti", server.base_url));
    assert_eq!(
        v["health"]["url"],
        format!("{}/yukti/health", server.base_url)
    );
}
