//! Neo4j HTTP Query API Client and Abstraction Trait.

use async_trait::async_trait;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use serde_json::json;
use std::time::Duration;
use tracing::{debug, error};

use super::config::GraphConfig;
use super::model::CypherStatement;
use crate::error::AppError;

/// Asynchronous client abstraction for executing Cypher queries against a graph database.
#[async_trait]
pub trait GraphClient: Send + Sync {
    /// Executes a single parameterized Cypher statement and returns the JSON result.
    async fn execute_query(
        &self,
        statement: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, AppError>;

    /// Executes a sequence of Cypher statements in order.
    async fn execute_batch(&self, statements: Vec<CypherStatement>) -> Result<(), AppError>;

    /// Tests connectivity to the graph database.
    async fn ping(&self) -> Result<(), AppError>;
}

/// Official Neo4j HTTP Query API client implementation (`/db/{database}/query/v2`).
#[derive(Clone)]
pub struct Neo4jHttpClient {
    client: reqwest::Client,
    query_endpoint: String,
    ping_endpoint: String,
    config: GraphConfig,
}

impl Neo4jHttpClient {
    /// Creates a new `Neo4jHttpClient` from graph configuration.
    pub fn new(config: GraphConfig) -> Self {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(config.connect_timeout_seconds))
            .build()
            .unwrap_or_default();

        let base = config.uri.trim_end_matches('/');
        let query_endpoint = format!("{}/db/{}/query/v2", base, config.database);
        let ping_endpoint = format!("{}/", base);

        Self {
            client,
            query_endpoint,
            ping_endpoint,
            config,
        }
    }

    pub fn config(&self) -> &GraphConfig {
        &self.config
    }
}

#[async_trait]
impl GraphClient for Neo4jHttpClient {
    async fn execute_query(
        &self,
        statement: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, AppError> {
        let body = json!({
            "statement": statement,
            "parameters": params,
        });

        debug!(statement = %statement, "Executing Cypher statement via Query API v2");

        let response = self
            .client
            .post(&self.query_endpoint)
            .basic_auth(&self.config.user, Some(&self.config.password))
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                AppError::Internal(anyhow::anyhow!(
                    "Failed to communicate with Neo4j at '{}': {}",
                    self.query_endpoint,
                    e
                ))
            })?;

        let status = response.status();
        let resp_body: serde_json::Value = response.json().await.unwrap_or_default();

        if !status.is_success() {
            let error_msg = resp_body
                .get("errors")
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.first())
                .and_then(|err| err.get("message"))
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown Neo4j error");

            let error_code = resp_body
                .get("errors")
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.first())
                .and_then(|err| err.get("code"))
                .and_then(|c| c.as_str())
                .unwrap_or("Neo.DatabaseError.General.UnknownError");

            error!(code = %error_code, message = %error_msg, "Neo4j query execution failed");

            return Err(AppError::Internal(anyhow::anyhow!(
                "Neo4j query error [{}]: {}",
                error_code,
                error_msg
            )));
        }

        Ok(resp_body)
    }

    async fn execute_batch(&self, statements: Vec<CypherStatement>) -> Result<(), AppError> {
        for stmt in statements {
            self.execute_query(&stmt.statement, stmt.parameters).await?;
        }
        Ok(())
    }

    async fn ping(&self) -> Result<(), AppError> {
        let res = self
            .client
            .get(&self.ping_endpoint)
            .basic_auth(&self.config.user, Some(&self.config.password))
            .send()
            .await
            .map_err(|e| {
                AppError::Internal(anyhow::anyhow!("Neo4j connection test failed: {}", e))
            })?;

        if res.status().is_success() {
            Ok(())
        } else {
            Err(AppError::Internal(anyhow::anyhow!(
                "Neo4j ping returned status {}",
                res.status()
            )))
        }
    }
}
