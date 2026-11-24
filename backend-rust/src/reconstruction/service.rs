//! Comprehensive Forensic Reconstruction Coordinator Service (Member 4).
//!
//! Coordinates temporal timeline normalization, spatial trajectory building,
//! contradiction detection, and video evidence processing.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::Duration;

use super::adapters::{
    event_candidate_to_timeline_event_with_context, unified_extraction_to_timeline_events,
};
use super::contradictions::detect_contradictions;
use super::model::{
    ContradictionAnalysisResult, ExportSafeguard, ExportSafeguardStatus, ExtractedFrameArtifact,
    SpatialReconstructionResult, TimelineReconstructionResult, VideoMetadata,
};
use super::spatial::{reconstruct_spatial_trajectories, DEFAULT_REFERENCE_SPEED_THRESHOLD_KMH};
use super::temporal::{
    reconstruct_timeline, RawTimelineEvent, DEFAULT_ESTIMATED_HALF_WIDTH_SECONDS,
};
use super::video::{extract_keyframes, probe_video_metadata};
use super::video_handoff::{VideoHandoffRecord, VideoHandoffStatus};
use crate::error::AppError;

/// Configuration options for forensic reconstruction.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReconstructionOptions {
    #[serde(default = "default_timezone")]
    pub default_timezone: String,
    #[serde(default)]
    pub default_clock_offset_seconds: f64,
    #[serde(default = "default_speed_threshold")]
    pub speed_threshold_kmh: f64,
    #[serde(default = "default_estimated_half_width")]
    pub estimated_half_width_seconds: f64,
    pub case_id: Option<String>,
}

fn default_timezone() -> String {
    "UTC".to_string()
}
fn default_speed_threshold() -> f64 {
    DEFAULT_REFERENCE_SPEED_THRESHOLD_KMH
}
fn default_estimated_half_width() -> f64 {
    DEFAULT_ESTIMATED_HALF_WIDTH_SECONDS
}

impl Default for ReconstructionOptions {
    fn default() -> Self {
        Self {
            default_timezone: default_timezone(),
            default_clock_offset_seconds: 0.0,
            speed_threshold_kmh: default_speed_threshold(),
            estimated_half_width_seconds: default_estimated_half_width(),
            case_id: None,
        }
    }
}

/// Unified result from a complete case reconstruction.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FullCaseReconstructionResult {
    pub case_id: Option<String>,
    pub timeline: TimelineReconstructionResult,
    pub spatial: SpatialReconstructionResult,
    pub contradictions: ContradictionAnalysisResult,
    /// Judicial export gate: BLOCKED when any finding rests on an assumption or a contradiction
    /// lead is unresolved. Results stored before this field existed read back as BLOCKED.
    #[serde(default = "export_safeguard_not_evaluated")]
    pub export_safeguard: ExportSafeguard,
    /// One record per video exhibit: frames extracted, anchored and handed to the consumer.
    #[serde(default)]
    pub video_handoff: Vec<VideoHandoffRecord>,
    /// Canonical reconstruction run ID, preserved for relational linkage and provenance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reconstruction_run_id: Option<String>,
    /// Backwards-compatible alias for reconstruction run ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    /// Evidence gaps identifying missing corroboration, unlocated events, uncalibrated clocks, etc.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_gaps: Vec<crate::reconstruction::gaps::EvidenceGap>,
}

impl FullCaseReconstructionResult {
    /// Attaches computed evidence gaps to the reconstruction result.
    pub fn with_evidence_gaps(
        mut self,
        gaps: Vec<crate::reconstruction::gaps::EvidenceGap>,
    ) -> Self {
        self.evidence_gaps = gaps;
        self
    }
    /// Attaches video handoff records and adds export-gate reasons for any video exhibit whose
    /// frames could not be extracted or handed off.
    pub fn attach_video_handoff(&mut self, records: Vec<VideoHandoffRecord>) {
        let failed: Vec<String> = records
            .iter()
            .filter(|r| {
                matches!(
                    r.status,
                    VideoHandoffStatus::RejectedIntegrityMismatch
                        | VideoHandoffStatus::ExtractionFailed
                )
            })
            .map(|r| r.evidence_id.clone())
            .collect();
        if !failed.is_empty() {
            self.export_safeguard.status = ExportSafeguardStatus::Blocked;
            self.export_safeguard.reasons.push(format!(
                "Video frames could not be extracted or verified for exhibit(s): {}.",
                failed.join(", ")
            ));
        }
        self.video_handoff = records;
    }
}

fn export_safeguard_not_evaluated() -> ExportSafeguard {
    ExportSafeguard {
        status: ExportSafeguardStatus::Blocked,
        reasons: vec!["Export safeguard was not evaluated for this stored result.".to_string()],
    }
}

/// Evaluates the export gate from the timeline assumptions and contradiction leads.
pub fn evaluate_export_safeguard(
    timeline: &TimelineReconstructionResult,
    contradictions: &ContradictionAnalysisResult,
) -> ExportSafeguard {
    let mut reasons = Vec::new();
    let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
    let mut assumed_events = 0usize;
    for entry in &timeline.entries {
        if !entry.assumptions.is_empty() {
            assumed_events += 1;
        }
        let mut seen = BTreeSet::new();
        for a in &entry.assumptions {
            if seen.insert(a.kind) {
                let key = serde_json::to_value(a.kind)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_string))
                    .unwrap_or_else(|| format!("{:?}", a.kind));
                *kinds.entry(key).or_insert(0) += 1;
            }
        }
    }
    if assumed_events > 0 {
        let detail: Vec<String> = kinds.iter().map(|(k, n)| format!("{k} ({n})")).collect();
        reasons.push(format!(
            "{assumed_events} of {} event(s) rest on values that were not observed: {}.",
            timeline.entries.len(),
            detail.join(", ")
        ));
    }
    if !timeline.excluded_events.is_empty() {
        reasons.push(format!(
            "{} event candidate(s) were excluded from the timeline.",
            timeline.excluded_events.len()
        ));
    }
    if !contradictions.leads.is_empty() {
        reasons.push(format!(
            "{} unresolved contradiction lead(s) require investigator review.",
            contradictions.leads.len()
        ));
    }
    ExportSafeguard {
        status: if reasons.is_empty() {
            ExportSafeguardStatus::Clear
        } else {
            ExportSafeguardStatus::Blocked
        },
        reasons,
    }
}

/// Adds a limitation to each lead whose events rest on assumptions, so a lead built on an
/// inferred position or defaulted clock is never shown as if it were based on observations.
fn annotate_leads_with_assumptions(
    timeline: &TimelineReconstructionResult,
    contradictions: &mut ContradictionAnalysisResult,
) {
    let by_id: BTreeMap<&str, _> = timeline
        .entries
        .iter()
        .map(|e| (e.event_id.as_str(), &e.assumptions))
        .collect();
    for lead in &mut contradictions.leads {
        for event_id in [lead.event_a_id.clone(), lead.event_b_id.clone()] {
            if let Some(assumptions) = by_id.get(event_id.as_str()) {
                if !assumptions.is_empty() {
                    let parts: Vec<String> = assumptions
                        .iter()
                        .map(|a| format!("{} = {}", a.field, a.assumed_value))
                        .collect();
                    lead.limitations.push(format!(
                        "Event {} rests on assumed values ({}); this lead is conditional on them.",
                        event_id,
                        parts.join("; ")
                    ));
                }
            }
        }
    }
}

/// Forensic Reconstruction Service.
#[derive(Debug, Clone, Default)]
pub struct ForensicReconstructionService;

impl ForensicReconstructionService {
    pub fn new() -> Self {
        Self
    }

    /// Reconstructs a full case sequence: timeline, spatial trajectories, and contradiction detection.
    pub fn reconstruct_case(
        &self,
        events: &[RawTimelineEvent],
        options: &ReconstructionOptions,
    ) -> Result<FullCaseReconstructionResult, AppError> {
        let timeline = reconstruct_timeline(
            events,
            &options.default_timezone,
            options.default_clock_offset_seconds,
            options.estimated_half_width_seconds,
            options.case_id.clone(),
        )?;

        let spatial = reconstruct_spatial_trajectories(
            &timeline.entries,
            options.speed_threshold_kmh,
            options.case_id.clone(),
        )?;

        let mut contradictions = detect_contradictions(
            &spatial,
            &timeline.entries,
            options.speed_threshold_kmh,
            options.case_id.clone(),
        )?;
        annotate_leads_with_assumptions(&timeline, &mut contradictions);
        let export_safeguard = evaluate_export_safeguard(&timeline, &contradictions);

        let mut result = FullCaseReconstructionResult {
            case_id: options.case_id.clone(),
            timeline,
            spatial,
            contradictions,
            export_safeguard,
            video_handoff: Vec::new(),
            reconstruction_run_id: None,
            run_id: None,
            evidence_gaps: Vec::new(),
        };

        let gap_report = crate::reconstruction::gaps::analyze_evidence_gaps(
            options.case_id.as_deref().unwrap_or(""),
            &result,
            &[],
            None,
        );
        result.evidence_gaps = gap_report.gaps;

        Ok(result)
    }

    /// Reconstructs case sequence directly from extraction event candidates.
    pub fn reconstruct_from_event_candidates(
        &self,
        candidates: &[crate::extraction::model::ExtractedEventCandidate],
        options: &ReconstructionOptions,
    ) -> Result<FullCaseReconstructionResult, AppError> {
        let raw_events: Vec<RawTimelineEvent> = candidates
            .iter()
            .map(|c| {
                event_candidate_to_timeline_event_with_context(
                    c,
                    Some("EXACT_SECOND"),
                    Some(&options.default_timezone),
                    Some(options.default_clock_offset_seconds),
                )
            })
            .collect();

        self.reconstruct_case(&raw_events, options)
    }

    /// Reconstructs case sequence directly from a UnifiedExtractionResult.
    pub fn reconstruct_from_unified_extraction(
        &self,
        extraction: &crate::extraction::model::UnifiedExtractionResult,
        options: &ReconstructionOptions,
    ) -> Result<FullCaseReconstructionResult, AppError> {
        let raw_events = unified_extraction_to_timeline_events(extraction, Some("EXACT_SECOND"));
        self.reconstruct_case(&raw_events, options)
    }

    /// Probes video container metadata using FFprobe.
    pub async fn probe_video(
        &self,
        video_path: &Path,
        timeout_duration: Duration,
    ) -> Result<VideoMetadata, AppError> {
        probe_video_metadata(video_path, timeout_duration).await
    }

    /// Extracts keyframes from video evidence using FFmpeg.
    pub async fn extract_video_frames(
        &self,
        video_path: &Path,
        output_dir: &Path,
        max_frames: usize,
        timeout_duration: Duration,
    ) -> Result<Vec<ExtractedFrameArtifact>, AppError> {
        extract_keyframes(video_path, output_dir, max_frames, timeout_duration).await
    }
}
