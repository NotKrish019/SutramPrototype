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

/// Validate whether a 10-digit numeric string corresponds to a valid Indian mobile number series.
///
/// Under DoT / TRAI National Numbering Plan regulations, Indian mobile numbers have 10 digits
/// and begin strictly with 6, 7, 8, or 9. Numbers beginning with 0-5 are landline STD codes,
/// level-1/2 service codes, or invalid numbers.
pub fn is_valid_indian_mobile_series(national_10: &str) -> bool {
    if national_10.len() != 10 {
        return false;
    }
    match national_10.chars().next() {
        Some('6' | '7' | '8' | '9') => national_10.chars().all(|c| c.is_ascii_digit()),
        _ => false,
    }
}

/// Deterministically normalize a phone string into E.164 (+<country_code><national_number>).
///
/// Forensic rules:
/// - Never guess a country code if not explicit, unless default_country_prefix is configured.
/// - Preserves raw input exactly.
/// - Explicit international prefix (+) is accepted directly.
/// - International trunk '00' is replaced with '+'.
/// - Domestic leading trunk '0' is stripped only when default_country_prefix is provided.
/// - Validates Indian mobile series (6, 7, 8, 9) before normalization under DoT / TRAI standards.
/// - Isolates invalid series and ambiguous landlines without merging.
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
        // Check for Indian country code (+91)
        if let Some(after_country_code) = digits.strip_prefix("91") {
            // Handle +91 (0) XXXXXXXXXX formatting (13 digits: 91 + trunk 0 + 10 digits)
            if digits.len() == 13 && digits.chars().nth(2) == Some('0') {
                let national = &digits[3..];
                if is_valid_indian_mobile_series(national) {
                    return PhoneNormalizationResult {
                        raw_input: cleaned.to_string(),
                        normalized_e164: Some(format!("+91{}", national)),
                        is_normalized: true,
                        resolution_status: "EXPLICIT_INDIAN_E164_TRUNK_STRIPPED".to_string(),
                        reason: "Normalized Indian mobile number using '+91' country code with domestic trunk '(0)' stripped.".to_string(),
                        country_prefix_applied: Some("91".to_string()),
                    };
                } else {
                    return PhoneNormalizationResult {
                        raw_input: cleaned.to_string(),
                        normalized_e164: None,
                        is_normalized: false,
                        resolution_status: "INVALID_INDIAN_MOBILE_SERIES".to_string(),
                        reason: format!(
                            "Indian national number '{}' after stripping trunk does not begin with a valid mobile series (6, 7, 8, 9).",
                            national
                        ),
                        country_prefix_applied: Some("91".to_string()),
                    };
                }
            } else if digits.len() == 12 {
                let national = after_country_code;
                if is_valid_indian_mobile_series(national) {
                    return PhoneNormalizationResult {
                        raw_input: cleaned.to_string(),
                        normalized_e164: Some(format!("+91{}", national)),
                        is_normalized: true,
                        resolution_status: "EXPLICIT_E164_PLUS".to_string(),
                        reason: "Normalized Indian mobile number using explicit leading '+91' country code.".to_string(),
                        country_prefix_applied: Some("91".to_string()),
                    };
                } else {
                    return PhoneNormalizationResult {
                        raw_input: cleaned.to_string(),
                        normalized_e164: None,
                        is_normalized: false,
                        resolution_status: "INVALID_INDIAN_MOBILE_SERIES".to_string(),
                        reason: format!(
                            "Indian number '+91{}' does not begin with a valid mobile series (6, 7, 8, 9). Ambiguous landline, shortcode, or invalid mobile.",
                            national
                        ),
                        country_prefix_applied: Some("91".to_string()),
                    };
                }
            } else if digits.len() < 12 {
                return PhoneNormalizationResult {
                    raw_input: cleaned.to_string(),
                    normalized_e164: None,
                    is_normalized: false,
                    resolution_status: "INVALID_INDIAN_PHONE_LENGTH".to_string(),
                    reason: format!(
                        "Indian number has only {} national digits; expected 10-digit mobile number.",
                        digits.len().saturating_sub(2)
                    ),
                    country_prefix_applied: Some("91".to_string()),
                };
            }
        }

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
        if digits.starts_with("0091") && digits.len() == 14 {
            let national = &digits[4..];
            if is_valid_indian_mobile_series(national) {
                return PhoneNormalizationResult {
                        raw_input: cleaned.to_string(),
                        normalized_e164: Some(format!("+91{}", national)),
                        is_normalized: true,
                        resolution_status: "INTERNATIONAL_TRUNK_00".to_string(),
                        reason: "Normalized Indian mobile number by replacing leading international trunk prefix '0091' with '+91'.".to_string(),
                        country_prefix_applied: Some("91".to_string()),
                    };
            } else {
                return PhoneNormalizationResult {
                        raw_input: cleaned.to_string(),
                        normalized_e164: None,
                        is_normalized: false,
                        resolution_status: "INVALID_INDIAN_MOBILE_SERIES".to_string(),
                        reason: format!(
                            "Indian number '0091{}' does not begin with a valid mobile series (6, 7, 8, 9).",
                            national
                        ),
                        country_prefix_applied: Some("91".to_string()),
                    };
            }
        }

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

    // Case 3: Indian 091 prefix (13 digits: domestic trunk 0 + country code 91 + 10-digit national number)
    if digits.starts_with("091") && digits.len() == 13 {
        let national = &digits[3..];
        if is_valid_indian_mobile_series(national) {
            return PhoneNormalizationResult {
                raw_input: cleaned.to_string(),
                normalized_e164: Some(format!("+91{}", national)),
                is_normalized: true,
                resolution_status: "INDIAN_TRUNK_091_PREFIX".to_string(),
                reason: "Normalized Indian mobile number by stripping domestic trunk '0' from '091' prefix.".to_string(),
                country_prefix_applied: Some("91".to_string()),
            };
        } else {
            return PhoneNormalizationResult {
                raw_input: cleaned.to_string(),
                normalized_e164: None,
                is_normalized: false,
                resolution_status: "INVALID_INDIAN_MOBILE_SERIES".to_string(),
                reason: format!(
                    "Indian number with '091' prefix contains '{}', which is not in valid mobile series (6, 7, 8, 9).",
                    national
                ),
                country_prefix_applied: Some("91".to_string()),
            };
        }
    }

    // Case 4: Indian 91 country code without '+' (12 digits total)
    if digits.starts_with("91") && digits.len() == 12 {
        let national = &digits[2..];
        if is_valid_indian_mobile_series(national) {
            return PhoneNormalizationResult {
                raw_input: cleaned.to_string(),
                normalized_e164: Some(format!("+91{}", national)),
                is_normalized: true,
                resolution_status: "INDIAN_COUNTRY_CODE_PREFIX".to_string(),
                reason: "Normalized Indian mobile number using leading '91' country code."
                    .to_string(),
                country_prefix_applied: Some("91".to_string()),
            };
        } else {
            return PhoneNormalizationResult {
                raw_input: cleaned.to_string(),
                normalized_e164: None,
                is_normalized: false,
                resolution_status: "INVALID_INDIAN_MOBILE_SERIES".to_string(),
                reason: format!(
                    "12-digit number starting with '91' contains '{}', not in valid Indian mobile series (6, 7, 8, 9).",
                    national
                ),
                country_prefix_applied: Some("91".to_string()),
            };
        }
    }

    // Case 5: Domestic number requiring default country prefix
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

        // When default country is India (91)
        if clean_prefix == "91" {
            // Check for 11 digits with leading trunk '0' (e.g. 09811002233)
            if digits.len() == 11 && digits.starts_with('0') {
                let national = &digits[1..];
                if is_valid_indian_mobile_series(national) {
                    return PhoneNormalizationResult {
                        raw_input: cleaned.to_string(),
                        normalized_e164: Some(format!("+91{}", national)),
                        is_normalized: true,
                        resolution_status: "CONFIGURED_DEFAULT_COUNTRY_PREFIX".to_string(),
                        reason: "Domestic Indian number normalized to '+91' using configured default country prefix assumption (stripped domestic trunk 0).".to_string(),
                        country_prefix_applied: Some("91".to_string()),
                    };
                } else {
                    return PhoneNormalizationResult {
                        raw_input: cleaned.to_string(),
                        normalized_e164: None,
                        is_normalized: false,
                        resolution_status: "INVALID_INDIAN_MOBILE_SERIES".to_string(),
                        reason: format!(
                            "Domestic 11-digit number '{}' with trunk '0' stripped yields '{}', which is not a valid Indian mobile series (6, 7, 8, 9). Ambiguous landline or invalid.",
                            digits, national
                        ),
                        country_prefix_applied: Some("91".to_string()),
                    };
                }
            }

            // Check for 10-digit national number
            if digits.len() == 10 {
                if is_valid_indian_mobile_series(&digits) {
                    return PhoneNormalizationResult {
                        raw_input: cleaned.to_string(),
                        normalized_e164: Some(format!("+91{}", digits)),
                        is_normalized: true,
                        resolution_status: "CONFIGURED_DEFAULT_COUNTRY_PREFIX".to_string(),
                        reason: "Domestic Indian number normalized to '+91' using configured default country prefix assumption (10-digit national number).".to_string(),
                        country_prefix_applied: Some("91".to_string()),
                    };
                } else {
                    return PhoneNormalizationResult {
                        raw_input: cleaned.to_string(),
                        normalized_e164: None,
                        is_normalized: false,
                        resolution_status: "INVALID_INDIAN_MOBILE_SERIES".to_string(),
                        reason: format!(
                            "Domestic 10-digit number '{}' starts with '{}', which is not a valid Indian mobile series (6, 7, 8, 9). Ambiguous landline, shortcode, or invalid number.",
                            digits,
                            digits.chars().next().unwrap_or('?')
                        ),
                        country_prefix_applied: Some("91".to_string()),
                    };
                }
            }
        } else {
            // General domestic number check for other countries (e.g. US +1)
            let national_digits = digits.strip_prefix('0').unwrap_or(&digits);
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
    }

    // Case 6: Ambiguous without explicit country code and no default prefix
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
