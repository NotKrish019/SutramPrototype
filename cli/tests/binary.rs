//! End-to-end tests of the compiled `yukti` binary: argument parsing, configuration
//! precedence through real environment variables, exit codes and output streams.

mod support;

use std::process::{Command, Output};

use serde_json::Value;
use support::{unused_base_url, MockServer};

fn yukti(args: &[&str], env: &[(&str, &str)]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_yukti"));
    cmd.args(args)
        .env_remove("YUKTI_API_URL")
        .env_remove("YUKTI_TIMEOUT_SECS");
    for (key, value) in env {
        cmd.env(key, value);
    }
    cmd.output().expect("run yukti")
}

fn code(out: &Output) -> i32 {
    out.status.code().expect("exit code")
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn help_and_version_exit_zero() {
    let help = yukti(&["--help"], &[]);
    assert_eq!(code(&help), 0);
    let text = stdout(&help);
    assert!(text.contains("status"));
    assert!(text.contains("--api-url"));
    assert!(text.contains("--json"));

    let version = yukti(&["--version"], &[]);
    assert_eq!(code(&version), 0);
    assert!(stdout(&version).starts_with("yukti "));
}

#[test]
fn usage_errors_exit_two() {
    assert_eq!(code(&yukti(&[], &[])), 2);
    assert_eq!(code(&yukti(&["frobnicate"], &[])), 2);
    assert_eq!(code(&yukti(&["status", "--timeout", "soon"], &[])), 2);
}

#[test]
fn invalid_api_url_exits_two_with_a_useful_message() {
    let out = yukti(&["--api-url", "localhost:8080", "status"], &[]);
    assert_eq!(code(&out), 2);
    assert!(stdout(&out).is_empty());
    let err = stderr(&out);
    assert!(err.contains("invalid API URL from --api-url"), "{err}");
    assert!(err.contains("http://localhost:8080"), "{err}");
}

#[test]
fn invalid_env_values_name_the_variable() {
    let out = yukti(&["status"], &[("YUKTI_API_URL", "ftp://h")]);
    assert_eq!(code(&out), 2);
    assert!(stderr(&out).contains("YUKTI_API_URL"));

    let out = yukti(&["status"], &[("YUKTI_TIMEOUT_SECS", "0")]);
    assert_eq!(code(&out), 2);
    assert!(stderr(&out).contains("YUKTI_TIMEOUT_SECS"));
}

#[test]
fn env_url_is_used_when_no_flag_is_given() {
    let server = MockServer::degraded_api();
    let out = yukti(&["status"], &[("YUKTI_API_URL", &server.base_url)]);
    assert_eq!(code(&out), 0, "{}", stdout(&out));
    assert!(stdout(&out).contains(&server.base_url));
}

#[test]
fn flag_url_overrides_env_url() {
    let server = MockServer::degraded_api();
    let out = yukti(
        &["--api-url", &server.base_url, "status"],
        &[("YUKTI_API_URL", "not a url at all")],
    );
    assert_eq!(code(&out), 0, "{}{}", stdout(&out), stderr(&out));
    assert!(stdout(&out).contains(&server.base_url));
}

#[test]
fn unreachable_api_never_exits_zero() {
    let url = unused_base_url();
    let out = yukti(&["--api-url", &url, "status"], &[]);
    assert_eq!(code(&out), 3);
    assert!(stdout(&out).contains("UNREACHABLE"));
}

#[test]
fn json_mode_emits_one_document_for_success_and_failure() {
    let server = MockServer::degraded_api();
    let ok = yukti(&["--api-url", &server.base_url, "status", "--json"], &[]);
    assert_eq!(code(&ok), 0);
    let v: Value = serde_json::from_slice(&ok.stdout).expect("JSON");
    assert_eq!(v["schema"], "yukti.cli.status/v1");
    assert_eq!(v["result"], "degraded");

    let url = unused_base_url();
    let down = yukti(&["--json", "--api-url", &url, "status"], &[]);
    assert_eq!(code(&down), 3);
    let v: Value = serde_json::from_slice(&down.stdout).expect("JSON");
    assert_eq!(v["exit_code"], 3);

    let bad_config = yukti(&["--json", "--api-url", "nope", "status"], &[]);
    assert_eq!(code(&bad_config), 2);
    assert!(stderr(&bad_config).is_empty());
    let v: Value = serde_json::from_slice(&bad_config.stdout).expect("JSON");
    assert_eq!(v["schema"], "yukti.cli.error/v1");
    assert_eq!(v["error"]["kind"], "config");
}

#[test]
fn case_usage_errors_exit_two() {
    let out = yukti(&["case", "create"], &[]);
    assert_eq!(code(&out), 2);
    assert!(stderr(&out).contains("--title"));

    let out = yukti(&["case", "show", "--case", "CASE-026"], &[]);
    assert_eq!(code(&out), 2);
    assert!(stderr(&out).contains("--case must be a case UUID"));
    assert!(stdout(&out).is_empty());
}

#[test]
fn case_commands_use_env_configuration_and_real_exit_codes() {
    let server = MockServer::start(vec![
        support::Route::json("/api/v1/cases", 200, "[]"),
        support::Route::json(
            "/api/v1/cases/3f2b8c1e-7a4d-4e1b-9c3a-5d6e7f8a9b0c",
            404,
            support::fixtures::NOT_FOUND,
        ),
        support::Route::json("/health/ready", 200, support::fixtures::DEGRADED),
    ]);
    let env = [("YUKTI_API_URL", server.base_url.as_str())];

    let list = yukti(&["case", "list"], &env);
    assert_eq!(code(&list), 0);
    assert_eq!(stdout(&list), "No cases returned by the API.\n");
    assert!(stderr(&list).contains("warning: The API is running without PostgreSQL"));

    let missing = yukti(
        &[
            "case",
            "show",
            "--case",
            "3f2b8c1e-7a4d-4e1b-9c3a-5d6e7f8a9b0c",
        ],
        &env,
    );
    assert_eq!(code(&missing), 4);

    let down = yukti(&["--api-url", &unused_base_url(), "case", "list"], &env);
    assert_eq!(code(&down), 3, "flag overrides env for case commands too");
}

#[test]
fn evidence_usage_errors_exit_two_without_contacting_the_api() {
    let server = MockServer::start(vec![]);
    let api = ["--api-url", server.base_url.as_str()];

    let mut args = api.to_vec();
    args.extend([
        "evidence",
        "ingest",
        "--case",
        "3f2b8c1e-7a4d-4e1b-9c3a-5d6e7f8a9b0c",
    ]);
    let out = yukti(&args, &[]);
    assert_eq!(code(&out), 2, "the FILE argument is required");
    assert!(stderr(&out).contains("<FILE>"));

    let mut args = api.to_vec();
    args.extend([
        "evidence",
        "ingest",
        "--case",
        "3f2b8c1e-7a4d-4e1b-9c3a-5d6e7f8a9b0c",
        "no-such-file.csv",
    ]);
    let out = yukti(&args, &[]);
    assert_eq!(code(&out), 2);
    assert!(stderr(&out).contains("no-such-file.csv does not exist"));

    let mut args = api.to_vec();
    args.extend(["evidence", "list"]);
    assert_eq!(code(&yukti(&args, &[])), 2, "--case is required");

    assert!(server.requests().is_empty());
}

#[test]
fn negative_clock_offsets_parse_as_values_not_flags() {
    // A missing file is reported, which proves "-30" was accepted as the option's value.
    let out = yukti(
        &[
            "evidence",
            "ingest",
            "--case",
            "3f2b8c1e-7a4d-4e1b-9c3a-5d6e7f8a9b0c",
            "missing.csv",
            "--clock-offset-seconds",
            "-30",
        ],
        &[],
    );
    assert_eq!(code(&out), 2);
    assert!(
        stderr(&out).contains("missing.csv does not exist"),
        "{}",
        stderr(&out)
    );
}
