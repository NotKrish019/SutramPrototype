//! Forensic Evidence Storage Management (Member 2).
//!
//! Enforces:
//! - Strict directory containment defenses preventing path traversal.
//! - UUID-based internal storage paths (client filenames are NEVER used as filesystem paths).
//! - Atomic streaming writes via temporary `.part` files.
//! - Non-destructive immutability (prohibits overwriting existing evidence files).
//! - Cross-platform compatibility (Windows device names, POSIX permissions).

use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tokio::fs;

use crate::error::AppError;
use crate::extraction::model::{
    ActiveRunPointer, ExtractionRunManifest, ExtractionRunStatus, UnifiedExtractionResult,
};
use crate::ingestion::model::{EvidenceManifest, EvidenceUploadReceipt};

/// Windows-reserved filenames that must not be created on disk.
const RESERVED_NAMES: &[&str] = &[
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// Sanitizes a client-provided original filename to prevent path traversal,
/// Windows device collisions, and illegal characters.
pub fn sanitize_filename(filename: &str) -> String {
    if filename.trim().is_empty() {
        return "unnamed_evidence.bin".to_string();
    }

    // Strip directory components (both Unix '/' and Windows '\')
    let base = filename
        .replace('\\', "/")
        .split('/')
        .next_back()
        .unwrap_or("")
        .trim()
        .to_string();

    // Filter control chars, null bytes, and filesystem-illegal characters
    let mut cleaned: String = base
        .chars()
        .filter(|c| {
            c.is_ascii()
                && !c.is_ascii_control()
                && !matches!(
                    *c,
                    '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' | '\0'
                )
        })
        .collect();

    // Strip leading/trailing dots and whitespace
    cleaned = cleaned
        .trim_matches(|c: char| c == '.' || c.is_whitespace())
        .to_string();

    if cleaned.is_empty() {
        return "sanitized_evidence.bin".to_string();
    }

    // Prefix safe_ if stem matches a Windows device name
    let stem = cleaned.split('.').next().unwrap_or("").to_ascii_lowercase();

    if RESERVED_NAMES.contains(&stem.as_str()) {
        cleaned = format!("safe_{}", cleaned);
    }

    cleaned
}

/// Validates that an identifier (case_id, evidence_id) consists strictly of safe alphanumeric characters.
pub fn validate_safe_id(id: &str, field_name: &str) -> Result<(), AppError> {
    let trimmed = id.trim();
    if trimmed.is_empty() {
        return Err(AppError::BadRequest(format!(
            "Missing or empty required identifier: '{}'",
            field_name
        )));
    }

    // Check for traversal sequences or non-alphanumeric characters (allow hyphens and underscores for UUIDs)
    let is_safe = trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');

    if !is_safe || trimmed.contains("..") {
        return Err(AppError::BadRequest(format!(
            "Invalid identifier for '{}': contains illegal or path traversal characters.",
            field_name
        )));
    }

    Ok(())
}

/// Forensic Storage Manager managing directory creation, atomic file placement, and containment.
#[derive(Debug, Clone)]
pub struct StorageManager {
    root_dir: PathBuf,
    derived_dir: PathBuf,
}

impl StorageManager {
    /// Creates a new StorageManager with the designated root path, deriving the derived output path.
    pub fn new(root_dir: PathBuf) -> Self {
        let derived_dir = if root_dir.ends_with("raw") {
            root_dir.parent().unwrap_or(&root_dir).join("derived")
        } else {
            root_dir.join("derived")
        };
        Self {
            root_dir,
            derived_dir,
        }
    }

    /// Creates a new StorageManager with explicit root (raw) and derived paths.
    pub fn with_derived(root_dir: PathBuf, derived_dir: PathBuf) -> Self {
        Self {
            root_dir,
            derived_dir,
        }
    }

    /// Returns the root storage directory.
    pub fn root_dir(&self) -> &Path {
        &self.root_dir
    }

    /// Returns the derived analytical output directory.
    pub fn derived_dir(&self) -> &Path {
        &self.derived_dir
    }

    /// Resolves the case-scoped and evidence-scoped storage directory:
    /// `{root_dir}/{case_id}/{evidence_id}/`
    ///
    /// Asserts strict containment to prevent path traversal attacks.
    pub fn get_evidence_dir(&self, case_id: &str, evidence_id: &str) -> Result<PathBuf, AppError> {
        validate_safe_id(case_id, "case_id")?;
        validate_safe_id(evidence_id, "evidence_id")?;

        let dir = self.root_dir.join(case_id.trim()).join(evidence_id.trim());

        // Containment verification: dir must not escape root_dir
        if dir
            .components()
            .any(|c| c == std::path::Component::ParentDir)
        {
            return Err(AppError::BadRequest(
                "Path traversal detected: directory attempts to escape storage root.".to_string(),
            ));
        }

        Ok(dir)
    }

    /// Resolves the destination path for the raw evidence file:
    /// `{root_dir}/{case_id}/{evidence_id}/{evidence_id}.raw`
    pub fn get_raw_file_path(&self, case_id: &str, evidence_id: &str) -> Result<PathBuf, AppError> {
        let dir = self.get_evidence_dir(case_id, evidence_id)?;
        Ok(dir.join(format!("{}.raw", evidence_id.trim())))
    }

    /// Resolves the temporary upload file path:
    /// `{root_dir}/{case_id}/{evidence_id}/{evidence_id}.part`
    pub fn get_part_file_path(
        &self,
        case_id: &str,
        evidence_id: &str,
    ) -> Result<PathBuf, AppError> {
        let dir = self.get_evidence_dir(case_id, evidence_id)?;
        Ok(dir.join(format!("{}.part", evidence_id.trim())))
    }

    /// Resolves the metadata manifest path:
    /// `{root_dir}/{case_id}/{evidence_id}/metadata.json`
    pub fn get_manifest_path(&self, case_id: &str, evidence_id: &str) -> Result<PathBuf, AppError> {
        let dir = self.get_evidence_dir(case_id, evidence_id)?;
        Ok(dir.join("metadata.json"))
    }

    /// Ensures the evidence directory exists.
    pub async fn ensure_evidence_dir(
        &self,
        case_id: &str,
        evidence_id: &str,
    ) -> Result<PathBuf, AppError> {
        let dir = self.get_evidence_dir(case_id, evidence_id)?;
        fs::create_dir_all(&dir).await.map_err(|e| {
            AppError::Internal(anyhow::anyhow!(
                "Failed to create evidence directory: {}",
                e
            ))
        })?;
        Ok(dir)
    }

    /// Safely cleans up a temporary `.part` or unverified file.
    pub async fn safe_unlink(path: &Path) {
        if path.exists() {
            let _ = fs::remove_file(path).await;
        }
    }

    /// Persists the evidence metadata manifest atomically as JSON:
    /// Writes to `metadata.json.tmp` then renames to `metadata.json`.
    pub async fn save_manifest(&self, manifest: &EvidenceManifest) -> Result<(), AppError> {
        let manifest_path =
            self.get_manifest_path(&manifest.receipt.case_id, &manifest.receipt.evidence_id)?;

        let tmp_path = manifest_path.with_extension("json.tmp");
        let json_bytes = serde_json::to_vec_pretty(manifest).map_err(|e| {
            AppError::Internal(anyhow::anyhow!("Failed to serialize manifest: {}", e))
        })?;

        fs::write(&tmp_path, json_bytes).await.map_err(|e| {
            AppError::Internal(anyhow::anyhow!("Failed to write manifest temp file: {}", e))
        })?;

        fs::rename(&tmp_path, &manifest_path).await.map_err(|e| {
            AppError::Internal(anyhow::anyhow!("Failed to finalize manifest file: {}", e))
        })?;

        Ok(())
    }

    /// Reads and deserializes an evidence manifest from disk.
    /// Checks `metadata.json` first, falling back to `manifest.json`.
    pub async fn load_manifest(
        &self,
        case_id: &str,
        evidence_id: &str,
    ) -> Result<EvidenceManifest, AppError> {
        let primary_path = self.get_manifest_path(case_id, evidence_id)?;
        let manifest_path = if primary_path.exists() {
            primary_path
        } else {
            let fallback = self
                .get_evidence_dir(case_id, evidence_id)?
                .join("manifest.json");
            if fallback.exists() {
                fallback
            } else {
                return Err(AppError::NotFound(format!(
                    "Evidence manifest not found for id '{}'",
                    evidence_id
                )));
            }
        };

        let content = fs::read(&manifest_path).await.map_err(|e| {
            AppError::Internal(anyhow::anyhow!("Failed to read manifest file: {}", e))
        })?;

        let manifest: EvidenceManifest = serde_json::from_slice(&content).map_err(|e| {
            AppError::Internal(anyhow::anyhow!("Malformed manifest on disk: {}", e))
        })?;

        Ok(manifest)
    }

    /// Searches the storage root for an evidence manifest by evidence_id.
    /// Traverses `{storage_root}/*/{evidence_id}/metadata.json` (or `manifest.json`).
    pub async fn find_manifest_by_id(
        &self,
        evidence_id: &str,
    ) -> Result<Option<EvidenceManifest>, AppError> {
        validate_safe_id(evidence_id, "evidence_id")?;

        if !self.root_dir.exists() {
            return Ok(None);
        }

        let mut case_entries = match fs::read_dir(&self.root_dir).await {
            Ok(entries) => entries,
            Err(_) => return Ok(None),
        };

        while let Ok(Some(case_entry)) = case_entries.next_entry().await {
            if let Ok(file_type) = case_entry.file_type().await {
                if file_type.is_dir() {
                    let ev_dir = case_entry.path().join(evidence_id);
                    let mut target = ev_dir.join("metadata.json");
                    if !target.exists() {
                        target = ev_dir.join("manifest.json");
                    }
                    if target.exists() {
                        if let Ok(content) = fs::read(&target).await {
                            if let Ok(manifest) =
                                serde_json::from_slice::<EvidenceManifest>(&content)
                            {
                                return Ok(Some(manifest));
                            }
                        }
                    }
                }
            }
        }

        Ok(None)
    }

    /// Searches the storage root for an evidence manifest and returns its receipt.
    pub async fn find_receipt_by_id(
        &self,
        evidence_id: &str,
    ) -> Result<Option<EvidenceUploadReceipt>, AppError> {
        Ok(self
            .find_manifest_by_id(evidence_id)
            .await?
            .map(|m| m.receipt))
    }

    /// Finds the case_id for a given evidence_id.
    pub async fn find_case_id_for_evidence(
        &self,
        evidence_id: &str,
    ) -> Result<Option<String>, AppError> {
        Ok(self
            .find_manifest_by_id(evidence_id)
            .await?
            .map(|m| m.receipt.case_id))
    }

    /// Lists all evidence exhibits associated with a specific case from the filesystem.
    pub async fn list_manifests_for_case(
        &self,
        case_id: &str,
    ) -> Result<Vec<EvidenceManifest>, AppError> {
        validate_safe_id(case_id, "case_id")?;

        let case_dir = self.root_dir.join(case_id);
        if !case_dir.exists() {
            return Ok(Vec::new());
        }

        let mut manifests = Vec::new();
        let mut entries = match fs::read_dir(&case_dir).await {
            Ok(e) => e,
            Err(_) => return Ok(Vec::new()),
        };

        while let Ok(Some(entry)) = entries.next_entry().await {
            if let Ok(file_type) = entry.file_type().await {
                if file_type.is_dir() {
                    let ev_dir = entry.path();
                    let mut target = ev_dir.join("metadata.json");
                    if !target.exists() {
                        target = ev_dir.join("manifest.json");
                    }
                    if target.exists() {
                        if let Ok(content) = fs::read(&target).await {
                            if let Ok(manifest) =
                                serde_json::from_slice::<EvidenceManifest>(&content)
                            {
                                manifests.push(manifest);
                            }
                        }
                    }
                }
            }
        }

        manifests.sort_by(|a, b| a.receipt.created_at.cmp(&b.receipt.created_at));
        Ok(manifests)
    }

    /// Resolves the case-scoped and evidence-scoped derived storage directory:
    /// `{derived_dir}/{case_id}/{evidence_id}/`
    pub fn get_derived_evidence_dir(
        &self,
        case_id: &str,
        evidence_id: &str,
    ) -> Result<PathBuf, AppError> {
        validate_safe_id(case_id, "case_id")?;
        validate_safe_id(evidence_id, "evidence_id")?;

        let dir = self
            .derived_dir
            .join(case_id.trim())
            .join(evidence_id.trim());

        if dir
            .components()
            .any(|c| c == std::path::Component::ParentDir)
        {
            return Err(AppError::BadRequest(
                "Path traversal detected: directory attempts to escape derived storage root."
                    .to_string(),
            ));
        }

        Ok(dir)
    }

    /// Resolves the extraction-runs directory:
    /// `{derived_dir}/{case_id}/{evidence_id}/extraction-runs/`
    pub fn get_extraction_runs_dir(
        &self,
        case_id: &str,
        evidence_id: &str,
    ) -> Result<PathBuf, AppError> {
        let dir = self.get_derived_evidence_dir(case_id, evidence_id)?;
        Ok(dir.join("extraction-runs"))
    }

    /// Resolves a specific run directory:
    /// `{derived_dir}/{case_id}/{evidence_id}/extraction-runs/{run_id}/`
    pub fn get_run_dir(
        &self,
        case_id: &str,
        evidence_id: &str,
        run_id: &str,
    ) -> Result<PathBuf, AppError> {
        validate_safe_id(run_id, "run_id")?;
        let dir = self.get_extraction_runs_dir(case_id, evidence_id)?;
        let run_dir = dir.join(run_id.trim());

        if run_dir
            .components()
            .any(|c| c == std::path::Component::ParentDir)
        {
            return Err(AppError::BadRequest(
                "Path traversal detected: run directory attempts to escape extraction root."
                    .to_string(),
            ));
        }

        Ok(run_dir)
    }

    /// Resolves the run manifest path:
    /// `{derived_dir}/{case_id}/{evidence_id}/extraction-runs/{run_id}/run-manifest.json`
    pub fn get_run_manifest_path(
        &self,
        case_id: &str,
        evidence_id: &str,
        run_id: &str,
    ) -> Result<PathBuf, AppError> {
        let run_dir = self.get_run_dir(case_id, evidence_id, run_id)?;
        Ok(run_dir.join("run-manifest.json"))
    }

    /// Resolves the run result path:
    /// `{derived_dir}/{case_id}/{evidence_id}/extraction-runs/{run_id}/result.json`
    pub fn get_run_result_path(
        &self,
        case_id: &str,
        evidence_id: &str,
        run_id: &str,
    ) -> Result<PathBuf, AppError> {
        let run_dir = self.get_run_dir(case_id, evidence_id, run_id)?;
        Ok(run_dir.join("result.json"))
    }

    /// Resolves the active run pointer path:
    /// `{derived_dir}/{case_id}/{evidence_id}/active-run.json`
    pub fn get_active_run_pointer_path(
        &self,
        case_id: &str,
        evidence_id: &str,
    ) -> Result<PathBuf, AppError> {
        let dir = self.get_derived_evidence_dir(case_id, evidence_id)?;
        Ok(dir.join("active-run.json"))
    }

    /// Ensures the extraction run directory exists.
    pub async fn ensure_run_dir(
        &self,
        case_id: &str,
        evidence_id: &str,
        run_id: &str,
    ) -> Result<PathBuf, AppError> {
        let dir = self.get_run_dir(case_id, evidence_id, run_id)?;
        fs::create_dir_all(&dir).await.map_err(|e| {
            AppError::Internal(anyhow::anyhow!("Failed to create run directory: {}", e))
        })?;
        Ok(dir)
    }

    /// Persists a run manifest atomically as JSON:
    /// Writes to `{tmp_path}` then renames to `run-manifest.json`.
    pub async fn save_run_manifest(
        &self,
        manifest: &ExtractionRunManifest,
    ) -> Result<(), AppError> {
        self.ensure_run_dir(
            &manifest.case_id,
            &manifest.evidence_id,
            &manifest.extraction_run_id,
        )
        .await?;
        let manifest_path = self.get_run_manifest_path(
            &manifest.case_id,
            &manifest.evidence_id,
            &manifest.extraction_run_id,
        )?;

        let tmp_path = manifest_path.with_extension(format!("tmp.{}", uuid::Uuid::new_v4()));
        let json_bytes = serde_json::to_vec_pretty(manifest).map_err(|e| {
            AppError::Internal(anyhow::anyhow!("Failed to serialize run manifest: {}", e))
        })?;

        fs::write(&tmp_path, json_bytes).await.map_err(|e| {
            AppError::Internal(anyhow::anyhow!(
                "Failed to write run manifest temp file: {}",
                e
            ))
        })?;

        fs::rename(&tmp_path, &manifest_path).await.map_err(|e| {
            AppError::Internal(anyhow::anyhow!(
                "Failed to finalize run manifest file: {}",
                e
            ))
        })?;

        Ok(())
    }

    /// Reads and deserializes an extraction run manifest.
    pub async fn load_run_manifest(
        &self,
        case_id: &str,
        evidence_id: &str,
        run_id: &str,
    ) -> Result<ExtractionRunManifest, AppError> {
        let manifest_path = self.get_run_manifest_path(case_id, evidence_id, run_id)?;
        if !manifest_path.exists() {
            return Err(AppError::NotFound(format!(
                "Extraction run manifest not found for run_id '{}'",
                run_id
            )));
        }

        let content = fs::read(&manifest_path).await.map_err(|e| {
            AppError::Internal(anyhow::anyhow!("Failed to read run manifest file: {}", e))
        })?;

        let manifest: ExtractionRunManifest = serde_json::from_slice(&content).map_err(|e| {
            AppError::Internal(anyhow::anyhow!("Malformed run manifest on disk: {}", e))
        })?;

        Ok(manifest)
    }

    /// Persists extraction result JSON atomically and returns its SHA-256 digest.
    pub async fn save_run_result(
        &self,
        case_id: &str,
        evidence_id: &str,
        run_id: &str,
        result: &UnifiedExtractionResult,
    ) -> Result<String, AppError> {
        self.ensure_run_dir(case_id, evidence_id, run_id).await?;
        let result_path = self.get_run_result_path(case_id, evidence_id, run_id)?;

        let json_bytes = serde_json::to_vec_pretty(result).map_err(|e| {
            AppError::Internal(anyhow::anyhow!(
                "Failed to serialize extraction result: {}",
                e
            ))
        })?;

        let mut hasher = Sha256::new();
        hasher.update(&json_bytes);
        let result_sha256 = hex::encode(hasher.finalize());

        let tmp_path = result_path.with_extension(format!("tmp.{}", uuid::Uuid::new_v4()));
        fs::write(&tmp_path, json_bytes).await.map_err(|e| {
            AppError::Internal(anyhow::anyhow!("Failed to write result temp file: {}", e))
        })?;

        fs::rename(&tmp_path, &result_path).await.map_err(|e| {
            AppError::Internal(anyhow::anyhow!("Failed to finalize result file: {}", e))
        })?;

        Ok(result_sha256)
    }

    /// Reads and deserializes an extraction result JSON from disk.
    pub async fn load_run_result(
        &self,
        case_id: &str,
        evidence_id: &str,
        run_id: &str,
    ) -> Result<UnifiedExtractionResult, AppError> {
        let result_path = self.get_run_result_path(case_id, evidence_id, run_id)?;
        if !result_path.exists() {
            return Err(AppError::NotFound(format!(
                "Extraction result not found for run_id '{}'",
                run_id
            )));
        }

        let content = fs::read(&result_path).await.map_err(|e| {
            AppError::Internal(anyhow::anyhow!(
                "Failed to read extraction result file: {}",
                e
            ))
        })?;

        let result: UnifiedExtractionResult = serde_json::from_slice(&content).map_err(|e| {
            AppError::Internal(anyhow::anyhow!(
                "Malformed extraction result on disk: {}",
                e
            ))
        })?;

        Ok(result)
    }

    /// Persists the active run pointer atomically.
    pub async fn save_active_run_pointer(
        &self,
        pointer: &ActiveRunPointer,
    ) -> Result<(), AppError> {
        let dir = self.get_derived_evidence_dir(&pointer.case_id, &pointer.evidence_id)?;
        fs::create_dir_all(&dir).await.map_err(|e| {
            AppError::Internal(anyhow::anyhow!(
                "Failed to create derived evidence directory: {}",
                e
            ))
        })?;

        let pointer_path =
            self.get_active_run_pointer_path(&pointer.case_id, &pointer.evidence_id)?;
        let tmp_path = pointer_path.with_extension(format!("tmp.{}", uuid::Uuid::new_v4()));

        let json_bytes = serde_json::to_vec_pretty(pointer).map_err(|e| {
            AppError::Internal(anyhow::anyhow!(
                "Failed to serialize active run pointer: {}",
                e
            ))
        })?;

        fs::write(&tmp_path, json_bytes).await.map_err(|e| {
            AppError::Internal(anyhow::anyhow!(
                "Failed to write active run pointer temp file: {}",
                e
            ))
        })?;

        fs::rename(&tmp_path, &pointer_path).await.map_err(|e| {
            AppError::Internal(anyhow::anyhow!(
                "Failed to finalize active run pointer file: {}",
                e
            ))
        })?;

        Ok(())
    }

    /// Loads the active run pointer if present.
    pub async fn load_active_run_pointer(
        &self,
        case_id: &str,
        evidence_id: &str,
    ) -> Result<Option<ActiveRunPointer>, AppError> {
        let pointer_path = self.get_active_run_pointer_path(case_id, evidence_id)?;
        if !pointer_path.exists() {
            return Ok(None);
        }

        let content = fs::read(&pointer_path).await.map_err(|e| {
            AppError::Internal(anyhow::anyhow!(
                "Failed to read active run pointer file: {}",
                e
            ))
        })?;

        let pointer: ActiveRunPointer = serde_json::from_slice(&content).map_err(|e| {
            AppError::Internal(anyhow::anyhow!(
                "Malformed active run pointer on disk: {}",
                e
            ))
        })?;

        Ok(Some(pointer))
    }

    /// Lists all extraction runs for an evidence item sorted chronologically by started_at.
    pub async fn list_runs(
        &self,
        case_id: &str,
        evidence_id: &str,
    ) -> Result<Vec<ExtractionRunManifest>, AppError> {
        let runs_dir = self.get_extraction_runs_dir(case_id, evidence_id)?;
        if !runs_dir.exists() {
            return Ok(Vec::new());
        }

        let mut entries = match fs::read_dir(&runs_dir).await {
            Ok(ent) => ent,
            Err(_) => return Ok(Vec::new()),
        };

        let mut manifests = Vec::new();

        while let Ok(Some(entry)) = entries.next_entry().await {
            if let Ok(file_type) = entry.file_type().await {
                if file_type.is_dir() {
                    let manifest_file = entry.path().join("run-manifest.json");
                    if manifest_file.exists() {
                        if let Ok(content) = fs::read(&manifest_file).await {
                            if let Ok(m) = serde_json::from_slice::<ExtractionRunManifest>(&content)
                            {
                                manifests.push(m);
                            }
                        }
                    }
                }
            }
        }

        manifests.sort_by(|a, b| a.started_at.cmp(&b.started_at));
        Ok(manifests)
    }

    /// Startup recovery: Scans derived storage for any runs left in PENDING or RUNNING status
    /// and transitions them to INTERRUPTED.
    pub async fn recover_interrupted_runs(&self) -> Result<usize, AppError> {
        if !self.derived_dir.exists() {
            return Ok(0);
        }

        let mut count = 0;
        let mut case_entries = match fs::read_dir(&self.derived_dir).await {
            Ok(ent) => ent,
            Err(_) => return Ok(0),
        };

        let now_utc = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

        while let Ok(Some(case_entry)) = case_entries.next_entry().await {
            if let Ok(ft) = case_entry.file_type().await {
                if ft.is_dir() {
                    let mut ev_entries = match fs::read_dir(case_entry.path()).await {
                        Ok(ent) => ent,
                        Err(_) => continue,
                    };
                    while let Ok(Some(ev_entry)) = ev_entries.next_entry().await {
                        if let Ok(ev_ft) = ev_entry.file_type().await {
                            if ev_ft.is_dir() {
                                let runs_dir = ev_entry.path().join("extraction-runs");
                                if runs_dir.exists() {
                                    if let Ok(mut run_dirs) = fs::read_dir(&runs_dir).await {
                                        while let Ok(Some(run_dir_entry)) =
                                            run_dirs.next_entry().await
                                        {
                                            let manifest_path =
                                                run_dir_entry.path().join("run-manifest.json");
                                            if manifest_path.exists() {
                                                if let Ok(content) = fs::read(&manifest_path).await
                                                {
                                                    if let Ok(mut manifest) = serde_json::from_slice::<
                                                        ExtractionRunManifest,
                                                    >(
                                                        &content
                                                    ) {
                                                        if manifest.status
                                                            == ExtractionRunStatus::Pending
                                                            || manifest.status
                                                                == ExtractionRunStatus::Running
                                                        {
                                                            manifest.status =
                                                                ExtractionRunStatus::Interrupted;
                                                            manifest.completed_at =
                                                                Some(now_utc.clone());
                                                            manifest.error_message = Some("Extraction interrupted: server process terminated while running".to_string());
                                                            let _ = fs::write(
                                                                &manifest_path,
                                                                serde_json::to_vec_pretty(
                                                                    &manifest,
                                                                )
                                                                .unwrap_or_default(),
                                                            )
                                                            .await;
                                                            count += 1;
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if count > 0 {
            tracing::warn!(
                interrupted_runs = count,
                "Recovered interrupted extraction runs on startup"
            );
        }

        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_sanitize_filename_traversal() {
        assert_eq!(sanitize_filename("../../etc/passwd"), "passwd");
        assert_eq!(
            sanitize_filename("..\\..\\windows\\system32\\cmd.exe"),
            "cmd.exe"
        );
        assert_eq!(
            sanitize_filename("normal_evidence.csv"),
            "normal_evidence.csv"
        );
        assert_eq!(sanitize_filename("..."), "sanitized_evidence.bin");
    }

    #[test]
    fn test_sanitize_filename_reserved_windows() {
        assert_eq!(sanitize_filename("CON.csv"), "safe_CON.csv");
        assert_eq!(sanitize_filename("NUL.txt"), "safe_NUL.txt");
        assert_eq!(sanitize_filename("aux.json"), "safe_aux.json");
    }

    #[test]
    fn test_validate_safe_id_rejects_traversal() {
        assert!(validate_safe_id("../case1", "case_id").is_err());
        assert!(validate_safe_id("case1/sub", "case_id").is_err());
        assert!(validate_safe_id("550e8400-e29b-41d4-a716-446655440000", "case_id").is_ok());
    }

    #[tokio::test]
    async fn test_storage_manager_path_generation() {
        let temp = tempdir().expect("Failed to create tempdir");
        let mgr = StorageManager::new(temp.path().to_path_buf());

        let raw_path = mgr
            .get_raw_file_path("case-123", "ev-456")
            .expect("Path generation failed");
        assert!(raw_path.ends_with(Path::new("case-123").join("ev-456").join("ev-456.raw")));
    }
}
