//! Error type and the process exit-code contract of the `yukti` CLI.
//!
//! The exit codes in [`exit`] are a public contract documented in `cli/README.md`.
//! Scripts may rely on them, so existing values must not change meaning.

use std::fmt;
use std::time::Duration;

use serde::Serialize;

/// Process exit codes.
pub mod exit {
    /// The operation succeeded.
    pub const SUCCESS: u8 = 0;
    /// Unexpected internal failure of the CLI itself (not caused by the API).
    pub const INTERNAL: u8 = 1;
    /// Invalid command, arguments or configuration.
    pub const USAGE: u8 = 2;
    /// The API could not be reached (connection refused, DNS failure, timeout).
    pub const UNREACHABLE: u8 = 3;
    /// The API answered with an HTTP 4xx client error.
    pub const CLIENT_ERROR: u8 = 4;
    /// The API answered with an HTTP 5xx server error, or reported that it is not ready.
    pub const SERVER_ERROR: u8 = 5;
    /// The API answered, but the response was not what the CLI expects (bad JSON, wrong shape).
    pub const BAD_RESPONSE: u8 = 6;
    /// A local integrity check failed: a SHA-256 computed by the CLI does not match the hash
    /// the API recorded (`evidence verify`, `evidence ingest`).
    pub const VERIFICATION_FAILED: u8 = 7;
    /// The requested functionality is not yet available in the API.
    pub const UNSUPPORTED: u8 = 69;
}

/// Maximum number of characters of a non-JSON error body echoed back to the user.
const MAX_BODY_EXCERPT: usize = 200;

/// Every failure the CLI can report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliError {
    /// Invalid configuration (API URL, timeout).
    Config(String),
    /// Invalid command argument that clap cannot check (blank title, bad UUID, bad JSON).
    InvalidArgument(String),
    /// The API could not be reached.
    Unreachable { url: String, reason: String },
    /// The request did not complete within the configured timeout.
    Timeout { url: String, timeout: Duration },
    /// The API answered with a non-success HTTP status.
    Http {
        url: String,
        status: u16,
        /// `error.code` from the API error envelope, when present.
        api_code: Option<String>,
        message: String,
    },
    /// The API answered with something the CLI cannot interpret.
    BadResponse { url: String, reason: String },
    /// The functionality is not yet offered by the API.
    Unsupported { feature: String },
    /// Unexpected failure inside the CLI.
    Internal(String),
}

impl CliError {
    /// The process exit code for this error.
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::Config(_) | Self::InvalidArgument(_) => exit::USAGE,
            Self::Unreachable { .. } | Self::Timeout { .. } => exit::UNREACHABLE,
            Self::Http { status, .. } => match status {
                400..=499 => exit::CLIENT_ERROR,
                500..=599 => exit::SERVER_ERROR,
                _ => exit::BAD_RESPONSE,
            },
            Self::BadResponse { .. } => exit::BAD_RESPONSE,
            Self::Unsupported { .. } => exit::UNSUPPORTED,
            Self::Internal(_) => exit::INTERNAL,
        }
    }

    /// Stable machine-readable error kind used in JSON output.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Config(_) => "config",
            Self::InvalidArgument(_) => "invalid_argument",
            Self::Unreachable { .. } => "unreachable",
            Self::Timeout { .. } => "timeout",
            Self::Http { status, .. } if (400..=499).contains(status) => "http_client_error",
            Self::Http { status, .. } if (500..=599).contains(status) => "http_server_error",
            Self::Http { .. } => "http_unexpected_status",
            Self::BadResponse { .. } => "unexpected_response",
            Self::Unsupported { .. } => "unsupported",
            Self::Internal(_) => "internal",
        }
    }

    fn url(&self) -> Option<&str> {
        match self {
            Self::Unreachable { url, .. }
            | Self::Timeout { url, .. }
            | Self::Http { url, .. }
            | Self::BadResponse { url, .. } => Some(url),
            Self::Config(_)
            | Self::InvalidArgument(_)
            | Self::Unsupported { .. }
            | Self::Internal(_) => None,
        }
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(msg) => write!(f, "invalid configuration: {msg}"),
            Self::InvalidArgument(msg) => write!(f, "invalid argument: {msg}"),
            Self::Unreachable { url, reason } => {
                write!(f, "cannot reach the YUKTI API at {url}: {reason}")
            }
            Self::Timeout { url, timeout } => write!(
                f,
                "request to {url} timed out after {}s",
                timeout.as_secs_f64()
            ),
            Self::Http {
                url,
                status,
                api_code,
                message,
            } => match api_code {
                Some(code) => write!(f, "{url} returned HTTP {status} ({code}): {message}"),
                None => write!(f, "{url} returned HTTP {status}: {message}"),
            },
            Self::BadResponse { url, reason } => {
                write!(f, "unexpected response from {url}: {reason}")
            }
            Self::Unsupported { feature } => {
                write!(f, "{feature} is not yet supported by the YUKTI API")
            }
            Self::Internal(msg) => write!(f, "internal CLI error: {msg}"),
        }
    }
}

impl std::error::Error for CliError {}

/// Serializable description of an error, used in JSON output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ErrorInfo {
    pub kind: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_status: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

impl From<&CliError> for ErrorInfo {
    fn from(err: &CliError) -> Self {
        let (http_status, api_code) = match err {
            CliError::Http {
                status, api_code, ..
            } => (Some(*status), api_code.clone()),
            _ => (None, None),
        };
        Self {
            kind: err.kind().to_string(),
            message: err.to_string(),
            http_status,
            api_code,
            url: err.url().map(str::to_string),
        }
    }
}

/// Builds the error for a non-success HTTP response.
///
/// Understands the API's `{"error": {"code": ..., "message": ...}}` envelope
/// (`backend-rust/src/error.rs`). Any other body is reduced to a short single-line excerpt,
/// because not every error passes through that envelope (for example framework-level
/// request rejections).
pub fn http_error(url: &str, status: u16, body: &str) -> CliError {
    let envelope = serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v.get("error").cloned());
    let code = envelope
        .as_ref()
        .and_then(|e| e.get("code"))
        .and_then(|c| c.as_str())
        .map(str::to_string);
    let envelope_message = envelope
        .as_ref()
        .and_then(|e| e.get("message"))
        .and_then(|m| m.as_str())
        .map(str::to_string);

    let message = envelope_message.unwrap_or_else(|| {
        let excerpt = body_excerpt(body);
        if excerpt.is_empty() {
            reqwest::StatusCode::from_u16(status)
                .ok()
                .and_then(|s| s.canonical_reason())
                .unwrap_or("no response body")
                .to_string()
        } else {
            excerpt
        }
    });

    CliError::Http {
        url: url.to_string(),
        status,
        api_code: code,
        message,
    }
}

/// First line of a body, trimmed and limited to [`MAX_BODY_EXCERPT`] characters.
pub fn body_excerpt(body: &str) -> String {
    let line = body.trim().lines().next().unwrap_or("").trim();
    if line.chars().count() > MAX_BODY_EXCERPT {
        let cut: String = line.chars().take(MAX_BODY_EXCERPT).collect();
        format!("{cut}...")
    } else {
        line.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_codes_follow_the_documented_contract() {
        assert_eq!(CliError::Config("x".into()).exit_code(), 2);
        assert_eq!(CliError::InvalidArgument("x".into()).exit_code(), 2);
        let unreachable = CliError::Unreachable {
            url: "u".into(),
            reason: "r".into(),
        };
        assert_eq!(unreachable.exit_code(), 3);
        let timeout = CliError::Timeout {
            url: "u".into(),
            timeout: Duration::from_secs(1),
        };
        assert_eq!(timeout.exit_code(), 3);
        assert_eq!(http_error("u", 404, "").exit_code(), 4);
        assert_eq!(http_error("u", 422, "").exit_code(), 4);
        assert_eq!(http_error("u", 500, "").exit_code(), 5);
        assert_eq!(http_error("u", 503, "").exit_code(), 5);
        assert_eq!(http_error("u", 302, "").exit_code(), 6);
        let bad = CliError::BadResponse {
            url: "u".into(),
            reason: "r".into(),
        };
        assert_eq!(bad.exit_code(), 6);
        let unsupported = CliError::Unsupported {
            feature: "f".into(),
        };
        assert_eq!(unsupported.exit_code(), 69);
        assert_eq!(CliError::Internal("x".into()).exit_code(), 1);
    }

    #[test]
    fn parses_the_api_error_envelope() {
        let body = r#"{"error":{"code":"NOT_FOUND","message":"Case not found for id 'x'"}}"#;
        match http_error("http://h/api/v1/cases/x", 404, body) {
            CliError::Http {
                status,
                api_code,
                message,
                ..
            } => {
                assert_eq!(status, 404);
                assert_eq!(api_code.as_deref(), Some("NOT_FOUND"));
                assert_eq!(message, "Case not found for id 'x'");
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn falls_back_to_plain_text_and_reason_phrase() {
        match http_error("u", 400, "Failed to parse the request body as JSON\nmore") {
            CliError::Http {
                api_code, message, ..
            } => {
                assert_eq!(api_code, None);
                assert_eq!(message, "Failed to parse the request body as JSON");
            }
            other => panic!("unexpected {other:?}"),
        }
        match http_error("u", 502, "   ") {
            CliError::Http { message, .. } => assert_eq!(message, "Bad Gateway"),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn long_bodies_are_truncated() {
        let long = "x".repeat(500);
        let excerpt = body_excerpt(&long);
        assert_eq!(excerpt.chars().count(), MAX_BODY_EXCERPT + 3);
        assert!(excerpt.ends_with("..."));
    }

    #[test]
    fn error_info_carries_http_details() {
        let err = http_error(
            "http://h/x",
            409,
            r#"{"error":{"code":"CONFLICT","message":"dup"}}"#,
        );
        let info = ErrorInfo::from(&err);
        assert_eq!(info.kind, "http_client_error");
        assert_eq!(info.http_status, Some(409));
        assert_eq!(info.api_code.as_deref(), Some("CONFLICT"));
        assert_eq!(info.url.as_deref(), Some("http://h/x"));
    }
}
