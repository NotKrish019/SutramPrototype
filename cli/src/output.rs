//! Output rendering.
//!
//! Commands produce a [`Rendered`] value rather than printing, so output can be tested
//! without spawning a process.
//!
//! * Human mode: results go to stdout; errors that stop a command go to stderr.
//! * JSON mode (`--json`): exactly one JSON document is written to stdout, for success and
//!   failure alike. Each document has a `schema` field naming its versioned shape.

use serde::Serialize;

use crate::error::{CliError, ErrorInfo};

/// Schema identifier of the generic JSON error document.
pub const ERROR_SCHEMA: &str = "yukti.cli.error/v1";

/// The complete output of one CLI invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rendered {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: u8,
}

impl Rendered {
    /// Output that goes to stdout.
    pub fn stdout(text: String, exit_code: u8) -> Self {
        Self {
            stdout: text,
            stderr: String::new(),
            exit_code,
        }
    }
}

#[derive(Serialize)]
struct ErrorDocument<'a> {
    schema: &'static str,
    exit_code: u8,
    error: &'a ErrorInfo,
}

/// Renders an error that prevented a command from running or completing.
pub fn render_error(err: &CliError, json: bool) -> Rendered {
    let exit_code = err.exit_code();
    if json {
        let info = ErrorInfo::from(err);
        let doc = ErrorDocument {
            schema: ERROR_SCHEMA,
            exit_code,
            error: &info,
        };
        Rendered::stdout(to_json(&doc), exit_code)
    } else {
        Rendered {
            stdout: String::new(),
            stderr: format!("error: {err}\n"),
            exit_code,
        }
    }
}

/// Pretty-printed JSON followed by a newline.
pub fn to_json<T: Serialize>(value: &T) -> String {
    // Serializing CLI-defined structs and serde_json values cannot fail: all map keys are strings.
    let mut text = serde_json::to_string_pretty(value).expect("CLI output is always serializable");
    text.push('\n');
    text
}

/// Human-mode warnings, one `warning: ...` line each (written to stderr).
pub fn render_warnings(warnings: &[String]) -> String {
    warnings.iter().map(|w| format!("warning: {w}\n")).collect()
}

/// Renders an error and, in human mode, adds `note: ...` lines after it on stderr.
/// JSON mode is unchanged: one error document on stdout.
pub fn render_error_with_notes(err: &CliError, json: bool, notes: &[String]) -> Rendered {
    let mut rendered = render_error(err, json);
    if !json {
        for note in notes {
            rendered.stderr.push_str(&format!("note: {note}\n"));
        }
    }
    rendered
}

/// Compact single-line JSON.
pub fn compact_json(value: &serde_json::Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn human_errors_go_to_stderr() {
        let r = render_error(&CliError::Config("bad".into()), false);
        assert_eq!(r.exit_code, 2);
        assert!(r.stdout.is_empty());
        assert_eq!(r.stderr, "error: invalid configuration: bad\n");
    }

    #[test]
    fn json_errors_are_one_document_on_stdout() {
        let r = render_error(
            &CliError::Unsupported {
                feature: "scenario comparison".into(),
            },
            true,
        );
        assert_eq!(r.exit_code, 69);
        assert!(r.stderr.is_empty());
        let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
        assert_eq!(v["schema"], ERROR_SCHEMA);
        assert_eq!(v["exit_code"], 69);
        assert_eq!(v["error"]["kind"], "unsupported");
        assert_eq!(
            v["error"]["message"],
            "scenario comparison is not yet supported by the YUKTI API"
        );
    }
}
