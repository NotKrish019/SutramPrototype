//! Evidence-Gap Intelligence (Member 4).
//!
//! Deterministically identifies missing evidence, weak corroboration, uncalibrated device clocks,
//! and unresolved links that limit or introduce uncertainty into a case reconstruction.
//!
//! Adheres strictly to SUTRAM 2026 forensic integrity principles:
//! - 100% explainable, deterministic rule-based evaluation (no opaque LLMs).
//! - Identifies the exact affected entity, finding, evidence item, or reconstruction area.
//! - Explains why each gap matters and what limitation it creates.
//! - Suggests actionable, appropriate corroborating evidence.
//! - Distinguishes confirmed missing information from unsupplied/unprocessed information.
//! - Never fabricates facts or asserts guilt.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

use crate::ingestion::{EvidenceManifest, IngestionStatus};
use crate::reconstruction::model::AssumptionKind;
use crate::reconstruction::service::FullCaseReconstructionResult;
use crate::reconstruction::video_handoff::VideoHandoffStatus;

/// Classification category of an evidence gap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EvidenceGapType {
    /// Ingested evidence exhibit was excluded due to validation, integrity or format errors.
    EvidenceExcluded,
    /// Ingested evidence exhibit has zero accepted event candidates in the reconstruction.
    EvidenceWithoutEvents,
    /// Event candidate was excluded from reconstruction due to invalid timestamps or unparseable temporal format.
    CandidateExcluded,
    /// Reconstructed timeline event lacks geographic coordinates, preventing spatial placement and trajectory tracking.
    EventWithoutCoordinates,
    /// Event references an entity/subject identifier that has no canonical registration or identity link.
    UnresolvedEntityReference,
    /// Trajectory movement segment spans across intervening unlocated events or significant spatial jump without intermediary waypoints.
    SpatialContinuityGap,
    /// Video exhibit was ingested but no keyframes were extracted or video handoff failed.
    VideoWithoutFrames,
    /// Event or exhibit relies on an uncalibrated clock offset or zero-offset assumption without hardware/NTP calibration.
    UncalibratedClock,
    /// Event lacks a specific record identifier within the source exhibit (row index, frame number, message ID), breaking source traceability.
    UnresolvedSourceRecord,
    /// Event lacks an extraction run reference or active run pointer, breaking analytical provenance.
    MissingExtractionProvenance,
    /// Critical finding (contradiction lead or single-sighting claim) relies on a single uncorroborated source with no independent secondary verification.
    MissingCorroboration,
}

impl EvidenceGapType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::EvidenceExcluded => "EVIDENCE_EXCLUDED",
            Self::EvidenceWithoutEvents => "EVIDENCE_WITHOUT_EVENTS",
            Self::CandidateExcluded => "CANDIDATE_EXCLUDED",
            Self::EventWithoutCoordinates => "EVENT_WITHOUT_COORDINATES",
            Self::UnresolvedEntityReference => "UNRESOLVED_ENTITY_REFERENCE",
            Self::SpatialContinuityGap => "SPATIAL_CONTINUITY_GAP",
            Self::VideoWithoutFrames => "VIDEO_WITHOUT_FRAMES",
            Self::UncalibratedClock => "UNCALIBRATED_CLOCK",
            Self::UnresolvedSourceRecord => "UNRESOLVED_SOURCE_RECORD",
            Self::MissingExtractionProvenance => "MISSING_EXTRACTION_PROVENANCE",
            Self::MissingCorroboration => "MISSING_CORROBORATION",
        }
    }
}

/// Severity classification for evidence gaps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GapSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl GapSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Low => "LOW",
            Self::Medium => "MEDIUM",
            Self::High => "HIGH",
            Self::Critical => "CRITICAL",
        }
    }
}

/// An explainable, evidence-grounded limitation or gap in the case reconstruction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceGap {
    /// Stable, deterministic identifier for this gap (e.g. `gap-coord-evt-01`, `gap-nocorr-lead-01`).
    pub gap_id: String,
    /// Category classification of the gap.
    pub gap_type: EvidenceGapType,
    /// Severity level of this gap regarding reconstruction reliability.
    pub severity: GapSeverity,
    /// Type of subject affected (e.g. "EVENT", "EVIDENCE", "MOVEMENT_SEGMENT", "CONTRADICTION_LEAD", "ENTITY").
    pub subject_type: String,
    /// Identifier of the affected entity, event, finding, or evidence item.
    pub subject_id: String,
    /// Clear human-readable summary title.
    pub title: String,
    /// Detailed explanation of why the gap exists and why it matters.
    pub explanation: String,
    /// Backwards-compatible alias for explanation matching Python reference schema.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Backwards-compatible reason code matching Python reference schema.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason_code: Option<String>,
    /// Explains what uncertainty or analytical limitation it creates in the reconstruction.
    pub impact_on_reconstruction: String,
    /// Suggests what type of additional corroborating evidence could address it.
    pub suggested_corroborating_evidence: String,
    /// Deterministic rule identifier that generated the gap.
    pub rule_id: String,
    /// Distinguishes confirmed missing information (e.g. excluded/corrupted file, unlocated waypoint)
    /// from information that simply has not been uploaded or supplied yet.
    pub is_confirmed_missing: bool,
}

/// Comprehensive case-level report auditing all evidence gaps across a reconstruction run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceGapAnalysisReport {
    pub case_id: String,
    pub reconstruction_run_id: String,
    pub audited_at: String,
    pub total_gaps_count: usize,
    pub gaps_by_severity: BTreeMap<String, usize>,
    pub gaps_by_type: BTreeMap<String, usize>,
    pub gaps: Vec<EvidenceGap>,
}

/// Deterministically analyzes a case reconstruction to produce an explainable `EvidenceGapAnalysisReport`.
pub fn analyze_evidence_gaps(
    case_id: &str,
    reconstruction: &FullCaseReconstructionResult,
    exhibits: &[EvidenceManifest],
    known_entities: Option<&[String]>,
) -> EvidenceGapAnalysisReport {
    let run_id = reconstruction
        .reconstruction_run_id
        .clone()
        .or_else(|| reconstruction.run_id.clone())
        .unwrap_or_else(|| "unknown-run".to_string());

    let mut gaps = Vec::new();
    let mut seen_keys = HashSet::new();

    let known_entity_set: Option<HashSet<&str>> =
        known_entities.map(|e| e.iter().map(|s| s.as_str()).collect());

    // 1. RULE-001: Evidence Excluded from Reconstruction
    for manifest in exhibits {
        let is_failed_status = manifest.receipt.status == IngestionStatus::Corrupted
            || manifest.receipt.status == IngestionStatus::ExtractionFailed;
        let is_video_failed = reconstruction.video_handoff.iter().any(|vh| {
            vh.evidence_id == manifest.receipt.evidence_id
                && (vh.status == VideoHandoffStatus::RejectedIntegrityMismatch
                    || vh.status == VideoHandoffStatus::ExtractionFailed)
        });

        if is_failed_status || is_video_failed {
            let gap_id = format!("gap-excl-{}", manifest.receipt.evidence_id);
            if seen_keys.insert(gap_id.clone()) {
                let explanation = format!(
                    "Exhibit '{}' ({}) was excluded from reconstruction due to cryptographic hash mismatch, integrity failure, or unverified format.",
                    manifest.receipt.evidence_id, manifest.receipt.original_filename
                );
                gaps.push(EvidenceGap {
                    gap_id,
                    gap_type: EvidenceGapType::EvidenceExcluded,
                    severity: GapSeverity::High,
                    subject_type: "EVIDENCE".to_string(),
                    subject_id: manifest.receipt.evidence_id.clone(),
                    title: format!("Evidence Exhibit '{}' Excluded", manifest.receipt.evidence_id),
                    explanation: explanation.clone(),
                    description: Some(explanation),
                    reason_code: Some("GAP-RULE-001-EVIDENCE-EXCLUDED".to_string()),
                    impact_on_reconstruction: "The evidence payload cannot be ingested or correlated, leaving potential corroborating facts absent from the timeline.".to_string(),
                    suggested_corroborating_evidence: "Re-acquire authentic exhibit file from original recording device and verify SHA-256 custody checksum.".to_string(),
                    rule_id: "GAP-RULE-001-EVIDENCE-EXCLUDED".to_string(),
                    is_confirmed_missing: true,
                });
            }
        }
    }

    // 2. RULE-002: Ingested Evidence Without Accepted Timeline Events
    let events_source_evidence_ids: HashSet<&str> = reconstruction
        .timeline
        .entries
        .iter()
        .map(|e| e.source_evidence_id.as_str())
        .collect();

    for manifest in exhibits {
        let eid = manifest.receipt.evidence_id.as_str();
        let is_excluded = reconstruction.video_handoff.iter().any(|vh| {
            vh.evidence_id == eid
                && (vh.status == VideoHandoffStatus::RejectedIntegrityMismatch
                    || vh.status == VideoHandoffStatus::ExtractionFailed)
        }) || manifest.receipt.status == IngestionStatus::Corrupted
            || manifest.receipt.status == IngestionStatus::ExtractionFailed;

        if !is_excluded && !events_source_evidence_ids.contains(eid) {
            let gap_id = format!("gap-noevt-{}", eid);
            if seen_keys.insert(gap_id.clone()) {
                let explanation = format!(
                    "Evidence exhibit '{}' ({}) is present in case storage, but zero accepted timeline events reference it.",
                    eid, manifest.receipt.original_filename
                );
                gaps.push(EvidenceGap {
                    gap_id,
                    gap_type: EvidenceGapType::EvidenceWithoutEvents,
                    severity: GapSeverity::Medium,
                    subject_type: "EVIDENCE".to_string(),
                    subject_id: eid.to_string(),
                    title: format!("Exhibit '{}' Ingested But Has Zero Timeline Events", eid),
                    explanation: explanation.clone(),
                    description: Some(explanation),
                    reason_code: Some("GAP-RULE-002-EVIDENCE-WITHOUT-EVENTS".to_string()),
                    impact_on_reconstruction: "Information contained in this exhibit remains unrepresented in temporal and spatial analyses.".to_string(),
                    suggested_corroborating_evidence: "Execute extraction pipeline on this exhibit or verify if exhibit contains parseable timestamped records.".to_string(),
                    rule_id: "GAP-RULE-002-EVIDENCE-WITHOUT-EVENTS".to_string(),
                    is_confirmed_missing: false,
                });
            }
        }
    }

    // 3. RULE-003: Excluded Event Candidates
    for excl in &reconstruction.timeline.excluded_events {
        let gap_id = format!("gap-candexcl-{}", excl.event_id);
        if seen_keys.insert(gap_id.clone()) {
            let explanation = format!(
                "Event candidate '{}' from exhibit '{}' was excluded: {}.",
                excl.event_id, excl.source_evidence_id, excl.reason
            );
            gaps.push(EvidenceGap {
                gap_id,
                gap_type: EvidenceGapType::CandidateExcluded,
                severity: GapSeverity::Medium,
                subject_type: "EVENT_CANDIDATE".to_string(),
                subject_id: excl.event_id.clone(),
                title: format!("Event Candidate '{}' Excluded from Timeline", excl.event_id),
                explanation: explanation.clone(),
                description: Some(explanation),
                reason_code: Some("GAP-RULE-003-CANDIDATE-EXCLUDED".to_string()),
                impact_on_reconstruction: "Temporal event cannot be positioned on the chronological timeline, potentially omitting critical transition times.".to_string(),
                suggested_corroborating_evidence: "Provide explicit ISO 8601 timestamps, calibrated source timezones, or re-extract timestamps with second-level precision.".to_string(),
                rule_id: "GAP-RULE-003-CANDIDATE-EXCLUDED".to_string(),
                is_confirmed_missing: true,
            });
        }
    }

    // 4. RULE-004: Events Without Usable Coordinates
    for entry in &reconstruction.timeline.entries {
        if entry.coordinates.is_none() {
            let gap_id = format!("gap-nocoord-{}", entry.event_id);
            if seen_keys.insert(gap_id.clone()) {
                let location_hint = entry
                    .raw_description
                    .as_deref()
                    .unwrap_or("unnamed location");
                let explanation = format!(
                    "Timeline event '{}' (exhibit '{}') has valid temporal ordering but no geographical coordinates.",
                    entry.event_id, entry.source_evidence_id
                );
                gaps.push(EvidenceGap {
                    gap_id,
                    gap_type: EvidenceGapType::EventWithoutCoordinates,
                    severity: GapSeverity::Medium,
                    subject_type: "EVENT".to_string(),
                    subject_id: entry.event_id.clone(),
                    title: format!("Event '{}' Lacks Geographic Coordinates", entry.event_id),
                    explanation: explanation.clone(),
                    description: Some(explanation),
                    reason_code: Some("GAP-RULE-004-EVENT-WITHOUT-COORDINATES".to_string()),
                    impact_on_reconstruction: "Event cannot be placed on spatial maps or included in movement trajectory calculations, creating unmonitored geographic intervals.".to_string(),
                    suggested_corroborating_evidence: format!(
                        "Acquire co-located cell tower coverage logs, GPS telemetry, transit tap data, or physical address geocoding for '{}'.",
                        location_hint
                    ),
                    rule_id: "GAP-RULE-004-EVENT-WITHOUT-COORDINATES".to_string(),
                    is_confirmed_missing: true,
                });
            }
        }
    }

    // 5. RULE-005: Unresolved Entity References
    for entry in &reconstruction.timeline.entries {
        let is_unresolved = if entry.entity_ids.is_empty()
            || entry.entity_ids.iter().any(|id| {
                let lower = id.trim().to_lowercase();
                lower == "unknown" || lower == "unidentified" || lower.is_empty()
            }) {
            true
        } else if let Some(ref known) = known_entity_set {
            entry
                .entity_ids
                .iter()
                .any(|id| !known.contains(id.as_str()))
        } else {
            false
        };

        if is_unresolved {
            let gap_id = format!("gap-noentity-{}", entry.event_id);
            if seen_keys.insert(gap_id.clone()) {
                let explanation = format!(
                    "Timeline event '{}' (exhibit '{}') has unassigned or unresolved entity identifiers (found: {:?}).",
                    entry.event_id, entry.source_evidence_id, entry.entity_ids
                );
                gaps.push(EvidenceGap {
                    gap_id,
                    gap_type: EvidenceGapType::UnresolvedEntityReference,
                    severity: GapSeverity::Low,
                    subject_type: "EVENT".to_string(),
                    subject_id: entry.event_id.clone(),
                    title: format!("Event '{}' Has Unresolved Entity Reference", entry.event_id),
                    explanation: explanation.clone(),
                    description: Some(explanation),
                    reason_code: Some("GAP-RULE-005-UNRESOLVED-ENTITY".to_string()),
                    impact_on_reconstruction: "Activity cannot be attributed to a specific person of interest, preventing trajectory reconstruction for that individual.".to_string(),
                    suggested_corroborating_evidence: "Cross-reference subscriber identity records (SIM/IMSI), biometric facial match reports, or witness identification to map actor.".to_string(),
                    rule_id: "GAP-RULE-005-UNRESOLVED-ENTITY".to_string(),
                    is_confirmed_missing: true,
                });
            }
        }
    }

    // 6. RULE-006: Spatial Continuity Gaps
    for trajectory in &reconstruction.spatial.trajectories {
        for segment in &trajectory.segments {
            let has_unlocated = segment.has_intervening_unlocated_events;
            let is_massive_jump = segment.transit_metrics.distance_km > 50.0
                && segment.transit_metrics.nominal_elapsed_seconds > 3600.0;

            if has_unlocated || is_massive_jump {
                let seg_id = format!(
                    "seg-{}-{}",
                    segment.origin_event_id, segment.destination_event_id
                );
                let gap_id = format!("gap-spatialcont-{}", seg_id);
                if seen_keys.insert(gap_id.clone()) {
                    let explanation = format!(
                        "Entity '{}' transit between '{}' and '{}' spans {:.1} km with {} unlocated intervening events.",
                        segment.entity_id,
                        segment.origin_event_id,
                        segment.destination_event_id,
                        segment.transit_metrics.distance_km,
                        segment.intervening_unlocated_event_ids.len()
                    );
                    gaps.push(EvidenceGap {
                        gap_id,
                        gap_type: EvidenceGapType::SpatialContinuityGap,
                        severity: GapSeverity::Medium,
                        subject_type: "MOVEMENT_SEGMENT".to_string(),
                        subject_id: seg_id,
                        title: format!(
                            "Spatial Continuity Gap Between '{}' and '{}'",
                            segment.origin_event_id, segment.destination_event_id
                        ),
                        explanation: explanation.clone(),
                        description: Some(explanation),
                        reason_code: Some("GAP-RULE-006-SPATIAL-CONTINUITY".to_string()),
                        impact_on_reconstruction: "Spatial path and intermediate stops cannot be verified during this interval; transit route remains unconfirmed.".to_string(),
                        suggested_corroborating_evidence: "Request highway FASTag/toll transaction records, intermediate ANPR camera captures, or transit route CCTV footage.".to_string(),
                        rule_id: "GAP-RULE-006-SPATIAL-CONTINUITY".to_string(),
                        is_confirmed_missing: true,
                    });
                }
            }
        }
    }

    // 7. RULE-007: Video Evidence Without Extracted Frames
    for manifest in exhibits {
        let is_video = manifest.receipt.source_type.contains("VIDEO")
            || manifest.receipt.mime_type.contains("video");

        if is_video {
            let eid = &manifest.receipt.evidence_id;
            let has_accepted_frames = reconstruction.video_handoff.iter().any(|vh| {
                &vh.evidence_id == eid
                    && (vh.status == VideoHandoffStatus::HandedOffAnchored
                        || vh.status == VideoHandoffStatus::HandedOffUnanchored)
                    && vh.frames_extracted > 0
            });

            if !has_accepted_frames {
                let gap_id = format!("gap-videoframes-{}", eid);
                if seen_keys.insert(gap_id.clone()) {
                    let explanation = format!(
                        "Video exhibit '{}' ({}) has no verified extracted keyframes or frame handoff records.",
                        eid, manifest.receipt.original_filename
                    );
                    gaps.push(EvidenceGap {
                        gap_id,
                        gap_type: EvidenceGapType::VideoWithoutFrames,
                        severity: GapSeverity::High,
                        subject_type: "EVIDENCE".to_string(),
                        subject_id: eid.clone(),
                        title: format!("Video Exhibit '{}' Lacks Extracted Frames", eid),
                        explanation: explanation.clone(),
                        description: Some(explanation),
                        reason_code: Some("GAP-RULE-007-VIDEO-WITHOUT-FRAMES".to_string()),
                        impact_on_reconstruction: "Visual temporal landmarks cannot be correlated with timeline events or inspected for person/vehicle presence.".to_string(),
                        suggested_corroborating_evidence: "Extract I-frame keyframes via video extraction pipeline, ensuring FFprobe metadata and SHA-256 per-frame integrity.".to_string(),
                        rule_id: "GAP-RULE-007-VIDEO-WITHOUT-FRAMES".to_string(),
                        is_confirmed_missing: true,
                    });
                }
            }
        }
    }

    // 8. RULE-008: Missing or Uncalibrated Device Clock Information
    for entry in &reconstruction.timeline.entries {
        let is_uncalibrated = entry
            .assumptions
            .iter()
            .any(|a| a.kind == AssumptionKind::ClockOffsetNotDeclaredOnEvent);

        if is_uncalibrated {
            let gap_id = format!("gap-clock-{}", entry.event_id);
            if seen_keys.insert(gap_id.clone()) {
                let explanation = format!(
                    "Timeline event '{}' (exhibit '{}') relies on an uncalibrated clock offset assumption (declared 0.0s without hardware NTP synchronization certificate).",
                    entry.event_id, entry.source_evidence_id
                );
                gaps.push(EvidenceGap {
                    gap_id,
                    gap_type: EvidenceGapType::UncalibratedClock,
                    severity: GapSeverity::Medium,
                    subject_type: "EVENT".to_string(),
                    subject_id: entry.event_id.clone(),
                    title: format!("Uncalibrated Device Clock on Event '{}'", entry.event_id),
                    explanation: explanation.clone(),
                    description: Some(explanation),
                    reason_code: Some("GAP-RULE-008-UNCALIBRATED-CLOCK".to_string()),
                    impact_on_reconstruction: "Potential device clock drift introduces temporal uncertainty into transit speed and contradiction calculations.".to_string(),
                    suggested_corroborating_evidence: "Obtain DVR system clock calibration audit, NTP time server synchronization logs, or simultaneous live broadcast timestamp.".to_string(),
                    rule_id: "GAP-RULE-008-UNCALIBRATED-CLOCK".to_string(),
                    is_confirmed_missing: true,
                });
            }
        }
    }

    // 9. RULE-009: Unresolved Source Record References
    for entry in &reconstruction.timeline.entries {
        let source_rec = entry
            .metadata
            .get("source_record_id")
            .and_then(|v| v.as_str())
            .map(|s| s.trim())
            .unwrap_or("");

        if source_rec.is_empty() {
            let gap_id = format!("gap-sourcerec-{}", entry.event_id);
            if seen_keys.insert(gap_id.clone()) {
                let explanation = format!(
                    "Timeline event '{}' lacks a specific source record identifier within exhibit '{}'.",
                    entry.event_id, entry.source_evidence_id
                );
                gaps.push(EvidenceGap {
                    gap_id,
                    gap_type: EvidenceGapType::UnresolvedSourceRecord,
                    severity: GapSeverity::Low,
                    subject_type: "EVENT".to_string(),
                    subject_id: entry.event_id.clone(),
                    title: format!("Event '{}' Lacks Source Record Reference", entry.event_id),
                    explanation: explanation.clone(),
                    description: Some(explanation),
                    reason_code: Some("GAP-RULE-009-UNRESOLVED-SOURCE-RECORD".to_string()),
                    impact_on_reconstruction: "Breaks granular claim traceability to individual raw log rows, CDR rows, or video frame indexes.".to_string(),
                    suggested_corroborating_evidence: "Re-run parser with record index mapping to link event to specific CSV row number, JSON object path, or frame index.".to_string(),
                    rule_id: "GAP-RULE-009-UNRESOLVED-SOURCE-RECORD".to_string(),
                    is_confirmed_missing: true,
                });
            }
        }
    }

    // 10. RULE-010: Missing Extraction or Provenance Links
    for entry in &reconstruction.timeline.entries {
        let extraction_run = entry
            .metadata
            .get("extraction_run_id")
            .and_then(|v| v.as_str())
            .map(|s| s.trim())
            .unwrap_or("");

        if extraction_run.is_empty() {
            let gap_id = format!("gap-extrun-{}", entry.event_id);
            if seen_keys.insert(gap_id.clone()) {
                let explanation = format!(
                    "Timeline event '{}' from exhibit '{}' lacks an associated extraction run ID.",
                    entry.event_id, entry.source_evidence_id
                );
                gaps.push(EvidenceGap {
                    gap_id,
                    gap_type: EvidenceGapType::MissingExtractionProvenance,
                    severity: GapSeverity::Low,
                    subject_type: "EVENT".to_string(),
                    subject_id: entry.event_id.clone(),
                    title: format!("Event '{}' Lacks Extraction Run Provenance", entry.event_id),
                    explanation: explanation.clone(),
                    description: Some(explanation),
                    reason_code: Some("GAP-RULE-010-MISSING-EXTRACTION-PROVENANCE".to_string()),
                    impact_on_reconstruction: "The analytical model execution and parser version that generated this event cannot be audited.".to_string(),
                    suggested_corroborating_evidence: "Bind extraction run metadata to event candidate during ingestion or re-run extraction with provenance tracking enabled.".to_string(),
                    rule_id: "GAP-RULE-010-MISSING-EXTRACTION-PROVENANCE".to_string(),
                    is_confirmed_missing: true,
                });
            }
        }
    }

    // 11. RULE-011: Missing Corroboration for Critical Contradiction Leads
    for lead in &reconstruction.contradictions.leads {
        let is_single_exhibit =
            lead.observed_basis.origin_evidence_id == lead.observed_basis.destination_evidence_id;

        if is_single_exhibit {
            let gap_id = format!("gap-nocorr-{}", lead.lead_id);
            if seen_keys.insert(gap_id.clone()) {
                let explanation = format!(
                    "Contradiction lead '{}' between events '{}' and '{}' rests entirely on a single evidence exhibit '{}' without independent cross-source corroboration.",
                    lead.lead_id,
                    lead.event_a_id,
                    lead.event_b_id,
                    lead.observed_basis.origin_evidence_id
                );
                gaps.push(EvidenceGap {
                    gap_id,
                    gap_type: EvidenceGapType::MissingCorroboration,
                    severity: GapSeverity::High,
                    subject_type: "CONTRADICTION_LEAD".to_string(),
                    subject_id: lead.lead_id.clone(),
                    title: format!(
                        "Contradiction Lead '{}' Relies on Single Uncorroborated Exhibit",
                        lead.lead_id
                    ),
                    explanation: explanation.clone(),
                    description: Some(explanation),
                    reason_code: Some("GAP-RULE-011-SINGLE-SOURCE-LEAD".to_string()),
                    impact_on_reconstruction: "If the single recording device has internal clock skew, operator error, or firmware glitches, the contradiction could be an internal artifact rather than physical impossibility.".to_string(),
                    suggested_corroborating_evidence: "Obtain independent secondary evidence (e.g. adjacent shop CCTV, merchant POS UPI receipts, witness testimony, or alternate telecom provider records) to corroborate physical presence.".to_string(),
                    rule_id: "GAP-RULE-011-SINGLE-SOURCE-LEAD".to_string(),
                    is_confirmed_missing: false,
                });
            }
        }
    }

    // Deterministic sorting: by severity (descending), then gap_type, then subject_type, then subject_id
    gaps.sort_by(|a, b| {
        b.severity
            .cmp(&a.severity)
            .then_with(|| a.gap_type.cmp(&b.gap_type))
            .then_with(|| a.subject_type.cmp(&b.subject_type))
            .then_with(|| a.subject_id.cmp(&b.subject_id))
    });

    // Compute aggregations
    let mut gaps_by_severity = BTreeMap::new();
    let mut gaps_by_type = BTreeMap::new();

    for gap in &gaps {
        *gaps_by_severity
            .entry(gap.severity.as_str().to_string())
            .or_insert(0) += 1;
        *gaps_by_type
            .entry(gap.gap_type.as_str().to_string())
            .or_insert(0) += 1;
    }

    EvidenceGapAnalysisReport {
        case_id: case_id.to_string(),
        reconstruction_run_id: run_id,
        audited_at: Utc::now().to_rfc3339(),
        total_gaps_count: gaps.len(),
        gaps_by_severity,
        gaps_by_type,
        gaps,
    }
}
