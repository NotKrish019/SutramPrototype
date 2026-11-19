//! Database Entity Models mapping to PostgreSQL Tables.
//!
//! Structured representations with SQLx derive support for type-safe queries.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Relational representation of an investigative case dossier.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, FromRow)]
pub struct CaseEntity {
    pub case_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub fir_number: Option<String>,
    pub status: String,
    pub investigator_id: Option<String>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Relational representation of an acquired evidence exhibit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, FromRow)]
pub struct EvidenceEntity {
    pub evidence_id: Uuid,
    pub case_id: Uuid,
    pub original_filename: String,
    pub source_type: String,
    pub mime_type: String,
    pub file_size_bytes: i64,
    pub sha256_hash: String,
    pub storage_uri: String,
    pub source_timezone: String,
    pub clock_offset_seconds: f64,
    pub ingestion_status: String,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Relational representation of a discrete extraction run attempt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, FromRow)]
pub struct ExtractionRunEntity {
    pub extraction_run_id: Uuid,
    pub evidence_id: Uuid,
    pub case_id: Uuid,
    pub source_sha256: String,
    pub extractor_name: String,
    pub extractor_version: String,
    pub configuration_hash: String,
    pub configuration: serde_json::Value,
    pub status: String,
    pub is_active: bool,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub duration_ms: Option<f64>,
    pub records_processed: i32,
    pub valid_records_count: i32,
    pub rejected_records_count: i32,
    pub validation_issue_count: i32,
    pub result_sha256: Option<String>,
    pub result_snapshot: Option<serde_json::Value>,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Relational representation of a normalized technical identifier.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, FromRow)]
pub struct ExtractedIdentifierEntity {
    pub identifier_id: Uuid,
    pub extraction_run_id: Uuid,
    pub evidence_id: Uuid,
    pub case_id: Uuid,
    pub source_record_id: String,
    pub identifier_type: String,
    pub raw_value: String,
    pub normalized_value: Option<String>,
    pub is_ambiguous: bool,
    pub location_reference: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Relational representation of a resolved technical entity candidate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, FromRow)]
pub struct ResolvedEntityEntity {
    pub entity_id: Uuid,
    pub extraction_run_id: Uuid,
    pub case_id: Uuid,
    pub entity_type: String,
    pub canonical_value: String,
    pub observed_identifiers: serde_json::Value,
    pub confidence: f64,
    pub evidence_references: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

/// Relational representation of an extracted event candidate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, FromRow)]
pub struct EventEntity {
    pub event_id: Uuid,
    pub extraction_run_id: Uuid,
    pub evidence_id: Uuid,
    pub case_id: Uuid,
    pub source_record_id: String,
    pub event_type: String,
    pub nominal_timestamp: DateTime<Utc>,
    pub timestamp_precision: String,
    pub duration_seconds: Option<f64>,
    pub uncertainty_window_start: Option<DateTime<Utc>>,
    pub uncertainty_window_end: Option<DateTime<Utc>>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub involved_entity_ids: serde_json::Value,
    pub payload: serde_json::Value,
    pub is_observed: bool,
    pub created_at: DateTime<Utc>,
}

/// Relational representation of a relationship candidate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, FromRow)]
pub struct RelationshipEntity {
    pub relationship_id: Uuid,
    pub extraction_run_id: Uuid,
    pub case_id: Uuid,
    pub source_id: String,
    pub source_kind: String,
    pub target_id: String,
    pub target_kind: String,
    pub relation_type: String,
    pub weight: Option<f64>,
    pub is_observed: bool,
    pub evidence_references: serde_json::Value,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

/// Relational representation of an extraction validation issue.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, FromRow)]
pub struct ValidationIssueEntity {
    pub issue_id: Uuid,
    pub extraction_run_id: Uuid,
    pub evidence_id: Uuid,
    pub case_id: Uuid,
    pub record_id: Option<String>,
    pub field_name: String,
    pub issue_type: String,
    pub raw_value: Option<String>,
    pub message: String,
    pub row_index: Option<i64>,
    pub created_at: DateTime<Utc>,
}

/// Relational representation of an entry in the append-only custody ledger.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, FromRow)]
pub struct CustodyEventEntity {
    pub custody_id: i64,
    pub event_id_uuid: Uuid,
    pub evidence_id: Uuid,
    pub case_id: Uuid,
    pub action: String,
    pub actor: String,
    pub payload_sha256: Option<String>,
    pub details: serde_json::Value,
    pub prev_event_hash: Option<String>,
    pub event_hash: String,
    pub timestamp: DateTime<Utc>,
}

/// Relational representation of a reconstruction run execution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, FromRow)]
pub struct ReconstructionRunEntity {
    pub reconstruction_run_id: Uuid,
    pub extraction_run_id: Option<Uuid>,
    pub case_id: Uuid,
    pub reconstruction_type: String,
    pub status: String,
    pub parameters: serde_json::Value,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub result_snapshot: Option<serde_json::Value>,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Relational representation of an input provenance link between a reconstruction run and an extraction run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, FromRow)]
pub struct ReconstructionRunInputEntity {
    pub reconstruction_run_id: Uuid,
    pub extraction_run_id: Uuid,
    pub created_at: DateTime<Utc>,
}

/// Relational representation of a detected contradiction lead.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, FromRow)]
pub struct ContradictionEntity {
    pub contradiction_id: Uuid,
    pub reconstruction_run_id: Uuid,
    pub case_id: Uuid,
    pub event_id_1: Uuid,
    pub event_id_2: Uuid,
    pub contradiction_type: String,
    pub severity: String,
    pub explanation: String,
    pub metrics: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

/// Relational representation of an entity spatial trajectory.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, FromRow)]
pub struct SpatialTrajectoryEntity {
    pub trajectory_id: Uuid,
    pub reconstruction_run_id: Uuid,
    pub case_id: Uuid,
    pub entity_id: String,
    pub waypoints: serde_json::Value,
    pub total_distance_km: Option<f64>,
    pub average_speed_kmh: Option<f64>,
    pub max_speed_kmh: Option<f64>,
    pub created_at: DateTime<Utc>,
}

/// Derives a deterministic UUID from a namespace UUID and an arbitrary identifier string using SHA-256.
///
/// Ensures reproducible UUID mapping across disparate exhibits and reconstruction runs without modifying Cargo.lock.
pub fn deterministic_uuid(namespace: &Uuid, identifier: &[u8]) -> Uuid {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(namespace.as_bytes());
    hasher.update(identifier);
    let hash = hasher.finalize();
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&hash[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x50; // Version 5 style
    bytes[8] = (bytes[8] & 0x3f) | 0x80; // Variant RFC 4122
    Uuid::from_bytes(bytes)
}
