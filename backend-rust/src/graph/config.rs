//! Graph Configuration for Neo4j Connectivity and Identifier Privacy.

use std::env;

use crate::error::AppError;

/// Configuration parameters for Neo4j connection and graph projection.
#[derive(Debug, Clone)]
pub struct GraphConfig {
    /// Whether graph projection and graph APIs are enabled.
    pub enabled: bool,
    /// Base HTTP/HTTPS URI for Neo4j Query API (e.g. "http://localhost:7474").
    pub uri: String,
    /// Target Neo4j database name (default: "neo4j").
    pub database: String,
    /// Neo4j authentication username.
    pub user: String,
    /// Neo4j authentication password.
    pub password: String,
    /// Connection timeout in seconds.
    pub connect_timeout_seconds: u64,
    /// Optional secret HMAC key for identifier fingerprinting (privacy preservation).
    pub identifier_hmac_key: Option<String>,
}

impl GraphConfig {
    /// Validates graph configuration settings for security invariants.
    pub fn validate(&self) -> Result<(), AppError> {
        if self.enabled && (self.password.trim().is_empty() || self.password == "password") {
            return Err(AppError::Internal(anyhow::anyhow!(
                "Graph functionality is enabled (GRAPH_ENABLED=true) but NEO4J_PASSWORD is not set or uses the insecure default 'password'. Please configure a secure NEO4J_PASSWORD in your environment."
            )));
        }
        Ok(())
    }
}

impl Default for GraphConfig {
    fn default() -> Self {
        Self {
            enabled: env::var("GRAPH_ENABLED")
                .map(|v| v.trim().eq_ignore_ascii_case("true") || v.trim() == "1")
                .unwrap_or(false),
            uri: env::var("NEO4J_URI").unwrap_or_else(|_| "http://localhost:7474".to_string()),
            database: env::var("NEO4J_DATABASE").unwrap_or_else(|_| "neo4j".to_string()),
            user: env::var("NEO4J_USER").unwrap_or_else(|_| "neo4j".to_string()),
            password: env::var("NEO4J_PASSWORD").unwrap_or_else(|_| "password".to_string()),
            connect_timeout_seconds: env::var("NEO4J_CONNECT_TIMEOUT_SECONDS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(5),
            identifier_hmac_key: env::var("GRAPH_IDENTIFIER_HMAC_KEY")
                .ok()
                .filter(|s| !s.trim().is_empty()),
        }
    }
}
