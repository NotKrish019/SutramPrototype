//! Comprehensive Integration and Parity Tests for Forensic Extraction Engine (Phase 1H).
//!
//! Covers all 25 mandated forensic scenarios:
//! 1. Valid CDR parsing.
//! 2. Malformed CDR rows.
//! 3. Quoted CSV fields.
//! 4. Delimiter handling.
//! 5. Required-header validation.
//! 6. Valid chat JSON.
//! 7. Multiple messages.
//! 8. Malformed chat records.
//! 9. Phone number extraction.
//! 10. Email extraction.
//! 11. IMEI extraction.
//! 12. IMSI extraction.
//! 13. IMEI checksum validity.
//! 14. Ambiguous numeric identifiers.
//! 15. E.164 normalization.
//! 16. Deterministic entity resolution.
//! 17. Relationship generation from observed calls.
//! 18. Relationship generation from observed messages.
//! 19. Provenance preservation.
//! 20. Unicode character offset parity.
//! 21. Extraction confidence remaining separate from checksum validity.
//! 22. Partial extraction.
//! 23. Unsupported evidence formats.
//! 24. Raw evidence remaining unchanged.
//! 25. SHA-256 mismatch rejection.

use std::path::{Path, PathBuf};
use sutram_backend_rust::{
    deduplicate_identifiers, extract_evidence, extract_identifiers_from_text,
    is_valid_indian_mobile_series, luhn_checksum_valid, normalize_phone_number, parse_cdr_csv,
    parse_cdr_csv_with_profile, parse_chat_json, CdrProfile, EvidenceExtractionRequest,
    EvidenceExtractionService, ExtractedIdentifier, ExtractionStatus, StorageManager,
    FORENSIC_DISCLAIMER,
};
use tempfile::tempdir;

fn sample_cdr_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data")
        .join("synthetic")
        .join("sample_cdr.csv")
}

fn sample_chat_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data")
        .join("synthetic")
        .join("sample_chat_export.json")
}

const SAMPLE_CDR_HASH: &str = "c874a7c8b0e05da52db5f29d5bb0f398439237ad69a757cb05bfe9ec76803a8d";
const SAMPLE_CHAT_HASH: &str = "78eaf79b73ec12a15b15f3d26386c3f2456020c914d7c496aa9efeca20fec40e";

// 1. Valid CDR parsing
#[test]
fn test_01_valid_cdr_parsing() {
    let path = sample_cdr_path();
    assert!(path.exists(), "Sample CDR fixture missing: {:?}", path);

    let content = std::fs::read_to_string(&path).unwrap();
    let res = parse_cdr_csv(&content, "ev-cdr-001", Some("91"));

    assert_eq!(res.source_evidence_id, "ev-cdr-001");
    assert_eq!(res.total_rows_processed, 5);
    assert_eq!(res.valid_records.len(), 5);
    assert_eq!(res.rejected_records.len(), 0);

    let first = &res.valid_records[0];
    assert_eq!(first.record_id, "cdr-rec-001");
    assert_eq!(first.caller_phone_raw, "+919811002233");
    assert_eq!(first.receiver_phone_raw, "+919899887766");
    assert_eq!(first.call_timestamp_raw, "2026-11-10T21:45:12+05:30");
    assert_eq!(first.duration_seconds, 184.0);
    assert_eq!(
        first.cell_tower_id.as_deref(),
        Some("TOWER-DELHI-SOUTH-04B")
    );
    assert_eq!(first.call_type, "VOICE_OUTGOING");
    assert_eq!(first.row_index, 2);

    assert_eq!(res.event_candidates.len(), 5);
    assert_eq!(res.relationship_candidates.len(), 5);

    // End-to-end dispatch test
    let req = EvidenceExtractionRequest {
        evidence_id: "ev-cdr-001".to_string(),
        case_id: "case-001".to_string(),
        source_type: "CALL_DETAIL_RECORD".to_string(),
        file_path: path,
        expected_sha256: SAMPLE_CDR_HASH.to_string(),
        source_timezone: Some("Asia/Kolkata".to_string()),
        clock_offset_seconds: None,
        metadata: serde_json::json!({ "carrier": "Airtel" }),
        default_country_prefix: Some("91".to_string()),
    };
    let dispatch_res = extract_evidence(&req);
    assert_eq!(dispatch_res.status, ExtractionStatus::Success);
    assert_eq!(dispatch_res.valid_records_count, 5);
}

// 2. Malformed CDR rows
#[test]
fn test_02_malformed_cdr_rows() {
    let csv_content = "\
record_id,caller_phone,receiver_phone,call_timestamp,duration_seconds,cell_tower_id\n\
rec-01,+919811002233,+919899887766,2026-11-10T12:00:00Z,60,TOWER-1\n\
,+919811002233,+919899887766,2026-11-10T12:05:00Z,30,TOWER-1\n\
rec-03,,+919899887766,2026-11-10T12:10:00Z,45,TOWER-1\n\
rec-04,+919811002233,+919899887766,2026-11-10T12:15:00Z,-15,TOWER-1\n\
rec-05,+919811002233,+919899887766,,120,TOWER-1\n\
rec-06,+919811002233,+919899887766,2026-11-10T12:25:00Z,invalid_dur,TOWER-1\n";

    let res = parse_cdr_csv(csv_content, "ev-cdr-err", None);
    assert_eq!(res.total_rows_processed, 6);
    assert_eq!(res.valid_records.len(), 1);
    assert_eq!(res.valid_records[0].record_id, "rec-01");

    assert_eq!(res.rejected_records.len(), 5);
    let fields: Vec<&str> = res
        .rejected_records
        .iter()
        .map(|r| r.field_name.as_str())
        .collect();
    assert!(fields.contains(&"record_id"));
    assert!(fields.contains(&"caller_phone"));
    assert!(fields.contains(&"duration_seconds"));
    assert!(fields.contains(&"call_timestamp"));
}

// 3. Quoted CSV fields
#[test]
fn test_03_quoted_csv_fields() {
    let csv_content = "\
\"record_id\",\"caller_phone\",\"receiver_phone\",\"call_timestamp\",\"duration_seconds\",\"cell_tower_id\"\n\
\"rec,001\",\"+919811002233\",\"+919899887766\",\"2026-11-10T12:00:00Z\",\"120.5\",\"TOWER-A, NORTH\"\n";

    let res = parse_cdr_csv(csv_content, "ev-quoted", None);
    assert_eq!(res.valid_records.len(), 1);
    let rec = &res.valid_records[0];
    assert_eq!(rec.record_id, "rec,001");
    assert_eq!(rec.duration_seconds, 120.5);
    assert_eq!(rec.cell_tower_id.as_deref(), Some("TOWER-A, NORTH"));
}

// 4. Delimiter and whitespace handling
#[test]
fn test_04_delimiter_and_whitespace_handling() {
    let csv_content = "\
record_id , caller_phone , receiver_phone , call_timestamp , duration_seconds \r\n\
rec-001 , +919811002233 , +919899887766 , 2026-11-10T12:00:00Z , 45 \r\n\
\r\n\
rec-002 , +919899887766 , +919811002233 , 2026-11-10T12:05:00Z , 90 \r\n";

    let res = parse_cdr_csv(csv_content, "ev-ws", None);
    assert_eq!(res.valid_records.len(), 2);
    assert_eq!(res.valid_records[0].caller_phone_raw, "+919811002233");
    assert_eq!(res.valid_records[1].duration_seconds, 90.0);
}

// 5. Required-header validation
#[test]
fn test_05_required_header_validation() {
    let csv_content = "id,source,dest,time\n1,123,456,2026-11-10\n";
    let res = parse_cdr_csv(csv_content, "ev-hdr-fail", None);

    assert_eq!(res.valid_records.len(), 0);
    assert_eq!(res.rejected_records.len(), 1);
    let issue = &res.rejected_records[0];
    assert_eq!(issue.issue_type, "MISSING_REQUIRED_HEADERS");
    assert!(issue.message.contains("CSV missing mandatory columns"));
}

// 6. Valid chat JSON
#[test]
fn test_06_valid_chat_json() {
    let path = sample_chat_path();
    assert!(path.exists(), "Sample chat fixture missing: {:?}", path);

    let content = std::fs::read_to_string(&path).unwrap();
    let res = parse_chat_json(&content, "ev-chat-001", Some("91"));

    assert_eq!(res.source_evidence_id, "ev-chat-001");
    assert_eq!(res.chat_id.as_deref(), Some("chat-grp-synthetic-901"));
    assert_eq!(res.total_messages_processed, 3);
    assert_eq!(res.valid_messages.len(), 3);
    assert_eq!(res.rejected_messages.len(), 0);

    let m1 = &res.valid_messages[0];
    assert_eq!(m1.message_id, "msg-001");
    assert_eq!(m1.sender_raw_id, "+919811002233");
    assert_eq!(m1.sender_name.as_deref(), Some("Synthetic Target Alpha"));
    assert_eq!(m1.recipients_raw, vec!["+919899887766"]);
    assert_eq!(m1.timestamp_raw, "2026-11-10T20:15:30+05:30");

    // End-to-end dispatch test
    let req = EvidenceExtractionRequest {
        evidence_id: "ev-chat-001".to_string(),
        case_id: "case-001".to_string(),
        source_type: "CHAT_EXPORT".to_string(),
        file_path: path,
        expected_sha256: SAMPLE_CHAT_HASH.to_string(),
        source_timezone: Some("Asia/Kolkata".to_string()),
        clock_offset_seconds: None,
        metadata: serde_json::json!({ "platform": "WhatsApp" }),
        default_country_prefix: Some("91".to_string()),
    };
    let dispatch_res = extract_evidence(&req);
    assert_eq!(dispatch_res.status, ExtractionStatus::Success);
    assert_eq!(dispatch_res.valid_records_count, 3);
}

// 7. Multiple messages and 1-to-many recipients
#[test]
fn test_07_multiple_messages_and_recipients() {
    let path = sample_chat_path();
    let content = std::fs::read_to_string(&path).unwrap();
    let res = parse_chat_json(&content, "ev-chat-001", Some("91"));

    // msg-001 has 1 recipient, msg-002 has 1 recipient, msg-003 has 2 recipients -> total 4 relationships
    assert_eq!(res.relationship_candidates.len(), 4);

    let rel_delta_1 = &res.relationship_candidates[2];
    assert_eq!(rel_delta_1.source_raw_id, "+919876543210");
    assert_eq!(rel_delta_1.target_raw_id, "+919811002233");

    let rel_delta_2 = &res.relationship_candidates[3];
    assert_eq!(rel_delta_2.source_raw_id, "+919876543210");
    assert_eq!(rel_delta_2.target_raw_id, "+919899887766");
}

// 8. Malformed chat records
#[test]
fn test_08_malformed_chat_records() {
    let json_content = r#"{
        "chat_id": "test-chat",
        "messages": [
            {"message_id": "m1", "sender_id": "s1", "timestamp": "t1", "content": "hello"},
            {"message_id": "m2", "timestamp": "t2", "content": "no sender"},
            {"sender_id": "s3", "timestamp": "t3", "content": "no message_id"},
            {"message_id": "m4", "sender_id": "s4", "content": "no timestamp"},
            {"message_id": "m5", "sender_id": "s5", "timestamp": "t5", "content": 12345}
        ]
    }"#;

    let res = parse_chat_json(json_content, "ev-chat-bad", None);
    assert_eq!(res.total_messages_processed, 5);
    assert_eq!(res.valid_messages.len(), 1);
    assert_eq!(res.valid_messages[0].message_id, "m1");
    assert_eq!(res.rejected_messages.len(), 4);
}

// 9. Phone number extraction
#[test]
fn test_09_phone_number_extraction() {
    let text = "Call target at +919811002233 or local line 09899887766 immediately.";
    let idents = extract_identifiers_from_text(text, "ev-01", "rec-01", None);

    let phones: Vec<&ExtractedIdentifier> = idents
        .iter()
        .filter(|i| i.identifier_type == "PHONE_NUMBER")
        .collect();
    assert_eq!(phones.len(), 2);
    let raw_phones: Vec<&str> = phones.iter().map(|p| p.raw_value.as_str()).collect();
    assert!(raw_phones.contains(&"+919811002233"));
    assert!(raw_phones.contains(&"09899887766"));
}

// 10. Email extraction
#[test]
fn test_10_email_extraction() {
    let text = "Send reports to forensic.analyst@cybercell.gov.in and lead.detective@agency.org.";
    let idents = extract_identifiers_from_text(text, "ev-01", "rec-01", None);

    let emails: Vec<&ExtractedIdentifier> = idents
        .iter()
        .filter(|i| i.identifier_type == "EMAIL")
        .collect();
    assert_eq!(emails.len(), 2);
    assert_eq!(emails[0].raw_value, "forensic.analyst@cybercell.gov.in");
    assert_eq!(
        emails[0].normalized_value.as_deref(),
        Some("forensic.analyst@cybercell.gov.in")
    );
    assert_eq!(
        emails[0].location_reference.as_deref(),
        Some("char_offset:16-49")
    );
    assert_eq!(emails[1].raw_value, "lead.detective@agency.org");
}

// 11. IMEI extraction
#[test]
fn test_11_imei_extraction() {
    let text = "Seized handset IMEI: 861234567890123 from suspect vehicle.";
    let idents = extract_identifiers_from_text(text, "ev-01", "rec-01", None);

    let imeis: Vec<&ExtractedIdentifier> = idents
        .iter()
        .filter(|i| i.identifier_type == "IMEI")
        .collect();
    assert_eq!(imeis.len(), 1);
    assert_eq!(imeis[0].raw_value, "861234567890123");
    assert!(!imeis[0].is_ambiguous);
}

// 12. IMSI extraction
#[test]
fn test_12_imsi_extraction() {
    let text = "Carrier SIM IMSI is 404450123456789 active.";
    let idents = extract_identifiers_from_text(text, "ev-01", "rec-01", None);

    let imsis: Vec<&ExtractedIdentifier> = idents
        .iter()
        .filter(|i| i.identifier_type == "IMSI")
        .collect();
    assert_eq!(imsis.len(), 1);
    assert_eq!(imsis[0].raw_value, "404450123456789");
    assert_eq!(imsis[0].is_checksum_valid, None);
    assert!(!imsis[0].is_ambiguous);
}

// 13. IMEI checksum validity
#[test]
fn test_13_imei_checksum_validity() {
    assert!(luhn_checksum_valid("352099001761481"));
    assert!(!luhn_checksum_valid("352099001761482"));
    assert!(!luhn_checksum_valid("12345"));

    let text_valid = "Device IMEI: 352099001761481";
    let text_invalid = "Device IMEI: 352099001761482";

    let idents_v = extract_identifiers_from_text(text_valid, "ev-01", "r1", None);
    let idents_inv = extract_identifiers_from_text(text_invalid, "ev-01", "r2", None);

    assert_eq!(idents_v[0].is_checksum_valid, Some(true));
    assert_eq!(idents_inv[0].is_checksum_valid, Some(false));
}

// 14. Ambiguous numeric identifiers
#[test]
fn test_14_ambiguous_numeric_identifiers() {
    let text_isolated = "Reference code 998877665544332 noted in ledger.";
    let idents1 = extract_identifiers_from_text(text_isolated, "ev-01", "r1", None);
    let amb1 = &idents1[0];
    assert_eq!(amb1.identifier_type, "AMBIGUOUS_NUMERIC_15");
    assert!(amb1.is_ambiguous);
    assert!(amb1.ambiguity_reason.as_ref().unwrap().contains("Isolated"));

    let text_conflict = "Device IMEI or SIM IMSI: 861234567890123.";
    let idents2 = extract_identifiers_from_text(text_conflict, "ev-01", "r2", None);
    let amb2 = &idents2[0];
    assert!(amb2.is_ambiguous);
    assert!(amb2
        .ambiguity_reason
        .as_ref()
        .unwrap()
        .contains("Conflicting"));
}

// 15. E.164 normalization
#[test]
fn test_15_e164_normalization() {
    // Explicit +
    let r1 = normalize_phone_number("+919811002233", None);
    assert!(r1.is_normalized);
    assert_eq!(r1.normalized_e164.as_deref(), Some("+919811002233"));

    // Trunk 00
    let r2 = normalize_phone_number("00919811002233", None);
    assert!(r2.is_normalized);
    assert_eq!(r2.normalized_e164.as_deref(), Some("+919811002233"));

    // Domestic trunk 0 with prefix 91
    let r3 = normalize_phone_number("09811002233", Some("91"));
    assert!(r3.is_normalized);
    assert_eq!(r3.normalized_e164.as_deref(), Some("+919811002233"));

    // Domestic 10-digit without prefix -> ambiguous
    let r4 = normalize_phone_number("09811002233", None);
    assert!(!r4.is_normalized);
    assert_eq!(r4.resolution_status, "AMBIGUOUS_COUNTRY_CODE");
}

// 16. Deterministic entity resolution
#[test]
fn test_16_deterministic_entity_resolution() {
    let idents = vec![
        ExtractedIdentifier {
            identifier_type: "PHONE_NUMBER".to_string(),
            raw_value: "+919811002233".to_string(),
            normalized_value: None,
            source_evidence_id: "ev-01".to_string(),
            source_record_id: "cdr-01".to_string(),
            location_reference: None,
            is_ambiguous: false,
            ambiguity_reason: None,
            context_snippet: None,
            is_checksum_valid: None,
            extraction_confidence: None,
            extraction_method: "test".to_string(),
        },
        ExtractedIdentifier {
            identifier_type: "PHONE_NUMBER".to_string(),
            raw_value: "09811002233".to_string(),
            normalized_value: None,
            source_evidence_id: "ev-02".to_string(),
            source_record_id: "chat-01".to_string(),
            location_reference: None,
            is_ambiguous: false,
            ambiguity_reason: None,
            context_snippet: None,
            is_checksum_valid: None,
            extraction_confidence: None,
            extraction_method: "test".to_string(),
        },
        ExtractedIdentifier {
            identifier_type: "EMAIL".to_string(),
            raw_value: "Alpha.Lead@Synthetic-Forensics.Internal".to_string(),
            normalized_value: None,
            source_evidence_id: "ev-02".to_string(),
            source_record_id: "chat-01".to_string(),
            location_reference: None,
            is_ambiguous: false,
            ambiguity_reason: None,
            context_snippet: None,
            is_checksum_valid: None,
            extraction_confidence: None,
            extraction_method: "test".to_string(),
        },
    ];

    let entities = deduplicate_identifiers(&idents, Some("91"));
    assert_eq!(entities.len(), 2);

    let phone_entity = entities
        .iter()
        .find(|e| e.identifier_type == "PHONE_NUMBER")
        .unwrap();
    assert_eq!(phone_entity.canonical_identifier, "+919811002233");
    assert_eq!(phone_entity.raw_mentions.len(), 2);
    assert_eq!(phone_entity.disclaimer, FORENSIC_DISCLAIMER);

    let email_entity = entities
        .iter()
        .find(|e| e.identifier_type == "EMAIL")
        .unwrap();
    assert_eq!(
        email_entity.canonical_identifier,
        "alpha.lead@synthetic-forensics.internal"
    );
}

// 17. Relationship generation from observed calls
#[test]
fn test_17_relationship_generation_from_observed_calls() {
    let path = sample_cdr_path();
    let content = std::fs::read_to_string(&path).unwrap();
    let res = parse_cdr_csv(&content, "ev-cdr-001", Some("91"));

    let rel0 = &res.relationship_candidates[0];
    assert_eq!(rel0.relation_type, "CALL_COMMUNICATION");
    assert!(rel0.is_observed);
    assert_eq!(rel0.source_raw_id, "+919811002233");
    assert_eq!(rel0.target_raw_id, "+919899887766");
    assert_eq!(rel0.source_normalized_id.as_deref(), Some("+919811002233"));
    assert_eq!(rel0.target_normalized_id.as_deref(), Some("+919899887766"));
    assert_eq!(rel0.evidence_basis, vec!["ev-cdr-001"]);
}

// 18. Relationship generation from observed messages
#[test]
fn test_18_relationship_generation_from_observed_messages() {
    let path = sample_chat_path();
    let content = std::fs::read_to_string(&path).unwrap();
    let res = parse_chat_json(&content, "ev-chat-001", Some("91"));

    let rel0 = &res.relationship_candidates[0];
    assert_eq!(rel0.relation_type, "MESSAGE_COMMUNICATION");
    assert!(rel0.is_observed);
    assert_eq!(rel0.source_raw_id, "+919811002233");
    assert_eq!(rel0.target_raw_id, "+919899887766");
    assert_eq!(rel0.evidence_basis, vec!["ev-chat-001"]);
}

// 19. Provenance preservation
#[test]
fn test_19_provenance_preservation() {
    let path = sample_cdr_path();
    let content = std::fs::read_to_string(&path).unwrap();
    let res = parse_cdr_csv(&content, "ev-prov-test", Some("91"));

    for ident in &res.identifiers {
        assert_eq!(ident.source_evidence_id, "ev-prov-test");
        assert!(!ident.source_record_id.is_empty());
        assert!(ident.location_reference.is_some());
    }

    for evt in &res.event_candidates {
        assert_eq!(evt.source_evidence_id, "ev-prov-test");
        assert!(!evt.source_record_id.is_empty());
    }

    for rel in &res.relationship_candidates {
        assert_eq!(rel.source_evidence_id, "ev-prov-test");
        assert!(!rel.source_record_id.is_empty());
    }
}

// 20. Unicode character offset parity
#[test]
fn test_20_unicode_character_offset_parity() {
    // Non-ASCII text with Hindi characters (3 bytes per character in UTF-8)
    let text = "नमस्ते! Contact lead at user@cybercell.gov.in or +919811002233.";
    let idents = extract_identifiers_from_text(text, "ev-unicode", "msg-uni-01", None);

    let email = idents
        .iter()
        .find(|i| i.identifier_type == "EMAIL")
        .unwrap();
    assert_eq!(email.raw_value, "user@cybercell.gov.in");
    // In Python: text.find("user@cybercell.gov.in") is 24, end is 45
    assert_eq!(
        email.location_reference.as_deref(),
        Some("char_offset:24-45")
    );

    let phone = idents
        .iter()
        .find(|i| i.identifier_type == "PHONE_NUMBER")
        .unwrap();
    assert_eq!(phone.raw_value, "+919811002233");
    // In Python: text.find("+919811002233") is 49, end is 62
    assert_eq!(
        phone.location_reference.as_deref(),
        Some("char_offset:49-62")
    );
}

// 21. Extraction confidence remaining separate from checksum validity
#[test]
fn test_21_confidence_separate_from_checksum() {
    let text = "Terminal IMEI 352099001761481 and handset IMEI 352099001761482.";
    let idents = extract_identifiers_from_text(text, "ev-01", "rec-01", None);

    let imeis: Vec<&ExtractedIdentifier> = idents
        .iter()
        .filter(|i| i.identifier_type == "IMEI")
        .collect();
    assert_eq!(imeis.len(), 2);

    // One valid Luhn, one invalid Luhn
    assert_eq!(imeis[0].is_checksum_valid, Some(true));
    assert_eq!(imeis[1].is_checksum_valid, Some(false));

    // Both have extraction_confidence = None (deterministic regex rules do not fabricate statistical confidence)
    assert_eq!(imeis[0].extraction_confidence, None);
    assert_eq!(imeis[1].extraction_confidence, None);
}

// 22. Partial extraction
#[test]
fn test_22_partial_extraction() {
    let temp = tempdir().unwrap();
    let csv_path = temp.path().join("partial_cdr.csv");
    let content = "\
record_id,caller_phone,receiver_phone,call_timestamp,duration_seconds\n\
rec-01,+919811002233,+919899887766,2026-11-10T12:00:00Z,60\n\
rec-02,+919811002233,+919899887766,2026-11-10T12:05:00Z,-10\n"; // invalid duration
    std::fs::write(&csv_path, content).unwrap();

    use sha2::{Digest, Sha256};
    let hash = hex::encode(Sha256::digest(content.as_bytes()));

    let req = EvidenceExtractionRequest {
        evidence_id: "ev-partial".to_string(),
        case_id: "case-01".to_string(),
        source_type: "CALL_DETAIL_RECORD".to_string(),
        file_path: csv_path,
        expected_sha256: hash,
        source_timezone: None,
        clock_offset_seconds: None,
        metadata: serde_json::json!({}),
        default_country_prefix: Some("91".to_string()),
    };

    let result = extract_evidence(&req);
    assert_eq!(result.status, ExtractionStatus::PartialSuccess);
    assert_eq!(result.valid_records_count, 1);
    assert_eq!(result.rejected_records_count, 1);
    assert_eq!(result.validation_issues.len(), 1);
}

// 23. Unsupported evidence formats
#[test]
fn test_23_unsupported_evidence_formats() {
    let temp = tempdir().unwrap();
    let pdf_path = temp.path().join("test.pdf");
    std::fs::write(&pdf_path, b"%PDF-1.4\n1 0 obj<<>>endobj\n").unwrap();

    use sha2::{Digest, Sha256};
    let hash = hex::encode(Sha256::digest(b"%PDF-1.4\n1 0 obj<<>>endobj\n"));

    let req = EvidenceExtractionRequest {
        evidence_id: "ev-pdf".to_string(),
        case_id: "case-01".to_string(),
        source_type: "DOCUMENT_PDF".to_string(),
        file_path: pdf_path,
        expected_sha256: hash,
        source_timezone: None,
        clock_offset_seconds: None,
        metadata: serde_json::json!({}),
        default_country_prefix: None,
    };

    let result = extract_evidence(&req);
    assert_eq!(result.status, ExtractionStatus::Failed);
    assert_eq!(result.validation_issues.len(), 1);
    assert_eq!(
        result.validation_issues[0].issue_type,
        "UNSUPPORTED_SOURCE_TYPE"
    );
}

// 24. Raw evidence remaining unchanged
#[test]
fn test_24_raw_evidence_remains_unchanged() {
    let temp = tempdir().unwrap();
    let file_path = temp.path().join("immutable.csv");
    let content = "record_id,caller_phone,receiver_phone,call_timestamp,duration_seconds\nrec-1,+919811002233,+919899887766,2026-11-10T12:00:00Z,60\n";
    std::fs::write(&file_path, content).unwrap();

    let bytes_before = std::fs::read(&file_path).unwrap();
    let mtime_before = std::fs::metadata(&file_path).unwrap().modified().unwrap();

    use sha2::{Digest, Sha256};
    let hash = hex::encode(Sha256::digest(&bytes_before));

    let req = EvidenceExtractionRequest {
        evidence_id: "ev-immutable".to_string(),
        case_id: "case-01".to_string(),
        source_type: "CALL_DETAIL_RECORD".to_string(),
        file_path: file_path.clone(),
        expected_sha256: hash,
        source_timezone: None,
        clock_offset_seconds: None,
        metadata: serde_json::json!({}),
        default_country_prefix: None,
    };

    let result = extract_evidence(&req);
    assert_eq!(result.status, ExtractionStatus::Success);

    let bytes_after = std::fs::read(&file_path).unwrap();
    let mtime_after = std::fs::metadata(&file_path).unwrap().modified().unwrap();

    assert_eq!(bytes_before, bytes_after);
    assert_eq!(mtime_before, mtime_after);
}

// 25. SHA-256 mismatch rejection
#[test]
fn test_25_sha256_mismatch_rejection() {
    let temp = tempdir().unwrap();
    let file_path = temp.path().join("tampered.csv");
    std::fs::write(&file_path, "record_id,caller_phone,receiver_phone,call_timestamp,duration_seconds\nrec-1,+919811002233,+919899887766,2026-11-10T12:00:00Z,60\n").unwrap();

    let req = EvidenceExtractionRequest {
        evidence_id: "ev-tampered".to_string(),
        case_id: "case-01".to_string(),
        source_type: "CALL_DETAIL_RECORD".to_string(),
        file_path,
        expected_sha256: "0000000000000000000000000000000000000000000000000000000000000000"
            .to_string(),
        source_timezone: None,
        clock_offset_seconds: None,
        metadata: serde_json::json!({}),
        default_country_prefix: None,
    };

    let result = extract_evidence(&req);
    assert_eq!(result.status, ExtractionStatus::Failed);
    assert_eq!(result.validation_issues.len(), 1);
    assert_eq!(result.validation_issues[0].issue_type, "HASH_MISMATCH");
    assert_eq!(result.valid_records_count, 0);
}

// Ingestion manifest builder test
#[test]
fn test_manifest_builder_integration() {
    let temp = tempdir().unwrap();
    let storage = StorageManager::new(temp.path().to_path_buf());

    let case_id = "550e8400-e29b-41d4-a716-446655440000";
    let evidence_id = "11111111-2222-3333-4444-555555555555";

    let exhibit_dir = temp.path().join(case_id).join(evidence_id);
    std::fs::create_dir_all(&exhibit_dir).unwrap();
    let raw_file = exhibit_dir.join(format!("{}.raw", evidence_id));
    std::fs::write(&raw_file, b"test content").unwrap();

    let receipt = sutram_backend_rust::EvidenceUploadReceipt {
        evidence_id: evidence_id.to_string(),
        case_id: case_id.to_string(),
        original_filename: "test.csv".to_string(),
        source_type: "CALL_DETAIL_RECORD".to_string(),
        mime_type: "text/csv".to_string(),
        file_size_bytes: 12,
        sha256_hash: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_string(),
        storage_uri: format!("urn:sutram:evidence:{}:storage", evidence_id),
        status: sutram_backend_rust::IngestionStatus::Ingested,
        metadata: serde_json::json!({ "source_timezone": "Asia/Kolkata", "clock_offset_seconds": 1.5 }),
        created_at: "2026-11-18T12:00:00Z".to_string(),
    };

    let manifest = sutram_backend_rust::EvidenceManifest {
        receipt,
        relative_raw_path: format!("{}/{}.raw", evidence_id, evidence_id),
        stored_at: "2026-11-18T12:00:00Z".to_string(),
    };

    let req = EvidenceExtractionService::build_request_from_manifest(
        &manifest,
        &storage,
        Some("91".to_string()),
    )
    .unwrap();

    assert_eq!(req.evidence_id, evidence_id);
    assert_eq!(req.case_id, case_id);
    assert_eq!(req.file_path, raw_file);
    assert_eq!(req.source_timezone.as_deref(), Some("Asia/Kolkata"));
    assert_eq!(req.clock_offset_seconds, Some(1.5));
    assert_eq!(req.default_country_prefix.as_deref(), Some("91"));
}

// =========================================================================
// PHASE 6: Bounded India-Focused Evidence Normalization Tests
// =========================================================================

// 26. Valid Indian numbers in standard formats (+91, 91, 091, 0, plain 10-digit)
#[test]
fn test_phase6_indian_phone_standard_formats() {
    // 1. Explicit +91
    let r1 = normalize_phone_number("+919811002233", None);
    assert!(r1.is_normalized);
    assert_eq!(r1.normalized_e164.as_deref(), Some("+919811002233"));
    assert_eq!(r1.country_prefix_applied.as_deref(), Some("91"));

    // 2. 91 country code without + (12 digits)
    let r2 = normalize_phone_number("919811002233", None);
    assert!(r2.is_normalized);
    assert_eq!(r2.normalized_e164.as_deref(), Some("+919811002233"));
    assert_eq!(r2.resolution_status, "INDIAN_COUNTRY_CODE_PREFIX");

    // 3. 091 prefix (13 digits: domestic trunk 0 + 91 + 10 digits)
    let r3 = normalize_phone_number("0919811002233", None);
    assert!(r3.is_normalized);
    assert_eq!(r3.normalized_e164.as_deref(), Some("+919811002233"));
    assert_eq!(r3.resolution_status, "INDIAN_TRUNK_091_PREFIX");

    // 4. Domestic 0 trunk prefix with configured default prefix 91 (11 digits)
    let r4 = normalize_phone_number("09811002233", Some("91"));
    assert!(r4.is_normalized);
    assert_eq!(r4.normalized_e164.as_deref(), Some("+919811002233"));
    assert_eq!(r4.resolution_status, "CONFIGURED_DEFAULT_COUNTRY_PREFIX");

    // 5. Plain 10-digit mobile with configured default prefix 91
    let r5 = normalize_phone_number("9811002233", Some("91"));
    assert!(r5.is_normalized);
    assert_eq!(r5.normalized_e164.as_deref(), Some("+919811002233"));

    // 6. Test all valid Indian mobile series: 6, 7, 8, 9
    assert!(is_valid_indian_mobile_series("6301234567"));
    assert!(is_valid_indian_mobile_series("7890123456"));
    assert!(is_valid_indian_mobile_series("8765432109"));
    assert!(is_valid_indian_mobile_series("9876543210"));

    let r_6 = normalize_phone_number("+916301234567", None);
    assert!(r_6.is_normalized);
    assert_eq!(r_6.normalized_e164.as_deref(), Some("+916301234567"));

    let r_7 = normalize_phone_number("+917890123456", None);
    assert!(r_7.is_normalized);
    assert_eq!(r_7.normalized_e164.as_deref(), Some("+917890123456"));

    let r_8 = normalize_phone_number("+918765432109", None);
    assert!(r_8.is_normalized);
    assert_eq!(r_8.normalized_e164.as_deref(), Some("+918765432109"));
}

// 27. Numbers with formatting artifacts (spaces, hyphens, parentheses, dots)
#[test]
fn test_phase6_formatting_artifacts() {
    let raw_cases = vec![
        "+91 98110 02233",
        "+91 (0) 98110-02233",
        "+91-98110-02233",
        "+91.98110.02233",
        "(091) 98110 02233",
        "91-98110-02233",
        "+91 (98110) 02233",
        "+91/98110/02233",
    ];

    for raw in raw_cases {
        let res = normalize_phone_number(raw, None);
        assert!(
            res.is_normalized,
            "Failed to normalize formatted number: '{}', reason: {}",
            raw, res.reason
        );
        assert_eq!(
            res.normalized_e164.as_deref(),
            Some("+919811002233"),
            "Mismatch for raw '{}'",
            raw
        );
        assert_eq!(res.raw_input, raw, "Raw input must be preserved verbatim");
    }

    // Formatting with default prefix 91
    let domestic_cases = vec![
        "098110-02233",
        "0 98110 02233",
        "98110-02233",
        "(98110) 02233",
        "98110.02233",
    ];
    for raw in domestic_cases {
        let res = normalize_phone_number(raw, Some("91"));
        assert!(
            res.is_normalized,
            "Failed to normalize domestic number: '{}'",
            raw
        );
        assert_eq!(res.normalized_e164.as_deref(), Some("+919811002233"));
        assert_eq!(res.raw_input, raw);
    }
}

// 28. Invalid numbers (wrong length, invalid series starting with 0-5)
#[test]
fn test_phase6_invalid_indian_numbers() {
    // Landline numbers or invalid mobile series starting with 0-5
    assert!(!is_valid_indian_mobile_series("5555555555"));
    assert!(!is_valid_indian_mobile_series("1234567890"));
    assert!(!is_valid_indian_mobile_series("0112345678")); // Delhi landline
    assert!(!is_valid_indian_mobile_series("2334455667")); // PSTN

    // Explicit +91 with invalid series
    let r1 = normalize_phone_number("+91 5555555555", None);
    assert!(!r1.is_normalized);
    assert_eq!(r1.normalized_e164, None);
    assert_eq!(r1.resolution_status, "INVALID_INDIAN_MOBILE_SERIES");

    // Domestic 10-digit with invalid series under 91 prefix
    let r2 = normalize_phone_number("1234567890", Some("91"));
    assert!(!r2.is_normalized);
    assert_eq!(r2.normalized_e164, None);
    assert_eq!(r2.resolution_status, "INVALID_INDIAN_MOBILE_SERIES");

    // 091 prefix with invalid series
    let r3 = normalize_phone_number("091 5555555555", None);
    assert!(!r3.is_normalized);
    assert_eq!(r3.normalized_e164, None);
    assert_eq!(r3.resolution_status, "INVALID_INDIAN_MOBILE_SERIES");

    // 91 prefix without + with invalid series
    let r4 = normalize_phone_number("915555555555", None);
    assert!(!r4.is_normalized);
    assert_eq!(r4.normalized_e164, None);
    assert_eq!(r4.resolution_status, "INVALID_INDIAN_MOBILE_SERIES");

    // Wrong lengths for Indian numbers
    let r5 = normalize_phone_number("+91 98110", None); // Too short
    assert!(!r5.is_normalized);
    assert_eq!(r5.resolution_status, "INVALID_INDIAN_PHONE_LENGTH");

    let r6 = normalize_phone_number("+91 98110022334455", None); // Too long
    assert!(!r6.is_normalized);
    assert_eq!(r6.resolution_status, "EXCESSIVE_DIGITS");
}

// 29. Ambiguous cases that should not be merged
#[test]
fn test_phase6_ambiguous_cases_not_merged() {
    // 10-digit without default prefix -> ambiguous country code
    let r1 = normalize_phone_number("9811002233", None);
    assert!(!r1.is_normalized);
    assert_eq!(r1.resolution_status, "AMBIGUOUS_COUNTRY_CODE");
    assert_eq!(r1.normalized_e164, None);

    let r2 = normalize_phone_number("9822003344", None);
    assert!(!r2.is_normalized);
    assert_eq!(r2.resolution_status, "AMBIGUOUS_COUNTRY_CODE");

    // Verify deduplicate_identifiers keeps ambiguous unnormalized numbers strictly isolated
    let idents = vec![
        ExtractedIdentifier {
            identifier_type: "PHONE_NUMBER".to_string(),
            raw_value: "9811002233".to_string(),
            normalized_value: None,
            source_evidence_id: "ev-01".to_string(),
            source_record_id: "rec-01".to_string(),
            location_reference: None,
            is_ambiguous: true,
            ambiguity_reason: Some("Ambiguous country code".to_string()),
            context_snippet: None,
            is_checksum_valid: None,
            extraction_confidence: None,
            extraction_method: "test".to_string(),
        },
        ExtractedIdentifier {
            identifier_type: "PHONE_NUMBER".to_string(),
            raw_value: "9822003344".to_string(),
            normalized_value: None,
            source_evidence_id: "ev-02".to_string(),
            source_record_id: "rec-02".to_string(),
            location_reference: None,
            is_ambiguous: true,
            ambiguity_reason: Some("Ambiguous country code".to_string()),
            context_snippet: None,
            is_checksum_valid: None,
            extraction_confidence: None,
            extraction_method: "test".to_string(),
        },
    ];

    // Deduplicate without default country prefix
    let resolved = deduplicate_identifiers(&idents, None);
    assert_eq!(
        resolved.len(),
        2,
        "Ambiguous unnormalized numbers must remain in distinct entity groups without merging"
    );
}

// 30. International numbers ensuring no regression
#[test]
fn test_phase6_international_numbers_no_regression() {
    // US number with explicit +
    let r1 = normalize_phone_number("+1 555 123 4567", None);
    assert!(r1.is_normalized);
    assert_eq!(r1.normalized_e164.as_deref(), Some("+15551234567"));

    // UK number with explicit +
    let r2 = normalize_phone_number("+44 7911 123456", None);
    assert!(r2.is_normalized);
    assert_eq!(r2.normalized_e164.as_deref(), Some("+447911123456"));

    // International trunk 00 to UK
    let r3 = normalize_phone_number("00447911123456", None);
    assert!(r3.is_normalized);
    assert_eq!(r3.normalized_e164.as_deref(), Some("+447911123456"));

    // Domestic US number with configured prefix 1
    let r4 = normalize_phone_number("5551234567", Some("1"));
    assert!(r4.is_normalized);
    assert_eq!(r4.normalized_e164.as_deref(), Some("+15551234567"));
}

// 31. CDR parsing with Indian telecom profiles (Airtel, Jio, Vi, BSNL)
#[test]
fn test_phase6_cdr_indian_telecom_profiles() {
    // Airtel format: Calling_Number, Called_Number, Call_Date_Time, Call_Duration, First_CGI, Call_Type
    let airtel_csv =
        "Calling_Number,Called_Number,Call_Date_Time,Call_Duration,First_CGI,Call_Type\n\
                      9811002233,9899887766,2026-11-10 20:15:30,120,404-45-12345-6789,OUTGOING\n\
                      +919876543210,09811002233,2026-11-10 21:00:00,45,404-45-12345-6790,INCOMING";

    let res_airtel = parse_cdr_csv_with_profile(airtel_csv, "ev-airtel", None, CdrProfile::Airtel);
    assert_eq!(res_airtel.valid_records.len(), 2);
    assert_eq!(res_airtel.rejected_records.len(), 0);
    assert_eq!(res_airtel.valid_records[0].caller_phone_raw, "9811002233");
    assert_eq!(res_airtel.valid_records[0].receiver_phone_raw, "9899887766");
    assert_eq!(res_airtel.valid_records[0].duration_seconds, 120.0);
    assert_eq!(
        res_airtel.valid_records[0].cell_tower_id.as_deref(),
        Some("404-45-12345-6789")
    );

    // Auto-detection of Jio format: calling_party_number, called_party_number, start_timestamp, call_duration_seconds, first_cell_id
    let jio_csv = "calling_party_number,called_party_number,start_timestamp,call_duration_seconds,first_cell_id\n\
                   +919811002233,919899887766,2026-11-10T20:15:30+05:30,90,JIO-DEL-045";

    let res_jio = parse_cdr_csv(jio_csv, "ev-jio", None);
    assert_eq!(res_jio.valid_records.len(), 1);
    assert_eq!(res_jio.rejected_records.len(), 0);
    assert_eq!(
        res_jio.valid_records[0].call_timestamp_raw,
        "2026-11-10T20:15:30+05:30"
    );

    // Split Date + Time columns format (common in Vi / BSNL)
    let bsnl_csv = "calling_no,called_no,call_date,call_time,call_duration,first_cgi\n\
                    09811002233,9899887766,10/11/2026,20:15:30,60,BSNL-TOWER-12";

    let res_bsnl = parse_cdr_csv(bsnl_csv, "ev-bsnl", None);
    assert_eq!(res_bsnl.valid_records.len(), 1);
    assert_eq!(res_bsnl.rejected_records.len(), 0);
    assert_eq!(
        res_bsnl.valid_records[0].call_timestamp_raw,
        "10/11/2026 20:15:30"
    );

    // Hardware identifiers (IMEI / IMSI) in Indian CDR
    let imei_cdr = "calling_number,called_number,call_date_time,duration_seconds,imei,imsi\n\
                    9811002233,9899887766,2026-11-10 20:15:30,30,352099001761481,404450123456789";

    let res_imei = parse_cdr_csv(imei_cdr, "ev-imei", None);
    assert_eq!(res_imei.valid_records.len(), 1);
    let imeis: Vec<_> = res_imei
        .identifiers
        .iter()
        .filter(|i| i.identifier_type == "IMEI")
        .collect();
    assert_eq!(imeis.len(), 1);
    assert_eq!(imeis[0].raw_value, "352099001761481");
    assert_eq!(imeis[0].is_checksum_valid, Some(true));
}

// 32. Unsupported CDR format rejection
#[test]
fn test_phase6_unsupported_cdr_schema_rejection() {
    // Header without required telephony columns
    let invalid_csv = "random_col_1,random_col_2,amount,status\n\
                       val1,val2,100,SUCCESS";

    // AutoDetect rejects missing mandatory headers
    let res = parse_cdr_csv(invalid_csv, "ev-unsupported", None);
    assert_eq!(res.valid_records.len(), 0);
    assert_eq!(res.rejected_records.len(), 1);
    assert_eq!(
        res.rejected_records[0].issue_type,
        "MISSING_REQUIRED_HEADERS"
    );
    assert!(res.rejected_records[0]
        .message
        .contains("Format unsupported under Standard or IndianTelecom profiles"));

    // Explicit carrier profile rejects incompatible headers
    let res_explicit = parse_cdr_csv_with_profile(
        invalid_csv,
        "ev-unsupported-airtel",
        None,
        CdrProfile::Airtel,
    );
    assert_eq!(res_explicit.valid_records.len(), 0);
    assert_eq!(res_explicit.rejected_records.len(), 1);
    assert_eq!(
        res_explicit.rejected_records[0].issue_type,
        "UNSUPPORTED_CDR_SCHEMA"
    );
    assert!(res_explicit.rejected_records[0]
        .message
        .contains("Missing required telephony columns"));
}

// 33. IST timestamp handling and conversion
#[test]
fn test_phase6_ist_timezone_handling_and_conversion() {
    use sutram_backend_rust::reconstruction::temporal::{
        parse_and_normalize_timestamp, validate_timezone,
    };

    // Test IST alias support
    assert!(validate_timezone("IST").is_ok());
    assert!(validate_timezone("Asia/Kolkata").is_ok());
    assert!(validate_timezone("Asia/Calcutta").is_ok());
    assert!(validate_timezone("+05:30").is_ok());

    // 20:15:30 IST is 14:45:30 UTC
    let ts_ist = "2026-11-10 20:15:30";
    let utc_dt = parse_and_normalize_timestamp(ts_ist, "Asia/Kolkata", 0.0).unwrap();
    assert_eq!(utc_dt.to_rfc3339(), "2026-11-10T14:45:30+00:00");

    let utc_dt_alias = parse_and_normalize_timestamp(ts_ist, "IST", 0.0).unwrap();
    assert_eq!(utc_dt_alias.to_rfc3339(), "2026-11-10T14:45:30+00:00");

    // Indian format DD/MM/YYYY HH:MM:SS
    let ts_ddmmyyyy = "10/11/2026 20:15:30";
    let utc_dt_dd = parse_and_normalize_timestamp(ts_ddmmyyyy, "Asia/Kolkata", 0.0).unwrap();
    assert_eq!(utc_dt_dd.to_rfc3339(), "2026-11-10T14:45:30+00:00");

    // Verify naive timestamp with UTC is NOT assumed to be IST
    let utc_dt_utc = parse_and_normalize_timestamp(ts_ist, "UTC", 0.0).unwrap();
    assert_eq!(utc_dt_utc.to_rfc3339(), "2026-11-10T20:15:30+00:00");
}

// 34. Chat phone extraction with original value preservation
#[test]
fn test_phase6_chat_phone_extraction_and_original_preservation() {
    let chat_json = r#"{
        "chat_id": "test-chat-delhi",
        "chat_title": "Operation Alpha",
        "messages": [
            {
                "message_id": "msg-001",
                "sender_id": "9811002233",
                "sender_name": "Rajesh Kumar",
                "recipients": ["+91 9899887766"],
                "timestamp": "2026-11-10T20:15:30+05:30",
                "content": "Call informant at 09876543210 or 6301234567 immediately."
            }
        ]
    }"#;

    let res = parse_chat_json(chat_json, "ev-chat-delhi", Some("91"));
    assert_eq!(res.valid_messages.len(), 1);
    assert_eq!(res.rejected_messages.len(), 0);

    let msg = &res.valid_messages[0];
    // Preserves original message data verbatim
    assert_eq!(msg.sender_raw_id, "9811002233");
    assert_eq!(msg.recipients_raw, vec!["+91 9899887766"]);
    assert_eq!(msg.timestamp_raw, "2026-11-10T20:15:30+05:30");
    assert_eq!(
        msg.content_raw,
        "Call informant at 09876543210 or 6301234567 immediately."
    );

    // Verify sender normalized properly to +919811002233
    let sender_idents: Vec<_> = res
        .identifiers
        .iter()
        .filter(|i| i.raw_value == "9811002233")
        .collect();
    assert_eq!(sender_idents.len(), 1);
    assert_eq!(
        sender_idents[0].normalized_value.as_deref(),
        Some("+919811002233")
    );

    // Verify recipient normalized properly
    let rec_idents: Vec<_> = res
        .identifiers
        .iter()
        .filter(|i| i.raw_value == "+91 9899887766")
        .collect();
    assert_eq!(rec_idents.len(), 1);
    assert_eq!(
        rec_idents[0].normalized_value.as_deref(),
        Some("+919899887766")
    );

    // Verify text embedded phone numbers extracted with Indian normalization
    let extracted_phones: Vec<_> = res
        .identifiers
        .iter()
        .filter(|i| i.extraction_method == "regex_phone")
        .collect();
    assert_eq!(extracted_phones.len(), 2);
    let norm_vals: Vec<_> = extracted_phones
        .iter()
        .filter_map(|i| i.normalized_value.as_deref())
        .collect();
    assert!(norm_vals.contains(&"+919876543210"));
    assert!(norm_vals.contains(&"+916301234567"));
}
