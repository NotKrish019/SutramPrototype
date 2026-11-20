//! Evidence API Route Handlers (Member 2 / Ingestion Subsystem).
//!
//! Provides endpoints:
//! - `POST /api/v1/evidence/upload`: Bounded streaming upload, hashing, validation, safe storage.
//! - `GET  /api/v1/evidence/:evidence_id`: Metadata receipt retrieval.

use axum::{
    extract::{Multipart, Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use bytes::Bytes;
use futures_util::StreamExt;
use serde::Deserialize;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::app::AppState;
use crate::error::AppError;
use crate::ingestion::hashing::{verify_file_sha256, IncrementalHasher};
use crate::ingestion::model::{EvidenceManifest, IngestionStatus};
use crate::ingestion::storage::{sanitize_filename, validate_safe_id, StorageManager};
use crate::ingestion::validation::sniff_and_validate_content;

/// Optional query parameters for upload if not supplied via form fields.
#[derive(Debug, Deserialize)]
pub struct UploadQuery {
    pub case_id: Option<String>,
    pub auto_extract: Option<bool>,
}

/// Dynamic response enum for evidence upload.
pub enum UploadResponse {
    Receipt(Json<crate::ingestion::EvidenceUploadReceipt>),
    WithExtraction(Json<crate::extraction::model::EvidenceUploadAndExtractResponse>),
}

impl IntoResponse for UploadResponse {
    fn into_response(self) -> axum::response::Response {
        match self {
            Self::Receipt(j) => (StatusCode::CREATED, j).into_response(),
            Self::WithExtraction(j) => (StatusCode::CREATED, j).into_response(),
        }
    }
}

/// Handler for `POST /api/v1/evidence/upload`.
///
/// Handles multipart streaming upload with bounded memory footprint.
pub async fn upload_evidence(
    State(state): State<AppState>,
    Query(query): Query<UploadQuery>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, AppError> {
    let service = &state.ingestion_service;
    let storage = service.storage();
    let max_size = state.config.max_upload_size_bytes;

    // Create a staging directory under storage root for stream intake
    let staging_dir = storage.root_dir().join(".staging");
    tokio::fs::create_dir_all(&staging_dir).await.map_err(|e| {
        AppError::Internal(anyhow::anyhow!("Failed to create staging directory: {}", e))
    })?;

    let staging_id = Uuid::new_v4().to_string();
    let staging_part_path = staging_dir.join(format!("{}.part", staging_id));

    let mut form_case_id: Option<String> = query.case_id;
    let mut form_auto_extract: bool = query.auto_extract.unwrap_or(false);
    let mut form_source_type: Option<String> = None;
    let mut form_source_timezone: Option<String> = None;
    let mut form_clock_offset: Option<f64> = None;
    let mut form_metadata_raw: Option<String> = None;

    let mut file_received = false;
    let mut original_filename = "unnamed_evidence.bin".to_string();
    let mut bytes_written: u64 = 0;
    let mut hasher = IncrementalHasher::new();

    // Iterate through multipart fields
    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("Malformed multipart request: {}", e)))?
    {
        let name = field.name().unwrap_or("").to_string();

        if name == "file" {
            file_received = true;
            if let Some(fname) = field.file_name() {
                if !fname.trim().is_empty() {
                    original_filename = fname.to_string();
                }
            }

            let mut staging_file = match File::create(&staging_part_path).await {
                Ok(f) => f,
                Err(e) => {
                    StorageManager::safe_unlink(&staging_part_path).await;
                    return Err(AppError::Internal(anyhow::anyhow!(
                        "Failed to create staging file: {}",
                        e
                    )));
                }
            };

            // Stream chunks directly from socket to staging file
            while let Some(chunk_res) = field.next().await {
                let chunk: Bytes = match chunk_res {
                    Ok(c) => c,
                    Err(e) => {
                        drop(staging_file);
                        StorageManager::safe_unlink(&staging_part_path).await;
                        return Err(AppError::BadRequest(format!(
                            "Streaming read error during upload: {}",
                            e
                        )));
                    }
                };

                let chunk_len = chunk.len() as u64;
                if (bytes_written + chunk_len) > max_size as u64 {
                    drop(staging_file);
                    StorageManager::safe_unlink(&staging_part_path).await;
                    return Err(AppError::PayloadTooLarge(format!(
                        "Upload exceeds maximum allowable evidence size of {} bytes.",
                        max_size
                    )));
                }

                hasher.update(&chunk);

                if let Err(e) = staging_file.write_all(&chunk).await {
                    drop(staging_file);
                    StorageManager::safe_unlink(&staging_part_path).await;
                    return Err(AppError::Internal(anyhow::anyhow!(
                        "Failed to write chunk to staging file: {}",
                        e
                    )));
                }

                bytes_written += chunk_len;
            }

            if let Err(e) = staging_file.flush().await {
                drop(staging_file);
                StorageManager::safe_unlink(&staging_part_path).await;
                return Err(AppError::Internal(anyhow::anyhow!(
                    "Failed to flush staging file: {}",
                    e
                )));
            }
            if let Err(e) = staging_file.sync_all().await {
                drop(staging_file);
                StorageManager::safe_unlink(&staging_part_path).await;
                return Err(AppError::Internal(anyhow::anyhow!(
                    "Failed to sync staging file: {}",
                    e
                )));
            }
            drop(staging_file);
        } else {
            // Text form field
            let text_val = field.text().await.map_err(|e| {
                AppError::BadRequest(format!("Failed to read form field '{}': {}", name, e))
            })?;

            match name.as_str() {
                "case_id" if !text_val.trim().is_empty() => {
                    form_case_id = Some(text_val.trim().to_string());
                }
                "source_type" if !text_val.trim().is_empty() => {
                    form_source_type = Some(text_val.trim().to_string());
                }
                "source_timezone" if !text_val.trim().is_empty() => {
                    form_source_timezone = Some(text_val.trim().to_string());
                }
                "clock_offset_seconds" => {
                    if let Ok(v) = text_val.trim().parse::<f64>() {
                        form_clock_offset = Some(v);
                    }
                }
                "auto_extract" => {
                    let val = text_val.trim().to_ascii_lowercase();
                    if val == "true" || val == "1" || val == "yes" {
                        form_auto_extract = true;
                    }
                }
                "metadata" if !text_val.trim().is_empty() => {
                    form_metadata_raw = Some(text_val);
                }
                _ => {}
            }
        }
    }

    // Validation 1: File must be present
    if !file_received {
        StorageManager::safe_unlink(&staging_part_path).await;
        return Err(AppError::BadRequest(
            "Missing required multipart field: 'file'".to_string(),
        ));
    }

    // Validation 2: File must not be empty
    if bytes_written == 0 {
        StorageManager::safe_unlink(&staging_part_path).await;
        return Err(AppError::BadRequest(
            "Uploaded evidence file is empty (0 bytes).".to_string(),
        ));
    }

    // Validation 3: Case ID must be present and valid UUID
    let case_id = match form_case_id {
        Some(cid) if !cid.trim().is_empty() => cid,
        _ => {
            StorageManager::safe_unlink(&staging_part_path).await;
            return Err(AppError::BadRequest(
                "Missing required form field: 'case_id'".to_string(),
            ));
        }
    };

    validate_safe_id(&case_id, "case_id")?;
    if Uuid::parse_str(&case_id).is_err() {
        StorageManager::safe_unlink(&staging_part_path).await;
        return Err(AppError::BadRequest(format!(
            "Invalid case_id '{}': must be a valid UUIDv4 string.",
            case_id
        )));
    }

    // Parse optional metadata JSON
    let mut metadata_map = serde_json::Map::new();
    if let Some(tz) = form_source_timezone {
        metadata_map.insert("source_timezone".to_string(), serde_json::Value::String(tz));
    }
    if let Some(offset) = form_clock_offset {
        if let Some(num) = serde_json::Number::from_f64(offset) {
            metadata_map.insert(
                "clock_offset_seconds".to_string(),
                serde_json::Value::Number(num),
            );
        }
    }

    if let Some(raw_meta) = form_metadata_raw {
        match serde_json::from_str::<serde_json::Value>(&raw_meta) {
            Ok(serde_json::Value::Object(obj)) => {
                for (k, v) in obj {
                    metadata_map.insert(k, v);
                }
            }
            Ok(_) => {
                StorageManager::safe_unlink(&staging_part_path).await;
                return Err(AppError::BadRequest(
                    "Form field 'metadata' must be a valid JSON object/dictionary.".to_string(),
                ));
            }
            Err(e) => {
                StorageManager::safe_unlink(&staging_part_path).await;
                return Err(AppError::BadRequest(format!(
                    "Form field 'metadata' contains invalid JSON: {}",
                    e
                )));
            }
        }
    }

    let evidence_id = Uuid::new_v4().to_string();
    let sanitized_filename = sanitize_filename(&original_filename);

    // Prepare case/evidence storage directory
    storage.ensure_evidence_dir(&case_id, &evidence_id).await?;
    let raw_dest_path = storage.get_raw_file_path(&case_id, &evidence_id)?;

    if raw_dest_path.exists() {
        StorageManager::safe_unlink(&staging_part_path).await;
        return Err(AppError::Conflict(format!(
            "Evidence destination already exists for id '{}'. Overwrite prohibited.",
            evidence_id
        )));
    }

    // Atomic move from staging to final case-scoped raw file
    if let Err(e) = tokio::fs::rename(&staging_part_path, &raw_dest_path).await {
        StorageManager::safe_unlink(&staging_part_path).await;
        return Err(AppError::Internal(anyhow::anyhow!(
            "Failed to move staged file to raw storage: {}",
            e
        )));
    }

    let sha256_hash = hasher.finalize_hex();

    // Independent post-storage verification
    match verify_file_sha256(&raw_dest_path, &sha256_hash).await {
        Ok(true) => {}
        Ok(false) => {
            StorageManager::safe_unlink(&raw_dest_path).await;
            return Err(AppError::Internal(anyhow::anyhow!(
                "Post-storage SHA-256 verification failed: stored bytes do not match ingestion digest."
            )));
        }
        Err(e) => {
            StorageManager::safe_unlink(&raw_dest_path).await;
            return Err(AppError::Internal(anyhow::anyhow!(
                "Failed to read stored evidence for post-storage verification: {}",
                e
            )));
        }
    }

    // Content sniffing and format validation
    let inspection = match sniff_and_validate_content(&raw_dest_path, &sanitized_filename).await {
        Ok(insp) => insp,
        Err(e) => {
            StorageManager::safe_unlink(&raw_dest_path).await;
            return Err(e);
        }
    };

    let source_type = form_source_type.unwrap_or(inspection.source_type);
    let storage_uri = format!("urn:sutram:evidence:{}:storage", evidence_id);
    let now_utc = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

    let receipt = crate::ingestion::model::EvidenceUploadReceipt {
        evidence_id: evidence_id.clone(),
        case_id: case_id.clone(),
        original_filename: sanitized_filename,
        source_type,
        mime_type: inspection.mime_type,
        file_size_bytes: bytes_written,
        sha256_hash,
        storage_uri,
        status: IngestionStatus::Ingested,
        metadata: serde_json::Value::Object(metadata_map),
        created_at: now_utc.clone(),
    };

    let manifest = EvidenceManifest {
        receipt: receipt.clone(),
        relative_raw_path: format!("{}/{}/{}.raw", case_id, evidence_id, evidence_id),
        stored_at: now_utc,
    };

    if let Err(e) = storage.save_manifest(&manifest).await {
        StorageManager::safe_unlink(&raw_dest_path).await;
        return Err(e);
    }

    // Synchronize exhibit to PostgreSQL if database is active
    sync_evidence_to_db(&state, &receipt, &manifest.relative_raw_path).await;

    // Cache in service index
    let _ = service.get_evidence(&evidence_id).await; // Populates cache

    tracing::info!(
        evidence_id = %evidence_id,
        case_id = %case_id,
        size = bytes_written,
        "Evidence intake completed successfully"
    );

    if form_auto_extract {
        let run_manifest = state
            .extraction_service
            .execute_extraction_from_receipt(&receipt, None)
            .await?;
        sync_run_to_db(&state, &run_manifest).await;
        let summary = crate::extraction::model::ExtractionRunSummary::from(&run_manifest);
        let resp = crate::extraction::model::EvidenceUploadAndExtractResponse {
            receipt,
            extraction: summary,
        };
        Ok(UploadResponse::WithExtraction(Json(resp)))
    } else {
        Ok(UploadResponse::Receipt(Json(receipt)))
    }
}

/// Handler for `GET /api/v1/evidence/:evidence_id`.
///
/// Retrieves the metadata receipt for an ingested evidence item.
pub async fn get_evidence_item(
    State(state): State<AppState>,
    Path(evidence_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let receipt = state.ingestion_service.get_evidence(&evidence_id).await?;
    Ok((StatusCode::OK, Json(receipt)))
}

/// Handler for `POST /api/v1/evidence/:evidence_id/extract`.
///
/// Starts a new versioned extraction run using the current evidence exhibit and default extraction parameters.
pub async fn trigger_extraction(
    State(state): State<AppState>,
    Path(evidence_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    validate_safe_id(&evidence_id, "evidence_id")?;

    let manifest = state
        .ingestion_service
        .storage()
        .find_manifest_by_id(&evidence_id)
        .await?
        .ok_or_else(|| {
            AppError::NotFound(format!(
                "Evidence exhibit not found for id '{}'",
                evidence_id
            ))
        })?;

    let run_manifest = state
        .extraction_service
        .execute_extraction_from_receipt(&manifest.receipt, None)
        .await?;
    sync_run_to_db(&state, &run_manifest).await;

    Ok((StatusCode::OK, Json(run_manifest)))
}

/// Handler for `POST /api/v1/evidence/:evidence_id/reprocess`.
///
/// Starts a new versioned extraction run with custom reprocessing parameters (timezone, offset, country code).
pub async fn reprocess_evidence(
    State(state): State<AppState>,
    Path(evidence_id): Path<String>,
    Json(payload): Json<crate::extraction::model::ReprocessRequest>,
) -> Result<impl IntoResponse, AppError> {
    validate_safe_id(&evidence_id, "evidence_id")?;

    if let Some(ref tz) = payload.source_timezone {
        if tz.trim().is_empty() {
            return Err(AppError::BadRequest(
                "Field 'source_timezone' cannot be empty".to_string(),
            ));
        }
    }
    if let Some(offset) = payload.clock_offset_seconds {
        if !offset.is_finite() {
            return Err(AppError::BadRequest(
                "Field 'clock_offset_seconds' must be a finite number".to_string(),
            ));
        }
    }
    if let Some(ref cc) = payload.default_country_code {
        if cc.trim().is_empty() || !cc.chars().all(|c| c.is_ascii_digit()) {
            return Err(AppError::BadRequest(
                "Field 'default_country_code' must consist of numeric digits".to_string(),
            ));
        }
    }

    let manifest = state
        .ingestion_service
        .storage()
        .find_manifest_by_id(&evidence_id)
        .await?
        .ok_or_else(|| {
            AppError::NotFound(format!(
                "Evidence exhibit not found for id '{}'",
                evidence_id
            ))
        })?;

    let run_manifest = state
        .extraction_service
        .execute_extraction_from_receipt(&manifest.receipt, Some(payload))
        .await?;
    sync_run_to_db(&state, &run_manifest).await;

    Ok((StatusCode::OK, Json(run_manifest)))
}

/// Handler for `GET /api/v1/evidence/:evidence_id/extraction`.
///
/// Retrieves the latest active accepted UnifiedExtractionResult for an evidence exhibit.
pub async fn get_active_extraction(
    State(state): State<AppState>,
    Path(evidence_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    validate_safe_id(&evidence_id, "evidence_id")?;

    let case_id = state
        .ingestion_service
        .storage()
        .find_case_id_for_evidence(&evidence_id)
        .await?
        .ok_or_else(|| {
            AppError::NotFound(format!(
                "Evidence exhibit not found for id '{}'",
                evidence_id
            ))
        })?;

    let pointer = state
        .ingestion_service
        .storage()
        .load_active_run_pointer(&case_id, &evidence_id)
        .await?
        .ok_or_else(|| {
            AppError::NotFound(format!(
                "No active extraction result found for evidence_id '{}'",
                evidence_id
            ))
        })?;

    let result = state
        .ingestion_service
        .storage()
        .load_run_result(&case_id, &evidence_id, &pointer.active_run_id)
        .await?;

    Ok((StatusCode::OK, Json(result)))
}

/// Handler for `GET /api/v1/evidence/:evidence_id/extraction/runs`.
///
/// Lists all historical extraction runs for an evidence exhibit in chronological order.
pub async fn list_extraction_runs(
    State(state): State<AppState>,
    Path(evidence_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    validate_safe_id(&evidence_id, "evidence_id")?;

    let case_id = state
        .ingestion_service
        .storage()
        .find_case_id_for_evidence(&evidence_id)
        .await?
        .ok_or_else(|| {
            AppError::NotFound(format!(
                "Evidence exhibit not found for id '{}'",
                evidence_id
            ))
        })?;

    let runs = state
        .ingestion_service
        .storage()
        .list_runs(&case_id, &evidence_id)
        .await?;

    Ok((StatusCode::OK, Json(runs)))
}

/// Handler for `GET /api/v1/cases/:case_id/evidence`.
///
/// Retrieves all evidence exhibit receipts associated with a specific case.
pub async fn list_case_evidence(
    State(state): State<AppState>,
    Path(case_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    validate_safe_id(&case_id, "case_id")?;

    // If PostgreSQL repository is connected and case_id is a valid UUID, query database
    if let (Some(ref ev_repo), Ok(case_uuid)) = (&state.evidence_repo, Uuid::parse_str(&case_id)) {
        if let Ok(entities) = ev_repo.list_evidence_for_case(case_uuid).await {
            let receipts: Vec<crate::ingestion::EvidenceUploadReceipt> = entities
                .into_iter()
                .map(|e| crate::ingestion::EvidenceUploadReceipt {
                    evidence_id: e.evidence_id.to_string(),
                    case_id: e.case_id.to_string(),
                    original_filename: e.original_filename,
                    source_type: e.source_type,
                    mime_type: e.mime_type,
                    file_size_bytes: u64::try_from(e.file_size_bytes).unwrap_or(0),
                    sha256_hash: e.sha256_hash,
                    storage_uri: e.storage_uri,
                    status: IngestionStatus::Ingested,
                    metadata: e.metadata,
                    created_at: e
                        .created_at
                        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                })
                .collect();
            return Ok((StatusCode::OK, Json(receipts)));
        }
    }

    // Filesystem fallback
    let manifests = state
        .ingestion_service
        .storage()
        .list_manifests_for_case(&case_id)
        .await?;
    let receipts: Vec<crate::ingestion::EvidenceUploadReceipt> =
        manifests.into_iter().map(|m| m.receipt).collect();
    Ok((StatusCode::OK, Json(receipts)))
}

/// Asynchronously synchronizes newly ingested evidence exhibit into PostgreSQL.
async fn sync_evidence_to_db(
    state: &AppState,
    receipt: &crate::ingestion::EvidenceUploadReceipt,
    _relative_path: &str,
) {
    if let (Some(evidence_repo), Some(case_repo), Some(custody_repo)) = (
        state.evidence_repo.as_ref(),
        state.case_repo.as_ref(),
        state.custody_repo.as_ref(),
    ) {
        if let (Ok(case_uuid), Ok(evidence_uuid)) = (
            Uuid::parse_str(&receipt.case_id),
            Uuid::parse_str(&receipt.evidence_id),
        ) {
            let _ = case_repo
                .ensure_case_exists(case_uuid, &format!("Case {}", case_uuid))
                .await;

            let now = chrono::Utc::now();
            let entity = crate::persistence::EvidenceEntity {
                evidence_id: evidence_uuid,
                case_id: case_uuid,
                original_filename: receipt.original_filename.clone(),
                source_type: receipt.source_type.clone(),
                mime_type: receipt.mime_type.clone(),
                file_size_bytes: i64::try_from(receipt.file_size_bytes).unwrap_or(i64::MAX),
                sha256_hash: receipt.sha256_hash.clone(),
                storage_uri: receipt.storage_uri.clone(),
                source_timezone: receipt
                    .metadata
                    .get("source_timezone")
                    .and_then(|v| v.as_str())
                    .unwrap_or("UTC")
                    .to_string(),
                clock_offset_seconds: receipt
                    .metadata
                    .get("clock_offset_seconds")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0),
                ingestion_status: "INGESTED".to_string(),
                metadata: receipt.metadata.clone(),
                created_at: now,
                updated_at: now,
            };

            let _ = evidence_repo.register_evidence(&entity).await;

            let _ = custody_repo
                .append_event(
                    evidence_uuid,
                    case_uuid,
                    "EVIDENCE_STORED",
                    "SYSTEM_INGESTION",
                    Some(&receipt.sha256_hash),
                    serde_json::json!({
                        "original_filename": receipt.original_filename,
                        "file_size_bytes": receipt.file_size_bytes,
                        "mime_type": receipt.mime_type,
                    }),
                )
                .await;

            // Enqueue graph projection outbox event
            if let Some(db) = &state.database {
                if let Ok(mut tx) = db.pool().begin().await {
                    let payload = serde_json::json!({
                        "sha256_hash": receipt.sha256_hash,
                        "source_type": receipt.source_type,
                        "mime_type": receipt.mime_type,
                        "file_size_bytes": receipt.file_size_bytes,
                        "storage_uri": receipt.storage_uri,
                        "created_at": receipt.created_at,
                    });
                    let _ = crate::graph::GraphOutboxRepository::enqueue_transactional(
                        &mut tx,
                        case_uuid,
                        "EVIDENCE",
                        evidence_uuid,
                        1,
                        "EVIDENCE_INGESTED",
                        &payload,
                    )
                    .await;
                    let _ = tx.commit().await;
                }
            }
        }
    }
}

/// Asynchronously synchronizes completed extraction run and child entities into PostgreSQL.
async fn sync_run_to_db(
    state: &AppState,
    run_manifest: &crate::extraction::model::ExtractionRunManifest,
) {
    if let (Some(extraction_repo), Some(custody_repo)) =
        (state.extraction_repo.as_ref(), state.custody_repo.as_ref())
    {
        if let (Ok(case_uuid), Ok(evidence_uuid), Ok(run_uuid)) = (
            Uuid::parse_str(&run_manifest.case_id),
            Uuid::parse_str(&run_manifest.evidence_id),
            Uuid::parse_str(&run_manifest.extraction_run_id),
        ) {
            let started_at = chrono::DateTime::parse_from_rfc3339(&run_manifest.started_at)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now());

            let run_entity = crate::persistence::ExtractionRunEntity {
                extraction_run_id: run_uuid,
                evidence_id: evidence_uuid,
                case_id: case_uuid,
                source_sha256: run_manifest.source_sha256.clone(),
                extractor_name: run_manifest.extractor_name.clone(),
                extractor_version: run_manifest.extractor_version.clone(),
                configuration_hash: run_manifest.configuration_hash.clone(),
                configuration: serde_json::to_value(&run_manifest.configuration)
                    .unwrap_or_default(),
                status: run_manifest.status.to_string(),
                is_active: false,
                result_sha256: run_manifest.result_sha256.clone(),
                error_message: run_manifest.error_message.clone(),
                records_processed: i32::try_from(run_manifest.records_processed)
                    .unwrap_or(i32::MAX),
                valid_records_count: i32::try_from(run_manifest.valid_records_count)
                    .unwrap_or(i32::MAX),
                rejected_records_count: i32::try_from(run_manifest.rejected_records_count)
                    .unwrap_or(i32::MAX),
                validation_issue_count: i32::try_from(run_manifest.validation_issue_count)
                    .unwrap_or(i32::MAX),
                duration_ms: run_manifest.duration_ms,
                started_at,
                completed_at: run_manifest.completed_at.as_deref().and_then(|s| {
                    chrono::DateTime::parse_from_rfc3339(s)
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .ok()
                }),
                result_snapshot: None,
                created_at: chrono::Utc::now(),
            };

            let _ = extraction_repo.create_run(&run_entity).await;

            let is_success = run_manifest.status
                == crate::extraction::model::ExtractionRunStatus::Success
                || run_manifest.status
                    == crate::extraction::model::ExtractionRunStatus::PartialSuccess;

            if is_success {
                if let Ok(result) = state
                    .extraction_service
                    .storage()
                    .load_run_result(
                        &run_manifest.case_id,
                        &run_manifest.evidence_id,
                        &run_manifest.extraction_run_id,
                    )
                    .await
                {
                    let _ = extraction_repo
                        .complete_run_transactional(run_manifest, &result, true)
                        .await;
                }
            }

            let _ = custody_repo
                .append_event(
                    evidence_uuid,
                    case_uuid,
                    "EXTRACTION_COMPLETED",
                    "SYSTEM_PIPELINE",
                    run_manifest.result_sha256.as_deref(),
                    serde_json::json!({
                        "run_id": run_manifest.extraction_run_id,
                        "status": run_manifest.status,
                        "records_processed": run_manifest.records_processed,
                        "valid_records_count": run_manifest.valid_records_count,
                    }),
                )
                .await;
        }
    }
}
