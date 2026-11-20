//! Forensic Timeline, Spatial, Video, and Contradiction Reconstruction Subsystem (Member 4).

pub mod adapters;
pub mod contradictions;
pub mod model;
pub mod report;
pub mod service;
pub mod spatial;
pub mod temporal;
pub mod video;

pub use adapters::{
    event_candidate_to_timeline_event, event_candidate_to_timeline_event_with_context,
    resolve_cell_tower_centroid, unified_extraction_to_timeline_events,
};
pub use contradictions::{
    build_standard_limitations, check_temporal_activity_feasibility, detect_contradictions,
    format_spatial_lead_explanation,
};
pub use model::{
    CanonicalTimePrecision, ContradictionAnalysisResult, ContradictionLead, ContradictionLeadType,
    DerivedLeadInference, EntityTrajectory, ExcludedTimelineEvent, ExtractedFrameArtifact,
    GeoPoint, MovementSegment, NormalizedTimeWindow, ObservedLeadBasis,
    SpatialReconstructionResult, SpatialTemporalOverlap, SpeedThresholdEvaluation,
    TemporalOrderingStatus, TimelineEntry, TimelineOverlapPair, TimelineReconstructionResult,
    TransitTimeMetrics, VideoExtractionResult, VideoMetadata, VideoStreamMetadata,
    EARTH_MEAN_RADIUS_KM,
};
pub use report::{
    generate_case_report, CaseForensicReport, STATUS_SHIP_EXPORT_BLOCKED,
    STATUS_SHIP_EXPORT_CLEARED,
};
pub use service::{
    ForensicReconstructionService, FullCaseReconstructionResult, ReconstructionOptions,
};
pub use spatial::{
    calculate_transit_metrics, create_movement_segment, haversine_distance_km,
    reconstruct_spatial_trajectories, DEFAULT_REFERENCE_SPEED_THRESHOLD_KMH,
};
pub use temporal::{
    compute_uncertainty_window, intervals_overlap, normalize_time_precision,
    parse_and_normalize_timestamp, reconstruct_timeline, validate_timezone, RawTimelineEvent,
    DEFAULT_ESTIMATED_HALF_WIDTH_SECONDS,
};
pub use video::{compute_file_sha256, extract_keyframes, probe_video_metadata};
