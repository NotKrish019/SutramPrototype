//! CLI configuration: API base URL and request timeout.
//!
//! Precedence for every setting: command-line flag, then environment variable, then default.
//! An environment variable that is set but blank counts as unset.

use std::time::Duration;

use reqwest::Url;
use serde::Serialize;

use crate::error::CliError;
use crate::ui::TerminalEnv;

/// Default API base URL (the Rust API's default `HOST`/`PORT` in `backend-rust/src/config.rs`).
pub const DEFAULT_API_URL: &str = "http://127.0.0.1:8080";
/// Environment variable for the API base URL.
pub const ENV_API_URL: &str = "YUKTI_API_URL";
/// Environment variable for the request timeout in whole seconds.
pub const ENV_TIMEOUT_SECS: &str = "YUKTI_TIMEOUT_SECS";
/// Default request timeout in seconds.
pub const DEFAULT_TIMEOUT_SECS: u64 = 10;
/// Largest accepted timeout in seconds.
pub const MAX_TIMEOUT_SECS: u64 = 3600;

/// Where a configuration value came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ValueSource {
    Flag,
    Env,
    Default,
}

impl ValueSource {
    fn label(self, env_name: &str, flag: &str) -> String {
        match self {
            Self::Flag => flag.to_string(),
            Self::Env => env_name.to_string(),
            Self::Default => "the default".to_string(),
        }
    }
}

/// Configuration-relevant environment variables, captured once so resolution is testable.
/// The default has no variables set and describes a non-terminal (uncoloured) output.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EnvVars {
    pub api_url: Option<String>,
    pub timeout_secs: Option<String>,
    /// `YUKTI_CASE`, shown by `yukti config` (clap reads it for `--case` itself).
    pub case_id: Option<String>,
    pub terminal: TerminalEnv,
}

impl EnvVars {
    /// Reads the variables from the current process environment.
    pub fn from_process() -> Self {
        let read = |name: &str| std::env::var_os(name).map(|v| v.to_string_lossy().into_owned());
        Self {
            api_url: read(ENV_API_URL),
            timeout_secs: read(ENV_TIMEOUT_SECS),
            case_id: read(crate::cli::ENV_CASE),
            terminal: TerminalEnv::from_process(),
        }
    }
}

/// Resolved CLI configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub api_url: Url,
    pub api_url_source: ValueSource,
    pub timeout: Duration,
    pub timeout_source: ValueSource,
}

impl Config {
    /// Resolves the configuration from flags and environment variables.
    pub fn resolve(
        flag_api_url: Option<&str>,
        flag_timeout_secs: Option<u64>,
        env: &EnvVars,
    ) -> Result<Self, CliError> {
        let (raw_url, api_url_source) =
            match (non_blank(flag_api_url), non_blank(env.api_url.as_deref())) {
                (Some(flag), _) => (flag, ValueSource::Flag),
                (None, Some(env_value)) => (env_value, ValueSource::Env),
                (None, None) => (DEFAULT_API_URL, ValueSource::Default),
            };
        let api_url = parse_api_url(raw_url, api_url_source)?;

        let (timeout_secs, timeout_source) =
            match (flag_timeout_secs, non_blank(env.timeout_secs.as_deref())) {
                (Some(flag), _) => (flag, ValueSource::Flag),
                (None, Some(env_value)) => (
                    env_value.parse::<u64>().map_err(|_| {
                        CliError::Config(format!(
                        "{ENV_TIMEOUT_SECS} must be a whole number of seconds, got {env_value:?}"
                    ))
                    })?,
                    ValueSource::Env,
                ),
                (None, None) => (DEFAULT_TIMEOUT_SECS, ValueSource::Default),
            };
        if timeout_secs == 0 || timeout_secs > MAX_TIMEOUT_SECS {
            return Err(CliError::Config(format!(
                "timeout from {} must be between 1 and {MAX_TIMEOUT_SECS} seconds, got {timeout_secs}",
                timeout_source.label(ENV_TIMEOUT_SECS, "--timeout")
            )));
        }

        Ok(Self {
            api_url,
            api_url_source,
            timeout: Duration::from_secs(timeout_secs),
            timeout_source,
        })
    }

    /// The API base URL without a trailing slash, ready for appending `/path` segments.
    pub fn base_url(&self) -> String {
        self.api_url.as_str().trim_end_matches('/').to_string()
    }
}

fn non_blank(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|v| !v.is_empty())
}

/// Validates an API base URL: absolute http(s) URL with a host, no credentials, no query or
/// fragment. A path prefix (for a reverse proxy) is allowed.
pub fn parse_api_url(raw: &str, source: ValueSource) -> Result<Url, CliError> {
    let origin = source.label(ENV_API_URL, "--api-url");
    let raw = raw.trim();
    let invalid =
        |reason: String| CliError::Config(format!("invalid API URL from {origin}: {reason}"));

    let url = Url::parse(raw).map_err(|e| {
        if raw.contains("://") {
            invalid(format!("{raw:?} is not a valid URL ({e})"))
        } else {
            invalid(format!(
                "{raw:?} has no scheme; use an absolute URL such as http://{raw}"
            ))
        }
    })?;

    // Checked before anything that echoes the URL, so credentials are never printed.
    if !url.username().is_empty() || url.password().is_some() {
        return Err(invalid(
            "credentials embedded in the URL are not supported".to_string(),
        ));
    }
    match url.scheme() {
        "http" | "https" => {}
        other if !raw.contains("://") => {
            return Err(invalid(format!(
                "{raw:?} has no http(s) scheme (parsed scheme {other:?}); use an absolute URL such as http://{raw}"
            )))
        }
        other => {
            return Err(invalid(format!(
                "unsupported scheme {other:?} in {raw:?}; use http or https"
            )))
        }
    }
    if url.host_str().is_none_or(str::is_empty) {
        return Err(invalid(format!("{raw:?} has no host")));
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err(invalid(format!(
            "{raw:?} must not contain a query string or fragment"
        )));
    }
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(url: Option<&str>, timeout: Option<&str>) -> EnvVars {
        EnvVars {
            api_url: url.map(str::to_string),
            timeout_secs: timeout.map(str::to_string),
            ..EnvVars::default()
        }
    }

    #[test]
    fn default_applies_when_nothing_is_set() {
        let c = Config::resolve(None, None, &EnvVars::default()).unwrap();
        assert_eq!(c.base_url(), "http://127.0.0.1:8080");
        assert_eq!(c.api_url_source, ValueSource::Default);
        assert_eq!(c.timeout, Duration::from_secs(DEFAULT_TIMEOUT_SECS));
        assert_eq!(c.timeout_source, ValueSource::Default);
    }

    #[test]
    fn env_overrides_default() {
        let c =
            Config::resolve(None, None, &env(Some("http://10.0.0.5:9000"), Some("30"))).unwrap();
        assert_eq!(c.base_url(), "http://10.0.0.5:9000");
        assert_eq!(c.api_url_source, ValueSource::Env);
        assert_eq!(c.timeout, Duration::from_secs(30));
        assert_eq!(c.timeout_source, ValueSource::Env);
    }

    #[test]
    fn flag_overrides_env() {
        let c = Config::resolve(
            Some("https://api.example.test"),
            Some(5),
            &env(Some("http://10.0.0.5:9000"), Some("30")),
        )
        .unwrap();
        assert_eq!(c.base_url(), "https://api.example.test");
        assert_eq!(c.api_url_source, ValueSource::Flag);
        assert_eq!(c.timeout, Duration::from_secs(5));
        assert_eq!(c.timeout_source, ValueSource::Flag);
    }

    #[test]
    fn flag_wins_even_when_env_is_invalid() {
        let c = Config::resolve(
            Some("http://127.0.0.1:1"),
            None,
            &env(Some("not a url"), None),
        )
        .unwrap();
        assert_eq!(c.api_url_source, ValueSource::Flag);
    }

    #[test]
    fn blank_env_counts_as_unset() {
        let c = Config::resolve(None, None, &env(Some("   "), Some(""))).unwrap();
        assert_eq!(c.api_url_source, ValueSource::Default);
        assert_eq!(c.timeout_source, ValueSource::Default);
    }

    #[test]
    fn path_prefix_is_kept_without_trailing_slash() {
        let c =
            Config::resolve(Some("http://proxy.local/yukti/"), None, &EnvVars::default()).unwrap();
        assert_eq!(c.base_url(), "http://proxy.local/yukti");
    }

    #[test]
    fn rejects_invalid_urls_with_the_offending_source() {
        let cases = [
            (
                "127.0.0.1:8080",
                "use an absolute URL such as http://127.0.0.1:8080",
            ),
            (
                "localhost:8080",
                "use an absolute URL such as http://localhost:8080",
            ),
            ("api.local", "use an absolute URL such as http://api.local"),
            ("ftp://host", "unsupported scheme"),
            ("http://", "not a valid URL"),
            ("http://h:8080/?x=1", "query string"),
            ("http://h:8080/#frag", "query string or fragment"),
        ];
        for (raw, expected) in cases {
            let err = Config::resolve(Some(raw), None, &EnvVars::default()).unwrap_err();
            let msg = err.to_string();
            assert_eq!(err.exit_code(), 2, "{raw}");
            assert!(msg.contains("--api-url"), "{raw}: {msg}");
            assert!(msg.contains(expected), "{raw}: {msg}");
        }
        let err = Config::resolve(None, None, &env(Some("nope"), None)).unwrap_err();
        assert!(err.to_string().contains(ENV_API_URL));
    }

    #[test]
    fn credentials_are_rejected_without_being_echoed() {
        let err = Config::resolve(Some("http://user:s3cret@h:8080"), None, &EnvVars::default())
            .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("credentials"));
        assert!(!msg.contains("s3cret"));
    }

    #[test]
    fn rejects_bad_timeouts() {
        assert!(Config::resolve(None, Some(0), &EnvVars::default()).is_err());
        assert!(Config::resolve(None, Some(MAX_TIMEOUT_SECS + 1), &EnvVars::default()).is_err());
        let err = Config::resolve(None, None, &env(None, Some("ten"))).unwrap_err();
        assert!(err.to_string().contains(ENV_TIMEOUT_SECS));
        assert_eq!(err.exit_code(), 2);
    }
}
