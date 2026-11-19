//! Deterministic Entity & Identifier Resolution Module (Member 1).
//!
//! Implements deterministic phone number canonicalization (E.164-style), audit-safe
//! equivalence tracking, and provenance-preserving entity candidate deduplication.

use crate::extraction::model::{ExtractedIdentifier, ResolvedEntityCandidate};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Result of attempting to normalize a phone number string into standard E.164 format.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhoneNormalizationResult {
    pub raw_input: String,
    pub normalized_e164: Option<String>,
    pub is_normalized: bool,
    pub resolution_status: String,
    pub reason: String,
    pub country_prefix_applied: Option<String>,
}

/// Deterministically normalize a phone string into E.164 (+<country_code><national_number>).
///
/// Forensic rules:
/// - Never guess a country code if not explicit, unless default_country_prefix is configured.
/// - Preserves raw input exactly.
/// - Explicit international prefix (+) is accepted directly.
/// - International trunk '00' is replaced with '+'.
/// - Domestic leading trunk '0' is stripped only when default_country_prefix is provided.
pub fn normalize_phone_number(
    raw_phone: &str,
    default_country_prefix: Option<&str>,
) -> PhoneNormalizationResult {
    let cleaned = raw_phone.trim();
    if cleaned.is_empty() {
        return PhoneNormalizationResult {
            raw_input: raw_phone.to_string(),
            normalized_e164: None,
            is_normalized: false,
            resolution_status: "EMPTY_OR_NON_STRING".to_string(),
            reason: "Input phone number was empty or not a string.".to_string(),
            country_prefix_applied: None,
        };
    }

    let has_plus = cleaned.starts_with('+');
    let digits: String = cleaned.chars().filter(|c| c.is_ascii_digit()).collect();

    if digits.len() < 7 {
        return PhoneNormalizationResult {
            raw_input: cleaned.to_string(),
            normalized_e164: None,
            is_normalized: false,
            resolution_status: "INSUFFICIENT_DIGITS".to_string(),
            reason: format!(
                "Phone number contains only {} digits; minimum valid length is 7.",
                digits.len()
            ),
            country_prefix_applied: None,
        };
    }

    if digits.len() > 15 {
        return PhoneNormalizationResult {
            raw_input: cleaned.to_string(),
            normalized_e164: None,
            is_normalized: false,
            resolution_status: "EXCESSIVE_DIGITS".to_string(),
            reason: format!(
                "Phone number contains {} digits; maximum ITU-T E.164 length is 15.",
                digits.len()
            ),
            country_prefix_applied: None,
        };
    }

    // Case 1: Explicit leading '+'
    if has_plus {
        let norm = format!("+{}", digits);
        return PhoneNormalizationResult {
            raw_input: cleaned.to_string(),
            normalized_e164: Some(norm),
            is_normalized: true,
            resolution_status: "EXPLICIT_E164_PLUS".to_string(),
            reason: "Normalized using explicit leading '+' international country code indicator."
                .to_string(),
            country_prefix_applied: None,
        };
    }

    // Case 2: International trunk '00'
    if cleaned.starts_with("00") && digits.len() >= 9 {
        let norm = format!("+{}", &digits[2..]);
        return PhoneNormalizationResult {
            raw_input: cleaned.to_string(),
            normalized_e164: Some(norm),
            is_normalized: true,
            resolution_status: "INTERNATIONAL_TRUNK_00".to_string(),
            reason: "Normalized by replacing leading international trunk prefix '00' with '+'."
                .to_string(),
            country_prefix_applied: None,
        };
    }

    // Case 3: Domestic number requiring default country prefix
    if let Some(prefix) = default_country_prefix {
        let clean_prefix: String = prefix.chars().filter(|c| c.is_ascii_digit()).collect();
        if clean_prefix.is_empty() {
            return PhoneNormalizationResult {
                raw_input: cleaned.to_string(),
                normalized_e164: None,
                is_normalized: false,
                resolution_status: "INVALID_DEFAULT_PREFIX".to_string(),
                reason: format!(
                    "Provided default_country_prefix '{}' contains no digits.",
                    prefix
                ),
                country_prefix_applied: None,
            };
        }

        // If starts with leading domestic trunk '0' (e.g. 09811002233)
        let national_digits = digits.strip_prefix('0').unwrap_or(&digits);

        // Standard 10-digit national number check for common test environments
        if national_digits.len() == 10 {
            let norm = format!("+{}{}", clean_prefix, national_digits);
            let detail = if digits.starts_with('0') {
                "stripped domestic trunk 0"
            } else {
                "10-digit national number"
            };
            return PhoneNormalizationResult {
                raw_input: cleaned.to_string(),
                normalized_e164: Some(norm),
                is_normalized: true,
                resolution_status: "CONFIGURED_DEFAULT_COUNTRY_PREFIX".to_string(),
                reason: format!(
                    "Domestic number normalized to '+{}' using configured default country prefix assumption ({}).",
                    clean_prefix, detail
                ),
                country_prefix_applied: Some(clean_prefix),
            };
        }
    }

    // Case 4: Ambiguous without explicit country code and no default prefix
    PhoneNormalizationResult {
        raw_input: cleaned.to_string(),
        normalized_e164: None,
        is_normalized: false,
        resolution_status: "AMBIGUOUS_COUNTRY_CODE".to_string(),
        reason: "Number lacks explicit international indicator ('+' or '00') and no default country code was configured. Not normalized to prevent erroneous cross-jurisdiction entity merges.".to_string(),
        country_prefix_applied: None,
    }
}

/// Deterministically group and deduplicate extracted identifiers.
///
/// Phone numbers are grouped by normalized E.164 when resolvable.
/// Emails are grouped case-insensitively.
/// IMEI and IMSIs are grouped by exact numeric sequence.
/// Ambiguous identifiers remain isolated under their raw strings.
pub fn deduplicate_identifiers(
    identifiers: &[ExtractedIdentifier],
    default_country_prefix: Option<&str>,
) -> Vec<ResolvedEntityCandidate> {
    struct Group {
        identifier_type: String,
        raw_mentions: BTreeSet<String>,
        equivalence_reasons: BTreeSet<String>,
        source_evidence_ids: BTreeSet<String>,
        source_record_ids: BTreeSet<String>,
    }

    let mut grouped: BTreeMap<String, Group> = BTreeMap::new();

    for ident in identifiers {
        let (c_id, reason) = match ident.identifier_type.as_str() {
            "PHONE_NUMBER" => {
                let res = normalize_phone_number(&ident.raw_value, default_country_prefix);
                if let (true, Some(norm)) = (res.is_normalized, res.normalized_e164) {
                    (norm, res.reason)
                } else {
                    (
                        ident.raw_value.trim().to_string(),
                        format!("Unnormalized raw phone: {}", res.reason),
                    )
                }
            }
            "EMAIL" => (
                ident.raw_value.trim().to_lowercase(),
                "Case-insensitive exact email match".to_string(),
            ),
            "IMEI" | "IMSI" => (
                ident.raw_value.trim().to_string(),
                format!(
                    "Exact {} hardware/subscriber identifier match",
                    ident.identifier_type
                ),
            ),
            _ => (
                format!("{}:{}", ident.identifier_type, ident.raw_value.trim()),
                ident
                    .ambiguity_reason
                    .clone()
                    .unwrap_or_else(|| "Ambiguous raw mention kept isolated".to_string()),
            ),
        };

        let entry = grouped.entry(c_id.clone()).or_insert_with(|| Group {
            identifier_type: ident.identifier_type.clone(),
            raw_mentions: BTreeSet::new(),
            equivalence_reasons: BTreeSet::new(),
            source_evidence_ids: BTreeSet::new(),
            source_record_ids: BTreeSet::new(),
        });

        entry.raw_mentions.insert(ident.raw_value.clone());
        entry.equivalence_reasons.insert(reason);
        entry
            .source_evidence_ids
            .insert(ident.source_evidence_id.clone());
        entry
            .source_record_ids
            .insert(ident.source_record_id.clone());
    }

    grouped
        .into_iter()
        .map(|(c_id, group)| {
            ResolvedEntityCandidate::new(
                c_id,
                group.identifier_type,
                group.raw_mentions.into_iter().collect(),
                group.equivalence_reasons.into_iter().collect(),
                group.source_evidence_ids.into_iter().collect(),
                group.source_record_ids.into_iter().collect(),
            )
        })
        .collect()
}
