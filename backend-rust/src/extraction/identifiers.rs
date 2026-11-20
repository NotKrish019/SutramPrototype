//! Deterministic Pattern-Based Identifier Extraction (Member 1).
//!
//! Extracts and validates phone numbers, email addresses, IMEIs, and IMSIs from
//! unstructured text and structured field values. Preserves exact Unicode character offsets,
//! raw matches, source provenance, and flags ambiguous numeric sequences.

use crate::extraction::model::ExtractedIdentifier;
use regex::Regex;
use std::sync::LazyLock;

static EMAIL_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b")
        .expect("Failed to compile EMAIL_PATTERN")
});

static PHONE_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:(?:\+|00)[1-9]\d{0,3}[\s.-]?)?(?:(?:\(\d{1,4}\)|\d{1,4})[\s.-]?)?\d{3,4}[\s.-]?\d{3,4}\b")
        .expect("Failed to compile PHONE_PATTERN")
});

static NUMERIC_14_15_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b\d{14,15}\b").expect("Failed to compile NUMERIC_14_15_PATTERN")
});

static IMEI_KEYWORDS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:imei|device|handset|terminal|hardware|tac)\b")
        .expect("Failed to compile IMEI_KEYWORDS")
});

static IMSI_KEYWORDS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:imsi|sim|subscriber|iccid|telecom)\b")
        .expect("Failed to compile IMSI_KEYWORDS")
});

/// Validate 15-digit Luhn checksum (standard for GSM/3GPP IMEI).
pub fn luhn_checksum_valid(digits: &str) -> bool {
    let clean = digits.trim();
    if clean.len() != 15 || !clean.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    let mut total = 0;
    for (idx, char) in clean.chars().rev().enumerate() {
        let val = char.to_digit(10).unwrap() as i32;
        if idx % 2 == 1 {
            let doubled = val * 2;
            total += if doubled > 9 { doubled - 9 } else { doubled };
        } else {
            total += val;
        }
    }
    total % 10 == 0
}

/// Extract a small snippet of surrounding text for forensic audit and contextual validation.
///
/// Uses Unicode character indices matching Python's character slicing semantics.
fn extract_context_window(chars: &[char], start: usize, end: usize, window_size: usize) -> String {
    let ctx_start = start.saturating_sub(window_size);
    let ctx_end = (end + window_size).min(chars.len());
    chars[ctx_start..ctx_end]
        .iter()
        .collect::<String>()
        .trim()
        .to_string()
}

/// Deterministically extract email addresses, phone numbers, IMEIs, and IMSIs from text.
///
/// Preserves character offsets (Unicode scalar values matching Python string indices),
/// surrounding context, and marks ambiguous numeric strings.
pub fn extract_identifiers_from_text(
    text: &str,
    source_evidence_id: &str,
    source_record_id: &str,
    _default_country_code: Option<&str>,
) -> Vec<ExtractedIdentifier> {
    if text.is_empty() {
        return Vec::new();
    }

    let chars: Vec<char> = text.chars().collect();
    let total_chars = chars.len();

    // Map UTF-8 byte offset to Unicode character offset
    // precomputed prefix count for fast, exact lookups
    let byte_to_char = |byte_idx: usize| -> usize { text[..byte_idx].chars().count() };

    let mut identifiers = Vec::new();
    let mut consumed_char_spans: Vec<(usize, usize)> = Vec::new();

    // 1. Email Extraction
    for m in EMAIL_PATTERN.find_iter(text) {
        let char_start = byte_to_char(m.start());
        let char_end = char_start + m.as_str().chars().count();
        let raw_val = m.as_str().trim().to_string();

        consumed_char_spans.push((char_start, char_end));
        identifiers.push(ExtractedIdentifier {
            identifier_type: "EMAIL".to_string(),
            normalized_value: Some(raw_val.to_lowercase()),
            raw_value: raw_val,
            source_evidence_id: source_evidence_id.to_string(),
            source_record_id: source_record_id.to_string(),
            location_reference: Some(format!("char_offset:{}-{}", char_start, char_end)),
            is_ambiguous: false,
            ambiguity_reason: None,
            context_snippet: Some(extract_context_window(&chars, char_start, char_end, 40)),
            is_checksum_valid: None,
            extraction_confidence: None,
            extraction_method: "regex_email".to_string(),
        });
    }

    // 2. 14-15 Digit Numeric Sequences (IMEI / IMSI / Ambiguous)
    for m in NUMERIC_14_15_PATTERN.find_iter(text) {
        let char_start = byte_to_char(m.start());
        let char_end = char_start + m.as_str().chars().count();
        let raw_val = m.as_str().trim().to_string();

        consumed_char_spans.push((char_start, char_end));

        // Check immediate preceding and following context in character space
        let pre_start = char_start.saturating_sub(30);
        let pre_ctx: String = chars[pre_start..char_start].iter().collect();

        let post_end = (char_end + 30).min(total_chars);
        let post_ctx: String = chars[char_end..post_end].iter().collect();

        let context_window = extract_context_window(&chars, char_start, char_end, 40);

        let has_pre_imei = IMEI_KEYWORDS.is_match(&pre_ctx);
        let has_pre_imsi = IMSI_KEYWORDS.is_match(&pre_ctx);
        let has_post_imei = IMEI_KEYWORDS.is_match(&post_ctx);
        let has_post_imsi = IMSI_KEYWORDS.is_match(&post_ctx);

        if has_pre_imei && !has_pre_imsi {
            identifiers.push(ExtractedIdentifier {
                identifier_type: "IMEI".to_string(),
                normalized_value: Some(raw_val.clone()),
                raw_value: raw_val.clone(),
                source_evidence_id: source_evidence_id.to_string(),
                source_record_id: source_record_id.to_string(),
                location_reference: Some(format!("char_offset:{}-{}", char_start, char_end)),
                is_ambiguous: false,
                ambiguity_reason: None,
                context_snippet: Some(context_window),
                is_checksum_valid: Some(luhn_checksum_valid(&raw_val)),
                extraction_confidence: None,
                extraction_method: "regex_imei_contextual".to_string(),
            });
        } else if has_pre_imsi && !has_pre_imei {
            identifiers.push(ExtractedIdentifier {
                identifier_type: "IMSI".to_string(),
                normalized_value: Some(raw_val.clone()),
                raw_value: raw_val.clone(),
                source_evidence_id: source_evidence_id.to_string(),
                source_record_id: source_record_id.to_string(),
                location_reference: Some(format!("char_offset:{}-{}", char_start, char_end)),
                is_ambiguous: false,
                ambiguity_reason: None,
                context_snippet: Some(context_window),
                is_checksum_valid: None,
                extraction_confidence: None,
                extraction_method: "regex_imsi_contextual".to_string(),
            });
        } else if has_post_imei && !has_post_imsi {
            identifiers.push(ExtractedIdentifier {
                identifier_type: "IMEI".to_string(),
                normalized_value: Some(raw_val.clone()),
                raw_value: raw_val.clone(),
                source_evidence_id: source_evidence_id.to_string(),
                source_record_id: source_record_id.to_string(),
                location_reference: Some(format!("char_offset:{}-{}", char_start, char_end)),
                is_ambiguous: false,
                ambiguity_reason: None,
                context_snippet: Some(context_window),
                is_checksum_valid: Some(luhn_checksum_valid(&raw_val)),
                extraction_confidence: None,
                extraction_method: "regex_imei_contextual".to_string(),
            });
        } else if has_post_imsi && !has_post_imei {
            identifiers.push(ExtractedIdentifier {
                identifier_type: "IMSI".to_string(),
                normalized_value: Some(raw_val.clone()),
                raw_value: raw_val.clone(),
                source_evidence_id: source_evidence_id.to_string(),
                source_record_id: source_record_id.to_string(),
                location_reference: Some(format!("char_offset:{}-{}", char_start, char_end)),
                is_ambiguous: false,
                ambiguity_reason: None,
                context_snippet: Some(context_window),
                is_checksum_valid: None,
                extraction_confidence: None,
                extraction_method: "regex_imsi_contextual".to_string(),
            });
        } else {
            // Ambiguous
            let reason = if (has_pre_imei || has_post_imei) && (has_pre_imsi || has_post_imsi) {
                "Conflicting IMEI and IMSI keywords in immediate context".to_string()
            } else {
                "Isolated 14/15-digit numeric sequence lacking explicit IMEI or IMSI contextual keywords".to_string()
            };

            identifiers.push(ExtractedIdentifier {
                identifier_type: "AMBIGUOUS_NUMERIC_15".to_string(),
                normalized_value: None,
                raw_value: raw_val,
                source_evidence_id: source_evidence_id.to_string(),
                source_record_id: source_record_id.to_string(),
                location_reference: Some(format!("char_offset:{}-{}", char_start, char_end)),
                is_ambiguous: true,
                ambiguity_reason: Some(reason),
                context_snippet: Some(context_window),
                is_checksum_valid: None,
                extraction_confidence: None,
                extraction_method: "regex_numeric_15".to_string(),
            });
        }
    }

    // 3. Phone Number Extraction
    for m in PHONE_PATTERN.find_iter(text) {
        let char_start = byte_to_char(m.start());
        let char_end = char_start + m.as_str().chars().count();

        // Avoid overlapping with previously consumed spans (emails, 14-15 digit numbers)
        let overlaps = consumed_char_spans.iter().any(|&(cs, ce)| {
            (cs <= char_start && char_start < ce) || (cs < char_end && char_end <= ce)
        });
        if overlaps {
            continue;
        }

        let raw_val = m.as_str().trim();
        let clean_raw = raw_val.trim_end_matches(['.', ',', ';', '!', '?', ')', ']']);
        let digits_only: String = clean_raw.chars().filter(|c| c.is_ascii_digit()).collect();

        // Minimum phone length 7 digits, max 15 digits
        if digits_only.len() < 7 || digits_only.len() > 15 {
            continue;
        }

        let clean_char_len = clean_raw.chars().count();
        let actual_char_end = char_start + clean_char_len;

        let norm_res = crate::correlation::resolution::normalize_phone_number(
            clean_raw,
            _default_country_code,
        );

        let (norm_val, is_ambig, ambig_reason) = if norm_res.is_normalized {
            (norm_res.normalized_e164, false, None)
        } else {
            (
                None,
                norm_res.resolution_status.contains("AMBIGUOUS")
                    || norm_res.resolution_status.contains("INVALID"),
                Some(norm_res.reason),
            )
        };

        identifiers.push(ExtractedIdentifier {
            identifier_type: "PHONE_NUMBER".to_string(),
            normalized_value: norm_val,
            raw_value: clean_raw.to_string(),
            source_evidence_id: source_evidence_id.to_string(),
            source_record_id: source_record_id.to_string(),
            location_reference: Some(format!("char_offset:{}-{}", char_start, actual_char_end)),
            is_ambiguous: is_ambig,
            ambiguity_reason: ambig_reason,
            context_snippet: Some(extract_context_window(
                &chars,
                char_start,
                actual_char_end,
                40,
            )),
            is_checksum_valid: None,
            extraction_confidence: None,
            extraction_method: "regex_phone".to_string(),
        });
    }

    identifiers
}
