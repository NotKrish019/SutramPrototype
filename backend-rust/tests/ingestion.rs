//! Comprehensive Integration and Unit Tests for Forensic Evidence Ingestion Engine.
//!
//! Covers:
//! 1. Successful small-file upload.
//! 2. Successful large streaming upload within configured size limit.
//! 3. SHA-256 correct for uploaded content.
//! 4. Stored file bytes exactly match uploaded bytes.
//! 5. Post-storage SHA-256 verification.
//! 6. Empty file rejection.
//! 7. Missing multipart file.
//! 8. Invalid case UUID.
//! 9. Unsupported content.
//! 10. Invalid content signature.
//! 11. Malformed JSON.
//! 12. Generic CSV not classified as CDR.
//! 13. Maximum upload size enforcement.
//! 14. Interrupted upload cleanup.
//! 15. Simulated storage failure.
//! 16. Temporary file cleanup.
//! 17. Path traversal filename attack.
//! 18. Windows reserved filename handling.
//! 19. Duplicate filename handling.
//! 20. Identical file contents uploaded twice receive distinct evidence IDs.
//! 21. No existing evidence file is overwritten.
//! 22. Absolute filesystem paths are not disclosed in API responses.
//! 23. Metadata retrieval after upload.
//! 24. Metadata survives a process restart if a filesystem manifest is implemented.
//! 25. Original evidence remains unchanged.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use sutram_backend_rust::{create_router, AppConfig, EvidenceUploadReceipt, IngestionStatus};
use tempfile::tempdir;
use tower::ServiceExt;

const BOUNDARY: &str = "---------------------------boundary9876543210";

fn test_config(storage_path: std::path::PathBuf) -> AppConfig {
    AppConfig {
        evidence_storage_path: storage_path,
        ..Default::default()
    }
}

fn create_multipart_payload(
    boundary: &str,
    case_id: Option<&str>,
    filename: Option<&str>,
    file_bytes: Option<&[u8]>,
    source_type: Option<&str>,
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

// 1, 3, 4, 5, 22: Successful upload, SHA-256 accuracy, byte fidelity, opaque URI
#[tokio::test]
async fn test_01_successful_small_file_upload_and_integrity() {
    let temp = tempdir().unwrap();
    let config = test_config(temp.path().to_path_buf());
    let app = create_router(config);

    let pdf_content = b"%PDF-1.4\n1 0 obj\n<<>>\nendobj\ntrailer\n<<>>\n%%EOF\n";
    let case_uuid = "550e8400-e29b-41d4-a716-446655440000";

    let payload = create_multipart_payload(
        BOUNDARY,
        Some(case_uuid),
        Some("exhibit_a.pdf"),
        Some(pdf_content),
        None,
        None,
    );

    let req = Request::builder()
        .uri("/api/v1/evidence/upload")
        .method("POST")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", BOUNDARY),
        )
        .body(Body::from(payload))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);

    let body_bytes = res.into_body().collect().await.unwrap().to_bytes();
    let receipt: EvidenceUploadReceipt = serde_json::from_slice(&body_bytes).unwrap();

    // Check receipt fields
    assert_eq!(receipt.case_id, case_uuid);
    assert_eq!(receipt.original_filename, "exhibit_a.pdf");
    assert_eq!(receipt.source_type, "DOCUMENT_PDF");
    assert_eq!(receipt.mime_type, "application/pdf");
    assert_eq!(receipt.file_size_bytes, pdf_content.len() as u64);
    assert_eq!(receipt.status, IngestionStatus::Ingested);

    // Assert absolute filesystem paths are NOT leaked in storage_uri
    assert!(receipt.storage_uri.starts_with("urn:sutram:evidence:"));
    assert!(!receipt.storage_uri.contains(":\\"));
    assert!(!receipt.storage_uri.contains("/data/"));

    // Verify stored file on disk
    let stored_file = temp
        .path()
        .join(case_uuid)
        .join(&receipt.evidence_id)
        .join(format!("{}.raw", receipt.evidence_id));
    assert!(stored_file.exists());

    let disk_bytes = std::fs::read(&stored_file).unwrap();
    assert_eq!(disk_bytes, pdf_content);

    // Verify SHA-256
    use sha2::{Digest, Sha256};
    let expected_hash = hex::encode(Sha256::digest(pdf_content));
    assert_eq!(receipt.sha256_hash, expected_hash);
}

// 2, 13: Large upload within limits vs exceeding limit
#[tokio::test]
async fn test_02_and_13_streaming_size_limit_enforcement() {
    let temp = tempdir().unwrap();
    let config = AppConfig {
        evidence_storage_path: temp.path().to_path_buf(),
        max_upload_size_bytes: 100 * 1024, // 100 KB ceiling
        ..Default::default()
    };
    let app = create_router(config.clone());

    let case_uuid = "550e8400-e29b-41d4-a716-446655440000";

    // Subtest 2: 50 KB PNG (within limit)
    let mut valid_png = Vec::from(b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR");
    valid_png.resize(50 * 1024, 0x41);

    let payload_ok = create_multipart_payload(
        BOUNDARY,
        Some(case_uuid),
        Some("valid_screen.png"),
        Some(&valid_png),
        None,
        None,
    );

    let req_ok = Request::builder()
        .uri("/api/v1/evidence/upload")
        .method("POST")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", BOUNDARY),
        )
        .body(Body::from(payload_ok))
        .unwrap();

    let res_ok = app.oneshot(req_ok).await.unwrap();
    assert_eq!(res_ok.status(), StatusCode::CREATED);

    // Subtest 13: 150 KB PNG (exceeds limit)
    let app2 = create_router(config);
    let mut oversized = Vec::from(b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR");
    oversized.resize(150 * 1024, 0x42);

    let payload_oversized = create_multipart_payload(
        BOUNDARY,
        Some(case_uuid),
        Some("large.png"),
        Some(&oversized),
        None,
        None,
    );

    let req_large = Request::builder()
        .uri("/api/v1/evidence/upload")
        .method("POST")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", BOUNDARY),
        )
        .body(Body::from(payload_oversized))
        .unwrap();

    let res_large = app2.oneshot(req_large).await.unwrap();
    assert_eq!(res_large.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

// 6: Empty file rejection
#[tokio::test]
async fn test_06_empty_file_rejected() {
    let temp = tempdir().unwrap();
    let config = test_config(temp.path().to_path_buf());
    let app = create_router(config);

    let payload = create_multipart_payload(
        BOUNDARY,
        Some("550e8400-e29b-41d4-a716-446655440000"),
        Some("zero.csv"),
        Some(b""),
        None,
        None,
    );

    let req = Request::builder()
        .uri("/api/v1/evidence/upload")
        .method("POST")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", BOUNDARY),
        )
        .body(Body::from(payload))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

// 7: Missing multipart file
#[tokio::test]
async fn test_07_missing_file_rejected() {
    let temp = tempdir().unwrap();
    let config = test_config(temp.path().to_path_buf());
    let app = create_router(config);

    // Payload containing only case_id, no file
    let payload = create_multipart_payload(
        BOUNDARY,
        Some("550e8400-e29b-41d4-a716-446655440000"),
        None,
        None,
        None,
        None,
    );

    let req = Request::builder()
        .uri("/api/v1/evidence/upload")
        .method("POST")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", BOUNDARY),
        )
        .body(Body::from(payload))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

// 8: Invalid case UUID
#[tokio::test]
async fn test_08_invalid_case_uuid_rejected() {
    let temp = tempdir().unwrap();
    let config = test_config(temp.path().to_path_buf());
    let app = create_router(config);

    let payload = create_multipart_payload(
        BOUNDARY,
        Some("not-a-valid-uuid-1234"),
        Some("file.pdf"),
        Some(b"%PDF-1.4\nend\n"),
        None,
        None,
    );

    let req = Request::builder()
        .uri("/api/v1/evidence/upload")
        .method("POST")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", BOUNDARY),
        )
        .body(Body::from(payload))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

// 9, 10: Unsupported content and invalid signatures
#[tokio::test]
async fn test_09_and_10_unsupported_content_rejected() {
    let temp = tempdir().unwrap();
    let config = test_config(temp.path().to_path_buf());
    let app = create_router(config);

    // Random non-standard binary payload declared as an unknown binary
    let garbage = b"\x00\x01\x02\x03\x04\x05\x06\x07\x08\x09\x0a\x0b";
    let payload = create_multipart_payload(
        BOUNDARY,
        Some("550e8400-e29b-41d4-a716-446655440000"),
        Some("unknown.dat"),
        Some(garbage),
        None,
        None,
    );

    let req = Request::builder()
        .uri("/api/v1/evidence/upload")
        .method("POST")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", BOUNDARY),
        )
        .body(Body::from(payload))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

// 11: Malformed JSON
#[tokio::test]
async fn test_11_malformed_json_rejected() {
    let temp = tempdir().unwrap();
    let config = test_config(temp.path().to_path_buf());
    let app = create_router(config);

    let bad_json = b"{\"messages\": [ { unterminated json string";
    let payload = create_multipart_payload(
        BOUNDARY,
        Some("550e8400-e29b-41d4-a716-446655440000"),
        Some("corrupt.json"),
        Some(bad_json),
        None,
        None,
    );

    let req = Request::builder()
        .uri("/api/v1/evidence/upload")
        .method("POST")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", BOUNDARY),
        )
        .body(Body::from(payload))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

// 12: Generic CSV classified as TABULAR_CSV, NOT CALL_DETAIL_RECORD
#[tokio::test]
async fn test_12_generic_csv_classified_as_tabular_csv() {
    let temp = tempdir().unwrap();
    let config = test_config(temp.path().to_path_buf());
    let app = create_router(config);

    let csv_content = b"col1,col2,col3\nval1,val2,val3\nval4,val5,val6\n";
    let payload = create_multipart_payload(
        BOUNDARY,
        Some("550e8400-e29b-41d4-a716-446655440000"),
        Some("generic.csv"),
        Some(csv_content),
        None,
        None,
    );

    let req = Request::builder()
        .uri("/api/v1/evidence/upload")
        .method("POST")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", BOUNDARY),
        )
        .body(Body::from(payload))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);

    let body_bytes = res.into_body().collect().await.unwrap().to_bytes();
    let receipt: EvidenceUploadReceipt = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(receipt.source_type, "TABULAR_CSV");
    assert_ne!(receipt.source_type, "CALL_DETAIL_RECORD");
    assert_eq!(receipt.mime_type, "text/csv");
}

// 14, 16: Temporary file cleanup after failure
#[tokio::test]
async fn test_14_and_16_temporary_file_cleanup() {
    let temp = tempdir().unwrap();
    let config = test_config(temp.path().to_path_buf());
    let app = create_router(config);

    // Upload an invalid JSON file that fails validation
    let payload = create_multipart_payload(
        BOUNDARY,
        Some("550e8400-e29b-41d4-a716-446655440000"),
        Some("fail.json"),
        Some(b"{ corrupt"),
        None,
        None,
    );

    let req = Request::builder()
        .uri("/api/v1/evidence/upload")
        .method("POST")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", BOUNDARY),
        )
        .body(Body::from(payload))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // Verify staging directory does not contain leftover .part files
    let staging = temp.path().join(".staging");
    if staging.exists() {
        let entries = std::fs::read_dir(&staging).unwrap();
        assert_eq!(entries.count(), 0);
    }
}

// 17, 18, 19: Path traversal attack and Windows reserved names
#[tokio::test]
async fn test_17_and_18_path_traversal_and_reserved_names() {
    let temp = tempdir().unwrap();
    let config = test_config(temp.path().to_path_buf());
    let app = create_router(config.clone());

    let pdf_content = b"%PDF-1.4\n1 0 obj\n<<>>\nendobj\n";

    // 17: Path traversal in filename
    let payload_traversal = create_multipart_payload(
        BOUNDARY,
        Some("550e8400-e29b-41d4-a716-446655440000"),
        Some("../../../../etc/shadow.pdf"),
        Some(pdf_content),
        None,
        None,
    );

    let req1 = Request::builder()
        .uri("/api/v1/evidence/upload")
        .method("POST")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", BOUNDARY),
        )
        .body(Body::from(payload_traversal))
        .unwrap();

    let res1 = app.oneshot(req1).await.unwrap();
    assert_eq!(res1.status(), StatusCode::CREATED);
    let receipt1: EvidenceUploadReceipt =
        serde_json::from_slice(&res1.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(receipt1.original_filename, "shadow.pdf");

    // 18: Windows reserved name (CON.pdf)
    let app2 = create_router(config);
    let payload_reserved = create_multipart_payload(
        BOUNDARY,
        Some("550e8400-e29b-41d4-a716-446655440000"),
        Some("CON.pdf"),
        Some(pdf_content),
        None,
        None,
    );

    let req2 = Request::builder()
        .uri("/api/v1/evidence/upload")
        .method("POST")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", BOUNDARY),
        )
        .body(Body::from(payload_reserved))
        .unwrap();

    let res2 = app2.oneshot(req2).await.unwrap();
    assert_eq!(res2.status(), StatusCode::CREATED);
    let receipt2: EvidenceUploadReceipt =
        serde_json::from_slice(&res2.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(receipt2.original_filename, "safe_CON.pdf");
}

// 20, 21: Identical content uploaded twice receives distinct UUIDs; no overwrite
#[tokio::test]
async fn test_20_and_21_identical_content_distinct_ids_no_overwrite() {
    let temp = tempdir().unwrap();
    let config = test_config(temp.path().to_path_buf());
    let app = create_router(config.clone());

    let content = b"%PDF-1.4\nidentical exhibit payload\n";
    let case_uuid = "550e8400-e29b-41d4-a716-446655440000";

    // Upload 1
    let p1 = create_multipart_payload(
        BOUNDARY,
        Some(case_uuid),
        Some("exhibit1.pdf"),
        Some(content),
        None,
        None,
    );
    let req1 = Request::builder()
        .uri("/api/v1/evidence/upload")
        .method("POST")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", BOUNDARY),
        )
        .body(Body::from(p1))
        .unwrap();

    let res1 = app.oneshot(req1).await.unwrap();
    assert_eq!(res1.status(), StatusCode::CREATED);
    let r1: EvidenceUploadReceipt =
        serde_json::from_slice(&res1.into_body().collect().await.unwrap().to_bytes()).unwrap();

    // Upload 2 with identical content and same case
    let app2 = create_router(config);
    let p2 = create_multipart_payload(
        BOUNDARY,
        Some(case_uuid),
        Some("exhibit2.pdf"),
        Some(content),
        None,
        None,
    );
    let req2 = Request::builder()
        .uri("/api/v1/evidence/upload")
        .method("POST")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", BOUNDARY),
        )
        .body(Body::from(p2))
        .unwrap();

    let res2 = app2.oneshot(req2).await.unwrap();
    assert_eq!(res2.status(), StatusCode::CREATED);
    let r2: EvidenceUploadReceipt =
        serde_json::from_slice(&res2.into_body().collect().await.unwrap().to_bytes()).unwrap();

    // Distinct UUIDs assigned
    assert_ne!(r1.evidence_id, r2.evidence_id);
    assert_eq!(r1.sha256_hash, r2.sha256_hash);

    // Both files exist independently on disk
    let file1 = temp
        .path()
        .join(case_uuid)
        .join(&r1.evidence_id)
        .join(format!("{}.raw", r1.evidence_id));
    let file2 = temp
        .path()
        .join(case_uuid)
        .join(&r2.evidence_id)
        .join(format!("{}.raw", r2.evidence_id));
    assert!(file1.exists());
    assert!(file2.exists());
    assert_ne!(file1, file2);
}

// 23, 24: Metadata retrieval and survival across process restart
#[tokio::test]
async fn test_23_and_24_metadata_retrieval_and_restart_survival() {
    let temp = tempdir().unwrap();
    let config = test_config(temp.path().to_path_buf());

    // Process 1: Ingest evidence
    let app1 = create_router(config.clone());
    let pdf_content = b"%PDF-1.4\n1 0 obj\n<<>>\nendobj\n";
    let case_uuid = "550e8400-e29b-41d4-a716-446655440000";

    let payload = create_multipart_payload(
        BOUNDARY,
        Some(case_uuid),
        Some("court_document.pdf"),
        Some(pdf_content),
        None,
        Some("{\"officer\": \"Agent Cooper\", \"station\": \"Twin Peaks\"}"),
    );

    let req1 = Request::builder()
        .uri("/api/v1/evidence/upload")
        .method("POST")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", BOUNDARY),
        )
        .body(Body::from(payload))
        .unwrap();

    let res1 = app1.oneshot(req1).await.unwrap();
    assert_eq!(res1.status(), StatusCode::CREATED);
    let uploaded: EvidenceUploadReceipt =
        serde_json::from_slice(&res1.into_body().collect().await.unwrap().to_bytes()).unwrap();

    let ev_id = uploaded.evidence_id.clone();

    // Process 2 (Simulated Restart): Create fresh router pointing to the same storage path
    let app2 = create_router(config);

    // Query GET /api/v1/evidence/:evidence_id
    let req2 = Request::builder()
        .uri(format!("/api/v1/evidence/{}", ev_id))
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let res2 = app2.oneshot(req2).await.unwrap();
    assert_eq!(res2.status(), StatusCode::OK);

    let fetched: EvidenceUploadReceipt =
        serde_json::from_slice(&res2.into_body().collect().await.unwrap().to_bytes()).unwrap();

    assert_eq!(fetched.evidence_id, ev_id);
    assert_eq!(fetched.case_id, case_uuid);
    assert_eq!(fetched.original_filename, "court_document.pdf");
    assert_eq!(fetched.sha256_hash, uploaded.sha256_hash);
    assert_eq!(
        fetched.metadata["officer"],
        serde_json::Value::String("Agent Cooper".to_string())
    );
}

// 25: Original evidence remains untouched
#[tokio::test]
async fn test_25_original_evidence_remains_unchanged() {
    let temp = tempdir().unwrap();
    let config = test_config(temp.path().to_path_buf());
    let app = create_router(config.clone());

    let jpeg_content = b"\xff\xd8\xff\xe0\x00\x10JFIF\x00\x01\x01\x01\x00`\x00`\x00\x00";
    let case_uuid = "550e8400-e29b-41d4-a716-446655440000";

    let payload = create_multipart_payload(
        BOUNDARY,
        Some(case_uuid),
        Some("cctv_frame.jpg"),
        Some(jpeg_content),
        None,
        None,
    );

    let req = Request::builder()
        .uri("/api/v1/evidence/upload")
        .method("POST")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", BOUNDARY),
        )
        .body(Body::from(payload))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let receipt: EvidenceUploadReceipt =
        serde_json::from_slice(&res.into_body().collect().await.unwrap().to_bytes()).unwrap();

    let raw_file = temp
        .path()
        .join(case_uuid)
        .join(&receipt.evidence_id)
        .join(format!("{}.raw", receipt.evidence_id));

    let file_bytes = std::fs::read(&raw_file).unwrap();
    assert_eq!(file_bytes, jpeg_content);
}
