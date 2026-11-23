//! `yukti doctor`: a setup checklist.
//!
//! Checks, in order, that:
//!
//! 1. the CLI itself runs (version),
//! 2. the configuration resolves (API URL and timeout, from flag, environment or default),
//! 3. the API answers `GET /health`,
//! 4. `GET /health/ready` reports readiness, with the database and graph states,
//! 5. the case API answers `GET /api/v1/cases`.
//!
//! Every request is read-only: doctor never creates, uploads or reconstructs anything.
//! A check that cannot run because an earlier one failed is reported as `skipped`.
//! Warnings (for example filesystem mode or a disabled graph) do not change the exit code.
//!
//! Doctor runs through [`crate::execute`] rather than [`crate::commands::dispatch`], because
//! it must report an invalid configuration as a failed check instead of stopping early.

use serde::Serialize;

use crate::client::{ApiClient, CASES_PATH, HEALTH_PATH, READINESS_PATH};
use crate::commands::status::{self, describe_database, describe_graph, StatusResult};
use crate::config::{Config, EnvVars, ValueSource, ENV_API_URL, ENV_TIMEOUT_SECS};
use crate::error::{exit, CliError, ErrorInfo};
use crate::output::{to_json, Rendered};

/// Schema identifier of the `doctor` JSON document.
pub const DOCTOR_SCHEMA: &str = "yukti.cli.doctor/v1";

/// Command that starts the API locally, shown in hints.
const START_API_HINT: &str =
    "Start the API in another terminal: cd backend-rust; cargo run --bin sutram-backend-rust";

/// Outcome of one check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    Pass,
    Warn,
    Fail,
    Skipped,
}

impl CheckStatus {
    fn tag(self) -> &'static str {
        match self {
            Self::Pass => "[PASS]",
            Self::Warn => "[WARN]",
            Self::Fail => "[FAIL]",
            Self::Skipped => "[SKIP]",
        }
    }
}

/// One line of the checklist.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Check {
    /// Stable identifier: `cli`, `config`, `api`, `readiness`, `database`, `graph`, `cases_api`.
    pub id: &'static str,
    pub label: &'static str,
    pub status: CheckStatus,
    pub detail: String,
    /// What to do about a warning or failure.
    pub hint: Option<String>,
    /// The error behind a failure, when there was one.
    pub error: Option<ErrorInfo>,
    /// Exit code this check contributes when it fails.
    #[serde(skip)]
    fail_code: u8,
}

impl Check {
    fn new(id: &'static str, label: &'static str, status: CheckStatus, detail: String) -> Self {
        Self {
            id,
            label,
            status,
            detail,
            hint: None,
            error: None,
            fail_code: exit::SUCCESS,
        }
    }

    fn pass(id: &'static str, label: &'static str, detail: String) -> Self {
        Self::new(id, label, CheckStatus::Pass, detail)
    }

    fn warn(id: &'static str, label: &'static str, detail: String, hint: &str) -> Self {
        Self::new(id, label, CheckStatus::Warn, detail).with_hint(hint)
    }

    fn fail(id: &'static str, label: &'static str, detail: String, code: u8) -> Self {
        let mut check = Self::new(id, label, CheckStatus::Fail, detail);
        check.fail_code = code;
        check
    }

    fn skipped(id: &'static str, label: &'static str, reason: &str) -> Self {
        Self::new(id, label, CheckStatus::Skipped, reason.to_string())
    }

    fn with_hint(mut self, hint: &str) -> Self {
        self.hint = Some(hint.to_string());
        self
    }

    fn with_error(mut self, err: &CliError) -> Self {
        self.error = Some(ErrorInfo::from(err));
        self
    }
}

/// Overall outcome of `yukti doctor`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DoctorResult {
    /// Every check passed.
    Ok,
    /// Nothing failed, but at least one check warned.
    OkWithWarnings,
    /// At least one check failed.
    Failed,
}

/// The `yukti.cli.doctor/v1` document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DoctorReport {
    pub schema: &'static str,
    pub cli_version: &'static str,
    /// The resolved API base URL, or `null` when the configuration is invalid.
    pub api_url: Option<String>,
    pub result: DoctorResult,
    pub exit_code: u8,
    pub passed: usize,
    pub warnings: usize,
    pub failed: usize,
    pub skipped: usize,
    pub checks: Vec<Check>,
}

/// Runs every check and renders the checklist.
pub async fn run(
    flag_api_url: Option<&str>,
    flag_timeout_secs: Option<u64>,
    env: &EnvVars,
    json: bool,
) -> Rendered {
    let report = check(flag_api_url, flag_timeout_secs, env).await;
    let text = if json {
        to_json(&report)
    } else {
        render_human(&report)
    };
    Rendered::stdout(text, report.exit_code)
}

/// Runs every check and classifies the outcome.
pub async fn check(
    flag_api_url: Option<&str>,
    flag_timeout_secs: Option<u64>,
    env: &EnvVars,
) -> DoctorReport {
    let version = env!("CARGO_PKG_VERSION");
    let mut checks = vec![Check::pass("cli", "CLI", format!("yukti {version}"))];
    let mut api_url = None;

    match Config::resolve(flag_api_url, flag_timeout_secs, env)
        .and_then(|config| ApiClient::new(&config).map(|client| (config, client)))
    {
        Ok((config, client)) => {
            api_url = Some(client.base_url().to_string());
            checks.push(Check::pass(
                "config",
                "Config",
                format!(
                    "API URL {} (from {}), timeout {} s (from {})",
                    client.base_url(),
                    source_label(config.api_url_source, ENV_API_URL, "--api-url"),
                    config.timeout.as_secs(),
                    source_label(config.timeout_source, ENV_TIMEOUT_SECS, "--timeout"),
                ),
            ));
            check_api(&client, &mut checks).await;
        }
        Err(err) => {
            checks.push(
                Check::fail("config", "Config", err.to_string(), err.exit_code())
                    .with_hint(&format!(
                        "Fix --api-url / {ENV_API_URL} or --timeout / {ENV_TIMEOUT_SECS}, or unset them to use the defaults."
                    ))
                    .with_error(&err),
            );
            for (id, label) in API_CHECKS {
                checks.push(Check::skipped(id, label, "configuration is invalid"));
            }
        }
    }

    summarize(version, api_url, checks)
}

/// The checks that need a working API client, in order.
const API_CHECKS: [(&str, &str); 5] = [
    ("api", "API"),
    ("readiness", "Readiness"),
    ("database", "Database"),
    ("graph", "Graph"),
    ("cases_api", "Cases API"),
];

async fn check_api(client: &ApiClient, checks: &mut Vec<Check>) {
    let probe = status::check(client).await;

    // 1. Liveness.
    if !probe.api_reachable || probe.service.is_none() {
        let err_message = probe
            .health
            .error
            .as_ref()
            .map(|e| e.message.clone())
            .unwrap_or_else(|| "no usable response".to_string());
        let mut fail = Check::fail(
            "api",
            "API",
            format!(
                "{} -> {err_message}",
                path_line(HEALTH_PATH, probe.health.http_status)
            ),
            probe.exit_code,
        );
        fail.error = probe.health.error.clone();
        let hint = match probe.result {
            StatusResult::Unreachable => format!(
                "{START_API_HINT}. If it runs elsewhere, pass --api-url or set {ENV_API_URL}."
            ),
            _ => format!(
                "Something answered at {}, but not like the YUKTI Rust API. Check --api-url / {ENV_API_URL}.",
                client.base_url()
            ),
        };
        checks.push(fail.with_hint(&hint));
        for (id, label) in &API_CHECKS[1..] {
            checks.push(Check::skipped(id, label, "the API is not usable"));
        }
        return;
    }
    checks.push(Check::pass(
        "api",
        "API",
        format!(
            "reachable at {} ({}, service {})",
            client.base_url(),
            path_line(HEALTH_PATH, probe.health.http_status),
            probe.service.as_deref().unwrap_or_default()
        ),
    ));

    // 2. Readiness, database and graph.
    let readiness_status = probe.readiness.as_ref().and_then(|r| r.http_status);
    let readiness_line = path_line(READINESS_PATH, readiness_status);
    match probe.result {
        StatusResult::Ready => {
            checks.push(Check::pass(
                "readiness",
                "Readiness",
                format!("ready ({readiness_line})"),
            ));
        }
        StatusResult::Degraded => checks.push(Check::warn(
            "readiness",
            "Readiness",
            format!("degraded ({readiness_line})"),
            "The API works, but an optional dependency is not configured (see below).",
        )),
        StatusResult::NotReady => {
            let detail = match probe.dependencies.as_ref().and_then(|d| d.error.as_deref()) {
                Some(api_error) => format!("NOT READY ({readiness_line}): {api_error}"),
                None => format!("NOT READY ({readiness_line})"),
            };
            let mut fail = Check::fail("readiness", "Readiness", detail, probe.exit_code)
                .with_hint("A configured dependency failed. Check that PostgreSQL / Neo4j are running and read the API's log.");
            fail.error = probe.readiness.as_ref().and_then(|r| r.error.clone());
            checks.push(fail);
        }
        _ => {
            let err = probe.readiness.as_ref().and_then(|r| r.error.clone());
            let message = err
                .as_ref()
                .map(|e| e.message.clone())
                .unwrap_or_else(|| "readiness could not be determined".to_string());
            let mut fail = Check::fail(
                "readiness",
                "Readiness",
                format!("{readiness_line} -> {message}"),
                probe.exit_code,
            )
            .with_hint("The API is up but its readiness probe misbehaved. Check the API's log.");
            fail.error = err;
            checks.push(fail);
        }
    }

    match &probe.dependencies {
        Some(deps) => {
            checks.push(database_check(&deps.database));
            checks.push(graph_check(&deps.graph));
        }
        None => {
            checks.push(Check::skipped(
                "database",
                "Database",
                "readiness was not reported",
            ));
            checks.push(Check::skipped(
                "graph",
                "Graph",
                "readiness was not reported",
            ));
        }
    }

    // 3. The case API, read-only.
    checks.push(match client.list_cases().await {
        Ok(cases) => Check::pass(
            "cases_api",
            "Cases API",
            format!("GET {CASES_PATH} -> {} case(s)", cases.len()),
        ),
        Err(err) => Check::fail(
            "cases_api",
            "Cases API",
            format!("GET {CASES_PATH} -> {err}"),
            err.exit_code(),
        )
        .with_hint("The API is up, but listing cases failed. Check the API's log.")
        .with_error(&err),
    });
}

fn database_check(value: &str) -> Check {
    let detail = describe_database(value);
    match value {
        "connected" => Check::pass("database", "Database", detail),
        "unconfigured_filesystem_mode" => Check::warn(
            "database",
            "Database",
            detail,
            "Fine for local use: cases and evidence are kept on disk. To use PostgreSQL, set the DATABASE_URL environment variable in the terminal that runs the API and restart it (see docs/development/setup-guide.md).",
        ),
        _ => Check::fail("database", "Database", detail, exit::SERVER_ERROR).with_hint(
            "PostgreSQL is configured but not usable. Check that it is running and that DATABASE_URL is correct.",
        ),
    }
}

fn graph_check(value: &str) -> Check {
    let detail = describe_graph(value);
    match value {
        "connected" => Check::pass("graph", "Graph", detail),
        "disabled" => Check::warn(
            "graph",
            "Graph",
            detail,
            "Optional: knowledge-graph projection is off. To enable it, set the GRAPH_ENABLED and NEO4J_* environment variables in the terminal that runs the API and restart it (see docs/development/setup-guide.md).",
        ),
        _ => Check::fail("graph", "Graph", detail, exit::SERVER_ERROR).with_hint(
            "Neo4j is configured but not usable. Check that it is running and that the NEO4J_* values are correct.",
        ),
    }
}

fn summarize(version: &'static str, api_url: Option<String>, checks: Vec<Check>) -> DoctorReport {
    let count = |status: CheckStatus| checks.iter().filter(|c| c.status == status).count();
    let (passed, warnings, failed, skipped) = (
        count(CheckStatus::Pass),
        count(CheckStatus::Warn),
        count(CheckStatus::Fail),
        count(CheckStatus::Skipped),
    );
    // The first failure decides the exit code, following the CLI's exit-code contract.
    let exit_code = checks
        .iter()
        .find(|c| c.status == CheckStatus::Fail)
        .map_or(exit::SUCCESS, |c| c.fail_code);
    let result = if failed > 0 {
        DoctorResult::Failed
    } else if warnings > 0 {
        DoctorResult::OkWithWarnings
    } else {
        DoctorResult::Ok
    };
    DoctorReport {
        schema: DOCTOR_SCHEMA,
        cli_version: version,
        api_url,
        result,
        exit_code,
        passed,
        warnings,
        failed,
        skipped,
        checks,
    }
}

fn source_label(source: ValueSource, env_name: &str, flag: &str) -> String {
    match source {
        ValueSource::Flag => flag.to_string(),
        ValueSource::Env => env_name.to_string(),
        ValueSource::Default => "default".to_string(),
    }
}

fn path_line(path: &str, http_status: Option<u16>) -> String {
    match http_status {
        Some(code) => format!("GET {path} -> HTTP {code}"),
        None => format!("GET {path} -> no response"),
    }
}

/// Renders the checklist as plain ASCII text.
pub fn render_human(report: &DoctorReport) -> String {
    let mut out = String::from("yukti doctor\n\n");
    for check in &report.checks {
        out.push_str(&format!(
            "  {} {:<11} {}\n",
            check.status.tag(),
            check.label,
            check.detail
        ));
        if let Some(hint) = &check.hint {
            out.push_str(&format!("{:21}hint: {hint}\n", ""));
        }
    }
    let counts = format!(
        "{} passed, {} warning(s), {} failed, {} skipped",
        report.passed, report.warnings, report.failed, report.skipped
    );
    let verdict = match report.result {
        DoctorResult::Ok => "OK: yukti is set up and the API is fully ready.",
        DoctorResult::OkWithWarnings => {
            "OK with warnings: yukti is set up and you can use every command. Warnings are optional setup."
        }
        DoctorResult::Failed => "FAILED: fix the [FAIL] items above, then run `yukti doctor` again.",
    };
    out.push_str(&format!(
        "\n{counts}\nResult  {verdict} (exit {})\n",
        report.exit_code
    ));
    out
}
