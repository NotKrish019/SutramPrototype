//! Data Models and Enums for Forensic Reconstruction (Member 4).
//!
//! Provides data structures for:
//! - Temporal reconstruction: canonical time precision, normalized time windows, half-open intervals.
//! - Spatial reconstruction: WGS84 GeoPoints, transit metrics, movement segments, trajectories.
//! - Contradictions: neutral, evidence-grounded investigative leads without fabricated probabilities.
//! - Video forensics: technical video metadata, extracted frames, cryptographic binding.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::AppError;

/// Mean spherical radius of the Earth in kilometers (WGS84 / IUGG).
pub const EARTH_MEAN_RADIUS_KM: f64 = 6371.0088;

/// Canonical time precision levels (shared/enums/README.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CanonicalTimePrecision {
    ExactSecond,
    Minute,
    Hour,
    Day,
    Estimated,
}

impl CanonicalTimePrecision {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ExactSecond => "EXACT_SECOND",
            Self::Minute => "MINUTE",
            Self::Hour => "HOUR",
            Self::Day => "DAY",
            Self::Estimated => "ESTIMATED",
        }
    }
}

/// Temporal ordering relationship between events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TemporalOrderingStatus {
    StrictlyOrdered,
    ConcurrentUncertain,
}

/// Normalized temporal uncertainty window with half-open interval semantics [start, end).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NormalizedTimeWindow {
    pub nominal_utc: DateTime<Utc>,
    pub precision: CanonicalTimePrecision,
    pub raw_precision_input: String,
    pub window_start_utc: DateTime<Utc>,
    pub window_end_utc: DateTime<Utc>,
    pub uncertainty_span_seconds: f64,
    pub has_explicit_bounds: bool,
    pub is_estimated_window_assumption: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_half_width_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<f64>,
    pub source_timezone: String,
    pub clock_offset_seconds: f64,
}

/// Validated WGS84 geographic coordinate pair.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GeoPoint {
    pub lat: f64,
    pub lon: f64,
}

impl GeoPoint {
    pub fn new(lat: f64, lon: f64) -> Result<Self, AppError> {
        if !lat.is_finite() || !(-90.0..=90.0).contains(&lat) {
            return Err(AppError::BadRequest(format!(
                "Invalid WGS84 latitude {}: must be finite and within [-90.0, 90.0].",
                lat
            )));
        }
        if !lon.is_finite() || !(-180.0..=180.0).contains(&lon) {
            return Err(AppError::BadRequest(format!(
                "Invalid WGS84 longitude {}: must be finite and within [-180.0, 180.0].",
                lon
            )));
        }
        Ok(Self { lat, lon })
    }
}

/// Kind of value the reconstruction used that was not supplied as an observed fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AssumptionKind {
    /// The producer gave no `timestamp_precision`; a default precision was used.
    PrecisionDefaulted,
    /// The event declared no timezone; an exhibit-level or configured value was used. Member 2
    /// ingestion writes `UTC` when nothing is declared, so this is not proof of a declaration.
    TimezoneNotDeclaredOnEvent,
    /// The event declared no clock offset; an exhibit-level or configured value was used.
    ClockOffsetNotDeclaredOnEvent,
    /// Coordinates were looked up from a cell tower identifier, not observed.
    CoordinatesInferredFromTowerId,
}

/// One labelled assumption behind a reconstructed event. Values that are not observed facts are
/// always recorded here, so they are never presented as observed (AGENTS.md §4).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ReconstructionAssumption {
    pub kind: AssumptionKind,
    /// The field the assumed value fills (e.g. `timestamp_precision`).
    pub field: String,
    /// The value that was used.
    pub assumed_value: String,
    /// Where the value came from and why it is not an observed fact.
    pub basis: String,
}

/// Judicial export status of a reconstruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExportSafeguardStatus {
    /// No assumption and no unresolved contradiction lead.
    Clear,
    /// At least one finding rests on an assumption, or a contradiction lead is unresolved.
    Blocked,
}

/// Export gate for a reconstruction, with the reasons it is blocked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportSafeguard {
    pub status: ExportSafeguardStatus,
    pub reasons: Vec<String>,
}

/// Reconstructed entry in a timeline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimelineEntry {
    pub event_id: String,
    pub source_evidence_id: String,
    /// The timestamp exactly as the source/producer supplied it (never overwritten).
    #[serde(default)]
    pub raw_timestamp: String,
    pub time_window: NormalizedTimeWindow,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coordinates: Option<GeoPoint>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_description: Option<String>,
    pub entity_ids: Vec<String>,
    pub metadata: serde_json::Value,
    pub ordering_status: TemporalOrderingStatus,
    pub concurrent_event_ids: Vec<String>,
    /// Values this entry rests on that were not observed (empty when everything was supplied).
    #[serde(default)]
    pub assumptions: Vec<ReconstructionAssumption>,
}

/// Pairwise overlapping temporal intervals.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimelineOverlapPair {
    pub event_a_id: String,
    pub event_b_id: String,
    pub overlap_start_utc: DateTime<Utc>,
    pub overlap_end_utc: DateTime<Utc>,
    pub overlap_span_seconds: f64,
}

/// A timeline candidate that was excluded from the reconstructed timeline due to invalid precision, malformed timestamp, or unknown timezone.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExcludedTimelineEvent {
    pub event_id: String,
    pub source_evidence_id: String,
    pub reason: String,
}

/// Comprehensive timeline reconstruction result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimelineReconstructionResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub case_id: Option<String>,
    pub total_events: usize,
    pub entries: Vec<TimelineEntry>,
    pub overlaps: Vec<TimelineOverlapPair>,
    pub strictly_ordered_count: usize,
    pub concurrent_uncertain_count: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub excluded_events: Vec<ExcludedTimelineEvent>,
}

/// Evaluation category for transit speed against reference threshold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SpeedThresholdEvaluation {
    WithinConfiguredThreshold,
    ExceedsConfiguredThreshold,
    ZeroDistanceStationary,
    NonPositiveElapsedTime,
}

/// Metrics measuring elapsed time and conservative speed between two located events.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransitTimeMetrics {
    pub distance_km: f64,
    pub nominal_elapsed_seconds: f64,
    pub min_possible_elapsed_seconds: f64,
    pub max_possible_elapsed_seconds: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_required_speed_kmh: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nominal_speed_kmh: Option<f64>,
    pub configured_speed_threshold_kmh: f64,
    pub exceeds_configured_speed_threshold: bool,
    pub threshold_evaluation: SpeedThresholdEvaluation,
}

/// Directed segment connecting two consecutive located timeline events for an entity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MovementSegment {
    pub origin_event_id: String,
    pub destination_event_id: String,
    pub entity_id: String,
    pub origin_coordinates: GeoPoint,
    pub destination_coordinates: GeoPoint,
    pub transit_metrics: TransitTimeMetrics,
    pub has_intervening_unlocated_events: bool,
    pub intervening_unlocated_event_ids: Vec<String>,
}

/// Pairwise temporal overlap between located events for the same entity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpatialTemporalOverlap {
    pub entity_id: String,
    pub event_a_id: String,
    pub event_b_id: String,
    pub overlap_start_utc: DateTime<Utc>,
    pub overlap_end_utc: DateTime<Utc>,
    pub distance_km: f64,
    pub is_co_located: bool,
}

/// Entity spatial trajectory tracking waypoints and movement segments.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntityTrajectory {
    pub entity_id: String,
    pub waypoints: Vec<TimelineEntry>,
    pub segments: Vec<MovementSegment>,
    pub total_distance_km: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_min_required_speed_kmh: Option<f64>,
}

/// Complete spatial reconstruction result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpatialReconstructionResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub case_id: Option<String>,
    pub trajectories: Vec<EntityTrajectory>,
    pub overlaps: Vec<SpatialTemporalOverlap>,
}

/// Classification of detected contradiction lead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ContradictionLeadType {
    SpatialTransitImpossible,
    TemporalActivityInfeasible,
}

/// Observed facts grounding a contradiction lead.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObservedLeadBasis {
    pub origin_event_id: String,
    pub destination_event_id: String,
    pub origin_evidence_id: String,
    pub destination_evidence_id: String,
    pub origin_timestamp_raw: String,
    pub destination_timestamp_raw: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin_coordinates: Option<GeoPoint>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destination_coordinates: Option<GeoPoint>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_distance_km: Option<f64>,
}

/// Mathematical inferences derived from uncertainty bounds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DerivedLeadInference {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_activity_min_required_speed_kmh: Option<f64>,
    pub reference_speed_threshold_kmh: f64,
    pub max_possible_elapsed_seconds: f64,
    pub min_possible_elapsed_seconds: f64,
    pub nominal_elapsed_seconds: f64,
}

/// Evidence-backed investigative lead highlighting an observed contradiction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContradictionLead {
    pub lead_id: String,
    pub lead_type: ContradictionLeadType,
    pub entity_id: String,
    pub event_a_id: String,
    pub event_b_id: String,
    pub observed_basis: ObservedLeadBasis,
    pub derived_inference: DerivedLeadInference,
    pub neutral_explanation: String,
    pub limitations: Vec<String>,
    pub generated_at: DateTime<Utc>,
}

/// Overall result of contradiction detection across exhibits.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContradictionAnalysisResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub case_id: Option<String>,
    pub leads: Vec<ContradictionLead>,
    pub total_evaluated_segments: usize,
    pub flagged_segments_count: usize,
    pub disclaimer: String,
}

/// Video stream metadata extracted via FFprobe.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VideoStreamMetadata {
    pub index: usize,
    pub codec_name: String,
    pub codec_long_name: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub r_frame_rate: Option<String>,
    pub avg_frame_rate: Option<String>,
    pub duration_seconds: Option<f64>,
    pub nb_frames: Option<u64>,
}

/// Technical video container metadata extracted via FFprobe.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VideoMetadata {
    pub format_name: String,
    pub duration_seconds: f64,
    pub size_bytes: u64,
    pub bit_rate: Option<u64>,
    pub streams: Vec<VideoStreamMetadata>,
    pub probe_sha256: String,
}

/// Metadata and hashes for an individual extracted video keyframe.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtractedFrameArtifact {
    pub frame_index: usize,
    pub pts_seconds: f64,
    pub sha256_hash: String,
    pub relative_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_utc: Option<DateTime<Utc>>,
}

/// Complete video extraction result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VideoExtractionResult {
    pub evidence_id: String,
    pub case_id: String,
    pub source_sha256: String,
    pub metadata: VideoMetadata,
    pub extracted_frames: Vec<ExtractedFrameArtifact>,
    pub extraction_duration_ms: f64,
}
