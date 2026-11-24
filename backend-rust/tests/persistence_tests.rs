//! Tests for Phase 1J persistence models and custody integrity.

use serde_json::json;
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[test]
fn test_custody_hash_chaining_deterministic() {
    let evidence_id = Uuid::new_v4();
    let case_id = Uuid::new_v4();
    let action = "EVIDENCE_STORED";
    let actor = "SYSTEM_INGESTION";
    let now_str = "2026-11-18T12:00:00Z";
    let prev_hash = "0000000000000000000000000000000000000000000000000000000000000000";
    let details = json!({ "file_size_bytes": 1024 });

    let compute_hash = |prev: &str, act: &str| -> String {
        let mut hasher = Sha256::new();
        hasher.update(act.as_bytes());
        hasher.update(evidence_id.to_string().as_bytes());
        hasher.update(case_id.to_string().as_bytes());
        hasher.update(actor.as_bytes());
        hasher.update(now_str.as_bytes());
        hasher.update(prev.as_bytes());
        hasher.update(
            serde_json::to_string(&details)
                .unwrap_or_default()
                .as_bytes(),
        );
        hex::encode(hasher.finalize())
    };

    let hash_1 = compute_hash(prev_hash, action);
    assert_eq!(hash_1.len(), 64);

    // Identical input yields identical hash
    let hash_1_repeat = compute_hash(prev_hash, action);
    assert_eq!(hash_1, hash_1_repeat);

    // Chaining to event 2
    let hash_2 = compute_hash(&hash_1, "EXTRACTION_DISPATCHED");
    assert_ne!(hash_1, hash_2);
    assert_eq!(hash_2.len(), 64);
}

#[test]
fn test_database_config_defaults() {
    use sutram_backend_rust::config::AppConfig;
    let config = AppConfig::default();
    assert!(config.database_url.is_none());
    assert_eq!(config.database_max_connections, 10);
    assert_eq!(config.database_min_connections, 1);
    assert_eq!(config.database_connect_timeout_seconds, 5);
}

#[test]
fn test_checked_integer_conversion_overflow_safety() {
    // Normal values convert safely
    let normal_val: usize = 5000;
    assert_eq!(i32::try_from(normal_val).unwrap(), 5000);

    // Boundary value: i32::MAX converts safely
    let max_val: usize = i32::MAX as usize;
    assert_eq!(i32::try_from(max_val).unwrap(), i32::MAX);

    // Overflow value: exceeds i32::MAX must return Err
    let overflow_val: usize = (i32::MAX as usize) + 1;
    assert!(i32::try_from(overflow_val).is_err());

    // File size u64 -> i64 checked conversion
    let normal_size: u64 = 1024 * 1024 * 100;
    assert_eq!(i64::try_from(normal_size).unwrap(), 104857600);

    let overflow_size: u64 = (i64::MAX as u64) + 1;
    assert!(i64::try_from(overflow_size).is_err());
}

#[test]
fn test_reconstruction_result_run_id_serde_roundtrip() {
    use sutram_backend_rust::reconstruction::service::{
        ForensicReconstructionService, ReconstructionOptions,
    };
    use sutram_backend_rust::reconstruction::FullCaseReconstructionResult;

    let test_run_id = Uuid::new_v4().to_string();
    let service = ForensicReconstructionService::new();
    let opts = ReconstructionOptions {
        case_id: Some("00000000-0000-0000-0000-000000000001".to_string()),
        ..Default::default()
    };

    let mut result = service.reconstruct_case(&[], &opts).unwrap();
    result.reconstruction_run_id = Some(test_run_id.clone());
    result.run_id = Some(test_run_id.clone());

    let serialized = serde_json::to_value(&result).unwrap();
    assert_eq!(serialized["reconstruction_run_id"], test_run_id);
    assert_eq!(serialized["run_id"], test_run_id);

    let deserialized: FullCaseReconstructionResult = serde_json::from_value(serialized).unwrap();
    assert_eq!(
        deserialized.reconstruction_run_id.as_deref(),
        Some(test_run_id.as_str())
    );
    assert_eq!(deserialized.run_id.as_deref(), Some(test_run_id.as_str()));
}

#[test]
fn test_raw_evidence_integrity_verification() {
    use sutram_backend_rust::ingestion::hashing::IncrementalHasher;

    let raw_bytes_1 = b"FORENSIC_RAW_EVIDENCE_PAYLOAD_ORIGINAL";
    let raw_bytes_tampered = b"FORENSIC_RAW_EVIDENCE_PAYLOAD_TAMPERED";

    let mut hasher_1 = IncrementalHasher::new();
    hasher_1.update(raw_bytes_1);
    let digest_1 = hasher_1.finalize_hex();

    let mut hasher_repeat = IncrementalHasher::new();
    hasher_repeat.update(raw_bytes_1);
    let digest_repeat = hasher_repeat.finalize_hex();

    let mut hasher_tampered = IncrementalHasher::new();
    hasher_tampered.update(raw_bytes_tampered);
    let digest_tampered = hasher_tampered.finalize_hex();

    assert_eq!(
        digest_1, digest_repeat,
        "Identical raw bytes must produce identical SHA-256"
    );
    assert_ne!(
        digest_1, digest_tampered,
        "Any byte change in raw evidence must produce distinct SHA-256"
    );
}
