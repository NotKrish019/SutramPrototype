//! Centralized Application Configuration (Member 2 / Rust Backend).
//!
//! Provides type-safe environment variable parsing with sensible development defaults
//! and structured configuration error handling.

use std::env;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;

/// Centralized application configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppConfig {
    /// Host IP address to bind (defaults to 127.0.0.1 for secure local development).
    pub host: IpAddr,
    /// TCP port to bind (defaults to 8080).
    pub port: u16,
    /// Log directive filter string for `tracing_subscriber` (defaults to info/debug).
    pub rust_log: String,
    /// Root directory for immutable forensic evidence storage (defaults to data/evidence/raw).
    pub evidence_storage_path: PathBuf,
    /// Root directory for derived forensic analysis and extraction artifacts (defaults to data/evidence/derived).
    pub derived_storage_path: PathBuf,
    /// Maximum allowable upload size in bytes (defaults to 50 MB = 52,428,800 bytes).
    pub max_upload_size_bytes: usize,
    /// PostgreSQL connection URL (optional; if unset or unreachable, database persistence is disabled).
    pub database_url: Option<String>,
    /// Maximum number of connections in PostgreSQL connection pool (defaults to 10).
    pub database_max_connections: u32,
    /// Minimum number of connections in PostgreSQL connection pool (defaults to 1).
    pub database_min_connections: u32,
    /// Timeout in seconds when acquiring or establishing a database connection (defaults to 5s).
    pub database_connect_timeout_seconds: u64,
}

/// Structured configuration errors that prevent panics at startup.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Invalid HOST environment variable '{value}': {source}")]
    InvalidHost {
        value: String,
        #[source]
        source: std::net::AddrParseError,
    },

    #[error("Invalid PORT environment variable '{value}': {source}")]
    InvalidPort {
        value: String,
        #[source]
        source: std::num::ParseIntError,
    },

    #[error("Invalid MAX_UPLOAD_SIZE_BYTES environment variable '{value}': {source}")]
    InvalidMaxUploadSize {
        value: String,
        #[source]
        source: std::num::ParseIntError,
    },
}

impl AppConfig {
    /// Constructs `AppConfig` from environment variables, falling back to safe local defaults.
    pub fn from_env() -> Result<Self, ConfigError> {
        let host_str = env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
        let host: IpAddr = host_str
            .parse()
            .map_err(|source| ConfigError::InvalidHost {
                value: host_str,
                source,
            })?;

        let port_str = env::var("PORT").unwrap_or_else(|_| "8080".to_string());
        let port: u16 = port_str
            .parse()
            .map_err(|source| ConfigError::InvalidPort {
                value: port_str,
                source,
            })?;

        let rust_log = env::var("RUST_LOG")
            .unwrap_or_else(|_| "info,sutram_backend_rust=debug,tower_http=debug".to_string());

        let evidence_storage_path = env::var("EVIDENCE_STORAGE_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("data/evidence/raw"));

        let derived_storage_path = env::var("DERIVED_STORAGE_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("data/evidence/derived"));

        let max_upload_size_bytes = match env::var("MAX_UPLOAD_SIZE_BYTES") {
            Ok(val) => val
                .parse::<usize>()
                .map_err(|source| ConfigError::InvalidMaxUploadSize { value: val, source })?,
            Err(_) => 50 * 1024 * 1024,
        };

        let database_url = env::var("DATABASE_URL")
            .ok()
            .filter(|s| !s.trim().is_empty());
        let database_max_connections = env::var("DATABASE_MAX_CONNECTIONS")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(10);
        let database_min_connections = env::var("DATABASE_MIN_CONNECTIONS")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(1);
        let database_connect_timeout_seconds = env::var("DATABASE_CONNECT_TIMEOUT_SECONDS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(5);

        Ok(Self {
            host,
            port,
            rust_log,
            evidence_storage_path,
            derived_storage_path,
            max_upload_size_bytes,
            database_url,
            database_max_connections,
            database_min_connections,
            database_connect_timeout_seconds,
        })
    }

    /// Computes the `SocketAddr` for server binding.
    pub fn socket_addr(&self) -> SocketAddr {
        SocketAddr::new(self.host, self.port)
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            host: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            port: 8080,
            rust_log: "info,sutram_backend_rust=debug,tower_http=debug".to_string(),
            evidence_storage_path: PathBuf::from("data/evidence/raw"),
            derived_storage_path: PathBuf::from("data/evidence/derived"),
            max_upload_size_bytes: 50 * 1024 * 1024,
            database_url: None,
            database_max_connections: 10,
            database_min_connections: 1,
            database_connect_timeout_seconds: 5,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AppConfig::default();
        assert_eq!(config.host, IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)));
        assert_eq!(config.port, 8080);
        assert_eq!(
            config.socket_addr(),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080)
        );
        assert_eq!(
            config.evidence_storage_path,
            PathBuf::from("data/evidence/raw")
        );
        assert_eq!(
            config.derived_storage_path,
            PathBuf::from("data/evidence/derived")
        );
        assert_eq!(config.max_upload_size_bytes, 50 * 1024 * 1024);
        assert_eq!(config.database_url, None);
        assert_eq!(config.database_max_connections, 10);
        assert_eq!(config.database_min_connections, 1);
        assert_eq!(config.database_connect_timeout_seconds, 5);
    }
}
