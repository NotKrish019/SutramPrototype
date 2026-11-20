//! SUTRAM Digital Forensics Rust Backend Service.
//!
//! Production-grade Rust backend foundation providing high-performance, memory-safe,
//! concurrent evidence ingestion, extraction dispatch, and correlation services.

pub mod api;
pub mod app;
pub mod config;
pub mod correlation;
pub mod error;
pub mod extraction;
pub mod graph;
pub mod ingestion;
pub mod observability;
pub mod persistence;
pub mod reconstruction;

pub use app::{
    build_app_state, create_router, create_router_async, create_router_with_state, AppState,
};
pub use config::AppConfig;
pub use correlation::{
    deduplicate_identifiers, generate_cdr_relationship_candidate,
    generate_chat_relationship_candidates, is_valid_indian_mobile_series, normalize_phone_number,
    PhoneNormalizationResult,
};
pub use error::AppError;
pub use extraction::{
    extract_evidence, extract_identifiers_from_text, luhn_checksum_valid, parse_cdr_csv,
    parse_cdr_csv_with_profile, parse_chat_json, ActiveRunPointer, CDRExtractionResult,
    CdrProfile, ChatExtractionResult, EvidenceExtractionRequest, EvidenceExtractionService, EvidenceUploadAndExtractResponse,
    ExtractedEventCandidate, ExtractedIdentifier, ExtractionConfiguration, ExtractionRunManifest,
    ExtractionRunStatus, ExtractionRunSummary, ExtractionStatus, ExtractionValidationIssue,
    ParsedCDRRecord, ParsedChatMessage, ProvisionalRelationshipCandidate, ReprocessRequest,
    ResolvedEntityCandidate, UnifiedExtractionResult, FORENSIC_DISCLAIMER,
};
pub use ingestion::{
    EvidenceIngestionService, EvidenceManifest, EvidenceUploadReceipt, IngestionStatus,
    StorageManager,
};
