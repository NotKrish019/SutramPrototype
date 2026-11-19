//! Unified Forensic Extraction Subsystem (Member 1).

pub mod cdr;
pub mod chat;
pub mod identifiers;
pub mod model;
pub mod service;

pub use cdr::parse_cdr_csv;
pub use chat::parse_chat_json;
pub use identifiers::{extract_identifiers_from_text, luhn_checksum_valid};
pub use model::{
    ActiveRunPointer, CDRExtractionResult, ChatExtractionResult, EvidenceExtractionRequest,
    EvidenceUploadAndExtractResponse, ExtractedEventCandidate, ExtractedIdentifier,
    ExtractionConfiguration, ExtractionRunManifest, ExtractionRunStatus, ExtractionRunSummary,
    ExtractionStatus, ExtractionValidationIssue, ParsedCDRRecord, ParsedChatMessage,
    ProvisionalRelationshipCandidate, ReprocessRequest, ResolvedEntityCandidate,
    UnifiedExtractionResult, FORENSIC_DISCLAIMER,
};
pub use service::{extract_evidence, EvidenceExtractionService};
