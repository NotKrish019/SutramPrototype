//! Unified Forensic Extraction Subsystem (Member 1).

pub mod cdr;
pub mod chat;
pub mod identifiers;
pub mod model;
pub mod service;
pub mod whatsapp;

pub use cdr::{parse_cdr_csv, parse_cdr_csv_with_profile, CdrProfile};
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
pub use service::{consume_video_frames, extract_evidence, EvidenceExtractionService};
pub use whatsapp::{looks_like_whatsapp_txt, parse_whatsapp_txt};
