//! Phase 1I: Comprehensive End-to-End Rust Integration Tests.
//!
//! Validates the complete pipeline:
//! Ingestion -> Manifest Storage -> Unified Extraction -> Versioned Run Storage -> Active Pointer -> Reprocessing.
//!
//! Covers all 28 required test scenarios from Section 14:
//! 1. Upload CDR with auto_extract=true triggers extraction immediately and returns composite result.
//! 2. Upload chat JSON with auto_extract=true triggers extraction immediately.
//! 3. Upload with auto_extract=false creates evidence without triggering extraction.
//! 4. Upload response with auto_extract=true includes a correct evidence receipt.
//! 5. Upload response with auto_extract=true includes a successful extraction run with entities, events and relationships.
//! 6. Triggering extraction via POST /api/v1/evidence/:id/extract returns a successful extraction run.
//! 7. Subsequent GET /api/v1/evidence/:id/extraction returns the active extraction result.
//! 8. GET /api/v1/evidence/:id/extraction/runs returns the history of extraction runs for the evidence item.
//! 9. Failed extraction run does not mutate or delete the raw evidence file.
//! 10. Failed extraction run does not replace a previously active successful run.
//! 11. Reprocessing an evidence item with POST /api/v1/evidence/:id/reprocess creates a new extraction run with a new run_id.
//! 12. Reprocessing with a changed timezone produces a different configuration_hash.
//! 13. Reprocessing with a changed clock offset produces a different configuration_hash.
//! 14. Reprocessing with identical configuration preserves previous runs intact.
//! 15. Tampered raw evidence (SHA-256 mismatch) fails extraction with a structured error and does not update active-run.json.
//! 16. Ingestion of an unsupported evidence format followed by extraction returns a structured unsupported format error.
//! 17. Malformed CDR upload followed by extraction produces validation issues without panicking.
//! 18. Malformed chat JSON upload followed by extraction produces validation issues without panicking.
//! 19. Partial extraction result remains retrievable via GET /api/v1/evidence/:id/extraction if active.
//! 20. Restarting the backend preserves all historical extraction run manifests on disk.
//! 21. Restarting the backend preserves the active-run.json pointer on disk.
//! 22. Concurrent extraction requests for the same evidence item serialize cleanly without corrupting run metadata.
//! 23. Concurrent extraction runs across different evidence items proceed in parallel without deadlock.
//! 24. Interrupted run recovery: a run left in PENDING or RUNNING status on startup is marked as INTERRUPTED.
//! 25. Invalid evidence ID in extraction endpoints returns 400 Bad Request.
//! 26. Nonexistent evidence ID in extraction endpoints returns 404 Not Found.
//! 27. Extraction API responses never disclose raw server filesystem paths.
//! 28. Existing health check endpoint continues to return 200 OK.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use std::path::Path;
use sutram_backend_rust::{
    create_router, AppConfig, EvidenceUploadAndExtractResponse, EvidenceUploadReceipt,
    ExtractionConfiguration, ExtractionRunManifest, ExtractionRunStatus, ExtractionStatus,
    IngestionStatus, StorageManager, UnifiedExtractionResult,
};
use tempfile::tempdir;
use tower::ServiceExt;

const BOUNDARY: &str = "---------------------------boundary1234567890";

fn test_config(temp_dir: &Path) -> AppConfig {
    AppConfig {
        evidence_storage_path: temp_dir.join("raw"),
        derived_storage_path: temp_dir.join("derived"),
        ..Default::default()
    }
}

fn sample_cdr_bytes() -> Vec<u8> {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data")
        .join("synthetic")
        .join("sample_cdr.csv");
    std::fs::read(&p).expect("Failed to read synthetic sample_cdr.csv")
}

fn sample_chat_bytes() -> Vec<u8> {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data")
        .join("synthetic")
        .join("sample_chat_export.json");
    std::fs::read(&p).expect("Failed to read synthetic sample_chat_export.json")
}

fn create_multipart_payload(
    boundary: &str,
    case_id: Option<&str>,
    filename: Option<&str>,
    file_bytes: Option<&[u8]>,
    source_type: Option<&str>,
    auto_extract: Option<bool>,
    metadata_json: Option<&str>,
) -> Vec<u8> {
    let mut body = Vec::new();

    if let Some(cid) = case_id {
        body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body.extend_from_slice(b"Content-Disposition: form-data; name=\"case_id\"\r\n\r\n");
        body.extend_from_slice(cid.as_bytes());
        body.extend_from_slice(b"\r\n");
    }

    if let Some(st) = source_type {
        body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body.extend_from_slice(b"Content-Disposition: form-data; name=\"source_type\"\r\n\r\n");
        body.extend_from_slice(st.as_bytes());
        body.extend_from_slice(b"\r\n");
    }

    if let Some(ae) = auto_extract {
        body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body.extend_from_slice(b"Content-Disposition: form-data; name=\"auto_extract\"\r\n\r\n");
        body.extend_from_slice(if ae { b"true" } else { b"false" });
        body.extend_from_slice(b"\r\n");
    }

    if let Some(meta) = metadata_json {
        body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body.extend_from_slice(b"Content-Disposition: form-data; name=\"metadata\"\r\n\r\n");
        body.extend_from_slice(meta.as_bytes());
        body.extend_from_slice(b"\r\n");
    }

    if let (Some(fname), Some(bytes)) = (filename, file_bytes) {
        body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body.extend_from_slice(
            format!(
                "Content-Disposition: form-data; name=\"file\"; filename=\"{}\"\r\n",
                fname
            )
            .as_bytes(),
        );
        body.extend_from_slice(b"Content-Type: application/octet-stream\r\n\r\n");
        body.extend_from_slice(bytes);
        body.extend_from_slice(b"\r\n");
    }

    body.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());
    body
}

// 1. Upload CDR with auto_extract=true triggers extraction immediately and returns composite result
#[tokio::test]
async fn test_01_upload_cdr_auto_extract_true() {
    let temp = tempdir().unwrap();
    let config = test_config(temp.path());
    let app = create_router(config);

    let cdr_bytes = sample_cdr_bytes();
    let case_id = "550e8400-e29b-41d4-a716-446655440001";
    let body_bytes = create_multipart_payload(
        BOUNDARY,
        Some(case_id),
        Some("telecom_records.csv"),
        Some(&cdr_bytes),
        Some("CellPhoneCDR"),
        Some(true),
        None,
    );

    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/evidence/upload?auto_extract=true")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", BOUNDARY),
        )
        .body(Body::from(body_bytes))
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);

    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let composite: EvidenceUploadAndExtractResponse = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(composite.receipt.status, IngestionStatus::Ingested);
    assert_eq!(composite.receipt.case_id, case_id);
    assert_eq!(composite.extraction.status, ExtractionRunStatus::Success);
    assert!(composite.extraction.records_processed > 0);
    assert!(composite.extraction.valid_records_count > 0);

    // Verify extraction result has entities, events, relationships
    let get_req = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/v1/evidence/{}/extraction",
            composite.receipt.evidence_id
        ))
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(get_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let result: UnifiedExtractionResult = serde_json::from_slice(&bytes).unwrap();
    assert!(!result.identifiers.is_empty());
    assert!(!result.resolved_entities.is_empty());
    assert!(!result.event_candidates.is_empty());
    assert!(!result.relationship_candidates.is_empty());
}

// 2. Upload chat JSON with auto_extract=true triggers extraction immediately
#[tokio::test]
async fn test_02_upload_chat_json_auto_extract_true() {
    let temp = tempdir().unwrap();
    let config = test_config(temp.path());
    let app = create_router(config);

    let chat_bytes = sample_chat_bytes();
    let case_id = "550e8400-e29b-41d4-a716-446655440002";
    let body_bytes = create_multipart_payload(
        BOUNDARY,
        Some(case_id),
        Some("chat_export.json"),
        Some(&chat_bytes),
        Some("ChatExport"),
        Some(true),
        None,
    );

    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/evidence/upload")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", BOUNDARY),
        )
        .body(Body::from(body_bytes))
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);

    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let composite: EvidenceUploadAndExtractResponse = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(composite.extraction.status, ExtractionRunStatus::Success);
    assert!(composite.extraction.records_processed > 0);
    assert!(composite.extraction.valid_records_count > 0);

    // Verify extraction result
    let get_req = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/v1/evidence/{}/extraction",
            composite.receipt.evidence_id
        ))
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(get_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let result: UnifiedExtractionResult = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(result.source_type, "ChatExport");
    assert!(!result.event_candidates.is_empty());
    assert!(!result.resolved_entities.is_empty());
}

// 3. Upload with auto_extract=false creates evidence without triggering extraction
#[tokio::test]
async fn test_03_upload_auto_extract_false_or_omitted() {
    let temp = tempdir().unwrap();
    let config = test_config(temp.path());
    let app = create_router(config.clone());

    let cdr_bytes = sample_cdr_bytes();
    let case_id = "550e8400-e29b-41d4-a716-446655440003";
    let body_bytes = create_multipart_payload(
        BOUNDARY,
        Some(case_id),
        Some("records.csv"),
        Some(&cdr_bytes),
        Some("CellPhoneCDR"),
        Some(false),
        None,
    );

    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/evidence/upload")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", BOUNDARY),
        )
        .body(Body::from(body_bytes))
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);

    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let receipt: EvidenceUploadReceipt = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(receipt.status, IngestionStatus::Ingested);

    // Verify derived runs do not exist
    let storage = StorageManager::with_derived(
        config.evidence_storage_path.clone(),
        config.derived_storage_path.clone(),
    );
    let runs = storage
        .list_runs(case_id, &receipt.evidence_id)
        .await
        .unwrap();
    assert!(runs.is_empty());

    let pointer = storage
        .load_active_run_pointer(case_id, &receipt.evidence_id)
        .await
        .unwrap();
    assert!(pointer.is_none());
}

// 4. Upload response with auto_extract=true includes a correct evidence receipt
#[tokio::test]
async fn test_04_auto_extract_response_receipt_fields() {
    let temp = tempdir().unwrap();
    let config = test_config(temp.path());
    let app = create_router(config);

    let cdr_bytes = sample_cdr_bytes();
    let case_id = "550e8400-e29b-41d4-a716-446655440004";
    let body_bytes = create_multipart_payload(
        BOUNDARY,
        Some(case_id),
        Some("sample.csv"),
        Some(&cdr_bytes),
        Some("CellPhoneCDR"),
        Some(true),
        None,
    );

    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/evidence/upload?auto_extract=true")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", BOUNDARY),
        )
        .body(Body::from(body_bytes))
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);

    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let composite: EvidenceUploadAndExtractResponse = serde_json::from_slice(&bytes).unwrap();

    let receipt = composite.receipt;
    assert_eq!(receipt.case_id, case_id);
    assert!(!receipt.evidence_id.is_empty());
    assert_eq!(receipt.original_filename, "sample.csv");
    assert_eq!(receipt.file_size_bytes, cdr_bytes.len() as u64);
    assert!(!receipt.sha256_hash.is_empty());
    assert_eq!(receipt.status, IngestionStatus::Ingested);
    assert!(receipt.storage_uri.contains(&receipt.evidence_id));
}

// 5. Upload response with auto_extract=true includes a successful extraction run with entities, events and relationships
#[tokio::test]
async fn test_05_auto_extract_response_extraction_fields() {
    let temp = tempdir().unwrap();
    let config = test_config(temp.path());
    let app = create_router(config);

    let cdr_bytes = sample_cdr_bytes();
    let case_id = "550e8400-e29b-41d4-a716-446655440005";
    let body_bytes = create_multipart_payload(
        BOUNDARY,
        Some(case_id),
        Some("cdr.csv"),
        Some(&cdr_bytes),
        Some("CellPhoneCDR"),
        Some(true),
        None,
    );

    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/evidence/upload?auto_extract=true")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", BOUNDARY),
        )
        .body(Body::from(body_bytes))
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let composite: EvidenceUploadAndExtractResponse = serde_json::from_slice(&bytes).unwrap();

    let ext = composite.extraction;
    assert_eq!(ext.status, ExtractionRunStatus::Success);
    assert!(ext.records_processed > 0);
    assert!(ext.valid_records_count > 0);
    assert_eq!(ext.rejected_records_count, 0);
    assert_eq!(ext.validation_issue_count, 0);
    assert!(ext.duration_ms.is_some());

    // Verify entities, events and relationships exist in active extraction
    let get_req = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/v1/evidence/{}/extraction",
            composite.receipt.evidence_id
        ))
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(get_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let result: UnifiedExtractionResult = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(result.source_type, "CellPhoneCDR");
    assert!(!result.identifiers.is_empty());
    assert!(!result.resolved_entities.is_empty());
    assert!(!result.event_candidates.is_empty());
    assert!(!result.relationship_candidates.is_empty());
}

// 6. Triggering extraction via POST /api/v1/evidence/:id/extract returns a successful extraction run
#[tokio::test]
async fn test_06_trigger_extraction_endpoint() {
    let temp = tempdir().unwrap();
    let config = test_config(temp.path());
    let app = create_router(config);

    let cdr_bytes = sample_cdr_bytes();
    let case_id = "550e8400-e29b-41d4-a716-446655440006";
    let body_bytes = create_multipart_payload(
        BOUNDARY,
        Some(case_id),
        Some("cdr.csv"),
        Some(&cdr_bytes),
        Some("CellPhoneCDR"),
        Some(false),
        None,
    );

    // Upload first
    let upload_req = Request::builder()
        .method("POST")
        .uri("/api/v1/evidence/upload")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", BOUNDARY),
        )
        .body(Body::from(body_bytes))
        .unwrap();

    let res = app.clone().oneshot(upload_req).await.unwrap();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let receipt: EvidenceUploadReceipt = serde_json::from_slice(&bytes).unwrap();

    // Trigger extraction
    let extract_req = Request::builder()
        .method("POST")
        .uri(format!("/api/v1/evidence/{}/extract", receipt.evidence_id))
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(extract_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let manifest: ExtractionRunManifest = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(manifest.evidence_id, receipt.evidence_id);
    assert_eq!(manifest.case_id, case_id);
    assert_eq!(manifest.status, ExtractionRunStatus::Success);
    assert!(manifest.records_processed > 0);
    assert!(manifest.result_sha256.is_some());
}

// 7. Subsequent GET /api/v1/evidence/:id/extraction returns the active extraction result
#[tokio::test]
async fn test_07_get_active_extraction() {
    let temp = tempdir().unwrap();
    let config = test_config(temp.path());
    let app = create_router(config);

    let cdr_bytes = sample_cdr_bytes();
    let case_id = "550e8400-e29b-41d4-a716-446655440007";
    let body_bytes = create_multipart_payload(
        BOUNDARY,
        Some(case_id),
        Some("cdr.csv"),
        Some(&cdr_bytes),
        Some("CellPhoneCDR"),
        Some(true),
        None,
    );

    let upload_req = Request::builder()
        .method("POST")
        .uri("/api/v1/evidence/upload?auto_extract=true")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", BOUNDARY),
        )
        .body(Body::from(body_bytes))
        .unwrap();

    let res = app.clone().oneshot(upload_req).await.unwrap();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let composite: EvidenceUploadAndExtractResponse = serde_json::from_slice(&bytes).unwrap();
    let evidence_id = composite.receipt.evidence_id;

    // Get active extraction
    let get_req = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/evidence/{}/extraction", evidence_id))
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(get_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let result: UnifiedExtractionResult = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(result.evidence_id, evidence_id);
    assert_eq!(result.status, ExtractionStatus::Success);
    assert!(!result.identifiers.is_empty());
    assert!(!result.resolved_entities.is_empty());
    assert!(!result.event_candidates.is_empty());
}

// 8. GET /api/v1/evidence/:id/extraction/runs returns the history of extraction runs for the evidence item
#[tokio::test]
async fn test_08_list_extraction_runs() {
    let temp = tempdir().unwrap();
    let config = test_config(temp.path());
    let app = create_router(config);

    let cdr_bytes = sample_cdr_bytes();
    let case_id = "550e8400-e29b-41d4-a716-446655440008";
    let body_bytes = create_multipart_payload(
        BOUNDARY,
        Some(case_id),
        Some("cdr.csv"),
        Some(&cdr_bytes),
        Some("CellPhoneCDR"),
        Some(true),
        None,
    );

    let upload_req = Request::builder()
        .method("POST")
        .uri("/api/v1/evidence/upload?auto_extract=true")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", BOUNDARY),
        )
        .body(Body::from(body_bytes))
        .unwrap();

    let res = app.clone().oneshot(upload_req).await.unwrap();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let composite: EvidenceUploadAndExtractResponse = serde_json::from_slice(&bytes).unwrap();
    let evidence_id = composite.receipt.evidence_id;

    // List runs
    let list_req = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/evidence/{}/extraction/runs", evidence_id))
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(list_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let runs: Vec<ExtractionRunManifest> = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(runs.len(), 1);
    assert_eq!(
        runs[0].extraction_run_id,
        composite.extraction.extraction_run_id
    );
}

// 9. Failed extraction run does not mutate or delete the raw evidence file
#[tokio::test]
async fn test_09_failed_extraction_preserves_raw_evidence() {
    let temp = tempdir().unwrap();
    let config = test_config(temp.path());
    let app = create_router(config.clone());

    let cdr_bytes = sample_cdr_bytes();
    let case_id = "550e8400-e29b-41d4-a716-446655440009";
    let body_bytes = create_multipart_payload(
        BOUNDARY,
        Some(case_id),
        Some("cdr.csv"),
        Some(&cdr_bytes),
        Some("CellPhoneCDR"),
        Some(false),
        None,
    );

    let upload_req = Request::builder()
        .method("POST")
        .uri("/api/v1/evidence/upload")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", BOUNDARY),
        )
        .body(Body::from(body_bytes))
        .unwrap();

    let res = app.clone().oneshot(upload_req).await.unwrap();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let receipt: EvidenceUploadReceipt = serde_json::from_slice(&bytes).unwrap();

    // Tamper with raw evidence file directly on disk
    let storage = StorageManager::with_derived(
        config.evidence_storage_path.clone(),
        config.derived_storage_path.clone(),
    );
    let raw_path = storage
        .get_raw_file_path(case_id, &receipt.evidence_id)
        .unwrap();
    let mut modified_bytes = cdr_bytes.clone();
    modified_bytes[0] = b'X';
    tokio::fs::write(&raw_path, &modified_bytes).await.unwrap();

    // Now trigger extraction -> should fail with SHA-256 mismatch
    let extract_req = Request::builder()
        .method("POST")
        .uri(format!("/api/v1/evidence/{}/extract", receipt.evidence_id))
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(extract_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let manifest: ExtractionRunManifest = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(manifest.status, ExtractionRunStatus::Failed);

    // Verify raw evidence file is STILL preserved on disk exactly as it was
    assert!(raw_path.exists());
    let current_bytes = tokio::fs::read(&raw_path).await.unwrap();
    assert_eq!(current_bytes, modified_bytes);
}

// 10. Failed extraction run does not replace a previously active successful run
#[tokio::test]
async fn test_10_failed_extraction_does_not_replace_active_run() {
    let temp = tempdir().unwrap();
    let config = test_config(temp.path());
    let app = create_router(config.clone());

    let cdr_bytes = sample_cdr_bytes();
    let case_id = "550e8400-e29b-41d4-a716-446655440010";
    let body_bytes = create_multipart_payload(
        BOUNDARY,
        Some(case_id),
        Some("cdr.csv"),
        Some(&cdr_bytes),
        Some("CellPhoneCDR"),
        Some(true),
        None,
    );

    let upload_req = Request::builder()
        .method("POST")
        .uri("/api/v1/evidence/upload?auto_extract=true")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", BOUNDARY),
        )
        .body(Body::from(body_bytes))
        .unwrap();

    let res = app.clone().oneshot(upload_req).await.unwrap();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let composite: EvidenceUploadAndExtractResponse = serde_json::from_slice(&bytes).unwrap();
    let original_run_id = composite.extraction.extraction_run_id;

    // Verify active run points to original_run_id
    let storage = StorageManager::with_derived(
        config.evidence_storage_path.clone(),
        config.derived_storage_path.clone(),
    );
    let pointer = storage
        .load_active_run_pointer(case_id, &composite.receipt.evidence_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(pointer.active_run_id, original_run_id);

    // Tamper with raw evidence on disk so second run fails
    let raw_path = storage
        .get_raw_file_path(case_id, &composite.receipt.evidence_id)
        .unwrap();
    let mut tampered = cdr_bytes.clone();
    tampered[1] = b'Z';
    tokio::fs::write(&raw_path, &tampered).await.unwrap();

    // Trigger reprocess -> will fail
    let reprocess_req = Request::builder()
        .method("POST")
        .uri(format!(
            "/api/v1/evidence/{}/reprocess",
            composite.receipt.evidence_id
        ))
        .header("content-type", "application/json")
        .body(Body::from(b"{}".as_slice()))
        .unwrap();

    let res = app.clone().oneshot(reprocess_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let failed_manifest: ExtractionRunManifest = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(failed_manifest.status, ExtractionRunStatus::Failed);

    // Verify active run pointer STILL points to original_run_id
    let pointer_after = storage
        .load_active_run_pointer(case_id, &composite.receipt.evidence_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(pointer_after.active_run_id, original_run_id);

    // Verify GET /extraction STILL returns the original successful result
    let get_req = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/v1/evidence/{}/extraction",
            composite.receipt.evidence_id
        ))
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(get_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let active_res: UnifiedExtractionResult = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(active_res.status, ExtractionStatus::Success);
}

// 11. Reprocessing an evidence item with POST /api/v1/evidence/:id/reprocess creates a new extraction run with a new run_id
#[tokio::test]
async fn test_11_reprocess_creates_new_run_id() {
    let temp = tempdir().unwrap();
    let config = test_config(temp.path());
    let app = create_router(config);

    let cdr_bytes = sample_cdr_bytes();
    let case_id = "550e8400-e29b-41d4-a716-446655440011";
    let body_bytes = create_multipart_payload(
        BOUNDARY,
        Some(case_id),
        Some("cdr.csv"),
        Some(&cdr_bytes),
        Some("CellPhoneCDR"),
        Some(true),
        None,
    );

    let upload_req = Request::builder()
        .method("POST")
        .uri("/api/v1/evidence/upload?auto_extract=true")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", BOUNDARY),
        )
        .body(Body::from(body_bytes))
        .unwrap();

    let res = app.clone().oneshot(upload_req).await.unwrap();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let composite: EvidenceUploadAndExtractResponse = serde_json::from_slice(&bytes).unwrap();
    let run_1_id = composite.extraction.extraction_run_id;

    // Reprocess
    let reprocess_req = Request::builder()
        .method("POST")
        .uri(format!(
            "/api/v1/evidence/{}/reprocess",
            composite.receipt.evidence_id
        ))
        .header("content-type", "application/json")
        .body(Body::from(b"{}".as_slice()))
        .unwrap();

    let res = app.clone().oneshot(reprocess_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let run_2_manifest: ExtractionRunManifest = serde_json::from_slice(&bytes).unwrap();

    assert_ne!(run_2_manifest.extraction_run_id, run_1_id);
    assert_eq!(run_2_manifest.status, ExtractionRunStatus::Success);

    // List runs -> should be 2
    let list_req = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/v1/evidence/{}/extraction/runs",
            composite.receipt.evidence_id
        ))
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(list_req).await.unwrap();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let runs: Vec<ExtractionRunManifest> = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(runs.len(), 2);
}

// 12. Reprocessing with a changed timezone produces a different configuration_hash
#[tokio::test]
async fn test_12_reprocess_changed_timezone_hash() {
    let temp = tempdir().unwrap();
    let config = test_config(temp.path());
    let app = create_router(config);

    let cdr_bytes = sample_cdr_bytes();
    let case_id = "550e8400-e29b-41d4-a716-446655440012";
    let body_bytes = create_multipart_payload(
        BOUNDARY,
        Some(case_id),
        Some("cdr.csv"),
        Some(&cdr_bytes),
        Some("CellPhoneCDR"),
        Some(true),
        None,
    );

    let upload_req = Request::builder()
        .method("POST")
        .uri("/api/v1/evidence/upload?auto_extract=true")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", BOUNDARY),
        )
        .body(Body::from(body_bytes))
        .unwrap();

    let res = app.clone().oneshot(upload_req).await.unwrap();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let composite: EvidenceUploadAndExtractResponse = serde_json::from_slice(&bytes).unwrap();

    // Check run 1 manifest hash
    let list_req = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/v1/evidence/{}/extraction/runs",
            composite.receipt.evidence_id
        ))
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(list_req).await.unwrap();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let runs: Vec<ExtractionRunManifest> = serde_json::from_slice(&bytes).unwrap();
    let hash_1 = runs[0].configuration_hash.clone();

    // Reprocess with different timezone
    let payload = serde_json::json!({
        "source_timezone": "Asia/Kolkata"
    });
    let reprocess_req = Request::builder()
        .method("POST")
        .uri(format!(
            "/api/v1/evidence/{}/reprocess",
            composite.receipt.evidence_id
        ))
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&payload).unwrap()))
        .unwrap();

    let res = app.oneshot(reprocess_req).await.unwrap();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let run_2_manifest: ExtractionRunManifest = serde_json::from_slice(&bytes).unwrap();
    let hash_2 = run_2_manifest.configuration_hash;

    assert_ne!(hash_1, hash_2);
}

// 13. Reprocessing with a changed clock offset produces a different configuration_hash
#[tokio::test]
async fn test_13_reprocess_changed_clock_offset_hash() {
    let temp = tempdir().unwrap();
    let config = test_config(temp.path());
    let app = create_router(config);

    let cdr_bytes = sample_cdr_bytes();
    let case_id = "550e8400-e29b-41d4-a716-446655440013";
    let body_bytes = create_multipart_payload(
        BOUNDARY,
        Some(case_id),
        Some("cdr.csv"),
        Some(&cdr_bytes),
        Some("CellPhoneCDR"),
        Some(true),
        None,
    );

    let upload_req = Request::builder()
        .method("POST")
        .uri("/api/v1/evidence/upload?auto_extract=true")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", BOUNDARY),
        )
        .body(Body::from(body_bytes))
        .unwrap();

    let res = app.clone().oneshot(upload_req).await.unwrap();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let composite: EvidenceUploadAndExtractResponse = serde_json::from_slice(&bytes).unwrap();

    // Run 1 hash
    let list_req = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/v1/evidence/{}/extraction/runs",
            composite.receipt.evidence_id
        ))
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(list_req).await.unwrap();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let runs: Vec<ExtractionRunManifest> = serde_json::from_slice(&bytes).unwrap();
    let hash_1 = runs[0].configuration_hash.clone();

    // Reprocess with clock offset
    let payload = serde_json::json!({
        "clock_offset_seconds": 3600.0
    });
    let reprocess_req = Request::builder()
        .method("POST")
        .uri(format!(
            "/api/v1/evidence/{}/reprocess",
            composite.receipt.evidence_id
        ))
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&payload).unwrap()))
        .unwrap();

    let res = app.oneshot(reprocess_req).await.unwrap();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let run_2_manifest: ExtractionRunManifest = serde_json::from_slice(&bytes).unwrap();
    let hash_2 = run_2_manifest.configuration_hash;

    assert_ne!(hash_1, hash_2);
}

// 14. Reprocessing with identical configuration preserves the previous successful result
#[tokio::test]
async fn test_14_reprocess_preserves_previous_successful_result() {
    let temp = tempdir().unwrap();
    let config = test_config(temp.path());
    let app = create_router(config.clone());

    let cdr_bytes = sample_cdr_bytes();
    let case_id = "550e8400-e29b-41d4-a716-446655440014";
    let body_bytes = create_multipart_payload(
        BOUNDARY,
        Some(case_id),
        Some("cdr.csv"),
        Some(&cdr_bytes),
        Some("CellPhoneCDR"),
        Some(true),
        None,
    );

    let upload_req = Request::builder()
        .method("POST")
        .uri("/api/v1/evidence/upload?auto_extract=true")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", BOUNDARY),
        )
        .body(Body::from(body_bytes))
        .unwrap();

    let res = app.clone().oneshot(upload_req).await.unwrap();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let composite: EvidenceUploadAndExtractResponse = serde_json::from_slice(&bytes).unwrap();
    let run_1_id = composite.extraction.extraction_run_id;

    // Reprocess
    let reprocess_req = Request::builder()
        .method("POST")
        .uri(format!(
            "/api/v1/evidence/{}/reprocess",
            composite.receipt.evidence_id
        ))
        .header("content-type", "application/json")
        .body(Body::from(b"{}".as_slice()))
        .unwrap();

    let _res = app.clone().oneshot(reprocess_req).await.unwrap();

    // Verify run 1 artifacts on disk are still intact
    let storage = StorageManager::with_derived(
        config.evidence_storage_path.clone(),
        config.derived_storage_path.clone(),
    );
    let run_1_manifest = storage
        .load_run_manifest(case_id, &composite.receipt.evidence_id, &run_1_id)
        .await
        .unwrap();
    assert_eq!(run_1_manifest.extraction_run_id, run_1_id);

    let run_1_result = storage
        .load_run_result(case_id, &composite.receipt.evidence_id, &run_1_id)
        .await
        .unwrap();
    assert_eq!(run_1_result.status, ExtractionStatus::Success);
}

// 15. Tampered raw evidence (SHA-256 mismatch) fails extraction with a structured error and does not update active-run.json
#[tokio::test]
async fn test_15_sha256_mismatch_prevents_extraction() {
    let temp = tempdir().unwrap();
    let config = test_config(temp.path());
    let app = create_router(config.clone());

    let cdr_bytes = sample_cdr_bytes();
    let case_id = "550e8400-e29b-41d4-a716-446655440015";
    let body_bytes = create_multipart_payload(
        BOUNDARY,
        Some(case_id),
        Some("cdr.csv"),
        Some(&cdr_bytes),
        Some("CellPhoneCDR"),
        Some(false),
        None,
    );

    let upload_req = Request::builder()
        .method("POST")
        .uri("/api/v1/evidence/upload")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", BOUNDARY),
        )
        .body(Body::from(body_bytes))
        .unwrap();

    let res = app.clone().oneshot(upload_req).await.unwrap();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let receipt: EvidenceUploadReceipt = serde_json::from_slice(&bytes).unwrap();

    // Corrupt 1 byte in the raw file
    let storage = StorageManager::with_derived(
        config.evidence_storage_path.clone(),
        config.derived_storage_path.clone(),
    );
    let raw_path = storage
        .get_raw_file_path(case_id, &receipt.evidence_id)
        .unwrap();
    let mut bad_bytes = cdr_bytes.clone();
    bad_bytes[5] = b'!';
    tokio::fs::write(&raw_path, &bad_bytes).await.unwrap();

    // Trigger extraction
    let extract_req = Request::builder()
        .method("POST")
        .uri(format!("/api/v1/evidence/{}/extract", receipt.evidence_id))
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(extract_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let manifest: ExtractionRunManifest = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(manifest.status, ExtractionRunStatus::Failed);
    assert!(manifest
        .error_message
        .unwrap()
        .contains("Cryptographic hash mismatch"));

    // Active run pointer should NOT exist
    let pointer = storage
        .load_active_run_pointer(case_id, &receipt.evidence_id)
        .await
        .unwrap();
    assert!(pointer.is_none());
}

// 16. Ingestion of an unsupported evidence format followed by extraction returns a structured unsupported format error
#[tokio::test]
async fn test_16_unsupported_evidence_format() {
    let temp = tempdir().unwrap();
    let config = test_config(temp.path());
    let app = create_router(config);

    let pdf_content = b"%PDF-1.4\n1 0 obj\n<<>>\nendobj\ntrailer\n<<>>\n%%EOF\n";
    let case_id = "550e8400-e29b-41d4-a716-446655440016";
    let body_bytes = create_multipart_payload(
        BOUNDARY,
        Some(case_id),
        Some("document.pdf"),
        Some(pdf_content),
        Some("DocumentPDF"),
        Some(false),
        None,
    );

    let upload_req = Request::builder()
        .method("POST")
        .uri("/api/v1/evidence/upload")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", BOUNDARY),
        )
        .body(Body::from(body_bytes))
        .unwrap();

    let res = app.clone().oneshot(upload_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let receipt: EvidenceUploadReceipt = serde_json::from_slice(&bytes).unwrap();

    // Trigger extraction on PDF
    let extract_req = Request::builder()
        .method("POST")
        .uri(format!("/api/v1/evidence/{}/extract", receipt.evidence_id))
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(extract_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let manifest: ExtractionRunManifest = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(manifest.status, ExtractionRunStatus::Failed);
    assert!(manifest
        .error_message
        .unwrap()
        .contains("UNSUPPORTED_SOURCE_TYPE"));
}

// 17. Malformed CDR upload followed by extraction produces validation issues and PARTIAL_SUCCESS or FAILED without panicking
#[tokio::test]
async fn test_17_malformed_cdr_validation_issues() {
    let temp = tempdir().unwrap();
    let config = test_config(temp.path());
    let app = create_router(config);

    // Header without required timestamp column
    let malformed_cdr =
        b"call_id,calling_number,called_number,call_duration\n1,+12025550101,+12025550102,60\n";
    let case_id = "550e8400-e29b-41d4-a716-446655440017";
    let body_bytes = create_multipart_payload(
        BOUNDARY,
        Some(case_id),
        Some("bad_cdr.csv"),
        Some(malformed_cdr),
        Some("CellPhoneCDR"),
        Some(true),
        None,
    );

    let upload_req = Request::builder()
        .method("POST")
        .uri("/api/v1/evidence/upload?auto_extract=true")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", BOUNDARY),
        )
        .body(Body::from(body_bytes))
        .unwrap();

    let res = app.oneshot(upload_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let composite: EvidenceUploadAndExtractResponse = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(composite.extraction.status, ExtractionRunStatus::Failed);
    assert!(composite.extraction.validation_issue_count > 0);
}

// 18. Malformed chat JSON upload followed by extraction produces validation issues without panicking
#[tokio::test]
async fn test_18_malformed_chat_json_validation_issues() {
    let temp = tempdir().unwrap();
    let config = test_config(temp.path());
    let app = create_router(config);

    // Chat JSON with message missing required sender_id field
    let malformed_chat = br#"{
        "chat_id": "test-chat",
        "messages": [
            {
                "message_id": "m2",
                "timestamp": "2026-03-01T10:00:00Z",
                "content": "no sender"
            }
        ]
    }"#;

    let case_id = "550e8400-e29b-41d4-a716-446655440018";
    let body_bytes = create_multipart_payload(
        BOUNDARY,
        Some(case_id),
        Some("bad_chat.json"),
        Some(malformed_chat),
        Some("ChatExport"),
        Some(true),
        None,
    );

    let upload_req = Request::builder()
        .method("POST")
        .uri("/api/v1/evidence/upload?auto_extract=true")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", BOUNDARY),
        )
        .body(Body::from(body_bytes))
        .unwrap();

    let res = app.oneshot(upload_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let composite: EvidenceUploadAndExtractResponse = serde_json::from_slice(&bytes).unwrap();

    assert!(composite.extraction.validation_issue_count > 0);
}

// 19. Partial extraction result remains retrievable via GET /api/v1/evidence/:id/extraction if active
#[tokio::test]
async fn test_19_partial_extraction_retrievable() {
    let temp = tempdir().unwrap();
    let config = test_config(temp.path());
    let app = create_router(config);

    // CDR with 1 valid row and 1 invalid row (duration -10) -> PartialSuccess
    let partial_cdr = b"record_id,caller_phone,receiver_phone,call_timestamp,duration_seconds\nrec-01,+919811002233,+919899887766,2026-11-10T12:00:00Z,60\nrec-02,+919811002233,+919899887766,2026-11-10T12:05:00Z,-10\n";

    let case_id = "550e8400-e29b-41d4-a716-446655440019";
    let body_bytes = create_multipart_payload(
        BOUNDARY,
        Some(case_id),
        Some("partial_cdr.csv"),
        Some(partial_cdr),
        Some("CALL_DETAIL_RECORD"),
        Some(true),
        None,
    );

    let upload_req = Request::builder()
        .method("POST")
        .uri("/api/v1/evidence/upload?auto_extract=true")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", BOUNDARY),
        )
        .body(Body::from(body_bytes))
        .unwrap();

    let res = app.clone().oneshot(upload_req).await.unwrap();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let composite: EvidenceUploadAndExtractResponse = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(
        composite.extraction.status,
        ExtractionRunStatus::PartialSuccess
    );

    // Retrieve active extraction
    let get_req = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/v1/evidence/{}/extraction",
            composite.receipt.evidence_id
        ))
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(get_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let result: UnifiedExtractionResult = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(result.status, ExtractionStatus::PartialSuccess);
    assert_eq!(result.valid_records_count, 1);
    assert_eq!(result.rejected_records_count, 1);
}

// 20. Restarting the backend preserves all historical extraction run manifests on disk
#[tokio::test]
async fn test_20_restart_preserves_run_manifests() {
    let temp = tempdir().unwrap();
    let config = test_config(temp.path());

    let evidence_id;
    let run_1_id;
    let run_2_id;

    // Instance 1
    {
        let app = create_router(config.clone());
        let cdr_bytes = sample_cdr_bytes();
        let case_id = "550e8400-e29b-41d4-a716-446655440020";
        let body_bytes = create_multipart_payload(
            BOUNDARY,
            Some(case_id),
            Some("cdr.csv"),
            Some(&cdr_bytes),
            Some("CellPhoneCDR"),
            Some(true),
            None,
        );

        let upload_req = Request::builder()
            .method("POST")
            .uri("/api/v1/evidence/upload?auto_extract=true")
            .header(
                "content-type",
                format!("multipart/form-data; boundary={}", BOUNDARY),
            )
            .body(Body::from(body_bytes))
            .unwrap();

        let res = app.clone().oneshot(upload_req).await.unwrap();
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let composite: EvidenceUploadAndExtractResponse = serde_json::from_slice(&bytes).unwrap();
        evidence_id = composite.receipt.evidence_id;
        run_1_id = composite.extraction.extraction_run_id;

        // Reprocess to get run 2
        let reprocess_req = Request::builder()
            .method("POST")
            .uri(format!("/api/v1/evidence/{}/reprocess", evidence_id))
            .header("content-type", "application/json")
            .body(Body::from(b"{}".as_slice()))
            .unwrap();

        let res = app.oneshot(reprocess_req).await.unwrap();
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let run_2: ExtractionRunManifest = serde_json::from_slice(&bytes).unwrap();
        run_2_id = run_2.extraction_run_id;
    }

    // Instance 2 (Simulated restart)
    {
        let restarted_app = create_router(config);
        let list_req = Request::builder()
            .method("GET")
            .uri(format!("/api/v1/evidence/{}/extraction/runs", evidence_id))
            .body(Body::empty())
            .unwrap();

        let res = restarted_app.oneshot(list_req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let runs: Vec<ExtractionRunManifest> = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(runs.len(), 2);
        let run_ids: Vec<String> = runs.into_iter().map(|r| r.extraction_run_id).collect();
        assert!(run_ids.contains(&run_1_id));
        assert!(run_ids.contains(&run_2_id));
    }
}

// 21. Restarting the backend preserves the active-run.json pointer on disk
#[tokio::test]
async fn test_21_restart_preserves_active_run_pointer() {
    let temp = tempdir().unwrap();
    let config = test_config(temp.path());

    let evidence_id;

    // Instance 1
    {
        let app = create_router(config.clone());
        let cdr_bytes = sample_cdr_bytes();
        let case_id = "550e8400-e29b-41d4-a716-446655440021";
        let body_bytes = create_multipart_payload(
            BOUNDARY,
            Some(case_id),
            Some("cdr.csv"),
            Some(&cdr_bytes),
            Some("CellPhoneCDR"),
            Some(true),
            None,
        );

        let upload_req = Request::builder()
            .method("POST")
            .uri("/api/v1/evidence/upload?auto_extract=true")
            .header(
                "content-type",
                format!("multipart/form-data; boundary={}", BOUNDARY),
            )
            .body(Body::from(body_bytes))
            .unwrap();

        let res = app.oneshot(upload_req).await.unwrap();
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let composite: EvidenceUploadAndExtractResponse = serde_json::from_slice(&bytes).unwrap();
        evidence_id = composite.receipt.evidence_id;
    }

    // Instance 2 (Simulated restart)
    {
        let restarted_app = create_router(config);
        let get_req = Request::builder()
            .method("GET")
            .uri(format!("/api/v1/evidence/{}/extraction", evidence_id))
            .body(Body::empty())
            .unwrap();

        let res = restarted_app.oneshot(get_req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let result: UnifiedExtractionResult = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(result.status, ExtractionStatus::Success);
    }
}

// 22. Concurrent extraction requests for the same evidence item serialize cleanly without corrupting run metadata
#[tokio::test]
async fn test_22_concurrent_extraction_same_evidence() {
    let temp = tempdir().unwrap();
    let config = test_config(temp.path());
    let app = create_router(config.clone());

    let cdr_bytes = sample_cdr_bytes();
    let case_id = "550e8400-e29b-41d4-a716-446655440022";
    let body_bytes = create_multipart_payload(
        BOUNDARY,
        Some(case_id),
        Some("cdr.csv"),
        Some(&cdr_bytes),
        Some("CellPhoneCDR"),
        Some(false),
        None,
    );

    let upload_req = Request::builder()
        .method("POST")
        .uri("/api/v1/evidence/upload")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", BOUNDARY),
        )
        .body(Body::from(body_bytes))
        .unwrap();

    let res = app.clone().oneshot(upload_req).await.unwrap();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let receipt: EvidenceUploadReceipt = serde_json::from_slice(&bytes).unwrap();
    let evidence_id = receipt.evidence_id;

    // Concurrently trigger 2 extractions for the exact same evidence_id
    let req1 = Request::builder()
        .method("POST")
        .uri(format!("/api/v1/evidence/{}/reprocess", evidence_id))
        .header("content-type", "application/json")
        .body(Body::from(b"{}".as_slice()))
        .unwrap();

    let req2 = Request::builder()
        .method("POST")
        .uri(format!("/api/v1/evidence/{}/reprocess", evidence_id))
        .header("content-type", "application/json")
        .body(Body::from(b"{}".as_slice()))
        .unwrap();

    let app1 = app.clone();
    let app2 = app.clone();

    let (res1, res2) = tokio::join!(app1.oneshot(req1), app2.oneshot(req2),);

    assert_eq!(res1.unwrap().status(), StatusCode::OK);
    assert_eq!(res2.unwrap().status(), StatusCode::OK);

    // Verify both runs are registered and history is uncorrupted
    let storage = StorageManager::with_derived(
        config.evidence_storage_path.clone(),
        config.derived_storage_path.clone(),
    );
    let runs = storage.list_runs(case_id, &evidence_id).await.unwrap();
    assert_eq!(runs.len(), 2);
    assert_ne!(runs[0].extraction_run_id, runs[1].extraction_run_id);
}

// 23. Concurrent extraction runs across different evidence items proceed in parallel without deadlock
#[tokio::test]
async fn test_23_concurrent_extraction_different_evidence() {
    let temp = tempdir().unwrap();
    let config = test_config(temp.path());
    let app = create_router(config);

    let cdr_bytes = sample_cdr_bytes();
    let chat_bytes = sample_chat_bytes();
    let case_id = "550e8400-e29b-41d4-a716-446655440023";

    // Upload CDR
    let cdr_payload = create_multipart_payload(
        BOUNDARY,
        Some(case_id),
        Some("cdr.csv"),
        Some(&cdr_bytes),
        Some("CellPhoneCDR"),
        Some(false),
        None,
    );
    let req1 = Request::builder()
        .method("POST")
        .uri("/api/v1/evidence/upload")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", BOUNDARY),
        )
        .body(Body::from(cdr_payload))
        .unwrap();
    let res1 = app.clone().oneshot(req1).await.unwrap();
    let bytes1 = res1.into_body().collect().await.unwrap().to_bytes();
    let receipt1: EvidenceUploadReceipt = serde_json::from_slice(&bytes1).unwrap();

    // Upload Chat
    let chat_payload = create_multipart_payload(
        BOUNDARY,
        Some(case_id),
        Some("chat.json"),
        Some(&chat_bytes),
        Some("ChatExport"),
        Some(false),
        None,
    );
    let req2 = Request::builder()
        .method("POST")
        .uri("/api/v1/evidence/upload")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", BOUNDARY),
        )
        .body(Body::from(chat_payload))
        .unwrap();
    let res2 = app.clone().oneshot(req2).await.unwrap();
    let bytes2 = res2.into_body().collect().await.unwrap().to_bytes();
    let receipt2: EvidenceUploadReceipt = serde_json::from_slice(&bytes2).unwrap();

    // Trigger extraction on both simultaneously
    let ext1 = Request::builder()
        .method("POST")
        .uri(format!("/api/v1/evidence/{}/extract", receipt1.evidence_id))
        .body(Body::empty())
        .unwrap();

    let ext2 = Request::builder()
        .method("POST")
        .uri(format!("/api/v1/evidence/{}/extract", receipt2.evidence_id))
        .body(Body::empty())
        .unwrap();

    let app1 = app.clone();
    let app2 = app.clone();

    let (res_a, res_b) = tokio::join!(app1.oneshot(ext1), app2.oneshot(ext2),);

    assert_eq!(res_a.unwrap().status(), StatusCode::OK);
    assert_eq!(res_b.unwrap().status(), StatusCode::OK);
}

// 24. Interrupted run recovery: a run left in PENDING or RUNNING status on startup is marked as INTERRUPTED
#[tokio::test]
async fn test_24_interrupted_run_recovery() {
    let temp = tempdir().unwrap();
    let config = test_config(temp.path());
    let storage = StorageManager::with_derived(
        config.evidence_storage_path.clone(),
        config.derived_storage_path.clone(),
    );

    let case_id = "550e8400-e29b-41d4-a716-446655440024";
    let evidence_id = "770e8400-e29b-41d4-a716-446655440024";
    let run_id = "run-simulated-crash-001";

    // Manually create a run manifest with status RUNNING
    let run_manifest = ExtractionRunManifest {
        extraction_run_id: run_id.to_string(),
        evidence_id: evidence_id.to_string(),
        case_id: case_id.to_string(),
        source_sha256: "somehash".to_string(),
        extractor_name: "yukti_unified_extractor".to_string(),
        extractor_version: "1.0.0".to_string(),
        configuration_hash: "confighash".to_string(),
        configuration: ExtractionConfiguration {
            extractor_name: "yukti_unified_extractor".to_string(),
            extractor_version: "1.0.0".to_string(),
            source_timezone: Some("UTC".to_string()),
            clock_offset_seconds: Some(0.0),
            default_country_prefix: Some("+1".to_string()),
            custom_options: serde_json::json!({}),
        },
        started_at: "2026-03-01T12:00:00Z".to_string(),
        completed_at: None,
        duration_ms: None,
        status: ExtractionRunStatus::Running,
        validation_issue_count: 0,
        records_processed: 0,
        valid_records_count: 0,
        rejected_records_count: 0,
        result_sha256: None,
        error_message: None,
    };

    storage.save_run_manifest(&run_manifest).await.unwrap();

    // Trigger recovery
    let recovered_count = storage.recover_interrupted_runs().await.unwrap();
    assert_eq!(recovered_count, 1);

    // Verify status was changed to Interrupted
    let updated = storage
        .load_run_manifest(case_id, evidence_id, run_id)
        .await
        .unwrap();
    assert_eq!(updated.status, ExtractionRunStatus::Interrupted);
    assert!(updated.completed_at.is_some());
    assert!(updated.error_message.unwrap().contains("interrupted"));
}

// 25. Invalid evidence ID in extraction endpoints returns 400 Bad Request
#[tokio::test]
async fn test_25_invalid_evidence_id_traversal() {
    let temp = tempdir().unwrap();
    let config = test_config(temp.path());
    let app = create_router(config);

    let malicious_id = "..%2F..%2Fetc";
    let req = Request::builder()
        .method("POST")
        .uri(format!("/api/v1/evidence/{}/extract", malicious_id))
        .body(Body::empty())
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

// 26. Nonexistent evidence ID in extraction endpoints returns 404 Not Found
#[tokio::test]
async fn test_26_nonexistent_evidence_id() {
    let temp = tempdir().unwrap();
    let config = test_config(temp.path());
    let app = create_router(config);

    let ghost_id = "00000000-0000-0000-0000-000000000000";
    let req = Request::builder()
        .method("POST")
        .uri(format!("/api/v1/evidence/{}/extract", ghost_id))
        .body(Body::empty())
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

// 27. Extraction API responses never disclose raw server filesystem paths
#[tokio::test]
async fn test_27_no_absolute_path_disclosure() {
    let temp = tempdir().unwrap();
    let config = test_config(temp.path());
    let app = create_router(config);

    let cdr_bytes = sample_cdr_bytes();
    let case_id = "550e8400-e29b-41d4-a716-446655440027";
    let body_bytes = create_multipart_payload(
        BOUNDARY,
        Some(case_id),
        Some("cdr.csv"),
        Some(&cdr_bytes),
        Some("CellPhoneCDR"),
        Some(true),
        None,
    );

    let upload_req = Request::builder()
        .method("POST")
        .uri("/api/v1/evidence/upload?auto_extract=true")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", BOUNDARY),
        )
        .body(Body::from(body_bytes))
        .unwrap();

    let res = app.oneshot(upload_req).await.unwrap();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8_lossy(&bytes);

    // Verify temp dir absolute path is NOT leaked
    let temp_path_str = temp.path().to_string_lossy();
    assert!(!body_str.contains(&*temp_path_str));
    assert!(!body_str.contains(":\\"));
}

// 28. Existing health check endpoint continues to return 200 OK
#[tokio::test]
async fn test_28_health_endpoint_intact() {
    let temp = tempdir().unwrap();
    let config = test_config(temp.path());
    let app = create_router(config);

    let req = Request::builder()
        .method("GET")
        .uri("/health")
        .body(Body::empty())
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(json["status"], "ok");
    assert_eq!(json["service"], "sutram-backend-rust");
}

// 29. Readiness probe returns 200 OK in unconfigured filesystem mode
#[tokio::test]
async fn test_29_readiness_probe_filesystem_mode() {
    let temp = tempdir().unwrap();
    let config = test_config(temp.path());
    let app = create_router(config);

    let req = Request::builder()
        .method("GET")
        .uri("/health/ready")
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(json["status"], "degraded");
    assert_eq!(json["service"], "sutram-backend-rust");
    assert_eq!(json["database"], "unconfigured_filesystem_mode");
    assert_eq!(json["graph"], "disabled");
}

// 30. Case creation and retrieval endpoints
#[tokio::test]
async fn test_30_case_creation_and_retrieval() {
    let temp = tempdir().unwrap();
    let config = test_config(temp.path());
    let app = create_router(config);

    // Create case
    let payload = serde_json::json!({
        "title": "Cyber Incident ALPHA",
        "description": "Cross-border financial forensics",
        "fir_number": "FIR-2026-0091"
    });
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/cases")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_vec(&payload).unwrap()))
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);

    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let created: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let case_id = created["case_id"].as_str().unwrap().to_string();
    assert_eq!(created["title"], "Cyber Incident ALPHA");
    assert_eq!(created["fir_number"], "FIR-2026-0091");
    assert_eq!(created["status"], "ACTIVE");

    // Retrieve case by ID
    let req_get = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/cases/{}", case_id))
        .body(Body::empty())
        .unwrap();

    let res_get = app.clone().oneshot(req_get).await.unwrap();
    assert_eq!(res_get.status(), StatusCode::OK);
    let bytes_get = res_get.into_body().collect().await.unwrap().to_bytes();
    let fetched: serde_json::Value = serde_json::from_slice(&bytes_get).unwrap();
    assert_eq!(fetched["case_id"], case_id);
    assert_eq!(fetched["title"], "Cyber Incident ALPHA");

    // List cases
    let req_list = Request::builder()
        .method("GET")
        .uri("/api/v1/cases")
        .body(Body::empty())
        .unwrap();

    let res_list = app.oneshot(req_list).await.unwrap();
    assert_eq!(res_list.status(), StatusCode::OK);
    let bytes_list = res_list.into_body().collect().await.unwrap().to_bytes();
    let cases: Vec<serde_json::Value> = serde_json::from_slice(&bytes_list).unwrap();
    assert!(cases.iter().any(|c| c["case_id"] == case_id));
}

// 31. Case-scoped evidence listing and cross-case isolation
#[tokio::test]
async fn test_31_case_scoped_evidence_listing() {
    let temp = tempdir().unwrap();
    let config = test_config(temp.path());
    let app = create_router(config);

    let case_id = "00000000-0000-0000-0000-000000000042";

    // Upload evidence to case 42
    let body = format!(
        "{boundary}\r\n\
         Content-Disposition: form-data; name=\"case_id\"\r\n\r\n\
         {case_id}\r\n\
         {boundary}\r\n\
         Content-Disposition: form-data; name=\"file\"; filename=\"telecom_records.csv\"\r\n\
         Content-Type: text/csv\r\n\r\n\
         caller_msisdn,recipient_msisdn,call_timestamp,call_duration_seconds\r\n\
         +919876543210,+919123456789,2026-03-01T10:00:00Z,120\r\n\
         {boundary}--\r\n",
        boundary = BOUNDARY,
        case_id = case_id
    );

    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/evidence/upload?auto_extract=false")
        .header(
            "Content-Type",
            format!("multipart/form-data; boundary={}", &BOUNDARY[2..]),
        )
        .body(Body::from(body))
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let receipt: EvidenceUploadReceipt = serde_json::from_slice(&bytes).unwrap();
    let evidence_id = receipt.evidence_id.clone();

    // Query evidence list for case 42
    let req_ev = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/cases/{}/evidence", case_id))
        .body(Body::empty())
        .unwrap();

    let res_ev = app.clone().oneshot(req_ev).await.unwrap();
    assert_eq!(res_ev.status(), StatusCode::OK);
    let bytes_ev = res_ev.into_body().collect().await.unwrap().to_bytes();
    let list: Vec<EvidenceUploadReceipt> = serde_json::from_slice(&bytes_ev).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].evidence_id, evidence_id);
    assert_eq!(list[0].case_id, case_id);
    assert_eq!(list[0].original_filename, "telecom_records.csv");

    // Cross-case isolation: query for a different case ID
    let other_case_id = "00000000-0000-0000-0000-000000000099";
    let req_other = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/cases/{}/evidence", other_case_id))
        .body(Body::empty())
        .unwrap();

    let res_other = app.oneshot(req_other).await.unwrap();
    assert_eq!(res_other.status(), StatusCode::OK);
    let bytes_other = res_other.into_body().collect().await.unwrap().to_bytes();
    let list_other: Vec<EvidenceUploadReceipt> = serde_json::from_slice(&bytes_other).unwrap();
    assert_eq!(list_other.len(), 0);
}
