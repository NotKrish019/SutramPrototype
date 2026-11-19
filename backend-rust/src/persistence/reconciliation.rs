//! Reconciliation Service: Idempotent synchronization between filesystem manifests and PostgreSQL.

use std::sync::Arc;
use tokio::fs;
use tracing::{info, warn};
use uuid::Uuid;

use super::case_repository::CaseRepository;
use super::custody_repository::CustodyRepository;
use super::evidence_repository::EvidenceRepository;
use super::extraction_run_repository::ExtractionRunRepository;
use super::model::{EvidenceEntity, ExtractionRunEntity};
use crate::error::AppError;
use crate::extraction::model::ActiveRunPointer;
use crate::ingestion::StorageManager;

/// Summary metrics of a reconciliation run.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ReconciliationReport {
    pub cases_checked: usize,
    pub cases_created: usize,
    pub evidence_checked: usize,
    pub evidence_registered: usize,
    pub runs_checked: usize,
    pub runs_registered: usize,
    pub warnings: Vec<String>,
}

pub struct ReconciliationService {
    storage: StorageManager,
    case_repo: Arc<CaseRepository>,
    evidence_repo: Arc<EvidenceRepository>,
    extraction_repo: Arc<ExtractionRunRepository>,
    custody_repo: Arc<CustodyRepository>,
}

impl ReconciliationService {
    pub fn new(
        storage: StorageManager,
        case_repo: Arc<CaseRepository>,
        evidence_repo: Arc<EvidenceRepository>,
        extraction_repo: Arc<ExtractionRunRepository>,
        custody_repo: Arc<CustodyRepository>,
    ) -> Self {
        Self {
            storage,
            case_repo,
            evidence_repo,
            extraction_repo,
            custody_repo,
        }
    }

    /// Scans filesystem storage roots and synchronizes any untracked exhibits or runs into PostgreSQL.
    pub async fn reconcile_all(&self) -> Result<ReconciliationReport, AppError> {
        let mut report = ReconciliationReport::default();
        let raw_root = self.storage.root_dir();

        if !raw_root.exists() {
            info!(
                "Raw storage root '{}' does not exist; skipping filesystem reconciliation.",
                raw_root.display()
            );
            return Ok(report);
        }

        // 1. Scan case directories under raw storage
        let mut case_entries = match fs::read_dir(&raw_root).await {
            Ok(rd) => rd,
            Err(e) => {
                warn!(
                    "Unable to read raw storage root '{}': {}",
                    raw_root.display(),
                    e
                );
                return Ok(report);
            }
        };

        while let Ok(Some(case_entry)) = case_entries.next_entry().await {
            let case_path = case_entry.path();
            if !case_path.is_dir() {
                continue;
            }

            let case_id_str = case_entry.file_name().to_string_lossy().to_string();
            let case_id = match Uuid::parse_str(&case_id_str) {
                Ok(u) => u,
                Err(_) => continue,
            };

            report.cases_checked += 1;
            // Ensure case exists in PostgreSQL
            let case_existed = self.case_repo.get_case(case_id).await?.is_some();
            if !case_existed {
                self.case_repo
                    .ensure_case_exists(case_id, &format!("Case {}", case_id))
                    .await?;
                report.cases_created += 1;
            }

            // 2. Scan evidence exhibits in this case directory
            let mut ev_entries = match fs::read_dir(&case_path).await {
                Ok(rd) => rd,
                Err(_) => continue,
            };

            while let Ok(Some(ev_entry)) = ev_entries.next_entry().await {
                let ev_path = ev_entry.path();
                if !ev_path.is_dir() {
                    continue;
                }

                let ev_id_str = ev_entry.file_name().to_string_lossy().to_string();
                let evidence_id = match Uuid::parse_str(&ev_id_str) {
                    Ok(u) => u,
                    Err(_) => continue,
                };

                report.evidence_checked += 1;

                // Attempt to read metadata.json / manifest.json
                let manifest = match self.storage.load_manifest(&case_id_str, &ev_id_str).await {
                    Ok(m) => m,
                    Err(_) => {
                        report.warnings.push(format!(
                            "Missing or unparseable manifest for evidence '{}' in case '{}'",
                            ev_id_str, case_id_str
                        ));
                        continue;
                    }
                };

                // Check if evidence already exists in database
                let existing_ev = self.evidence_repo.get_evidence(evidence_id).await?;
                if existing_ev.is_none() {
                    let receipt = &manifest.receipt;
                    let now = chrono::Utc::now();
                    let entity = EvidenceEntity {
                        evidence_id,
                        case_id,
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

                    self.evidence_repo.register_evidence(&entity).await?;

                    // Record reconciliation custody event
                    let _ = self
                        .custody_repo
                        .append_event(
                            evidence_id,
                            case_id,
                            "RECONCILIATION_INGESTED",
                            "SYSTEM_RECONCILIATION",
                            Some(&receipt.sha256_hash),
                            serde_json::json!({
                                "source": "filesystem_reconciliation",
                                "file_size_bytes": receipt.file_size_bytes,
                            }),
                        )
                        .await;

                    report.evidence_registered += 1;
                }

                // 3. Scan extraction runs for this exhibit under derived directory
                let runs_dir = match self
                    .storage
                    .get_extraction_runs_dir(&case_id_str, &ev_id_str)
                {
                    Ok(rd) => rd,
                    Err(_) => continue,
                };

                if !runs_dir.exists() {
                    continue;
                }

                // Check active-run pointer
                let active_pointer: Option<ActiveRunPointer> = self
                    .storage
                    .load_active_run_pointer(&case_id_str, &ev_id_str)
                    .await
                    .unwrap_or_default();

                let mut run_entries = match fs::read_dir(&runs_dir).await {
                    Ok(rd) => rd,
                    Err(_) => continue,
                };

                while let Ok(Some(run_entry)) = run_entries.next_entry().await {
                    let run_path = run_entry.path();
                    if !run_path.is_dir() {
                        continue;
                    }

                    let run_id_str = run_entry.file_name().to_string_lossy().to_string();
                    let run_id = match Uuid::parse_str(&run_id_str) {
                        Ok(u) => u,
                        Err(_) => continue,
                    };

                    report.runs_checked += 1;

                    // Check if run already in database
                    if self.extraction_repo.get_run(run_id).await?.is_some() {
                        continue;
                    }

                    // Load run manifest and result
                    let run_manifest = match self
                        .storage
                        .load_run_manifest(&case_id_str, &ev_id_str, &run_id_str)
                        .await
                    {
                        Ok(rm) => rm,
                        Err(_) => continue,
                    };

                    let result = match self
                        .storage
                        .load_run_result(&case_id_str, &ev_id_str, &run_id_str)
                        .await
                    {
                        Ok(r) => r,
                        Err(_) => continue,
                    };

                    let started_at = chrono::DateTime::parse_from_rfc3339(&run_manifest.started_at)
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .unwrap_or_else(|_| chrono::Utc::now());

                    let run_entity = ExtractionRunEntity {
                        extraction_run_id: run_id,
                        evidence_id,
                        case_id,
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

                    self.extraction_repo.create_run(&run_entity).await?;

                    let is_active = active_pointer
                        .as_ref()
                        .map(|p| p.active_run_id == run_id_str)
                        .unwrap_or(false);

                    if is_active {
                        self.extraction_repo
                            .complete_run_transactional(&run_manifest, &result, true)
                            .await?;
                    }

                    let _ = self
                        .custody_repo
                        .append_event(
                            evidence_id,
                            case_id,
                            "RECONCILIATION_RUN_SYNCED",
                            "SYSTEM_RECONCILIATION",
                            None,
                            serde_json::json!({
                                "run_id": run_id_str,
                                "is_active": is_active,
                            }),
                        )
                        .await;

                    report.runs_registered += 1;
                }
            }
        }

        info!(
            "Reconciliation complete: {} cases checked ({} created), {} exhibits checked ({} registered), {} runs checked ({} registered).",
            report.cases_checked, report.cases_created,
            report.evidence_checked, report.evidence_registered,
            report.runs_checked, report.runs_registered
        );

        Ok(report)
    }
}
