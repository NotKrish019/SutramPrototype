//! Forensic Evidence Ingestion Service (Member 2).
//!
//! Orchestrates the complete evidence intake workflow:
//! 1. Bounded chunk streaming directly to a temporary `.part` file.
//! 2. Real-time incremental SHA-256 calculation.
//! 3. Maximum size limit enforcement.
//! 4. Atomic file rename to destination `{evidence_id}.raw`.
//! 5. Post-storage independent SHA-256 verification from disk bytes.
//! 6. Format-aware content sniffing and structure validation.
//! 7. Atomic metadata manifest persistence (`metadata.json`).
//! 8. In-memory caching with filesystem search fallback.

use bytes::Bytes;
use futures_util::{Stream, StreamExt};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::error::AppError;
use crate::ingestion::hashing::{verify_file_sha256, IncrementalHasher};
use crate::ingestion::model::{EvidenceManifest, EvidenceUploadReceipt, IngestionStatus};
use crate::ingestion::storage::{sanitize_filename, validate_safe_id, StorageManager};
use crate::ingestion::validation::sniff_and_validate_content;

/// Core evidence ingestion coordinator.
#[derive(Clone)]
pub struct EvidenceIngestionService {
    storage: StorageManager,
    max_upload_size_bytes: usize,
    index: Arc<RwLock<HashMap<String, EvidenceUploadReceipt>>>,
}

impl EvidenceIngestionService {
    /// Creates a new ingestion service with the provided storage manager and upload constraints.
    pub fn new(storage: StorageManager, max_upload_size_bytes: usize) -> Self {
        Self {
            storage,
            max_upload_size_bytes,
            index: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Returns a reference to the underlying storage manager.
    pub fn storage(&self) -> &StorageManager {
        &self.storage
    }

    /// Ingests an evidence file from an incoming stream in bounded chunks.
    ///
    /// Ensures:
    /// - Memory usage is strictly bounded (never buffers full payload into memory).
    /// - Rejects files exceeding `max_upload_size_bytes` immediately, cleaning up partial writes.
    /// - Computes cryptographic SHA-256 incrementally.
    /// - Performs post-storage verification to ensure disk bytes match the streaming digest.
    /// - Validates content structures and magic bytes.
    /// - Assigns an independent UUID to every upload exhibit (duplicates are NOT rejected).
    pub async fn ingest_stream<S, E>(
        &self,
        case_id: &str,
        declared_filename: &str,
        source_type_override: Option<String>,
        metadata: serde_json::Value,
        mut stream: S,
    ) -> Result<EvidenceUploadReceipt, AppError>
    where
        S: Stream<Item = Result<Bytes, E>> + Unpin,
        E: std::fmt::Display,
    {
        // 1. Validate case identifier
        let trimmed_case = case_id.trim();
        validate_safe_id(trimmed_case, "case_id")?;
        if Uuid::parse_str(trimmed_case).is_err() {
            return Err(AppError::BadRequest(format!(
                "Invalid case_id '{}': must be a valid UUIDv4 string.",
                case_id
            )));
        }

        // 2. Generate canonical evidence UUID
        let evidence_id = Uuid::new_v4().to_string();
        let sanitized_filename = sanitize_filename(declared_filename);

        // 3. Prepare storage paths
        self.storage
            .ensure_evidence_dir(trimmed_case, &evidence_id)
            .await?;

        let part_path = self
            .storage
            .get_part_file_path(trimmed_case, &evidence_id)?;
        let raw_dest_path = self.storage.get_raw_file_path(trimmed_case, &evidence_id)?;

        // Ensure destination raw file does not already exist
        if raw_dest_path.exists() {
            return Err(AppError::Conflict(format!(
                "Evidence file already exists at destination for id '{}'. Overwrite prohibited.",
                evidence_id
            )));
        }

        // 4. Open temporary file and initialize incremental hasher
        let mut part_file = match File::create(&part_path).await {
            Ok(f) => f,
            Err(e) => {
                StorageManager::safe_unlink(&part_path).await;
                return Err(AppError::Internal(anyhow::anyhow!(
                    "Failed to create temporary upload file on disk: {}",
                    e
                )));
            }
        };

        let mut hasher = IncrementalHasher::new();
        let mut bytes_written: u64 = 0;

        // 5. Stream chunks from network directly to disk
        while let Some(chunk_res) = stream.next().await {
            let chunk = match chunk_res {
                Ok(c) => c,
                Err(e) => {
                    drop(part_file);
                    StorageManager::safe_unlink(&part_path).await;
                    return Err(AppError::BadRequest(format!(
                        "Upload stream interrupted or failed: {}",
                        e
                    )));
                }
            };

            let chunk_len = chunk.len() as u64;
            if (bytes_written + chunk_len) > self.max_upload_size_bytes as u64 {
                drop(part_file);
                StorageManager::safe_unlink(&part_path).await;
                return Err(AppError::PayloadTooLarge(format!(
                    "Upload exceeds maximum allowable evidence size of {} bytes.",
                    self.max_upload_size_bytes
                )));
            }

            hasher.update(&chunk);

            if let Err(e) = part_file.write_all(&chunk).await {
                drop(part_file);
                StorageManager::safe_unlink(&part_path).await;
                return Err(AppError::Internal(anyhow::anyhow!(
                    "Failed to write chunk to temporary evidence file: {}",
                    e
                )));
            }

            bytes_written += chunk_len;
        }

        // Flush and sync to physical media
        if let Err(e) = part_file.flush().await {
            drop(part_file);
            StorageManager::safe_unlink(&part_path).await;
            return Err(AppError::Internal(anyhow::anyhow!(
                "Failed to flush temporary evidence file: {}",
                e
            )));
        }
        if let Err(e) = part_file.sync_all().await {
            drop(part_file);
            StorageManager::safe_unlink(&part_path).await;
            return Err(AppError::Internal(anyhow::anyhow!(
                "Failed to fsync temporary evidence file to storage: {}",
                e
            )));
        }
        drop(part_file);

        // 6. Check for empty upload
        if bytes_written == 0 {
            StorageManager::safe_unlink(&part_path).await;
            return Err(AppError::BadRequest(
                "Uploaded evidence file is empty (0 bytes).".to_string(),
            ));
        }

        let sha256_hash = hasher.finalize_hex();

        // 7. Atomic rename from `.part` to final destination
        if let Err(e) = tokio::fs::rename(&part_path, &raw_dest_path).await {
            StorageManager::safe_unlink(&part_path).await;
            return Err(AppError::Internal(anyhow::anyhow!(
                "Failed to atomically finalize evidence file: {}",
                e
            )));
        }

        // 8. Post-storage independent SHA-256 verification
        match verify_file_sha256(&raw_dest_path, &sha256_hash).await {
            Ok(true) => {}
            Ok(false) => {
                StorageManager::safe_unlink(&raw_dest_path).await;
                return Err(AppError::Internal(anyhow::anyhow!(
                    "Post-storage SHA-256 verification failed: disk bytes do not match ingestion digest."
                )));
            }
            Err(e) => {
                StorageManager::safe_unlink(&raw_dest_path).await;
                return Err(AppError::Internal(anyhow::anyhow!(
                    "Failed to read stored evidence for post-storage hash verification: {}",
                    e
                )));
            }
        }

        // 9. Format-aware content validation
        let inspection = match sniff_and_validate_content(&raw_dest_path, &sanitized_filename).await
        {
            Ok(insp) => insp,
            Err(e) => {
                // Remove rejected file
                StorageManager::safe_unlink(&raw_dest_path).await;
                return Err(e);
            }
        };

        // Determine effective source modality
        let source_type = source_type_override.unwrap_or(inspection.source_type);
        let storage_uri = format!("urn:sutram:evidence:{}:storage", evidence_id);
        let now_utc = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

        // 10. Assemble structured receipt
        let receipt = EvidenceUploadReceipt {
            evidence_id: evidence_id.clone(),
            case_id: trimmed_case.to_string(),
            original_filename: sanitized_filename,
            source_type,
            mime_type: inspection.mime_type,
            file_size_bytes: bytes_written,
            sha256_hash,
            storage_uri,
            status: IngestionStatus::Ingested,
            metadata,
            created_at: now_utc.clone(),
        };

        // 11. Persist metadata manifest atomically
        let manifest = EvidenceManifest {
            receipt: receipt.clone(),
            relative_raw_path: format!("{}/{}/{}.raw", trimmed_case, evidence_id, evidence_id),
            stored_at: now_utc,
        };

        if let Err(e) = self.storage.save_manifest(&manifest).await {
            StorageManager::safe_unlink(&raw_dest_path).await;
            return Err(e);
        }

        // 12. Update in-memory index
        self.index
            .write()
            .await
            .insert(evidence_id.clone(), receipt.clone());

        tracing::info!(
            evidence_id = %evidence_id,
            case_id = %trimmed_case,
            sha256 = %receipt.sha256_hash,
            size = bytes_written,
            "Forensic evidence ingested and persisted successfully"
        );

        Ok(receipt)
    }

    /// Retrieves an evidence receipt by evidence UUID.
    ///
    /// Checks the in-memory cache first, then scans storage manifests on disk.
    pub async fn get_evidence(&self, evidence_id: &str) -> Result<EvidenceUploadReceipt, AppError> {
        let trimmed_id = evidence_id.trim();
        validate_safe_id(trimmed_id, "evidence_id")?;

        // 1. Check in-memory index
        if let Some(receipt) = self.index.read().await.get(trimmed_id) {
            return Ok(receipt.clone());
        }

        // 2. Fall back to filesystem manifest search
        if let Some(receipt) = self.storage.find_receipt_by_id(trimmed_id).await? {
            // Re-populate in-memory cache
            self.index
                .write()
                .await
                .insert(trimmed_id.to_string(), receipt.clone());
            return Ok(receipt);
        }

        Err(AppError::NotFound(format!(
            "Evidence item with id '{}' was not found.",
            evidence_id
        )))
    }
}
