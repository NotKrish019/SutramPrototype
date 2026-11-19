//! Ingestion Data Transfer Objects and Domain Models.
//!
//! Canonical schemas for evidence upload receipts, lifecycle status enums,
//! and filesystem metadata manifests.

use serde::{Deserialize, Serialize};

/// Lifecycle status of an ingested digital evidence item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IngestionStatus {
    /// File successfully ingested, hashed, validated, and stored.
    Ingested,
    /// Ingestion complete, extraction queued or pending.
    ExtractionPending,
    /// Extraction completed successfully.
    Extracted,
    /// Extraction failed.
    ExtractionFailed,
    /// Integrity check failed on disk.
    Corrupted,
}

impl std::fmt::Display for IngestionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ingested => write!(f, "INGESTED"),
            Self::ExtractionPending => write!(f, "EXTRACTION_PENDING"),
            Self::Extracted => write!(f, "EXTRACTED"),
            Self::ExtractionFailed => write!(f, "EXTRACTION_FAILED"),
            Self::Corrupted => write!(f, "CORRUPTED"),
        }
    }
}

/// Structured receipt returned to investigator clients upon successful evidence intake.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvidenceUploadReceipt {
    /// Canonical UUIDv4 identifying this evidence exhibit.
    pub evidence_id: String,
    /// Target forensic case UUID.
    pub case_id: String,
    /// Original client filename (sanitized).
    pub original_filename: String,
    /// Categorical modality (e.g., DOCUMENT_PDF, FORENSIC_IMAGE, TABULAR_CSV, CCTV_VIDEO, CHAT_EXPORT).
    pub source_type: String,
    /// Content MIME type determined via structural and magic byte inspection.
    pub mime_type: String,
    /// Total file size in bytes written to storage.
    pub file_size_bytes: u64,
    /// Cryptographic SHA-256 digest (lowercase 64-character hexadecimal).
    pub sha256_hash: String,
    /// Safe opaque storage URI (e.g., urn:sutram:evidence:{evidence_id}:storage).
    pub storage_uri: String,
    /// Current forensic ingestion lifecycle status.
    pub status: IngestionStatus,
    /// Preserved acquisition metadata (source timezone, clock offset, investigator notes).
    pub metadata: serde_json::Value,
    /// UTC timestamp formatted per ISO 8601 with trailing 'Z'.
    pub created_at: String,
}

/// Transitional filesystem manifest saved atomically alongside raw evidence bytes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvidenceManifest {
    /// The public evidence receipt.
    pub receipt: EvidenceUploadReceipt,
    /// Case-scoped relative storage path of the raw evidence payload.
    pub relative_raw_path: String,
    /// Exact UTC timestamp when the record was persisted.
    pub stored_at: String,
}
