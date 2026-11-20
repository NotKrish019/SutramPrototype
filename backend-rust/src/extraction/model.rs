//! Extraction and Correlation Data Transfer Objects (Member 1).
//!
//! Canonical Rust equivalents for the SUTRAM forensic extraction and entity
//! resolution pipeline. Preserves forensic provenance, original observed values,
//! and audit disclaimers.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Execution status of an evidence extraction dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExtractionStatus {
    #[serde(rename = "SUCCESS")]
    Success,
    #[serde(rename = "PARTIAL_SUCCESS")]
    PartialSuccess,
    #[serde(rename = "FAILED")]
    Failed,
}

impl ExtractionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Success => "SUCCESS",
            Self::PartialSuccess => "PARTIAL_SUCCESS",
            Self::Failed => "FAILED",
        }
    }
}

/// Standard forensic integrity disclaimer for resolved technical entities.
pub const FORENSIC_DISCLAIMER: &str =
    "Normalized identifiers represent technical communication addresses or device \
     hardware signatures, and do NOT constitute conclusive legal proof of human identity.";

/// Deterministic entity candidate grouping raw mentions by verified canonical identifier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedEntityCandidate {
    pub canonical_identifier: String,
    pub identifier_type: String,
    #[serde(default)]
    pub raw_mentions: Vec<String>,
    #[serde(default)]
    pub equivalence_reasons: Vec<String>,
    #[serde(default)]
    pub source_evidence_ids: Vec<String>,
    #[serde(default)]
    pub source_record_ids: Vec<String>,
    #[serde(default = "default_disclaimer")]
    pub disclaimer: String,
}

fn default_disclaimer() -> String {
    FORENSIC_DISCLAIMER.to_string()
}

impl ResolvedEntityCandidate {
    pub fn new(
        canonical_identifier: String,
        identifier_type: String,
        raw_mentions: Vec<String>,
        equivalence_reasons: Vec<String>,
        source_evidence_ids: Vec<String>,
        source_record_ids: Vec<String>,
    ) -> Self {
        Self {
            canonical_identifier,
            identifier_type,
            raw_mentions,
            equivalence_reasons,
            source_evidence_ids,
            source_record_ids,
            disclaimer: FORENSIC_DISCLAIMER.to_string(),
        }
    }
}

/// Details of a parsing, schema, or value validation issue encountered during extraction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractionValidationIssue {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record_id: Option<String>,
    pub field_name: String,
    pub issue_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_value: Option<String>,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub row_index: Option<usize>,
}

/// A discrete identifier (Phone, Email, IMEI, IMSI) extracted from evidence text or records.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtractedIdentifier {
    pub identifier_type: String,
    pub raw_value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub normalized_value: Option<String>,
    pub source_evidence_id: String,
    pub source_record_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location_reference: Option<String>,
    #[serde(default)]
    pub is_ambiguous: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ambiguity_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_snippet: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_checksum_valid: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extraction_confidence: Option<f64>,
    #[serde(default = "default_extraction_method")]
    pub extraction_method: String,
}

fn default_extraction_method() -> String {
    "regex_pattern".to_string()
}

/// Provisional temporal event candidate generated directly from an evidence record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtractedEventCandidate {
    pub candidate_id: String,
    pub source_evidence_id: String,
    pub source_record_id: String,
    pub event_type: String,
    pub raw_timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp_precision: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location_name: Option<String>,
    #[serde(default)]
    pub involved_entity_raw_ids: Vec<String>,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

/// Directly observed communication or interaction relationship supported by evidence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProvisionalRelationshipCandidate {
    pub candidate_id: String,
    pub source_evidence_id: String,
    pub source_record_id: String,
    pub source_raw_id: String,
    pub target_raw_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_normalized_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_normalized_id: Option<String>,
    pub relation_type: String,
    #[serde(default = "default_is_observed")]
    pub is_observed: bool,
    #[serde(default)]
    pub metadata: serde_json::Value,
    #[serde(default)]
    pub evidence_basis: Vec<String>,
}

fn default_is_observed() -> bool {
    true
}

/// A validated row extracted from a Call Detail Record (CDR) CSV.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParsedCDRRecord {
    pub source_evidence_id: String,
    pub record_id: String,
    pub caller_phone_raw: String,
    pub receiver_phone_raw: String,
    pub call_timestamp_raw: String,
    pub duration_seconds: f64,
    pub cell_tower_id: Option<String>,
    pub call_type: String,
    pub row_index: usize,
}

/// A validated message extracted from a chat/messaging JSON export.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedChatMessage {
    pub source_evidence_id: String,
    pub message_id: String,
    pub chat_id: Option<String>,
    pub sender_raw_id: String,
    pub sender_name: Option<String>,
    pub recipients_raw: Vec<String>,
    pub timestamp_raw: String,
    pub content_raw: String,
}

/// Complete structured output from CDR CSV extraction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CDRExtractionResult {
    pub source_evidence_id: String,
    pub total_rows_processed: usize,
    pub valid_records: Vec<ParsedCDRRecord>,
    pub rejected_records: Vec<ExtractionValidationIssue>,
    pub identifiers: Vec<ExtractedIdentifier>,
    pub event_candidates: Vec<ExtractedEventCandidate>,
    pub relationship_candidates: Vec<ProvisionalRelationshipCandidate>,
}

/// Complete structured output from chat export JSON extraction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatExtractionResult {
    pub source_evidence_id: String,
    pub chat_id: Option<String>,
    pub total_messages_processed: usize,
    pub valid_messages: Vec<ParsedChatMessage>,
    pub rejected_messages: Vec<ExtractionValidationIssue>,
    pub identifiers: Vec<ExtractedIdentifier>,
    pub event_candidates: Vec<ExtractedEventCandidate>,
    pub relationship_candidates: Vec<ProvisionalRelationshipCandidate>,
}

/// Request DTO to dispatch unified evidence extraction across supported modalities.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceExtractionRequest {
    pub evidence_id: String,
    pub case_id: String,
    pub source_type: String,
    pub file_path: PathBuf,
    pub expected_sha256: String,
    pub source_timezone: Option<String>,
    pub clock_offset_seconds: Option<f64>,
    pub metadata: serde_json::Value,
    pub default_country_prefix: Option<String>,
}

/// Complete, typed result returned by the unified evidence extraction dispatcher.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnifiedExtractionResult {
    pub evidence_id: String,
    pub case_id: String,
    pub source_type: String,
    pub status: ExtractionStatus,
    pub identifiers: Vec<ExtractedIdentifier>,
    pub resolved_entities: Vec<ResolvedEntityCandidate>,
    pub event_candidates: Vec<ExtractedEventCandidate>,
    pub relationship_candidates: Vec<ProvisionalRelationshipCandidate>,
    pub validation_issues: Vec<ExtractionValidationIssue>,
    pub total_records_processed: usize,
    pub valid_records_count: usize,
    pub rejected_records_count: usize,
    pub processing_time_ms: Option<f64>,
    pub extraction_metadata: serde_json::Value,
}

/// Execution lifecycle status of a versioned extraction run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExtractionRunStatus {
    Pending,
    Running,
    Success,
    PartialSuccess,
    Failed,
    Interrupted,
}

impl std::fmt::Display for ExtractionRunStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "PENDING"),
            Self::Running => write!(f, "RUNNING"),
            Self::Success => write!(f, "SUCCESS"),
            Self::PartialSuccess => write!(f, "PARTIAL_SUCCESS"),
            Self::Failed => write!(f, "FAILED"),
            Self::Interrupted => write!(f, "INTERRUPTED"),
        }
    }
}

/// Canonical configuration parameters applied during an extraction attempt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtractionConfiguration {
    pub extractor_name: String,
    pub extractor_version: String,
    pub source_timezone: Option<String>,
    pub clock_offset_seconds: Option<f64>,
    pub default_country_prefix: Option<String>,
    #[serde(default)]
    pub custom_options: serde_json::Value,
}

impl ExtractionConfiguration {
    /// Deterministically computes the SHA-256 hash of canonical extraction configuration.
    pub fn compute_hash(&self) -> String {
        use sha2::{Digest, Sha256};
        let json_bytes = serde_json::to_vec(self).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(&json_bytes);
        hex::encode(hasher.finalize())
    }
}

/// Complete immutable manifest recorded for each discrete extraction attempt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtractionRunManifest {
    pub extraction_run_id: String,
    pub evidence_id: String,
    pub case_id: String,
    pub source_sha256: String,
    pub extractor_name: String,
    pub extractor_version: String,
    pub configuration_hash: String,
    pub configuration: ExtractionConfiguration,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub duration_ms: Option<f64>,
    pub status: ExtractionRunStatus,
    pub validation_issue_count: usize,
    pub records_processed: usize,
    pub valid_records_count: usize,
    pub rejected_records_count: usize,
    pub result_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

/// Lightweight run summary returned in API responses.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtractionRunSummary {
    pub extraction_run_id: String,
    pub evidence_id: String,
    pub case_id: String,
    pub status: ExtractionRunStatus,
    pub records_processed: usize,
    pub valid_records_count: usize,
    pub rejected_records_count: usize,
    pub validation_issue_count: usize,
    pub duration_ms: Option<f64>,
    pub configuration_hash: String,
    pub completed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

impl From<&ExtractionRunManifest> for ExtractionRunSummary {
    fn from(m: &ExtractionRunManifest) -> Self {
        Self {
            extraction_run_id: m.extraction_run_id.clone(),
            evidence_id: m.evidence_id.clone(),
            case_id: m.case_id.clone(),
            status: m.status,
            records_processed: m.records_processed,
            valid_records_count: m.valid_records_count,
            rejected_records_count: m.rejected_records_count,
            validation_issue_count: m.validation_issue_count,
            duration_ms: m.duration_ms,
            configuration_hash: m.configuration_hash.clone(),
            completed_at: m.completed_at.clone(),
            error_message: m.error_message.clone(),
        }
    }
}

/// Pointer identifying the latest accepted successful extraction run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveRunPointer {
    pub evidence_id: String,
    pub case_id: String,
    pub active_run_id: String,
    pub status: ExtractionRunStatus,
    pub activated_at: String,
}

/// Composite response returned when evidence upload is performed with auto_extract=true.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceUploadAndExtractResponse {
    pub receipt: crate::ingestion::EvidenceUploadReceipt,
    pub extraction: ExtractionRunSummary,
}

/// Client request payload for evidence reprocessing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ReprocessRequest {
    pub source_timezone: Option<String>,
    pub clock_offset_seconds: Option<f64>,
    pub default_country_code: Option<String>,
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}
