//! Neo4j Knowledge Graph Schema Constraints and Indexes.

use std::sync::Arc;
use tracing::info;

use super::client::GraphClient;
use crate::error::AppError;

/// Idempotent Cypher DDL statements initializing uniqueness constraints and performance indexes.
pub const SCHEMA_STATEMENTS: &[&str] = &[
    // Uniqueness Constraints
    "CREATE CONSTRAINT cst_case_unique IF NOT EXISTS FOR (c:Case) REQUIRE c.case_id IS UNIQUE",
    "CREATE CONSTRAINT cst_evidence_unique IF NOT EXISTS FOR (e:Evidence) REQUIRE e.evidence_id IS UNIQUE",
    "CREATE CONSTRAINT cst_run_unique IF NOT EXISTS FOR (r:ExtractionRun) REQUIRE r.extraction_run_id IS UNIQUE",
    "CREATE CONSTRAINT cst_entity_unique IF NOT EXISTS FOR (ent:EntityCandidate) REQUIRE ent.graph_entity_id IS UNIQUE",
    "CREATE CONSTRAINT cst_event_unique IF NOT EXISTS FOR (ev:EventCandidate) REQUIRE ev.event_id IS UNIQUE",
    "CREATE CONSTRAINT cst_assertion_unique IF NOT EXISTS FOR (rel:RelationshipAssertion) REQUIRE rel.assertion_id IS UNIQUE",
    "CREATE CONSTRAINT cst_reconstruction_unique IF NOT EXISTS FOR (rec:ReconstructionRun) REQUIRE rec.reconstruction_run_id IS UNIQUE",
    "CREATE CONSTRAINT cst_contradiction_unique IF NOT EXISTS FOR (lead:ContradictionLead) REQUIRE lead.contradiction_id IS UNIQUE",

    // Performance Indexes
    "CREATE INDEX idx_case_evidence IF NOT EXISTS FOR (e:Evidence) ON (e.case_id)",
    "CREATE INDEX idx_case_entity IF NOT EXISTS FOR (ent:EntityCandidate) ON (ent.case_id)",
    "CREATE INDEX idx_case_event IF NOT EXISTS FOR (ev:EventCandidate) ON (ev.case_id, ev.nominal_timestamp)",
    "CREATE INDEX idx_case_contradiction IF NOT EXISTS FOR (lead:ContradictionLead) ON (lead.case_id)",
    "CREATE INDEX idx_entity_fingerprint IF NOT EXISTS FOR (ent:EntityCandidate) ON (ent.fingerprint)",
];

/// Initializes the Neo4j schema constraints and indexes.
pub async fn initialize_schema(client: Arc<dyn GraphClient>) -> Result<(), AppError> {
    info!("Initializing Neo4j knowledge graph schema constraints and indexes...");
    for statement in SCHEMA_STATEMENTS {
        client
            .execute_query(statement, serde_json::json!({}))
            .await?;
    }
    info!("Neo4j schema constraints and indexes successfully verified.");
    Ok(())
}
