//! Call Detail Record (CDR) Extraction Engine (Member 1).
//!
//! Parses telephony Call Detail Record CSV files deterministically, performs strict
//! row and header validation, preserves original values without mutation, extracts
//! caller/receiver identifiers, and synthesizes event and relationship candidates.
//!
//! Supports standard canonical schema and configurable/auto-detected Indian telecom
//! carrier profiles (Bharti Airtel, Reliance Jio, Vodafone Idea, BSNL).

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
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

pub const REQUIRED_CDR_HEADERS: [&str; 5] = [
    "record_id",
    "caller_phone",
    "receiver_phone",
    "call_timestamp",
    "duration_seconds",
];

/// Supported Call Detail Record schema profiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CdrProfile {
    /// Standard canonical YUKTI CDR schema:
    /// record_id, caller_phone, receiver_phone, call_timestamp, duration_seconds
    Standard,
    /// Common Indian Telecom profile (Bharti Airtel, Reliance Jio, Vodafone Idea, BSNL).
    IndianTelecom,
    /// Bharti Airtel specific column mappings
    Airtel,
    /// Reliance Jio specific column mappings
    Jio,
    /// Vodafone Idea (Vi) specific column mappings
    Vi,
    /// BSNL specific column mappings
    Bsnl,
    /// Automatic detection based on header analysis
    #[default]
    AutoDetect,
}

/// Normalized column header matching helper.
fn normalize_header(s: &str) -> String {
    s.trim()
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect::<String>()
        .split('_')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

fn is_caller_col(h: &str) -> bool {
    matches!(
        h,
        "caller_phone"
            | "calling_number"
            | "calling_no"
            | "calling_num"
            | "caller_number"
            | "caller_no"
            | "caller_num"
            | "calling_party"
            | "calling_party_number"
            | "calling_party_no"
            | "calling_msisdn"
            | "caller_msisdn"
            | "source_msisdn"
            | "source_number"
            | "originating_number"
            | "originating_msisdn"
            | "a_party_no"
            | "a_party_number"
            | "a_party"
            | "msisdn_a"
            | "caller"
    )
}

fn is_receiver_col(h: &str) -> bool {
    matches!(
        h,
        "receiver_phone"
            | "called_number"
            | "called_no"
            | "called_num"
            | "callee_number"
            | "callee_no"
            | "callee_num"
            | "called_party"
            | "called_party_number"
            | "called_party_no"
            | "called_msisdn"
            | "callee_msisdn"
            | "receiver_msisdn"
            | "destination_msisdn"
            | "destination_number"
            | "terminating_number"
            | "terminating_msisdn"
            | "b_party_no"
            | "b_party_number"
            | "b_party"
            | "msisdn_b"
            | "receiver"
            | "callee"
    )
}

fn is_timestamp_col(h: &str) -> bool {
    matches!(
        h,
        "call_timestamp"
            | "call_date_time"
            | "call_datetime"
            | "date_time"
            | "datetime"
            | "start_time"
            | "call_start_time"
            | "call_start_timestamp"
            | "start_timestamp"
            | "start_date_time"
            | "call_date_and_time"
            | "timestamp"
    )
}

fn is_date_col(h: &str) -> bool {
    matches!(h, "call_date" | "date" | "start_date" | "call_start_date")
}

fn is_time_col(h: &str) -> bool {
    matches!(h, "call_time" | "time" | "start_time" | "call_start_time")
}

fn is_duration_col(h: &str) -> bool {
    matches!(
        h,
        "duration_seconds"
            | "call_duration"
            | "call_duration_seconds"
            | "call_duration_sec"
            | "call_duration_secs"
            | "duration_sec"
            | "duration_secs"
            | "duration_in_sec"
            | "duration_in_secs"
            | "duration"
            | "call_duration_in_seconds"
            | "duration_s"
    )
}

fn is_cell_tower_col(h: &str) -> bool {
    matches!(
        h,
        "cell_tower_id"
            | "first_cgi"
            | "last_cgi"
            | "cgi"
            | "first_cell_id"
            | "last_cell_id"
            | "cell_id"
            | "cell_site_id"
            | "site_id"
            | "bts_id"
            | "ecgi"
            | "tower_id"
            | "first_site_id"
            | "azimuth"
    )
}

fn is_call_type_col(h: &str) -> bool {
    matches!(
        h,
        "call_type"
            | "type_of_call"
            | "service_type"
            | "call_direction"
            | "direction"
            | "call_category"
            | "tele_service"
    )
}

fn is_record_id_col(h: &str) -> bool {
    matches!(
        h,
        "record_id"
            | "seq_no"
            | "sequence_number"
            | "sl_no"
            | "s_no"
            | "sn"
            | "call_id"
            | "transaction_id"
            | "record_no"
    )
}

fn is_imei_col(h: &str) -> bool {
    matches!(h, "imei" | "first_imei" | "device_imei" | "calling_imei")
}

fn is_imsi_col(h: &str) -> bool {
    matches!(h, "imsi" | "first_imsi" | "calling_imsi")
}

#[derive(Debug, Clone)]
struct ResolvedCdrSchema {
    caller_idx: usize,
    caller_col_name: String,
    callee_idx: usize,
    callee_col_name: String,
    timestamp_idx: Option<usize>,
    date_idx: Option<usize>,
    time_idx: Option<usize>,
    duration_idx: usize,
    record_id_idx: Option<usize>,
    cell_tower_idx: Option<usize>,
    call_type_idx: Option<usize>,
    imei_idx: Option<usize>,
    imsi_idx: Option<usize>,
    profile_name: String,
    is_indian_profile: bool,
}

/// Deterministically parse a Call Detail Record (CDR) CSV string into structured candidates and issues.
pub fn parse_cdr_csv(
    raw_csv: &str,
    source_evidence_id: &str,
    default_country_prefix: Option<&str>,
) -> CDRExtractionResult {
    parse_cdr_csv_with_profile(
        raw_csv,
        source_evidence_id,
        default_country_prefix,
        CdrProfile::AutoDetect,
    )
}

/// Deterministically parse a Call Detail Record (CDR) CSV string with an explicit or auto-detected carrier profile.
///
/// Forensic guarantees:
/// - Never modifies the source file.
/// - Rejects invalid rows with explicit structured diagnostic issues instead of dropping them.
/// - Preserves uncorrected timestamps and raw phone strings verbatim.
/// - Supports configurable Indian carrier profiles (Airtel, Jio, Vi, BSNL).
/// - Rejects unsupported/unrecognized schemas explicitly rather than silently guessing columns.
/// - Synthesizes observed call communication relationships and telephony event candidates.
pub fn parse_cdr_csv_with_profile(
    raw_csv: &str,
    source_evidence_id: &str,
    default_country_prefix: Option<&str>,
    profile: CdrProfile,
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

    // Process headers
    let raw_header: Vec<String> = raw_rows[0]
        .iter()
        .map(|h| h.trim().to_ascii_lowercase())
        .collect();

    // Check exact Standard schema matching
    let header_set: HashSet<&str> = raw_header.iter().map(|s| s.as_str()).collect();
    let has_all_standard_headers = REQUIRED_CDR_HEADERS
        .iter()
        .all(|req| header_set.contains(req));

    // Handle explicit Standard profile request
    if profile == CdrProfile::Standard {
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
    }

    let schema: ResolvedCdrSchema = if profile == CdrProfile::Standard
        || (profile == CdrProfile::AutoDetect && has_all_standard_headers)
    {
        let header_indices: HashMap<&str, usize> = raw_header
            .iter()
            .enumerate()
            .map(|(idx, col)| (col.as_str(), idx))
            .collect();

        ResolvedCdrSchema {
            caller_idx: *header_indices.get("caller_phone").unwrap(),
            caller_col_name: "caller_phone".to_string(),
            callee_idx: *header_indices.get("receiver_phone").unwrap(),
            callee_col_name: "receiver_phone".to_string(),
            timestamp_idx: Some(*header_indices.get("call_timestamp").unwrap()),
            date_idx: None,
            time_idx: None,
            duration_idx: *header_indices.get("duration_seconds").unwrap(),
            record_id_idx: header_indices.get("record_id").copied(),
            cell_tower_idx: header_indices.get("cell_tower_id").copied(),
            call_type_idx: header_indices.get("call_type").copied(),
            imei_idx: header_indices.get("imei").copied(),
            imsi_idx: header_indices.get("imsi").copied(),
            profile_name: "Standard".to_string(),
            is_indian_profile: false,
        }
    } else {
        // Resolve using flexible carrier profile aliases
        let normalized_headers: Vec<String> =
            raw_rows[0].iter().map(|h| normalize_header(h)).collect();
        let mut caller_opt = None;
        let mut callee_opt = None;
        let mut timestamp_opt = None;
        let mut date_opt = None;
        let mut time_opt = None;
        let mut duration_opt = None;
        let mut record_id_opt = None;
        let mut cell_tower_opt = None;
        let mut call_type_opt = None;
        let mut imei_opt = None;
        let mut imsi_opt = None;

        for (idx, norm) in normalized_headers.iter().enumerate() {
            if caller_opt.is_none() && is_caller_col(norm) {
                caller_opt = Some((idx, raw_header[idx].clone()));
            } else if callee_opt.is_none() && is_receiver_col(norm) {
                callee_opt = Some((idx, raw_header[idx].clone()));
            } else if timestamp_opt.is_none() && is_timestamp_col(norm) {
                timestamp_opt = Some(idx);
            } else if date_opt.is_none() && is_date_col(norm) {
                date_opt = Some(idx);
            } else if time_opt.is_none() && is_time_col(norm) {
                time_opt = Some(idx);
            } else if duration_opt.is_none() && is_duration_col(norm) {
                duration_opt = Some(idx);
            } else if record_id_opt.is_none() && is_record_id_col(norm) {
                record_id_opt = Some(idx);
            } else if cell_tower_opt.is_none() && is_cell_tower_col(norm) {
                cell_tower_opt = Some(idx);
            } else if call_type_opt.is_none() && is_call_type_col(norm) {
                call_type_opt = Some(idx);
            } else if imei_opt.is_none() && is_imei_col(norm) {
                imei_opt = Some(idx);
            } else if imsi_opt.is_none() && is_imsi_col(norm) {
                imsi_opt = Some(idx);
            }
        }

        let has_time = timestamp_opt.is_some() || (date_opt.is_some() && time_opt.is_some());
        if caller_opt.is_none() || callee_opt.is_none() || !has_time || duration_opt.is_none() {
            let mut missing_headers: Vec<&str> = REQUIRED_CDR_HEADERS
                .iter()
                .copied()
                .filter(|req| !header_set.contains(req))
                .collect();
            missing_headers.sort();

            let (issue_type, message) = match profile {
                CdrProfile::AutoDetect | CdrProfile::Standard => (
                    "MISSING_REQUIRED_HEADERS".to_string(),
                    format!(
                        "CSV missing mandatory columns: {:?}. Found headers: {:?}. Format unsupported under Standard or IndianTelecom profiles.",
                        missing_headers, raw_header
                    ),
                ),
                _ => (
                    "UNSUPPORTED_CDR_SCHEMA".to_string(),
                    format!(
                        "CSV headers do not match requested carrier profile '{:?}': {:?}. Missing required telephony columns (caller, callee, timestamp, duration).",
                        profile, raw_header
                    ),
                ),
            };

            return CDRExtractionResult {
                source_evidence_id: source_evidence_id.to_string(),
                total_rows_processed: raw_rows.len().saturating_sub(1),
                valid_records: Vec::new(),
                rejected_records: vec![ExtractionValidationIssue {
                    record_id: None,
                    field_name: "headers".to_string(),
                    issue_type,
                    raw_value: Some(raw_rows[0].join(",")),
                    message,
                    row_index: Some(1),
                }],
                identifiers: Vec::new(),
                event_candidates: Vec::new(),
                relationship_candidates: Vec::new(),
            };
        }

        let (c_idx, c_name) = caller_opt.unwrap();
        let (ce_idx, ce_name) = callee_opt.unwrap();

        let profile_label = match profile {
            CdrProfile::Airtel => "Airtel".to_string(),
            CdrProfile::Jio => "Jio".to_string(),
            CdrProfile::Vi => "Vi".to_string(),
            CdrProfile::Bsnl => "BSNL".to_string(),
            CdrProfile::IndianTelecom => "IndianTelecom".to_string(),
            _ => {
                // Auto-detect specific Indian telecom provider flavor if possible
                if normalized_headers
                    .iter()
                    .any(|h| h.contains("cgi") || h.contains("calling_party"))
                {
                    "IndianTelecom (Airtel/Standard)".to_string()
                } else if normalized_headers
                    .iter()
                    .any(|h| h.contains("ecgi") || h.contains("party_number"))
                {
                    "IndianTelecom (Jio)".to_string()
                } else if normalized_headers
                    .iter()
                    .any(|h| h.contains("a_party") || h.contains("b_party"))
                {
                    "IndianTelecom (Vi)".to_string()
                } else {
                    "IndianTelecom (Generic)".to_string()
                }
            }
        };

        ResolvedCdrSchema {
            caller_idx: c_idx,
            caller_col_name: c_name,
            callee_idx: ce_idx,
            callee_col_name: ce_name,
            timestamp_idx: timestamp_opt,
            date_idx: date_opt,
            time_idx: time_opt,
            duration_idx: duration_opt.unwrap(),
            record_id_idx: record_id_opt,
            cell_tower_idx: cell_tower_opt,
            call_type_idx: call_type_opt,
            imei_idx: imei_opt,
            imsi_idx: imsi_opt,
            profile_name: profile_label,
            is_indian_profile: true,
        }
    };

    let mut valid_records = Vec::new();
    let mut rejected_records = Vec::new();
    let mut identifiers = Vec::new();
    let mut event_candidates = Vec::new();
    let mut relationship_candidates = Vec::new();

    // In Indian CDR profiles, default country code is 91 unless explicitly overridden
    let effective_country_prefix = default_country_prefix.or({
        if schema.is_indian_profile {
            Some("91")
        } else {
            None
        }
    });

    let data_rows = &raw_rows[1..];
    for (idx, row) in data_rows.iter().enumerate() {
        let row_idx = idx + 2; // 1-indexed, skipping header
        if !row.iter().any(|field| !field.trim().is_empty()) {
            // Empty row
            continue;
        }

        // Determine row value by column index
        let get_by_idx = |opt_idx: Option<usize>| -> String {
            opt_idx
                .and_then(|i| row.get(i))
                .map(|s| s.trim().to_string())
                .unwrap_or_default()
        };

        let rec_id_raw = get_by_idx(schema.record_id_idx);
        let caller = get_by_idx(Some(schema.caller_idx));
        let callee = get_by_idx(Some(schema.callee_idx));

        let timestamp_raw = if let Some(ts_idx) = schema.timestamp_idx {
            get_by_idx(Some(ts_idx))
        } else if let (Some(d_idx), Some(t_idx)) = (schema.date_idx, schema.time_idx) {
            let d = get_by_idx(Some(d_idx));
            let t = get_by_idx(Some(t_idx));
            if d.is_empty() || t.is_empty() {
                String::new()
            } else {
                format!("{} {}", d, t)
            }
        } else {
            String::new()
        };

        let duration_raw = get_by_idx(Some(schema.duration_idx));
        let cell_tower_val = get_by_idx(schema.cell_tower_idx);
        let cell_tower = if cell_tower_val.is_empty() {
            None
        } else {
            Some(cell_tower_val)
        };
        let call_type_val = get_by_idx(schema.call_type_idx);
        let call_type = if call_type_val.is_empty() {
            "VOICE_CALL".to_string()
        } else {
            call_type_val
        };

        let mut has_error = false;
        let record_id_for_issue = if rec_id_raw.is_empty() {
            None
        } else {
            Some(rec_id_raw.clone())
        };
        let fallback_record_id = if rec_id_raw.is_empty() {
            format!("row-{}", row_idx)
        } else {
            rec_id_raw.clone()
        };

        // For Standard profile, record_id is mandatory. For carrier profiles, synthesize row ID if missing.
        let final_record_id = if rec_id_raw.is_empty() {
            if !schema.is_indian_profile {
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
            fallback_record_id.clone()
        } else {
            rec_id_raw
        };

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
            record_id: final_record_id.clone(),
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
        let caller_norm = normalize_phone_number(&caller, effective_country_prefix);
        identifiers.push(ExtractedIdentifier {
            identifier_type: "PHONE_NUMBER".to_string(),
            raw_value: caller.clone(),
            normalized_value: if caller_norm.is_normalized {
                caller_norm.normalized_e164
            } else {
                None
            },
            source_evidence_id: source_evidence_id.to_string(),
            source_record_id: final_record_id.clone(),
            location_reference: Some(format!("column:{}", schema.caller_col_name)),
            is_ambiguous: !caller_norm.is_normalized,
            ambiguity_reason: if !caller_norm.is_normalized {
                Some(caller_norm.reason)
            } else {
                None
            },
            context_snippet: None,
            is_checksum_valid: None,
            extraction_confidence: None,
            extraction_method: "cdr_column".to_string(),
        });

        let callee_norm = normalize_phone_number(&callee, effective_country_prefix);
        identifiers.push(ExtractedIdentifier {
            identifier_type: "PHONE_NUMBER".to_string(),
            raw_value: callee.clone(),
            normalized_value: if callee_norm.is_normalized {
                callee_norm.normalized_e164
            } else {
                None
            },
            source_evidence_id: source_evidence_id.to_string(),
            source_record_id: final_record_id.clone(),
            location_reference: Some(format!("column:{}", schema.callee_col_name)),
            is_ambiguous: !callee_norm.is_normalized,
            ambiguity_reason: if !callee_norm.is_normalized {
                Some(callee_norm.reason)
            } else {
                None
            },
            context_snippet: None,
            is_checksum_valid: None,
            extraction_confidence: None,
            extraction_method: "cdr_column".to_string(),
        });

        // Extract optional hardware identifiers if available in carrier schema
        let imei_val = get_by_idx(schema.imei_idx);
        if !imei_val.is_empty() {
            identifiers.push(ExtractedIdentifier {
                identifier_type: "IMEI".to_string(),
                raw_value: imei_val.clone(),
                normalized_value: Some(imei_val.clone()),
                source_evidence_id: source_evidence_id.to_string(),
                source_record_id: final_record_id.clone(),
                location_reference: Some("column:imei".to_string()),
                is_ambiguous: false,
                ambiguity_reason: None,
                context_snippet: None,
                is_checksum_valid: Some(crate::extraction::identifiers::luhn_checksum_valid(
                    &imei_val,
                )),
                extraction_confidence: None,
                extraction_method: "cdr_column_imei".to_string(),
            });
        }

        let imsi_val = get_by_idx(schema.imsi_idx);
        if !imsi_val.is_empty() {
            identifiers.push(ExtractedIdentifier {
                identifier_type: "IMSI".to_string(),
                raw_value: imsi_val.clone(),
                normalized_value: Some(imsi_val.clone()),
                source_evidence_id: source_evidence_id.to_string(),
                source_record_id: final_record_id.clone(),
                location_reference: Some("column:imsi".to_string()),
                is_ambiguous: false,
                ambiguity_reason: None,
                context_snippet: None,
                is_checksum_valid: None,
                extraction_confidence: None,
                extraction_method: "cdr_column_imsi".to_string(),
            });
        }

        // 2. Event Candidate Creation
        let mut evt_meta = serde_json::Map::new();
        evt_meta.insert("call_type".to_string(), serde_json::json!(call_type));
        if let Some(ref tower) = cell_tower {
            evt_meta.insert("cell_tower_id".to_string(), serde_json::json!(tower));
        }
        evt_meta.insert("row_index".to_string(), serde_json::json!(row_idx));
        evt_meta.insert(
            "detected_profile".to_string(),
            serde_json::json!(schema.profile_name),
        );
        if schema.is_indian_profile {
            evt_meta.insert(
                "carrier_timezone".to_string(),
                serde_json::json!("Asia/Kolkata"),
            );
        }

        let desc = format!(
            "Telephony {} between {} and {}{}",
            call_type,
            caller,
            callee,
            cell_tower
                .as_ref()
                .map(|t| format!(" via {}", t))
                .unwrap_or_default()
        );

        event_candidates.push(ExtractedEventCandidate {
            candidate_id: format!("evt-cdr-{}-{}", source_evidence_id, final_record_id),
            source_evidence_id: source_evidence_id.to_string(),
            source_record_id: final_record_id,
            event_type: "TELEPHONY_CALL".to_string(),
            raw_timestamp: timestamp_raw,
            timestamp_precision: Some("EXACT_SECOND".to_string()),
            description: Some(desc),
            duration_seconds: Some(duration),
            location_name: cell_tower,
            involved_entity_raw_ids: vec![caller, callee],
            metadata: serde_json::Value::Object(evt_meta),
        });

        // 3. Relationship Candidate Creation
        let rel_cand = generate_cdr_relationship_candidate(&parsed_rec, effective_country_prefix);
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
