//! Forensic Evidence Ingestion Module (Member 2 / Rust Backend).
//!
//! Subsystems:
//! - `model`: Evidence receipts, lifecycle enums, and manifest schemas.
//! - `storage`: Path containment, sanitization, and atomic storage operations.
//! - `hashing`: Streaming SHA-256 calculation and post-storage integrity verification.
//! - `validation`: Magic byte sniffing and structural format inspection.
//! - `service`: High-level intake and retrieval coordinator.

pub mod hashing;
pub mod model;
pub mod service;
pub mod storage;
pub mod validation;

pub use model::{EvidenceManifest, EvidenceUploadReceipt, IngestionStatus};
pub use service::EvidenceIngestionService;
pub use storage::StorageManager;
