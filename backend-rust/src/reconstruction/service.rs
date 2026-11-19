//! Comprehensive Forensic Reconstruction Coordinator Service (Member 4).
//!
//! Coordinates temporal timeline normalization, spatial trajectory building,
//! contradiction detection, and video evidence processing.

use std::path::Path;
use std::time::Duration;

use super::adapters::{
    event_candidate_to_timeline_event_with_context, unified_extraction_to_timeline_events,
};
use super::contradictions::detect_contradictions;
use super::model::{
    ContradictionAnalysisResult, ExtractedFrameArtifact, SpatialReconstructionResult,
    TimelineReconstructionResult, VideoMetadata,
};
use super::spatial::{reconstruct_spatial_trajectories, DEFAULT_REFERENCE_SPEED_THRESHOLD_KMH};
use super::temporal::{
    reconstruct_timeline, RawTimelineEvent, DEFAULT_ESTIMATED_HALF_WIDTH_SECONDS,
};
use super::video::{extract_keyframes, probe_video_metadata};
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

        let contradictions = detect_contradictions(
            &spatial,
            &timeline.entries,
            options.speed_threshold_kmh,
            options.case_id.clone(),
        )?;

        Ok(FullCaseReconstructionResult {
            case_id: options.case_id.clone(),
            timeline,
            spatial,
            contradictions,
        })
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
