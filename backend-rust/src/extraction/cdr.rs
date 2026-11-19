//! Call Detail Record (CDR) Extraction Engine (Member 1).
//!
//! Parses telephony Call Detail Record CSV files deterministically, performs strict
//! row and header validation, preserves original values without mutation, extracts
//! caller/receiver identifiers, and synthesizes event and relationship candidates.

use crate::{
    correlation::{
        relationships::generate_cdr_relationship_candidate, resolution::normalize_phone_number,
    },
    extraction::model::{
        CDRExtractionResult, ExtractedEventCandidate, ExtractedIdentifier,
        ExtractionValidationIssue, ParsedCDRRecord,
    },
};
use csv::ReaderBuilder;
use std::collections::{HashMap, HashSet};

const REQUIRED_CDR_HEADERS: [&str; 5] = [
    "record_id",
    "caller_phone",
    "receiver_phone",
    "call_timestamp",
    "duration_seconds",
];

/// Deterministically parse a Call Detail Record (CDR) CSV string into structured candidates and issues.
///
/// Forensic guarantees:
/// - Never modifies the source file.
/// - Rejects invalid rows with explicit structured diagnostic issues instead of dropping them.
/// - Preserves uncorrected timestamps and raw phone strings verbatim.
/// - Synthesizes observed call communication relationships and telephony event candidates.
pub fn parse_cdr_csv(
    raw_csv: &str,
    source_evidence_id: &str,
    default_country_prefix: Option<&str>,
) -> CDRExtractionResult {
    // Strip BOM if present
    let content = raw_csv.strip_prefix('\u{feff}').unwrap_or(raw_csv);

    let mut reader = ReaderBuilder::new()
        .flexible(true)
        .has_headers(false)
        .from_reader(content.as_bytes());

    let mut raw_rows: Vec<Vec<String>> = Vec::new();
    for record in reader.records() {
        match record {
            Ok(rec) => {
                let row: Vec<String> = rec.iter().map(|s| s.to_string()).collect();
                raw_rows.push(row);
            }
            Err(e) => {
                return CDRExtractionResult {
                    source_evidence_id: source_evidence_id.to_string(),
                    total_rows_processed: 0,
                    valid_records: Vec::new(),
                    rejected_records: vec![ExtractionValidationIssue {
                        record_id: None,
                        field_name: "csv".to_string(),
                        issue_type: "MALFORMED_CSV".to_string(),
                        raw_value: None,
                        message: format!("Malformed CSV content: {}", e),
                        row_index: None,
                    }],
                    identifiers: Vec::new(),
                    event_candidates: Vec::new(),
                    relationship_candidates: Vec::new(),
                };
            }
        }
    }

    if raw_rows.is_empty() {
        return CDRExtractionResult {
            source_evidence_id: source_evidence_id.to_string(),
            total_rows_processed: 0,
            valid_records: Vec::new(),
            rejected_records: vec![ExtractionValidationIssue {
                record_id: None,
                field_name: "file".to_string(),
                issue_type: "EMPTY_FILE".to_string(),
                raw_value: None,
                message: "The provided CDR CSV file is completely empty.".to_string(),
                row_index: Some(0),
            }],
            identifiers: Vec::new(),
            event_candidates: Vec::new(),
            relationship_candidates: Vec::new(),
        };
    }

    // Validate header row
    let raw_header: Vec<String> = raw_rows[0]
        .iter()
        .map(|h| h.trim().to_ascii_lowercase())
        .collect();

    let header_set: HashSet<&str> = raw_header.iter().map(|s| s.as_str()).collect();
    let mut missing_headers: Vec<&str> = REQUIRED_CDR_HEADERS
        .iter()
        .copied()
        .filter(|req| !header_set.contains(req))
        .collect();
    missing_headers.sort();

    if !missing_headers.is_empty() {
        return CDRExtractionResult {
            source_evidence_id: source_evidence_id.to_string(),
            total_rows_processed: raw_rows.len().saturating_sub(1),
            valid_records: Vec::new(),
            rejected_records: vec![ExtractionValidationIssue {
                record_id: None,
                field_name: "headers".to_string(),
                issue_type: "MISSING_REQUIRED_HEADERS".to_string(),
                raw_value: Some(raw_rows[0].join(",")),
                message: format!(
                    "CSV missing mandatory columns: {:?}. Found headers: {:?}",
                    missing_headers, raw_header
                ),
                row_index: Some(1),
            }],
            identifiers: Vec::new(),
            event_candidates: Vec::new(),
            relationship_candidates: Vec::new(),
        };
    }

    let header_indices: HashMap<&str, usize> = raw_header
        .iter()
        .enumerate()
        .map(|(idx, col)| (col.as_str(), idx))
        .collect();

    let mut valid_records = Vec::new();
    let mut rejected_records = Vec::new();
    let mut identifiers = Vec::new();
    let mut event_candidates = Vec::new();
    let mut relationship_candidates = Vec::new();

    let data_rows = &raw_rows[1..];
    for (idx, row) in data_rows.iter().enumerate() {
        let row_idx = idx + 2; // 1-indexed, skipping header
        if !row.iter().any(|field| !field.trim().is_empty()) {
            // Empty row
            continue;
        }

        if row.len() < header_indices.len() {
            rejected_records.push(ExtractionValidationIssue {
                record_id: None,
                field_name: "row".to_string(),
                issue_type: "MALFORMED_ROW_COLUMN_MISMATCH".to_string(),
                raw_value: Some(row.join(",")),
                message: format!(
                    "Row {} has {} columns, expected at least {}.",
                    row_idx,
                    row.len(),
                    header_indices.len()
                ),
                row_index: Some(row_idx),
            });
            continue;
        }

        let get_val = |col_name: &str| -> String {
            header_indices
                .get(col_name)
                .and_then(|&i| row.get(i))
                .map(|s| s.trim().to_string())
                .unwrap_or_default()
        };

        let rec_id = get_val("record_id");
        let caller = get_val("caller_phone");
        let callee = get_val("receiver_phone");
        let timestamp_raw = get_val("call_timestamp");
        let duration_raw = get_val("duration_seconds");
        let cell_tower_val = get_val("cell_tower_id");
        let cell_tower = if cell_tower_val.is_empty() {
            None
        } else {
            Some(cell_tower_val)
        };
        let call_type_val = get_val("call_type");
        let call_type = if call_type_val.is_empty() {
            "VOICE_CALL".to_string()
        } else {
            call_type_val
        };

        let mut has_error = false;
        let record_id_for_issue = if rec_id.is_empty() {
            None
        } else {
            Some(rec_id.clone())
        };
        let fallback_record_id = if rec_id.is_empty() {
            format!("row-{}", row_idx)
        } else {
            rec_id.clone()
        };

        if rec_id.is_empty() {
            rejected_records.push(ExtractionValidationIssue {
                record_id: record_id_for_issue,
                field_name: "record_id".to_string(),
                issue_type: "MISSING_REQUIRED_FIELD".to_string(),
                raw_value: Some("".to_string()),
                message: format!("Row {} lacks mandatory 'record_id'.", row_idx),
                row_index: Some(row_idx),
            });
            has_error = true;
        }

        if caller.is_empty() {
            rejected_records.push(ExtractionValidationIssue {
                record_id: Some(fallback_record_id.clone()),
                field_name: "caller_phone".to_string(),
                issue_type: "MISSING_REQUIRED_FIELD".to_string(),
                raw_value: Some("".to_string()),
                message: format!("Row {} lacks mandatory 'caller_phone'.", row_idx),
                row_index: Some(row_idx),
            });
            has_error = true;
        }

        if callee.is_empty() {
            rejected_records.push(ExtractionValidationIssue {
                record_id: Some(fallback_record_id.clone()),
                field_name: "receiver_phone".to_string(),
                issue_type: "MISSING_REQUIRED_FIELD".to_string(),
                raw_value: Some("".to_string()),
                message: format!("Row {} lacks mandatory 'receiver_phone'.", row_idx),
                row_index: Some(row_idx),
            });
            has_error = true;
        }

        if timestamp_raw.is_empty() {
            rejected_records.push(ExtractionValidationIssue {
                record_id: Some(fallback_record_id.clone()),
                field_name: "call_timestamp".to_string(),
                issue_type: "MISSING_REQUIRED_FIELD".to_string(),
                raw_value: Some("".to_string()),
                message: format!("Row {} lacks mandatory 'call_timestamp'.", row_idx),
                row_index: Some(row_idx),
            });
            has_error = true;
        }

        // Validate duration
        let duration = match duration_raw.parse::<f64>() {
            Ok(d) if d >= 0.0 => d,
            Ok(neg) => {
                rejected_records.push(ExtractionValidationIssue {
                    record_id: Some(fallback_record_id.clone()),
                    field_name: "duration_seconds".to_string(),
                    issue_type: "INVALID_DURATION".to_string(),
                    raw_value: Some(duration_raw.clone()),
                    message: format!(
                        "Row {} has invalid duration '{}': Duration cannot be negative.",
                        row_idx, neg
                    ),
                    row_index: Some(row_idx),
                });
                has_error = true;
                0.0
            }
            Err(e) => {
                rejected_records.push(ExtractionValidationIssue {
                    record_id: Some(fallback_record_id.clone()),
                    field_name: "duration_seconds".to_string(),
                    issue_type: "INVALID_DURATION".to_string(),
                    raw_value: Some(duration_raw.clone()),
                    message: format!(
                        "Row {} has invalid duration '{}': {}.",
                        row_idx, duration_raw, e
                    ),
                    row_index: Some(row_idx),
                });
                has_error = true;
                0.0
            }
        };

        if has_error {
            continue;
        }

        let parsed_rec = ParsedCDRRecord {
            source_evidence_id: source_evidence_id.to_string(),
            record_id: rec_id.clone(),
            caller_phone_raw: caller.clone(),
            receiver_phone_raw: callee.clone(),
            call_timestamp_raw: timestamp_raw.clone(),
            duration_seconds: duration,
            cell_tower_id: cell_tower.clone(),
            call_type: call_type.clone(),
            row_index: row_idx,
        };
        valid_records.push(parsed_rec.clone());

        // 1. Identifiers for Caller & Callee
        let caller_norm = normalize_phone_number(&caller, default_country_prefix);
        identifiers.push(ExtractedIdentifier {
            identifier_type: "PHONE_NUMBER".to_string(),
            raw_value: caller.clone(),
            normalized_value: if caller_norm.is_normalized {
                caller_norm.normalized_e164
            } else {
                None
            },
            source_evidence_id: source_evidence_id.to_string(),
            source_record_id: rec_id.clone(),
            location_reference: Some("column:caller_phone".to_string()),
            is_ambiguous: false,
            ambiguity_reason: None,
            context_snippet: None,
            is_checksum_valid: None,
            extraction_confidence: None,
            extraction_method: "cdr_column".to_string(),
        });

        let callee_norm = normalize_phone_number(&callee, default_country_prefix);
        identifiers.push(ExtractedIdentifier {
            identifier_type: "PHONE_NUMBER".to_string(),
            raw_value: callee.clone(),
            normalized_value: if callee_norm.is_normalized {
                callee_norm.normalized_e164
            } else {
                None
            },
            source_evidence_id: source_evidence_id.to_string(),
            source_record_id: rec_id.clone(),
            location_reference: Some("column:receiver_phone".to_string()),
            is_ambiguous: false,
            ambiguity_reason: None,
            context_snippet: None,
            is_checksum_valid: None,
            extraction_confidence: None,
            extraction_method: "cdr_column".to_string(),
        });

        // 2. Event Candidate Creation
        let mut evt_meta = serde_json::Map::new();
        evt_meta.insert("call_type".to_string(), serde_json::json!(call_type));
        if let Some(ref tower) = cell_tower {
            evt_meta.insert("cell_tower_id".to_string(), serde_json::json!(tower));
        }
        evt_meta.insert("row_index".to_string(), serde_json::json!(row_idx));

        event_candidates.push(ExtractedEventCandidate {
            candidate_id: format!("evt-cdr-{}-{}", source_evidence_id, rec_id),
            source_evidence_id: source_evidence_id.to_string(),
            source_record_id: rec_id,
            event_type: "TELEPHONY_CALL".to_string(),
            raw_timestamp: timestamp_raw,
            duration_seconds: Some(duration),
            location_name: cell_tower,
            involved_entity_raw_ids: vec![caller, callee],
            metadata: serde_json::Value::Object(evt_meta),
        });

        // 3. Relationship Candidate Creation
        let rel_cand = generate_cdr_relationship_candidate(&parsed_rec, default_country_prefix);
        relationship_candidates.push(rel_cand);
    }

    CDRExtractionResult {
        source_evidence_id: source_evidence_id.to_string(),
        total_rows_processed: data_rows.len(),
        valid_records,
        rejected_records,
        identifiers,
        event_candidates,
        relationship_candidates,
    }
}
