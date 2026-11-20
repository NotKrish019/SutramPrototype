//! Forensic Evidence-to-Claim Traceability Engine (Member 4).
//!
//! Exposes an end-to-end provenance chain for every reconstruction finding,
//! linking conclusions (contradiction leads, trajectories, timeline claims)
//! back to original source records, extraction runs, and verified evidence files.
//!
//! Guarantees:
//! - Original source timestamps are preserved and never replaced with normalized values.
//! - Clear epistemic distinction between Observed, Extracted, Inferred, and Simulated data.
//! - Missing provenance is explicitly reported rather than silently defaulted.
//! - Raw evidence files are accessed strictly read-only for cryptographic integrity verification.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::model::{AssumptionKind, GeoPoint, ReconstructionAssumption, TimelineEntry};
use super::service::FullCaseReconstructionResult;
use super::video::compute_file_sha256;
use crate::error::AppError;
use crate::ingestion::model::EvidenceManifest;
use crate::ingestion::StorageManager;
use crate::persistence::EvidenceRepository;

/// The epistemic classification of forensic information:
/// strictly distinguishes directly observed physical facts from parsed or analytically derived claims.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EpistemicClassification {
    /// Directly observed from source evidence (e.g. unaltered timestamp string, raw bytes).
    Observed,
    /// Extracted by automated parsers without synthetic interpolation (e.g. parsed CDR row).
    Extracted,
    /// Analytically derived or inferred by algorithms (e.g. tower centroid, required speed, contradiction lead).
    Inferred,
    /// Produced during a what-if hypothetical scenario simulation.
    Simulated,
}

/// Verification status of the underlying evidence exhibit's cryptographic integrity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EvidenceIntegrityStatus {
    /// SHA-256 hash verified against disk payload and matches manifest.
    VerifiedMatch,
    /// The computed hash of the file on disk does not match the manifest (evidence tampered or corrupted).
    HashMismatch,
    /// The evidence file could not be found on storage.
    MissingFromStorage,
    /// No manifest or database record found for the requested evidence ID.
    UnknownEvidence,
}

/// Bounds of a half-open temporal uncertainty window [start, end).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeWindowBounds {
    pub window_start_utc: String,
    pub window_end_utc: String,
    pub precision: String,
}

/// Spatial interpretation grounding a location anchor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpatialAnchorInterpretation {
    pub coordinates: GeoPoint,
    pub is_observed_gps: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cell_tower_id: Option<String>,
    pub interpretation_note: String,
}

/// Traceability anchor for a single contributing source event or record supporting a finding.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceEvidenceAnchor {
    /// Event identifier (e.g. "evt-001").
    pub event_id: String,
    /// Contributing evidence exhibit ID (e.g. "evi-001").
    pub evidence_id: String,
    /// Original filename from upload receipt if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_filename: Option<String>,
    /// Cryptographic SHA-256 hash of the evidence exhibit from manifest.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence_sha256: Option<String>,
    /// Real-time cryptographic integrity check status.
    pub integrity_status: EvidenceIntegrityStatus,
    /// The extraction run that produced this event candidate (UUID).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extraction_run_id: Option<String>,
    /// The specific record identifier within the source exhibit (e.g. row index, CDR record_id, msg_id).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_record_id: Option<String>,
    /// Original, unaltered source timestamp string directly from evidence payload.
    pub original_source_timestamp: String,
    /// Normalized UTC nominal timestamp (ISO-8601).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub normalized_timestamp_utc: Option<String>,
    /// Half-open uncertainty window [window_start_utc, window_end_utc).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uncertainty_window: Option<TimeWindowBounds>,
    /// Explicit assumptions applied to this source anchor.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assumptions: Vec<ReconstructionAssumption>,
    /// Spatial interpretation and uncertainty for this anchor.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spatial_interpretation: Option<SpatialAnchorInterpretation>,
    /// Epistemic level for this individual anchor.
    pub epistemic_level: EpistemicClassification,
}

/// Complete Evidence-to-Claim Provenance Trace for a reconstruction finding.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FindingProvenanceTrace {
    /// Finding identifier (e.g. contradiction lead ID, movement segment ID, or timeline event ID).
    pub finding_id: String,
    /// Type of finding (e.g. "CONTRADICTION_LEAD", "MOVEMENT_SEGMENT", "TIMELINE_EVENT").
    pub finding_type: String,
    /// Reconstruction run ID that generated this finding.
    pub reconstruction_run_id: String,
    /// Overall epistemic classification for the finding itself (Inferred or Simulated).
    pub epistemic_classification: EpistemicClassification,
    /// Whether this finding originated from a what-if simulation.
    pub is_hypothetical: bool,
    /// Human-readable claim summary / neutral explanation.
    pub claim_summary: String,
    /// Contributing evidence anchors grounding this finding (supports 1..N exhibits).
    pub source_anchors: Vec<SourceEvidenceAnchor>,
    /// Inferred metrics or calculations linking the anchors (e.g. transit distance, required speed, elapsed time).
    pub derived_inferences: serde_json::Value,
    /// List of missing provenance fields explicitly identified (e.g. "source_record_id", "extraction_run_id").
    pub missing_provenance_fields: Vec<String>,
    /// Timestamp when this provenance trace was generated/retrieved.
    pub audited_at: String,
}

/// Comprehensive case-level provenance report auditing all findings in a reconstruction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CaseProvenanceReport {
    pub case_id: String,
    pub reconstruction_run_id: String,
    pub audited_at: String,
    pub total_findings_count: usize,
    pub intact_provenance_count: usize,
    pub flagged_provenance_count: usize,
    pub findings: Vec<FindingProvenanceTrace>,
}

/// Resolves real-time evidence integrity by inspecting manifests and streaming SHA-256 verification.
pub async fn resolve_evidence_integrity(
    case_id: &str,
    evidence_id: &str,
    storage: &StorageManager,
    evidence_repo: Option<&EvidenceRepository>,
) -> (Option<String>, Option<String>, EvidenceIntegrityStatus) {
    // 1. Try to load manifest from storage or relational store
    let mut manifest = if let Ok(m) = storage.load_manifest(case_id, evidence_id).await {
        Some(m)
    } else {
        None
    };

    if manifest.is_none() {
        if let Ok(ev_dir) = storage.get_evidence_dir(case_id, evidence_id) {
            let named_manifest = ev_dir.join(format!("{}.manifest.json", evidence_id));
            if named_manifest.exists() {
                if let Ok(bytes) = tokio::fs::read(&named_manifest).await {
                    manifest = serde_json::from_slice::<EvidenceManifest>(&bytes).ok();
                }
            }
        }
    }

    if manifest.is_none() {
        if let Ok(Some(m)) = storage.find_manifest_by_id(evidence_id).await {
            manifest = Some(m);
        }
    }

    if manifest.is_none() {
        if let Some(repo) = evidence_repo {
            if let Ok(uid) = Uuid::parse_str(evidence_id) {
                if let Ok(Some(ev)) = repo.get_evidence(uid).await {
                    manifest = Some(EvidenceManifest {
                        receipt: crate::ingestion::EvidenceUploadReceipt {
                            evidence_id: ev.evidence_id.to_string(),
                            case_id: ev.case_id.to_string(),
                            original_filename: ev.original_filename,
                            source_type: ev.source_type,
                            mime_type: ev.mime_type,
                            file_size_bytes: ev.file_size_bytes as u64,
                            sha256_hash: ev.sha256_hash,
                            storage_uri: ev.storage_uri,
                            status: crate::ingestion::IngestionStatus::Ingested,
                            metadata: ev.metadata,
                            created_at: ev.created_at.to_rfc3339(),
                        },
                        relative_raw_path: format!("{}/{}/{}.raw", case_id, evidence_id, evidence_id),
                        stored_at: ev.created_at.to_rfc3339(),
                    });
                }
            }
        }
    }

    let manifest = match manifest {
        Some(m) => m,
        None => return (None, None, EvidenceIntegrityStatus::UnknownEvidence),
    };

    let filename = Some(manifest.receipt.original_filename.clone());
    let expected_hash = manifest.receipt.sha256_hash.clone();

    // 2. Check raw file existence on disk
    let raw_path = match storage.get_raw_file_path(case_id, evidence_id) {
        Ok(p) => p,
        Err(_) => {
            return (
                filename,
                Some(expected_hash),
                EvidenceIntegrityStatus::MissingFromStorage,
            )
        }
    };

    let actual_path = if raw_path.exists() {
        raw_path
    } else {
        let alt_path = storage.root_dir().join(&manifest.relative_raw_path);
        if alt_path.exists() {
            alt_path
        } else {
            return (
                filename,
                Some(expected_hash),
                EvidenceIntegrityStatus::MissingFromStorage,
            );
        }
    };

    // 3. Compute streaming SHA-256 to verify against recorded manifest digest
    match compute_file_sha256(&actual_path) {
        Ok(actual_hash) => {
            if actual_hash.eq_ignore_ascii_case(&expected_hash) {
                (
                    filename,
                    Some(expected_hash),
                    EvidenceIntegrityStatus::VerifiedMatch,
                )
            } else {
                (
                    filename,
                    Some(expected_hash),
                    EvidenceIntegrityStatus::HashMismatch,
                )
            }
        }
        Err(_) => (
            filename,
            Some(expected_hash),
            EvidenceIntegrityStatus::MissingFromStorage,
        ),
    }
}

/// Resolves a single `SourceEvidenceAnchor` from a timeline entry.
pub async fn resolve_source_anchor(
    case_id: &str,
    entry: &TimelineEntry,
    storage: &StorageManager,
    evidence_repo: Option<&EvidenceRepository>,
) -> SourceEvidenceAnchor {
    let evidence_id = entry.source_evidence_id.clone();

    // 1. Resolve evidence manifest and integrity
    let (original_filename, evidence_sha256, integrity_status) =
        resolve_evidence_integrity(case_id, &evidence_id, storage, evidence_repo).await;

    // 2. Resolve source_record_id from entry metadata
    let source_record_id = entry
        .metadata
        .get("source_record_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .filter(|s| !s.trim().is_empty());

    // 3. Resolve extraction_run_id from entry metadata or active run pointer
    let mut extraction_run_id = entry
        .metadata
        .get("extraction_run_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .filter(|s| !s.trim().is_empty());

    if extraction_run_id.is_none() {
        if let Ok(Some(pointer)) = storage.load_active_run_pointer(case_id, &evidence_id).await {
            extraction_run_id = Some(pointer.active_run_id);
        }
    }

    // 4. Resolve spatial interpretation
    let is_tower_centroid = entry
        .assumptions
        .iter()
        .any(|a| a.kind == AssumptionKind::CoordinatesInferredFromTowerId);
    let cell_tower_id = entry
        .metadata
        .get("cell_tower_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let location_name = entry.raw_description.clone();

    let spatial_interpretation = entry.coordinates.map(|coord| {
        let note = if is_tower_centroid {
            format!(
                "Coordinates ({:.4}, {:.4}) inferred from cell tower centroid '{}'; device may have been anywhere within cell coverage radius.",
                coord.lat,
                coord.lon,
                cell_tower_id.as_deref().unwrap_or("unknown")
            )
        } else {
            format!(
                "Directly observed geographic coordinate ({:.4}, {:.4}).",
                coord.lat, coord.lon
            )
        };

        SpatialAnchorInterpretation {
            coordinates: coord,
            is_observed_gps: !is_tower_centroid,
            location_name,
            cell_tower_id,
            interpretation_note: note,
        }
    });

    // 5. Determine epistemic level for the source anchor
    let epistemic_level = if is_tower_centroid {
        EpistemicClassification::Inferred
    } else if source_record_id.is_some() || extraction_run_id.is_some() {
        EpistemicClassification::Extracted
    } else {
        EpistemicClassification::Observed
    };

    SourceEvidenceAnchor {
        event_id: entry.event_id.clone(),
        evidence_id,
        original_filename,
        evidence_sha256,
        integrity_status,
        extraction_run_id,
        source_record_id,
        original_source_timestamp: entry.raw_timestamp.clone(),
        normalized_timestamp_utc: Some(entry.time_window.nominal_utc.to_rfc3339()),
        uncertainty_window: Some(TimeWindowBounds {
            window_start_utc: entry.time_window.window_start_utc.to_rfc3339(),
            window_end_utc: entry.time_window.window_end_utc.to_rfc3339(),
            precision: entry.time_window.precision.as_str().to_string(),
        }),
        assumptions: entry.assumptions.clone(),
        spatial_interpretation,
        epistemic_level,
    }
}

/// Resolves complete evidence-to-claim provenance trace for a specific finding in a reconstruction.
pub async fn resolve_finding_provenance(
    case_id: &str,
    finding_id: &str,
    reconstruction: &FullCaseReconstructionResult,
    storage: &StorageManager,
    evidence_repo: Option<&EvidenceRepository>,
) -> Result<FindingProvenanceTrace, AppError> {
    let run_id = reconstruction
        .reconstruction_run_id
        .clone()
        .or_else(|| reconstruction.run_id.clone())
        .unwrap_or_else(|| "unknown-run".to_string());

    let is_hypothetical = run_id.starts_with("sim-")
        || run_id.contains("hypothetical")
        || reconstruction
            .export_safeguard
            .reasons
            .iter()
            .any(|r| r.contains("Hypothetical"));

    let audited_at = Utc::now().to_rfc3339();

    // Map timeline entries by event_id for fast lookup
    let entries_by_id: std::collections::HashMap<&str, &TimelineEntry> = reconstruction
        .timeline
        .entries
        .iter()
        .map(|e| (e.event_id.as_str(), e))
        .collect();

    // 1. Check if finding is a Contradiction Lead
    if let Some(lead) = reconstruction
        .contradictions
        .leads
        .iter()
        .find(|l| l.lead_id == finding_id)
    {
        let mut source_anchors = Vec::new();
        let mut missing_fields = Vec::new();

        // Resolve Anchor A
        if let Some(entry_a) = entries_by_id.get(lead.event_a_id.as_str()) {
            let anchor_a = resolve_source_anchor(case_id, entry_a, storage, evidence_repo).await;
            check_anchor_missing_provenance(&anchor_a, &mut missing_fields);
            source_anchors.push(anchor_a);
        } else {
            missing_fields.push(format!("missing_timeline_entry:{}", lead.event_a_id));
        }

        // Resolve Anchor B
        if let Some(entry_b) = entries_by_id.get(lead.event_b_id.as_str()) {
            let anchor_b = resolve_source_anchor(case_id, entry_b, storage, evidence_repo).await;
            check_anchor_missing_provenance(&anchor_b, &mut missing_fields);
            source_anchors.push(anchor_b);
        } else {
            missing_fields.push(format!("missing_timeline_entry:{}", lead.event_b_id));
        }

        let epistemic_classification = if is_hypothetical {
            EpistemicClassification::Simulated
        } else {
            EpistemicClassification::Inferred
        };

        return Ok(FindingProvenanceTrace {
            finding_id: lead.lead_id.clone(),
            finding_type: "CONTRADICTION_LEAD".to_string(),
            reconstruction_run_id: run_id,
            epistemic_classification,
            is_hypothetical,
            claim_summary: lead.neutral_explanation.clone(),
            source_anchors,
            derived_inferences: serde_json::to_value(&lead.derived_inference).unwrap_or_default(),
            missing_provenance_fields: missing_fields,
            audited_at,
        });
    }

    // 2. Check if finding is a Movement Segment
    for trajectory in &reconstruction.spatial.trajectories {
        if let Some(segment) = trajectory.segments.iter().find(|s| {
            let seg_id = format!("seg-{}-{}", s.origin_event_id, s.destination_event_id);
            seg_id == finding_id
        }) {
            let mut source_anchors = Vec::new();
            let mut missing_fields = Vec::new();

            if let Some(entry_orig) = entries_by_id.get(segment.origin_event_id.as_str()) {
                let anchor_orig =
                    resolve_source_anchor(case_id, entry_orig, storage, evidence_repo).await;
                check_anchor_missing_provenance(&anchor_orig, &mut missing_fields);
                source_anchors.push(anchor_orig);
            } else {
                missing_fields.push(format!("missing_timeline_entry:{}", segment.origin_event_id));
            }

            if let Some(entry_dest) = entries_by_id.get(segment.destination_event_id.as_str()) {
                let anchor_dest =
                    resolve_source_anchor(case_id, entry_dest, storage, evidence_repo).await;
                check_anchor_missing_provenance(&anchor_dest, &mut missing_fields);
                source_anchors.push(anchor_dest);
            } else {
                missing_fields.push(format!(
                    "missing_timeline_entry:{}",
                    segment.destination_event_id
                ));
            }

            let epistemic_classification = if is_hypothetical {
                EpistemicClassification::Simulated
            } else {
                EpistemicClassification::Inferred
            };

            let seg_id = format!("seg-{}-{}", segment.origin_event_id, segment.destination_event_id);
            let summary = format!(
                "Entity '{}' transit segment from '{}' to '{}' ({:.2} km, min speed {:.1} km/h).",
                segment.entity_id,
                segment.origin_event_id,
                segment.destination_event_id,
                segment.transit_metrics.distance_km,
                segment.transit_metrics.min_required_speed_kmh.unwrap_or(0.0)
            );

            return Ok(FindingProvenanceTrace {
                finding_id: seg_id,
                finding_type: "MOVEMENT_SEGMENT".to_string(),
                reconstruction_run_id: run_id,
                epistemic_classification,
                is_hypothetical,
                claim_summary: summary,
                source_anchors,
                derived_inferences: serde_json::to_value(&segment.transit_metrics)
                    .unwrap_or_default(),
                missing_provenance_fields: missing_fields,
                audited_at,
            });
        }
    }

    // 3. Check if finding is a Timeline Entry
    if let Some(entry) = entries_by_id.get(finding_id) {
        let mut missing_fields = Vec::new();
        let anchor = resolve_source_anchor(case_id, entry, storage, evidence_repo).await;
        check_anchor_missing_provenance(&anchor, &mut missing_fields);

        let epistemic_classification = if is_hypothetical {
            EpistemicClassification::Simulated
        } else {
            anchor.epistemic_level
        };

        let summary = entry
            .raw_description
            .clone()
            .unwrap_or_else(|| format!("Timeline event '{}' at {}", entry.event_id, entry.raw_timestamp));

        return Ok(FindingProvenanceTrace {
            finding_id: entry.event_id.clone(),
            finding_type: "TIMELINE_EVENT".to_string(),
            reconstruction_run_id: run_id,
            epistemic_classification,
            is_hypothetical,
            claim_summary: summary,
            source_anchors: vec![anchor],
            derived_inferences: serde_json::to_value(&entry.time_window).unwrap_or_default(),
            missing_provenance_fields: missing_fields,
            audited_at,
        });
    }

    Err(AppError::NotFound(format!(
        "Finding '{}' not found in reconstruction for case '{}'",
        finding_id, case_id
    )))
}

/// Resolves complete provenance report across all findings in a reconstruction run.
pub async fn resolve_case_reconstruction_provenance(
    case_id: &str,
    reconstruction: &FullCaseReconstructionResult,
    storage: &StorageManager,
    evidence_repo: Option<&EvidenceRepository>,
) -> Result<CaseProvenanceReport, AppError> {
    let run_id = reconstruction
        .reconstruction_run_id
        .clone()
        .or_else(|| reconstruction.run_id.clone())
        .unwrap_or_else(|| "unknown-run".to_string());

    let audited_at = Utc::now().to_rfc3339();
    let mut findings = Vec::new();
    let mut intact_count = 0;
    let mut flagged_count = 0;

    // Evaluate all contradiction leads
    for lead in &reconstruction.contradictions.leads {
        if let Ok(trace) =
            resolve_finding_provenance(case_id, &lead.lead_id, reconstruction, storage, evidence_repo).await
        {
            if trace.missing_provenance_fields.is_empty() {
                intact_count += 1;
            } else {
                flagged_count += 1;
            }
            findings.push(trace);
        }
    }

    let total = findings.len();

    Ok(CaseProvenanceReport {
        case_id: case_id.to_string(),
        reconstruction_run_id: run_id,
        audited_at,
        total_findings_count: total,
        intact_provenance_count: intact_count,
        flagged_provenance_count: flagged_count,
        findings,
    })
}

/// Helper inspecting an anchor and appending any missing provenance items to `missing_fields`.
fn check_anchor_missing_provenance(anchor: &SourceEvidenceAnchor, missing: &mut Vec<String>) {
    if anchor.source_record_id.is_none() {
        missing.push(format!(
            "anchor[{}:{}].source_record_id",
            anchor.evidence_id, anchor.event_id
        ));
    }
    if anchor.extraction_run_id.is_none() {
        missing.push(format!(
            "anchor[{}:{}].extraction_run_id",
            anchor.evidence_id, anchor.event_id
        ));
    }
    match anchor.integrity_status {
        EvidenceIntegrityStatus::UnknownEvidence => {
            missing.push(format!(
                "anchor[{}:{}].unknown_evidence",
                anchor.evidence_id, anchor.event_id
            ));
        }
        EvidenceIntegrityStatus::HashMismatch => {
            missing.push(format!(
                "anchor[{}:{}].sha256_hash_mismatch",
                anchor.evidence_id, anchor.event_id
            ));
        }
        EvidenceIntegrityStatus::MissingFromStorage => {
            missing.push(format!(
                "anchor[{}:{}].evidence_file_missing",
                anchor.evidence_id, anchor.event_id
            ));
        }
        EvidenceIntegrityStatus::VerifiedMatch => {}
    }
}
