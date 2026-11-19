//! Domain Models for Forensic Knowledge Graph & Projection Outbox.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A single parameterized Cypher statement ready for execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CypherStatement {
    pub statement: String,
    pub parameters: serde_json::Value,
}

impl CypherStatement {
    pub fn new(statement: impl Into<String>, parameters: serde_json::Value) -> Self {
        Self {
            statement: statement.into(),
            parameters,
        }
    }
}

/// Status of an outbox projection record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OutboxStatus {
    Pending,
    Processing,
    Succeeded,
    Failed,
    RetryWait,
    DeadLetter,
}

impl std::fmt::Display for OutboxStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "PENDING"),
            Self::Processing => write!(f, "PROCESSING"),
            Self::Succeeded => write!(f, "SUCCEEDED"),
            Self::Failed => write!(f, "FAILED"),
            Self::RetryWait => write!(f, "RETRY_WAIT"),
            Self::DeadLetter => write!(f, "DEAD_LETTER"),
        }
    }
}

/// Relational representation of a record in `graph_projection_outbox`.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct OutboxEvent {
    pub outbox_id: Uuid,
    pub case_id: Uuid,
    pub aggregate_type: String,
    pub aggregate_id: Uuid,
    pub aggregate_version: i32,
    pub event_type: String,
    pub payload: serde_json::Value,
    pub status: String,
    pub attempt_count: i32,
    pub max_attempts: i32,
    pub next_attempt_at: DateTime<Utc>,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub processed_at: Option<DateTime<Utc>>,
}

/// Aggregate metrics for outbox health monitoring.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OutboxMetrics {
    pub pending: i64,
    pub processing: i64,
    pub succeeded: i64,
    pub failed: i64,
    pub retry_wait: i64,
    pub dead_letter: i64,
}

/// Generic node representation for investigator API responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub label: String,
    pub properties: serde_json::Value,
}

/// Generic relationship edge representation for investigator API responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphRelationship {
    pub id: String,
    pub source: String,
    pub target: String,
    pub relation_type: String,
    pub properties: serde_json::Value,
}

/// Response payload for `GET /api/v1/cases/{case_id}/graph/summary`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphSummaryResponse {
    pub case_id: Uuid,
    pub evidence_count: i64,
    pub extraction_run_count: i64,
    pub entity_candidate_count: i64,
    pub event_candidate_count: i64,
    pub relationship_assertion_count: i64,
    pub reconstruction_run_count: i64,
    pub contradiction_lead_count: i64,
    pub graph_synchronization_status: String,
    pub last_projection_time: Option<DateTime<Utc>>,
    pub identifier_privacy: String,
}

/// Response payload for `GET /api/v1/cases/{case_id}/graph/entities/{entity_id}/neighbors`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNeighborResponse {
    pub case_id: Uuid,
    pub root_entity_id: String,
    pub depth: u32,
    pub nodes: Vec<GraphNode>,
    pub relationships: Vec<GraphRelationship>,
}

/// Response payload for `GET /api/v1/cases/{case_id}/graph/path`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphPathResponse {
    pub case_id: Uuid,
    pub from_entity_id: String,
    pub to_entity_id: String,
    pub found: bool,
    pub nodes: Vec<GraphNode>,
    pub relationships: Vec<GraphRelationship>,
}
