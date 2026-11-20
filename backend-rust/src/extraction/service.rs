//! Unified Evidence Extraction Dispatcher Service (Member 1).
//!
//! Serves as the canonical public entrypoint for evidence extraction across supported
//! modalities (Call Detail Records, Chat/Instant Messaging Exports).
//!
//! Performs:
//! 1. File existence validation.
//! 2. Pre-extraction cryptographic SHA-256 integrity verification.
//! 3. Modality dispatch to specialized parsers (parse_cdr_csv, parse_chat_json).
//! 4. Defense against premature CSV classification (validates required CDR headers).
//! 5. Cross-source identifier resolution and entity candidate deduplication.
//! 6. Unified result assembly with execution metrics, structured validation issues,
//!    and deterministic status computation (SUCCESS, PARTIAL_SUCCESS, FAILED).
//! 7. Strict preservation of forensic provenance and raw evidence immutability.

use crate::{
    correlation::resolution::deduplicate_identifiers,
    error::AppError,
    extraction::{
        cdr::parse_cdr_csv,
        chat::parse_chat_json,
        model::{
            ActiveRunPointer, EvidenceExtractionRequest, ExtractionConfiguration,
            ExtractionRunManifest, ExtractionRunStatus, ExtractionStatus,
            ExtractionValidationIssue, ReprocessRequest, UnifiedExtractionResult,
        },
    },
    ingestion::{EvidenceManifest, StorageManager},
};
use sha2::{Digest, Sha256};
use std::{collections::HashMap, fs::File, io::Read, path::Path, sync::Arc, time::Instant};

/// Compute streaming SHA-256 of a target file on disk.
fn compute_sha256(file_path: &Path) -> std::io::Result<String> {
    let mut file = File::open(file_path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// Canonical entrypoint to execute extraction across supported evidence modalities.
///
/// Forensic invariants:
/// - Never mutates the raw evidence artifact.
/// - Opens files strictly in read-only mode.
/// - Verifies file existence and SHA-256 digest before initiating parser logic.
/// - Rejects unsupported evidence modalities with structured diagnostic issues.
/// - Protects against generic CSV misclassification as CDR.
/// - Preserves uncorrected timestamps and original observed identifiers.
/// - Decouples observed facts from inferred entity candidates.
/// - Decouples mathematical checksum telemetry from probabilistic confidence.
/// - Traps unexpected exceptions to prevent panics or crashes.
pub fn extract_evidence(request: &EvidenceExtractionRequest) -> UnifiedExtractionResult {
    let start_time = Instant::now();
    let target_path = &request.file_path;

    // 1. File existence verification
    let display_name = target_path
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| "unnamed_evidence".to_string());

    if !target_path.exists() || !target_path.is_file() {
        let elapsed_ms = start_time.elapsed().as_secs_f64() * 1000.0;
        let issue = ExtractionValidationIssue {
            record_id: None,
            field_name: "file_path".to_string(),
            issue_type: "FILE_NOT_FOUND".to_string(),
            raw_value: Some(display_name.clone()),
            message: format!(
                "Evidence file '{}' not found in secure storage",
                display_name
            ),
            row_index: None,
        };
        return UnifiedExtractionResult {
            evidence_id: request.evidence_id.clone(),
            case_id: request.case_id.clone(),
            source_type: request.source_type.clone(),
            status: ExtractionStatus::Failed,
            identifiers: Vec::new(),
            resolved_entities: Vec::new(),
            event_candidates: Vec::new(),
            relationship_candidates: Vec::new(),
            validation_issues: vec![issue],
            total_records_processed: 0,
            valid_records_count: 0,
            rejected_records_count: 1,
            processing_time_ms: Some((elapsed_ms * 100.0).round() / 100.0),
            extraction_metadata: serde_json::json!({ "error": "FILE_NOT_FOUND" }),
        };
    }

    // 2. Cryptographic SHA-256 pre-extraction integrity verification
    let computed_sha256 = match compute_sha256(target_path) {
        Ok(hash) => hash,
        Err(e) => {
            let elapsed_ms = start_time.elapsed().as_secs_f64() * 1000.0;
            let issue = ExtractionValidationIssue {
                record_id: None,
                field_name: "file_path".to_string(),
                issue_type: "FILE_READ_ERROR".to_string(),
                raw_value: Some(display_name.clone()),
                message: format!(
                    "Failed to read file '{}' for cryptographic verification: {}",
                    display_name, e
                ),
                row_index: None,
            };
            return UnifiedExtractionResult {
                evidence_id: request.evidence_id.clone(),
                case_id: request.case_id.clone(),
                source_type: request.source_type.clone(),
                status: ExtractionStatus::Failed,
                identifiers: Vec::new(),
                resolved_entities: Vec::new(),
                event_candidates: Vec::new(),
                relationship_candidates: Vec::new(),
                validation_issues: vec![issue],
                total_records_processed: 0,
                valid_records_count: 0,
                rejected_records_count: 1,
                processing_time_ms: Some((elapsed_ms * 100.0).round() / 100.0),
                extraction_metadata: serde_json::json!({ "error": "FILE_READ_ERROR" }),
            };
        }
    };

    let expected_clean = request.expected_sha256.trim().to_ascii_lowercase();
    if computed_sha256 != expected_clean {
        let elapsed_ms = start_time.elapsed().as_secs_f64() * 1000.0;
        let issue = ExtractionValidationIssue {
            record_id: None,
            field_name: "sha256_hash".to_string(),
            issue_type: "HASH_MISMATCH".to_string(),
            raw_value: Some(computed_sha256.clone()),
            message: format!(
                "Cryptographic hash mismatch for evidence '{}': expected {}, computed {}.",
                request.evidence_id, expected_clean, computed_sha256
            ),
            row_index: None,
        };
        return UnifiedExtractionResult {
            evidence_id: request.evidence_id.clone(),
            case_id: request.case_id.clone(),
            source_type: request.source_type.clone(),
            status: ExtractionStatus::Failed,
            identifiers: Vec::new(),
            resolved_entities: Vec::new(),
            event_candidates: Vec::new(),
            relationship_candidates: Vec::new(),
            validation_issues: vec![issue],
            total_records_processed: 0,
            valid_records_count: 0,
            rejected_records_count: 1,
            processing_time_ms: Some((elapsed_ms * 100.0).round() / 100.0),
            extraction_metadata: serde_json::json!({
                "error": "HASH_MISMATCH",
                "computed_sha256": computed_sha256,
                "expected_sha256": expected_clean,
            }),
        };
    }

    // 3. Source-type dispatch & normalization
    let normalized_source_type = request.source_type.trim().to_ascii_uppercase();
    let canonical_modality = match normalized_source_type.as_str() {
        "CALL_DETAIL_RECORD" | "CDR" | "CELLPHONECDR" | "TABULAR_CSV" => "CALL_DETAIL_RECORD",
        "CHAT_EXPORT" | "CHAT" | "CHATEXPORT" => "CHAT_EXPORT",
        _ => {
            let elapsed_ms = start_time.elapsed().as_secs_f64() * 1000.0;
            let issue = ExtractionValidationIssue {
                record_id: None,
                field_name: "source_type".to_string(),
                issue_type: "UNSUPPORTED_SOURCE_TYPE".to_string(),
                raw_value: Some(request.source_type.clone()),
                message: format!(
                    "Unsupported evidence source type: '{}'. Supported modalities in Phase 1E: CALL_DETAIL_RECORD, CHAT_EXPORT.",
                    request.source_type
                ),
                row_index: None,
            };
            return UnifiedExtractionResult {
                evidence_id: request.evidence_id.clone(),
                case_id: request.case_id.clone(),
                source_type: request.source_type.clone(),
                status: ExtractionStatus::Failed,
                identifiers: Vec::new(),
                resolved_entities: Vec::new(),
                event_candidates: Vec::new(),
                relationship_candidates: Vec::new(),
                validation_issues: vec![issue],
                total_records_processed: 0,
                valid_records_count: 0,
                rejected_records_count: 1,
                processing_time_ms: Some((elapsed_ms * 100.0).round() / 100.0),
                extraction_metadata: serde_json::json!({ "error": "UNSUPPORTED_SOURCE_TYPE" }),
            };
        }
    };

    // Read content safely
    let raw_bytes = match std::fs::read(target_path) {
        Ok(b) => b,
        Err(e) => {
            let elapsed_ms = start_time.elapsed().as_secs_f64() * 1000.0;
            let issue = ExtractionValidationIssue {
                record_id: None,
                field_name: "file_path".to_string(),
                issue_type: "FILE_READ_ERROR".to_string(),
                raw_value: Some(request.file_path.display().to_string()),
                message: format!("Failed to read raw evidence bytes: {}", e),
                row_index: None,
            };
            return UnifiedExtractionResult {
                evidence_id: request.evidence_id.clone(),
                case_id: request.case_id.clone(),
                source_type: request.source_type.clone(),
                status: ExtractionStatus::Failed,
                identifiers: Vec::new(),
                resolved_entities: Vec::new(),
                event_candidates: Vec::new(),
                relationship_candidates: Vec::new(),
                validation_issues: vec![issue],
                total_records_processed: 0,
                valid_records_count: 0,
                rejected_records_count: 1,
                processing_time_ms: Some((elapsed_ms * 100.0).round() / 100.0),
                extraction_metadata: serde_json::json!({ "error": "FILE_READ_ERROR" }),
            };
        }
    };

    let raw_text = String::from_utf8_lossy(&raw_bytes).to_string();

    // 4. Modality parser execution
    let (
        parser_name,
        parser_version,
        raw_identifiers,
        event_candidates,
        relationship_candidates,
        validation_issues,
        valid_records_count,
        rejected_records_count,
        total_records_processed,
    ) = match canonical_modality {
        "CALL_DETAIL_RECORD" => {
            let res = parse_cdr_csv(
                &raw_text,
                &request.evidence_id,
                request.default_country_prefix.as_deref(),
            );
            let valid_cnt = res.valid_records.len();
            let rej_cnt = res.rejected_records.len();
            (
                "cdr_parser",
                "1.0.0",
                res.identifiers,
                res.event_candidates,
                res.relationship_candidates,
                res.rejected_records,
                valid_cnt,
                rej_cnt,
                res.total_rows_processed,
            )
        }
        "CHAT_EXPORT" => {
            let res = parse_chat_json(
                &raw_text,
                &request.evidence_id,
                request.default_country_prefix.as_deref(),
            );
            let valid_cnt = res.valid_messages.len();
            let rej_cnt = res.rejected_messages.len();
            (
                "chat_parser",
                "1.0.0",
                res.identifiers,
                res.event_candidates,
                res.relationship_candidates,
                res.rejected_messages,
                valid_cnt,
                rej_cnt,
                res.total_messages_processed,
            )
        }
        _ => unreachable!(),
    };

    // 5. Entity resolution & deduplication across extracted identifiers
    let resolved_entities =
        deduplicate_identifiers(&raw_identifiers, request.default_country_prefix.as_deref());

    // 6. Status determination
    let status = if valid_records_count > 0 && rejected_records_count == 0 {
        ExtractionStatus::Success
    } else if valid_records_count > 0 && rejected_records_count > 0 {
        ExtractionStatus::PartialSuccess
    } else {
        ExtractionStatus::Failed
    };

    let elapsed_ms = start_time.elapsed().as_secs_f64() * 1000.0;

    let mut extraction_meta_map = serde_json::Map::new();
    extraction_meta_map.insert("parser_name".to_string(), serde_json::json!(parser_name));
    extraction_meta_map.insert(
        "parser_version".to_string(),
        serde_json::json!(parser_version),
    );
    if let Some(ref tz) = request.source_timezone {
        extraction_meta_map.insert("source_timezone".to_string(), serde_json::json!(tz));
    }
    if let Some(ref off) = request.clock_offset_seconds {
        extraction_meta_map.insert("clock_offset_seconds".to_string(), serde_json::json!(off));
    }
    if let Some(ref dcp) = request.default_country_prefix {
        extraction_meta_map.insert("default_country_prefix".to_string(), serde_json::json!(dcp));
    }
    extraction_meta_map.insert(
        "verified_sha256".to_string(),
        serde_json::json!(computed_sha256),
    );

    if let Some(obj) = request.metadata.as_object() {
        for (k, v) in obj {
            if !extraction_meta_map.contains_key(k) {
                extraction_meta_map.insert(k.clone(), v.clone());
            }
        }
    }

    UnifiedExtractionResult {
        evidence_id: request.evidence_id.clone(),
        case_id: request.case_id.clone(),
        source_type: request.source_type.clone(),
        status,
        identifiers: raw_identifiers,
        resolved_entities,
        event_candidates,
        relationship_candidates,
        validation_issues,
        total_records_processed,
        valid_records_count,
        rejected_records_count,
        processing_time_ms: Some((elapsed_ms * 100.0).round() / 100.0),
        extraction_metadata: serde_json::Value::Object(extraction_meta_map),
    }
}

/// Helper service to build an extraction request and manage versioned extraction runs.
#[derive(Clone)]
pub struct EvidenceExtractionService {
    storage: StorageManager,
    locks: Arc<std::sync::Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>>,
}

impl EvidenceExtractionService {
    /// Creates a new EvidenceExtractionService bound to a forensic StorageManager.
    pub fn new(storage: StorageManager) -> Self {
        Self {
            storage,
            locks: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }

    /// Returns a reference to the underlying StorageManager.
    pub fn storage(&self) -> &StorageManager {
        &self.storage
    }

    /// Obtains a process-local per-evidence lock to serialize concurrent extraction attempts.
    fn get_lock(&self, evidence_id: &str) -> Arc<tokio::sync::Mutex<()>> {
        let mut map = self.locks.lock().unwrap();
        map.entry(evidence_id.to_string())
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
            .clone()
    }

    /// Constructs a trusted extraction request directly from an ingestion manifest.
    ///
    /// Validates directory containment and resolves the physical `.raw` path
    /// through `StorageManager` to prevent directory traversal or arbitrary client paths.
    pub fn build_request_from_manifest(
        manifest: &EvidenceManifest,
        storage: &StorageManager,
        default_country_prefix: Option<String>,
    ) -> Result<EvidenceExtractionRequest, AppError> {
        Self::build_request_from_receipt(&manifest.receipt, storage, default_country_prefix)
    }

    /// Constructs a trusted extraction request directly from an evidence upload receipt.
    pub fn build_request_from_receipt(
        receipt: &crate::ingestion::EvidenceUploadReceipt,
        storage: &StorageManager,
        default_country_prefix: Option<String>,
    ) -> Result<EvidenceExtractionRequest, AppError> {
        let raw_file_path = storage.get_raw_file_path(&receipt.case_id, &receipt.evidence_id)?;

        let source_timezone = receipt
            .metadata
            .get("source_timezone")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let clock_offset_seconds = receipt
            .metadata
            .get("clock_offset_seconds")
            .and_then(|v| v.as_f64());

        Ok(EvidenceExtractionRequest {
            evidence_id: receipt.evidence_id.clone(),
            case_id: receipt.case_id.clone(),
            source_type: receipt.source_type.clone(),
            file_path: raw_file_path,
            expected_sha256: receipt.sha256_hash.clone(),
            source_timezone,
            clock_offset_seconds,
            metadata: receipt.metadata.clone(),
            default_country_prefix,
        })
    }

    /// Executes a versioned forensic extraction run for an evidence exhibit.
    ///
    /// Features:
    /// - Serializes concurrent attempts for the same evidence exhibit.
    /// - Dispatches CPU-bound parsing to `tokio::task::spawn_blocking`.
    /// - Derives a unique UUIDv4 `extraction_run_id`.
    /// - Computes a deterministic `configuration_hash`.
    /// - Atomically stores `run-manifest.json` and `result.json`.
    /// - Updates `active-run.json` ONLY if extraction status is SUCCESS or PARTIAL_SUCCESS.
    /// - Leaves existing active runs and raw evidence completely intact if extraction fails.
    pub async fn execute_extraction_from_receipt(
        &self,
        receipt: &crate::ingestion::EvidenceUploadReceipt,
        reprocess: Option<ReprocessRequest>,
    ) -> Result<ExtractionRunManifest, AppError> {
        let lock = self.get_lock(&receipt.evidence_id);
        let _guard = lock.lock().await;

        let default_country = reprocess
            .as_ref()
            .and_then(|r| r.default_country_code.clone());
        let mut request =
            Self::build_request_from_receipt(receipt, &self.storage, default_country)?;

        if let Some(ref rep) = reprocess {
            if let Some(ref tz) = rep.source_timezone {
                request.source_timezone = Some(tz.clone());
            }
            if let Some(offset) = rep.clock_offset_seconds {
                request.clock_offset_seconds = Some(offset);
            }
            if let Some(ref meta) = rep.metadata {
                request.metadata = meta.clone();
            }
        }

        let run_id = uuid::Uuid::new_v4().to_string();
        let config = ExtractionConfiguration {
            extractor_name: "yukti_unified_extractor".to_string(),
            extractor_version: "1.0.0".to_string(),
            source_timezone: request.source_timezone.clone(),
            clock_offset_seconds: request.clock_offset_seconds,
            default_country_prefix: request.default_country_prefix.clone(),
            custom_options: serde_json::json!({}),
        };
        let config_hash = config.compute_hash();
        let started_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

        let mut run_manifest = ExtractionRunManifest {
            extraction_run_id: run_id.clone(),
            evidence_id: receipt.evidence_id.clone(),
            case_id: receipt.case_id.clone(),
            source_sha256: receipt.sha256_hash.clone(),
            extractor_name: "yukti_unified_extractor".to_string(),
            extractor_version: "1.0.0".to_string(),
            configuration_hash: config_hash,
            configuration: config,
            started_at,
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

        // Persist initial running manifest
        self.storage.save_run_manifest(&run_manifest).await?;

        let start_time = Instant::now();
        let req_clone = request.clone();
        let extraction_res = tokio::task::spawn_blocking(move || extract_evidence(&req_clone))
            .await
            .map_err(|e| {
                AppError::Internal(anyhow::anyhow!("Extraction worker panicked: {}", e))
            })?;

        let elapsed_ms = start_time.elapsed().as_secs_f64() * 1000.0;
        let completed_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

        let run_status = match extraction_res.status {
            ExtractionStatus::Success => ExtractionRunStatus::Success,
            ExtractionStatus::PartialSuccess => ExtractionRunStatus::PartialSuccess,
            ExtractionStatus::Failed => ExtractionRunStatus::Failed,
        };

        // Atomically save result.json and get its SHA-256
        let result_sha256 = self
            .storage
            .save_run_result(
                &receipt.case_id,
                &receipt.evidence_id,
                &run_id,
                &extraction_res,
            )
            .await?;

        run_manifest.status = run_status;
        run_manifest.completed_at = Some(completed_at.clone());
        run_manifest.duration_ms = Some((elapsed_ms * 100.0).round() / 100.0);
        run_manifest.records_processed = extraction_res.total_records_processed;
        run_manifest.valid_records_count = extraction_res.valid_records_count;
        run_manifest.rejected_records_count = extraction_res.rejected_records_count;
        run_manifest.validation_issue_count = extraction_res.validation_issues.len();
        run_manifest.result_sha256 = Some(result_sha256);
        if run_status == ExtractionRunStatus::Failed {
            if let Some(first_issue) = extraction_res.validation_issues.first() {
                run_manifest.error_message = Some(format!(
                    "[{}] {}",
                    first_issue.issue_type, first_issue.message
                ));
            } else {
                run_manifest.error_message =
                    Some("Extraction failed with no valid records".to_string());
            }
        }

        // Finalize run manifest on disk
        self.storage.save_run_manifest(&run_manifest).await?;

        // Update active run pointer ONLY if status is Success or PartialSuccess
        if run_status == ExtractionRunStatus::Success
            || run_status == ExtractionRunStatus::PartialSuccess
        {
            let pointer = ActiveRunPointer {
                evidence_id: receipt.evidence_id.clone(),
                case_id: receipt.case_id.clone(),
                active_run_id: run_id,
                status: run_status,
                activated_at: completed_at,
            };
            self.storage.save_active_run_pointer(&pointer).await?;
        }

        Ok(run_manifest)
    }
}

/// Consumes extracted video keyframe artifacts into standardized forensic event candidates (Member 1).
pub fn consume_video_frames(
    evidence_id: &str,
    frames: &[crate::reconstruction::model::ExtractedFrameArtifact],
    base_timestamp_utc: Option<&str>,
) -> Vec<crate::extraction::model::ExtractedEventCandidate> {
    frames
        .iter()
        .map(|f| {
            let timestamp_str = if let Some(ref est) = f.estimated_utc {
                est.to_rfc3339()
            } else if let Some(base) = base_timestamp_utc {
                if let Ok(base_dt) = chrono::DateTime::parse_from_rfc3339(base) {
                    let frame_dt = base_dt
                        + chrono::Duration::milliseconds((f.pts_seconds * 1000.0).round() as i64);
                    frame_dt.to_rfc3339()
                } else {
                    format!("PTS:{:.3}s", f.pts_seconds)
                }
            } else {
                format!("PTS:{:.3}s", f.pts_seconds)
            };

            let mut meta = serde_json::Map::new();
            meta.insert("frame_index".to_string(), serde_json::json!(f.frame_index));
            meta.insert("pts_seconds".to_string(), serde_json::json!(f.pts_seconds));
            meta.insert("sha256_hash".to_string(), serde_json::json!(f.sha256_hash));
            meta.insert(
                "relative_path".to_string(),
                serde_json::json!(f.relative_path),
            );

            crate::extraction::model::ExtractedEventCandidate {
                candidate_id: format!("evt-frame-{}-{}", evidence_id, f.frame_index),
                source_evidence_id: evidence_id.to_string(),
                source_record_id: format!("frame_{:04}", f.frame_index),
                event_type: "CCTV_KEYFRAME".to_string(),
                raw_timestamp: timestamp_str,
                timestamp_precision: Some("EXACT_SECOND".to_string()),
                description: Some(format!(
                    "Video keyframe {} at PTS {:.2}s (SHA-256: {})",
                    f.frame_index, f.pts_seconds, f.sha256_hash
                )),
                duration_seconds: None,
                location_name: None,
                involved_entity_raw_ids: Vec::new(),
                metadata: serde_json::Value::Object(meta),
            }
        })
        .collect()
}
