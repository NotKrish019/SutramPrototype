//! `yukti status`: API reachability and dependency readiness.
//!
//! Uses the two probes registered by the Rust API (`backend-rust/src/api/routes/health.rs`):
//!
//! * `GET /health` returns `HealthResponse { status, service }`, with `status` always `"ok"`.
//! * `GET /health/ready` returns `ReadinessResponse { status, service, database, graph, error? }`:
//!   - `200` + `"ready"`: every configured dependency answered.
//!   - `200` + `"degraded"`: no failure, but PostgreSQL runs in filesystem mode and/or Neo4j is disabled.
//!   - `503` + `"not_ready"`: a configured dependency failed.
//!
//! A dependency is reported as connected only when the API itself says `"connected"`.

use serde::Serialize;
use serde_json::Value;

use crate::client::{ApiClient, RawResponse};
pub use crate::client::{HEALTH_PATH, READINESS_PATH};
use crate::dto::{HealthResponse as HealthBody, ReadinessResponse as ReadinessBody};
use crate::error::{exit, CliError, ErrorInfo};
use crate::output::{to_json, Rendered};
use crate::ui::{Badge, Glyph, Tone, Ui, MARGIN};

/// Schema identifier of the `status` JSON document.
pub const STATUS_SCHEMA: &str = "yukti.cli.status/v1";

/// Overall outcome of `yukti status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StatusResult {
    /// API reachable and all configured dependencies ready.
    Ready,
    /// API reachable; filesystem mode and/or graph disabled, nothing failing.
    Degraded,
    /// API reachable, but a configured dependency failed.
    NotReady,
    /// The API could not be reached.
    Unreachable,
    /// The API answered `/health` with an HTTP error.
    HealthFailed,
    /// `/health` succeeded, but readiness could not be determined.
    ReadinessUnavailable,
    /// A response did not have the expected shape.
    UnexpectedResponse,
}

impl StatusResult {
    fn summary(self) -> &'static str {
        match self {
            Self::Ready => "API and all configured dependencies are ready.",
            Self::Degraded => {
                "API is up in degraded mode: at least one dependency is not configured (see below)."
            }
            Self::NotReady => "API is up, but a configured dependency is not ready.",
            Self::Unreachable => {
                "API unreachable. Check that the YUKTI API is running and that --api-url / YUKTI_API_URL is correct."
            }
            Self::HealthFailed => "API responded, but its health check failed.",
            Self::ReadinessUnavailable => "API is up, but its readiness could not be determined.",
            Self::UnexpectedResponse => {
                "Unexpected response. Is the API URL pointing at the YUKTI Rust API?"
            }
        }
    }
}

/// One probe request and what came back.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EndpointCheck {
    pub url: String,
    /// HTTP status, or `null` when no response was received.
    pub http_status: Option<u16>,
    /// Response body exactly as sent by the server when it was JSON, otherwise `null`.
    pub body: Option<Value>,
    pub error: Option<ErrorInfo>,
}

impl EndpointCheck {
    fn new(url: String) -> Self {
        Self {
            url,
            http_status: None,
            body: None,
            error: None,
        }
    }

    fn record(&mut self, raw: &RawResponse) {
        self.http_status = Some(raw.status);
        self.body = raw.json().ok();
    }
}

/// Dependency states exactly as reported by the API (`database`, `graph`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Dependencies {
    pub database: String,
    pub graph: String,
    /// The API's `error` field, when present.
    pub error: Option<String>,
}

/// The `yukti.cli.status/v1` document.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StatusReport {
    pub schema: &'static str,
    pub api_url: String,
    pub result: StatusResult,
    pub exit_code: u8,
    /// True when the API answered `/health` with any HTTP response.
    pub api_reachable: bool,
    /// `service` from `/health`, when it succeeded.
    pub service: Option<String>,
    pub health: EndpointCheck,
    /// `null` when readiness was not checked because `/health` failed.
    pub readiness: Option<EndpointCheck>,
    /// `null` unless `/health/ready` returned a well-formed `ReadinessResponse`.
    pub dependencies: Option<Dependencies>,
}

impl StatusReport {
    fn finish(mut self, result: StatusResult, exit_code: u8) -> Self {
        self.result = result;
        self.exit_code = exit_code;
        self
    }
}

/// Runs both probes and renders the result.
pub async fn run(client: &ApiClient, json: bool, ui: &Ui) -> Rendered {
    let report = check(client).await;
    let text = if json {
        to_json(&report)
    } else {
        render_human(&report, ui)
    };
    Rendered::stdout(text, report.exit_code)
}

/// Runs both probes and classifies the outcome.
pub async fn check(client: &ApiClient) -> StatusReport {
    let mut report = StatusReport {
        schema: STATUS_SCHEMA,
        api_url: client.base_url().to_string(),
        result: StatusResult::UnexpectedResponse,
        exit_code: exit::BAD_RESPONSE,
        api_reachable: false,
        service: None,
        health: EndpointCheck::new(client.url_for(HEALTH_PATH)),
        readiness: None,
        dependencies: None,
    };

    // 1. Liveness.
    let health = match client.health().await {
        Ok(raw) => raw,
        Err(err) => {
            report.health.error = Some(ErrorInfo::from(&err));
            let result = match err {
                CliError::Unreachable { .. } | CliError::Timeout { .. } => {
                    StatusResult::Unreachable
                }
                _ => StatusResult::UnexpectedResponse,
            };
            let code = err.exit_code();
            return report.finish(result, code);
        }
    };
    report.api_reachable = true;
    report.health.record(&health);

    if !health.is_success() {
        let err = health.to_error();
        report.health.error = Some(ErrorInfo::from(&err));
        let code = err.exit_code();
        return report.finish(StatusResult::HealthFailed, code);
    }
    match parse_health(&health) {
        Ok(body) => report.service = Some(body.service),
        Err(err) => {
            report.health.error = Some(ErrorInfo::from(&err));
            return report.finish(StatusResult::UnexpectedResponse, exit::BAD_RESPONSE);
        }
    }

    // 2. Readiness.
    let mut readiness = EndpointCheck::new(client.url_for(READINESS_PATH));
    let raw = match client.readiness().await {
        Ok(raw) => raw,
        Err(err) => {
            readiness.error = Some(ErrorInfo::from(&err));
            report.readiness = Some(readiness);
            let code = err.exit_code();
            return report.finish(StatusResult::ReadinessUnavailable, code);
        }
    };
    readiness.record(&raw);

    let parsed = raw
        .json()
        .ok()
        .and_then(|v| serde_json::from_value::<ReadinessBody>(v).ok());

    let Some(body) = parsed else {
        // Not a ReadinessResponse: report the HTTP error if there was one, else a shape error.
        let err = if raw.is_success() {
            match raw.json() {
                Err(err) => err,
                Ok(_) => CliError::BadResponse {
                    url: raw.url.clone(),
                    reason: "body does not match ReadinessResponse {status, service, database, graph, error?}".to_string(),
                },
            }
        } else {
            raw.to_error()
        };
        readiness.error = Some(ErrorInfo::from(&err));
        report.readiness = Some(readiness);
        let (result, code) = if raw.is_success() {
            (StatusResult::UnexpectedResponse, exit::BAD_RESPONSE)
        } else {
            (StatusResult::ReadinessUnavailable, err.exit_code())
        };
        return report.finish(result, code);
    };

    report.dependencies = Some(Dependencies {
        database: body.database.clone(),
        graph: body.graph.clone(),
        error: body.error.clone(),
    });

    let outcome = if body.status == "not_ready" || raw.status >= 500 {
        // A 5xx is never treated as ready, whatever the body claims.
        if body.status != "not_ready" {
            readiness.error = Some(ErrorInfo::from(&raw.to_error()));
        }
        (StatusResult::NotReady, exit::SERVER_ERROR)
    } else if !raw.is_success() {
        let err = raw.to_error();
        readiness.error = Some(ErrorInfo::from(&err));
        (StatusResult::ReadinessUnavailable, err.exit_code())
    } else {
        match body.status.as_str() {
            "ready" => (StatusResult::Ready, exit::SUCCESS),
            "degraded" => (StatusResult::Degraded, exit::SUCCESS),
            other => {
                let err = CliError::BadResponse {
                    url: raw.url.clone(),
                    reason: format!("unknown readiness status {other:?}"),
                };
                readiness.error = Some(ErrorInfo::from(&err));
                (StatusResult::UnexpectedResponse, exit::BAD_RESPONSE)
            }
        }
    };
    report.readiness = Some(readiness);
    report.finish(outcome.0, outcome.1)
}

fn parse_health(raw: &RawResponse) -> Result<HealthBody, CliError> {
    let value = raw.json()?;
    let body: HealthBody = serde_json::from_value(value).map_err(|e| CliError::BadResponse {
        url: raw.url.clone(),
        reason: format!("body does not match HealthResponse {{status, service}} ({e})"),
    })?;
    if body.status != "ok" {
        return Err(CliError::BadResponse {
            url: raw.url.clone(),
            reason: format!("health status is {:?}, expected \"ok\"", body.status),
        });
    }
    Ok(body)
}

/// Human description of the API's `database` value. Only `"connected"` reads as connected.
pub fn describe_database(value: &str) -> String {
    match value {
        "connected" => "connected".to_string(),
        "unconfigured_filesystem_mode" => {
            "not configured (filesystem mode, no PostgreSQL)".to_string()
        }
        "disconnected" => "DISCONNECTED".to_string(),
        "failed_to_initialize" => {
            "FAILED TO INITIALIZE (configured, but no connection at startup)".to_string()
        }
        other => format!("unrecognized value {other:?}"),
    }
}

/// Human description of the API's `graph` value. Only `"connected"` reads as connected.
pub fn describe_graph(value: &str) -> String {
    match value {
        "connected" => "connected".to_string(),
        "disabled" => "disabled (Neo4j not configured)".to_string(),
        "disconnected" => "DISCONNECTED".to_string(),
        other => format!("unrecognized value {other:?}"),
    }
}

fn probe_line(check: &EndpointCheck, path: &str, ui: &Ui) -> String {
    let arrow = ui.g(Glyph::Arrow);
    match check.http_status {
        Some(code) => format!("GET {path} {arrow} HTTP {code}"),
        None => format!("GET {path} {arrow} no response"),
    }
}

/// Badge and one-word state for an overall result.
pub fn result_badge(result: StatusResult) -> (Badge, &'static str) {
    match result {
        StatusResult::Ready => (Badge::Ok, "ready"),
        StatusResult::Degraded => (Badge::Warn, "degraded"),
        StatusResult::NotReady => (Badge::Flag, "not ready"),
        StatusResult::Unreachable => (Badge::Flag, "unreachable"),
        StatusResult::HealthFailed => (Badge::Flag, "health failed"),
        StatusResult::ReadinessUnavailable => (Badge::Warn, "unknown"),
        StatusResult::UnexpectedResponse => (Badge::Flag, "unexpected response"),
    }
}

/// Badge for a dependency value. Only `"connected"` is healthy.
pub fn dependency_badge(value: &str) -> Badge {
    match value {
        "connected" => Badge::Ok,
        "unconfigured_filesystem_mode" | "disabled" => Badge::Off,
        _ => Badge::Flag,
    }
}

/// Dependency lines (`Database`, `Graph`) with a state glyph and the description.
pub fn render_dependencies(deps: &Dependencies, ui: &Ui) -> String {
    let mut out = String::new();
    for (label, value, text) in [
        (
            "Database",
            &deps.database,
            describe_database(&deps.database),
        ),
        ("Graph", &deps.graph, describe_graph(&deps.graph)),
    ] {
        let badge = dependency_badge(value);
        let tone = match badge {
            Badge::Ok => Tone::Text,
            Badge::Flag => Tone::Flag,
            _ => Tone::Muted,
        };
        let glyph = match badge {
            Badge::Ok => ui.mark(Glyph::Check, Tone::Ok),
            Badge::Flag => ui.mark(Glyph::Cross, Tone::Flag),
            _ => ui.mark(Glyph::Inferred, Tone::Subtle),
        };
        out.push_str(&ui.kv_painted(label, &format!("{glyph} {}", ui.paint(tone, &text))));
    }
    if let Some(api_error) = &deps.error {
        out.push_str(&ui.kv("API error", Tone::Flag, api_error));
    }
    out
}

/// Themed rendering. Colour is applied only when `ui` enables it; the layout is the same.
pub fn render_human(report: &StatusReport, ui: &Ui) -> String {
    let mut out = ui.header(&["status"], &report.api_url);
    let (badge, label) = result_badge(report.result);
    out.push('\n');
    out.push_str(&format!("{MARGIN}{}\n", ui.badge(badge, label)));
    out.push_str(&ui.para(MARGIN.len(), Tone::Muted, report.result.summary()));

    out.push_str(&ui.section("Service", ""));
    out.push_str(&ui.kv("Endpoint", Tone::Text, &report.api_url));
    let (api_badge, api_state) = if !report.api_reachable {
        (Badge::Flag, "unreachable")
    } else if report.result == StatusResult::HealthFailed || report.service.is_none() {
        (Badge::Flag, "health check failed")
    } else {
        (Badge::Ok, "reachable")
    };
    let mut api_value = format!(
        "{}  {}",
        ui.badge(api_badge, api_state),
        ui.paint(Tone::Subtle, &probe_line(&report.health, HEALTH_PATH, ui))
    );
    if let Some(service) = &report.service {
        api_value.push_str(&ui.paint(Tone::Subtle, &format!("  service {service}")));
    }
    out.push_str(&ui.kv_painted("API", &api_value));
    if let Some(err) = &report.health.error {
        out.push_str(&ui.kv("", Tone::Flag, &err.message));
    }

    match &report.readiness {
        None => out.push_str(&ui.kv("Readiness", Tone::Muted, "not checked")),
        Some(readiness) => {
            let (b, state) = match report.result {
                StatusResult::Ready => (Badge::Ok, "ready"),
                StatusResult::Degraded => (Badge::Warn, "degraded"),
                StatusResult::NotReady => (Badge::Flag, "not ready"),
                _ => (Badge::Warn, "unknown"),
            };
            out.push_str(&ui.kv_painted(
                "Readiness",
                &format!(
                    "{}  {}",
                    ui.badge(b, state),
                    ui.paint(Tone::Subtle, &probe_line(readiness, READINESS_PATH, ui))
                ),
            ));
            if let Some(deps) = &report.dependencies {
                out.push_str(&render_dependencies(deps, ui));
            }
            if let Some(err) = &readiness.error {
                out.push_str(&ui.kv("", Tone::Flag, &err.message));
            }
        }
    }
    out.push_str(&ui.footnote(&format!("exit {}", report.exit_code)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_connected_reads_as_connected() {
        assert_eq!(describe_database("connected"), "connected");
        for v in [
            "unconfigured_filesystem_mode",
            "disconnected",
            "failed_to_initialize",
            "CONNECTED",
            "",
        ] {
            assert!(!describe_database(v).starts_with("connected"), "{v}");
        }
        assert_eq!(describe_graph("connected"), "connected");
        for v in ["disabled", "disconnected", "connected?"] {
            assert!(!describe_graph(v).starts_with("connected"), "{v}");
        }
    }
}
