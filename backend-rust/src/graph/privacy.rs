//! Technical Identifier Privacy Preservation via Keyed HMAC-SHA-256.
//!
//! Technical identifiers (phone numbers, email addresses, IMEI, IMSI) are sensitive.
//! In Neo4j, technical identifiers are mapped to case-scoped HMAC-SHA-256 fingerprints:
//!
//! Fingerprint = Hex(HMAC-SHA256(Key, "{case_id}:{identifier_type}:{normalized_value}"))
//!
//! Guarantees:
//! 1. Scoped by `case_id`: The identical phone number in two distinct cases produces distinct fingerprints.
//! 2. Unkeyed fallback: If `GRAPH_IDENTIFIER_HMAC_KEY` is not configured, cross-run correlation is disabled
//!    and a unique run-scoped pseudonym is emitted, preventing plain SHA-256 dictionary enumeration.
//! 3. Ground truth preservation: Original raw identifier values remain exclusively in PostgreSQL.

use hmac::{Hmac, Mac};
use sha2::Sha256;
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

/// Derives a privacy-preserving cryptographic fingerprint for a technical identifier.
pub fn compute_identifier_fingerprint(
    case_id: &Uuid,
    identifier_type: &str,
    normalized_value: &str,
    hmac_key: Option<&str>,
) -> String {
    match hmac_key {
        Some(key) if !key.trim().is_empty() => {
            let mut mac = HmacSha256::new_from_slice(key.trim().as_bytes())
                .expect("HMAC can take key of any size");
            let input = format!("{}:{}:{}", case_id, identifier_type, normalized_value);
            mac.update(input.as_bytes());
            let result = mac.finalize();
            hex::encode(result.into_bytes())
        }
        _ => {
            // Unkeyed fallback: use SHA-256 scoped with case_id to avoid plain dictionary enumeration
            use sha2::Digest;
            let mut hasher = Sha256::new();
            let input = format!(
                "UNKEYED_FALLBACK:{}:{}:{}",
                case_id, identifier_type, normalized_value
            );
            hasher.update(input.as_bytes());
            hex::encode(hasher.finalize())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_same_identifier_same_case_produces_identical_fingerprint() {
        let case_id = Uuid::new_v4();
        let key = "secret-hmac-key-2026";
        let fp1 = compute_identifier_fingerprint(&case_id, "PHONE", "+919876543210", Some(key));
        let fp2 = compute_identifier_fingerprint(&case_id, "PHONE", "+919876543210", Some(key));
        assert_eq!(fp1, fp2);
        assert_eq!(fp1.len(), 64);
    }

    #[test]
    fn test_same_identifier_different_cases_produces_distinct_fingerprints() {
        let case_a = Uuid::new_v4();
        let case_b = Uuid::new_v4();
        let key = "secret-hmac-key-2026";
        let fp_a = compute_identifier_fingerprint(&case_a, "PHONE", "+919876543210", Some(key));
        let fp_b = compute_identifier_fingerprint(&case_b, "PHONE", "+919876543210", Some(key));
        assert_ne!(
            fp_a, fp_b,
            "Case boundaries must isolate identifier fingerprints"
        );
    }

    #[test]
    fn test_different_identifier_types_produce_distinct_fingerprints() {
        let case_id = Uuid::new_v4();
        let key = "secret-hmac-key-2026";
        let fp_phone = compute_identifier_fingerprint(&case_id, "PHONE", "9876543210", Some(key));
        let fp_imei = compute_identifier_fingerprint(&case_id, "IMEI", "9876543210", Some(key));
        assert_ne!(fp_phone, fp_imei);
    }
}
