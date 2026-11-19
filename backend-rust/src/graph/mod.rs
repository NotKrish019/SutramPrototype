//! YUKTI Forensic Knowledge Graph Subsystem (Phase 2A).
//!
//! Provides Neo4j graph connectivity, schema constraints, privacy-preserving
//! identifier fingerprinting, transactional outbox projection from PostgreSQL,
//! and typed investigator query APIs.

pub mod client;
pub mod config;
pub mod model;
pub mod outbox;
pub mod privacy;
pub mod projection;
pub mod query;
pub mod reconciliation;
pub mod schema;

pub use client::{GraphClient, Neo4jHttpClient};
pub use config::GraphConfig;
pub use model::{
    CypherStatement, GraphNeighborResponse, GraphNode, GraphPathResponse, GraphRelationship,
    GraphSummaryResponse, OutboxEvent, OutboxMetrics, OutboxStatus,
};
pub use outbox::GraphOutboxRepository;
pub use privacy::compute_identifier_fingerprint;
pub use projection::GraphProjectionService;
pub use query::GraphQueryService;
pub use reconciliation::{GraphReconciliationReport, GraphReconciliationService};
pub use schema::initialize_schema;
