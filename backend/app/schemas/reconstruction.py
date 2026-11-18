"""Member 4 Temporal & Spatial Reconstruction Data Transfer Objects (DTOs) and Schemas.

Aligned with the committed canonical contracts in:
- shared/schemas/README.md (Event, Evidence)
- shared/enums/README.md (TimePrecision)
- shared/examples/README.md
"""

from datetime import datetime
from enum import StrEnum
import math
from typing import Any, Optional

from pydantic import BaseModel, ConfigDict, Field, field_validator, model_validator


class CanonicalTimePrecision(StrEnum):
    """Canonical TimePrecision values committed in shared/enums/README.md (lines 39-44)."""

    EXACT_SECOND = "EXACT_SECOND"
    MINUTE = "MINUTE"
    HOUR = "HOUR"
    DAY = "DAY"
    ESTIMATED = "ESTIMATED"


# Documented compatibility adapter mapping legacy table names in shared/schemas/README.md (line 64)
# to the committed canonical enum values in shared/enums/README.md (lines 39-44).
LEGACY_TIME_PRECISION_ALIASES: dict[str, CanonicalTimePrecision] = {
    "EXACT": CanonicalTimePrecision.EXACT_SECOND,
    "APPROXIMATE": CanonicalTimePrecision.ESTIMATED,
}


class TemporalOrderingStatus(StrEnum):
    """Epistemic chronological status of a timeline entry or event pair."""

    STRICTLY_ORDERED = "STRICTLY_ORDERED"
    CONCURRENT_UNCERTAIN = "CONCURRENT_UNCERTAIN"


class GeoPoint(BaseModel):
    """Validated WGS84 geographic coordinates in decimal degrees.

    Latitude must be within [-90.0, 90.0] and longitude within [-180.0, 180.0].
    Latitude and longitude are never silently swapped.
    """

    model_config = ConfigDict(frozen=True, extra="forbid")

    lat: float = Field(
        description="WGS84 latitude in decimal degrees [-90.0, 90.0].",
    )
    lon: float = Field(
        description="WGS84 longitude in decimal degrees [-180.0, 180.0].",
    )

    @field_validator("lat", mode="before")
    @classmethod
    def _validate_lat(cls, value: Any) -> float:
        if isinstance(value, bool) or not isinstance(value, (int, float)):
            raise ValueError(
                f"Invalid WGS84 latitude type: expected finite int or float, got {type(value).__name__}."
            )
        lat_val = float(value)
        if not math.isfinite(lat_val):
            raise ValueError(f"Invalid WGS84 latitude '{value}': must be a finite number.")
        if lat_val < -90.0 or lat_val > 90.0:
            raise ValueError(
                f"Invalid WGS84 latitude {lat_val}: must be between -90.0 and 90.0 degrees."
            )
        return 0.0 if lat_val == 0.0 else lat_val

    @field_validator("lon", mode="before")
    @classmethod
    def _validate_lon(cls, value: Any) -> float:
        if isinstance(value, bool) or not isinstance(value, (int, float)):
            raise ValueError(
                f"Invalid WGS84 longitude type: expected finite int or float, got {type(value).__name__}."
            )
        lon_val = float(value)
        if not math.isfinite(lon_val):
            raise ValueError(f"Invalid WGS84 longitude '{value}': must be a finite number.")
        if lon_val < -180.0 or lon_val > 180.0:
            raise ValueError(
                f"Invalid WGS84 longitude {lon_val}: must be between -180.0 and 180.0 degrees."
            )
        return 0.0 if lon_val == 0.0 else lon_val


class SpeedThresholdEvaluation(StrEnum):
    """Comparison of conservative minimum required speed against a configurable reference threshold.

    A threshold comparison is an analytical reference indicator and never proves physical impossibility.
    """

    ZERO_DISTANCE_STATIONARY = "ZERO_DISTANCE_STATIONARY"
    WITHIN_CONFIGURED_THRESHOLD = "WITHIN_CONFIGURED_THRESHOLD"
    EXCEEDS_CONFIGURED_THRESHOLD = "EXCEEDS_CONFIGURED_THRESHOLD"
    NON_POSITIVE_ELAPSED_TIME = "NON_POSITIVE_ELAPSED_TIME"


class NormalizedTimeWindow(BaseModel):
    """Normalized UTC half-open interval [window_start_utc, window_end_utc) for an event.

    Preserves epistemic start-time uncertainty strictly separate from event duration.
    """

    model_config = ConfigDict(frozen=True)

    nominal_utc: datetime = Field(
        description="Clock-corrected UTC nominal timestamp of the event."
    )
    precision: CanonicalTimePrecision = Field(
        description="Canonical precision level applied to derive the uncertainty interval."
    )
    raw_precision_input: str = Field(
        description="Original timestamp_precision value provided on the input Event."
    )
    window_start_utc: datetime = Field(
        description="Inclusive lower bound of the half-open UTC uncertainty interval [start, end)."
    )
    window_end_utc: datetime = Field(
        description="Exclusive upper bound of the half-open UTC uncertainty interval [start, end)."
    )
    uncertainty_span_seconds: float = Field(
        gt=0.0,
        description="Width of [window_start_utc, window_end_utc) in seconds.",
    )
    has_explicit_bounds: bool = Field(
        default=False,
        description="True if interval bounds were explicitly supplied rather than derived from precision.",
    )
    is_estimated_window_assumption: bool = Field(
        default=False,
        description=(
            "True when bounds were derived using a configurable ESTIMATED half-width assumption "
            "(not a hard forensic measurement)."
        ),
    )
    estimated_half_width_seconds: Optional[float] = Field(
        default=None,
        gt=0.0,
        description="Configurable half-width in seconds used when precision is ESTIMATED.",
    )
    duration_seconds: Optional[float] = Field(
        default=None,
        ge=0.0,
        description="Elapsed duration of the activity if explicitly known; never inferred from uncertainty.",
    )
    source_timezone: str = Field(
        default="UTC",
        description="IANA timezone identifier used to interpret local boundaries before UTC conversion.",
    )
    clock_offset_seconds: float = Field(
        default=0.0,
        description="Signed clock drift correction (seconds) applied to nominal and interval boundaries.",
    )

    @model_validator(mode="after")
    def _validate_half_open_interval(self) -> "NormalizedTimeWindow":
        if self.window_start_utc >= self.window_end_utc:
            raise ValueError(
                f"Invalid half-open interval [{self.window_start_utc.isoformat()}, "
                f"{self.window_end_utc.isoformat()}): window_start_utc must be strictly less than window_end_utc."
            )
        return self


class TimelineOverlapPair(BaseModel):
    """Represents a pair of events whose half-open uncertainty intervals [start, end) intersect.

    Overlapping windows indicate that relative chronological order is epistemically uncertain.
    """

    model_config = ConfigDict(frozen=True)

    event_a_id: str = Field(description="First overlapping event ID.")
    event_b_id: str = Field(description="Second overlapping event ID.")
    overlap_start_utc: datetime = Field(
        description="Inclusive start of the intersection interval [overlap_start_utc, overlap_end_utc)."
    )
    overlap_end_utc: datetime = Field(
        description="Exclusive end of the intersection interval [overlap_start_utc, overlap_end_utc)."
    )
    overlap_seconds: float = Field(
        gt=0.0,
        description="Duration of the intersecting uncertainty window in seconds.",
    )
    ordering_status: TemporalOrderingStatus = Field(
        default=TemporalOrderingStatus.CONCURRENT_UNCERTAIN,
        description="Marks that strict sequential ordering between event_a_id and event_b_id is unproven.",
    )


class TimelineEntry(BaseModel):
    """Single reconstructed event entry preserving full provenance and temporal uncertainty."""

    model_config = ConfigDict(frozen=True)

    event_id: str = Field(description="Unique event identifier.")
    case_id: str = Field(description="Parent case identifier.")
    title: str = Field(description="Short description of the occurrence.")
    source_evidence_id: str = Field(description="Direct reference to origin Evidence ID.")
    involved_entity_ids: list[str] = Field(
        default_factory=list,
        description="Entity IDs participating in this event.",
    )
    location_name: Optional[str] = Field(
        default=None,
        description="Optional human-readable location description.",
    )
    coordinates: Optional[dict[str, float]] = Field(
        default=None,
        description="Optional geographic coordinates preserved from the source event.",
    )
    time_window: NormalizedTimeWindow = Field(
        description="Normalized UTC timestamp and half-open uncertainty interval [start, end)."
    )
    ordering_status: TemporalOrderingStatus = Field(
        default=TemporalOrderingStatus.STRICTLY_ORDERED,
        description="CONCURRENT_UNCERTAIN if this event overlaps with at least one other event in the timeline.",
    )
    concurrent_event_ids: list[str] = Field(
        default_factory=list,
        description="Deterministic list of event IDs whose uncertainty windows intersect this event's window.",
    )


class TimelineReconstructionResult(BaseModel):
    """Complete chronological reconstruction output for a set of case events."""

    model_config = ConfigDict(frozen=True)

    case_id: str = Field(description="Case identifier for the reconstructed timeline.")
    entries: list[TimelineEntry] = Field(
        default_factory=list,
        description="Deterministically sorted timeline entries (display order does not imply strict sequence when overlapping).",
    )
    overlaps: list[TimelineOverlapPair] = Field(
        default_factory=list,
        description="All pairwise overlapping event uncertainty intervals.",
    )
    ordering_note: str = Field(
        default=(
            "Display order is deterministic by window start, window end, nominal timestamp, and event_id. "
            "Events listed in overlaps or concurrent_event_ids have intersecting half-open uncertainty "
            "windows [start, end) and their relative chronological sequence is not proven."
        ),
        description="Forensic interpretation disclaimer.",
    )


class TransitTimeMetrics(BaseModel):
    """Elapsed-time bounds and speed metrics between two time windows separated by distance_km."""

    model_config = ConfigDict(frozen=True)

    distance_km: float = Field(
        ge=0.0,
        description="Great-circle Haversine distance in kilometers.",
    )
    nominal_elapsed_seconds: float = Field(
        description="Nominal elapsed time (destination.nominal_utc - origin.nominal_utc) in seconds."
    )
    min_possible_elapsed_seconds: float = Field(
        description="Minimum possible elapsed time (destination.window_start_utc - origin.window_end_utc) in seconds."
    )
    max_possible_elapsed_seconds: float = Field(
        description="Conservative maximum possible elapsed time (destination.window_end_utc - origin.window_start_utc) in seconds."
    )
    min_required_speed_kmh: Optional[float] = Field(
        default=None,
        ge=0.0,
        description=(
            "Conservative lower-bound speed (km/h) required over max_possible_elapsed_seconds. "
            "None when max_possible_elapsed_seconds <= 0 and distance_km > 0."
        ),
    )
    nominal_speed_kmh: Optional[float] = Field(
        default=None,
        ge=0.0,
        description="Speed (km/h) computed over nominal_elapsed_seconds when nominal_elapsed_seconds > 0.",
    )
    configured_speed_threshold_kmh: float = Field(
        gt=0.0,
        description="Configurable reference transport speed threshold in km/h (assumption, not a universal fact).",
    )
    exceeds_configured_speed_threshold: bool = Field(
        description="True when min_required_speed_kmh exceeds configured_speed_threshold_kmh (or elapsed time <= 0 with distance > 0)."
    )
    threshold_evaluation: SpeedThresholdEvaluation = Field(
        description="Comparison classification against configured_speed_threshold_kmh."
    )
    threshold_assumption_note: str = Field(
        default=(
            "configured_speed_threshold_kmh is a user-configurable reference assumption for "
            "analytical triage and does not establish a physical transit limit or statistical likelihood."
        ),
        description="Forensic disclaimer on the speed threshold parameter.",
    )


class MovementSegment(BaseModel):
    """Directed spatiotemporal movement segment between two strictly ordered events for an entity."""

    model_config = ConfigDict(frozen=True)

    segment_id: str = Field(description="Deterministic identifier for this movement segment.")
    case_id: str = Field(description="Parent case identifier.")
    entity_id: str = Field(description="Entity identifier moving between origin and destination.")
    origin_event_id: str = Field(description="Origin event ID.")
    destination_event_id: str = Field(description="Destination event ID.")
    origin_source_evidence_id: str = Field(description="Source evidence ID of the origin event.")
    destination_source_evidence_id: str = Field(
        description="Source evidence ID of the destination event."
    )
    origin_location_name: Optional[str] = Field(default=None)
    destination_location_name: Optional[str] = Field(default=None)
    origin_coordinates: GeoPoint = Field(description="Validated WGS84 origin coordinates.")
    destination_coordinates: GeoPoint = Field(
        description="Validated WGS84 destination coordinates."
    )
    origin_time_window: NormalizedTimeWindow = Field(
        description="Normalized half-open UTC uncertainty window [start, end) of origin event."
    )
    destination_time_window: NormalizedTimeWindow = Field(
        description="Normalized half-open UTC uncertainty window [start, end) of destination event."
    )
    ordering_status: TemporalOrderingStatus = Field(
        default=TemporalOrderingStatus.STRICTLY_ORDERED,
        description="Movement segments are generated only when origin is strictly before destination.",
    )
    distance_km: float = Field(ge=0.0, description="Haversine distance in kilometers.")
    nominal_elapsed_seconds: float = Field(
        description="Nominal elapsed time (destination.nominal_utc - origin.nominal_utc) in seconds."
    )
    min_possible_elapsed_seconds: float = Field(
        description="Minimum possible elapsed time (destination.window_start_utc - origin.window_end_utc) in seconds."
    )
    max_possible_elapsed_seconds: float = Field(
        description="Conservative maximum possible elapsed time (destination.window_end_utc - origin.window_start_utc) in seconds."
    )
    min_required_speed_kmh: Optional[float] = Field(
        default=None,
        ge=0.0,
        description="Conservative minimum speed (km/h) using max_possible_elapsed_seconds.",
    )
    nominal_speed_kmh: Optional[float] = Field(default=None, ge=0.0)
    configured_speed_threshold_kmh: float = Field(gt=0.0)
    exceeds_configured_speed_threshold: bool
    threshold_evaluation: SpeedThresholdEvaluation
    threshold_assumption_note: str
    intervening_unlocated_event_ids: list[str] = Field(
        default_factory=list,
        description=(
            "Ordered event IDs for this entity that occurred or overlapped between origin and "
            "destination but lacked geographic coordinates."
        ),
    )
    has_unlocated_intervening_events: bool = Field(
        default=False,
        description=(
            "True when one or more unlocated events intervened between origin and destination; "
            "indicates this segment is net displacement between known points, not uninterrupted movement."
        ),
    )
    continuity_note: Optional[str] = Field(
        default=None,
        description=(
            "Explicit provenance note when intervening unlocated events exist between origin and destination."
        ),
    )


class SpatialTemporalOverlap(BaseModel):
    """Pair of geolocated events for the same entity whose uncertainty windows [start, end) overlap.

    Because their temporal windows intersect, no definite sequential direction (A -> B vs B -> A)
    is claimed.
    """

    model_config = ConfigDict(frozen=True)

    entity_id: str = Field(description="Involved entity ID.")
    event_a_id: str = Field(description="First overlapping located event ID.")
    event_b_id: str = Field(description="Second overlapping located event ID.")
    source_evidence_a_id: str = Field(description="Origin evidence ID for event_a_id.")
    source_evidence_b_id: str = Field(description="Origin evidence ID for event_b_id.")
    location_a_name: Optional[str] = Field(default=None)
    location_b_name: Optional[str] = Field(default=None)
    coordinates_a: GeoPoint
    coordinates_b: GeoPoint
    time_window_a: NormalizedTimeWindow
    time_window_b: NormalizedTimeWindow
    overlap_start_utc: datetime
    overlap_end_utc: datetime
    overlap_seconds: float = Field(gt=0.0)
    distance_km: float = Field(
        ge=0.0,
        description="Haversine distance in kilometers between the two concurrent/overlapping locations.",
    )
    max_possible_separation_seconds: float = Field(
        ge=0.0,
        description="Conservative maximum time separation between an instant in A and an instant in B.",
    )
    ordering_status: TemporalOrderingStatus = Field(
        default=TemporalOrderingStatus.CONCURRENT_UNCERTAIN,
        description="Marks that chronological sequence between event_a_id and event_b_id is uncertain.",
    )
    ordering_note: str = Field(
        default=(
            "Temporal uncertainty windows [start, end) overlap; a directed movement segment "
            "is withheld because chronological sequence between these two sightings is unproven."
        )
    )


class EntityTrajectory(BaseModel):
    """Reconstructed spatial-temporal record for a single entity across a case."""

    model_config = ConfigDict(frozen=True)

    case_id: str = Field(description="Parent case ID.")
    entity_id: str = Field(description="Entity ID.")
    timeline_entries: list[TimelineEntry] = Field(
        default_factory=list,
        description="All normalized timeline entries involving this entity, preserving unlocated events.",
    )
    located_event_ids: list[str] = Field(
        default_factory=list,
        description="Event IDs with validated WGS84 coordinates for this entity.",
    )
    unlocated_event_ids: list[str] = Field(
        default_factory=list,
        description="Event IDs involving this entity that lack geographic coordinates.",
    )
    segments: list[MovementSegment] = Field(
        default_factory=list,
        description="Directed movement segments generated only between strictly ordered consecutive located events.",
    )
    uncertain_spatial_overlaps: list[SpatialTemporalOverlap] = Field(
        default_factory=list,
        description="Pairs of located events with overlapping time windows where sequence is uncertain.",
    )
    total_strictly_ordered_distance_km: float = Field(
        default=0.0,
        ge=0.0,
        description="Sum of distance_km across strictly ordered segments.",
    )


class SpatialReconstructionResult(BaseModel):
    """Complete spatial reconstruction output across all entities in a case."""

    model_config = ConfigDict(frozen=True)

    case_id: str = Field(description="Case identifier.")
    timeline: TimelineReconstructionResult = Field(
        description="Underlying normalized timeline across all case events."
    )
    trajectories_by_entity: dict[str, EntityTrajectory] = Field(
        default_factory=dict,
        description="Deterministic mapping of entity_id to its reconstructed EntityTrajectory.",
    )
    configured_speed_threshold_kmh: float = Field(
        gt=0.0,
        description="Configurable reference speed threshold (km/h) used during evaluation.",
    )


class ContradictionLeadType(StrEnum):
    """Classification of conservative, non-accusatory investigative contradiction leads."""

    SPEED_THRESHOLD_EXCEEDANCE_LEAD = "SPEED_THRESHOLD_EXCEEDANCE_LEAD"
    TEMPORAL_ACTIVITY_OVERLAP_LEAD = "TEMPORAL_ACTIVITY_OVERLAP_LEAD"


class ObservedLeadBasis(BaseModel):
    """Strictly observed facts from source evidence supporting a contradiction lead."""

    model_config = ConfigDict(frozen=True)

    entity_id: str
    origin_event_id: str
    destination_event_id: str
    origin_source_evidence_id: str
    destination_source_evidence_id: str
    origin_location_name: Optional[str] = None
    destination_location_name: Optional[str] = None
    origin_coordinates: Optional[GeoPoint] = None
    destination_coordinates: Optional[GeoPoint] = None
    origin_time_window: NormalizedTimeWindow
    destination_time_window: NormalizedTimeWindow
    origin_duration_seconds: Optional[float] = None


class DerivedLeadInference(BaseModel):
    """Algorithmically derived quantities separated explicitly from observed facts."""

    model_config = ConfigDict(frozen=True)

    is_observed: bool = Field(
        default=False,
        description="Always False because transit speeds and temporal conflicts are derived inferences.",
    )
    ordering_status: TemporalOrderingStatus = Field(
        default=TemporalOrderingStatus.STRICTLY_ORDERED,
        description="Leads are emitted only when start-time windows are strictly ordered.",
    )
    distance_km: Optional[float] = Field(default=None, ge=0.0)
    nominal_elapsed_seconds: float
    min_possible_elapsed_seconds: float
    max_possible_elapsed_seconds: float
    post_activity_min_elapsed_seconds: Optional[float] = Field(
        default=None,
        description=(
            "Minimum remaining elapsed seconds after subtracting origin duration_seconds "
            "from the latest origin start time (B_start - (A_end + D))."
        ),
    )
    post_activity_max_elapsed_seconds: Optional[float] = Field(
        default=None,
        description=(
            "Conservative maximum remaining elapsed seconds after subtracting origin duration_seconds "
            "from the earliest origin start time (B_end - (A_start + D))."
        ),
    )
    min_required_speed_kmh: Optional[float] = Field(default=None, ge=0.0)
    post_activity_min_required_speed_kmh: Optional[float] = Field(default=None, ge=0.0)
    configured_speed_threshold_kmh: float = Field(gt=0.0)
    threshold_evaluation: Optional[SpeedThresholdEvaluation] = None
    intervening_unlocated_event_ids: list[str] = Field(default_factory=list)
    has_unlocated_intervening_events: bool = False


class DerivedContradictionRelationship(BaseModel):
    """Derived Relationship edge compatible with shared/schemas/README.md (section 5).

    Always sets relation_type='CONTRADICTS' and is_observed=False.
    """

    model_config = ConfigDict(frozen=True)

    relationship_id: str = Field(description="Stable deterministic relationship identifier.")
    source_id: str = Field(description="Origin Event ID.")
    target_id: str = Field(description="Destination Event ID.")
    relation_type: str = Field(
        default="CONTRADICTS",
        description="Canonical RelationType value from shared/enums/README.md.",
    )
    weight: Optional[float] = Field(
        default=None,
        description="Left None so arbitrary confidence or statistical weights are never fabricated.",
    )
    is_observed: bool = Field(
        default=False,
        description="Always False for algorithmically derived contradiction leads.",
    )
    evidence_basis: list[str] = Field(
        default_factory=list,
        description="Deduplicated origin and destination Evidence IDs backing this derived link.",
    )
    metadata: dict[str, Any] = Field(
        default_factory=dict,
        description="Explainable diagnostic metadata separating observed facts from derived metrics.",
    )


class ContradictionLead(BaseModel):
    """Investigative lead describing a conservative temporal or spatial-transit discrepancy."""

    model_config = ConfigDict(frozen=True)

    lead_id: str = Field(description="Stable deterministic lead identifier.")
    case_id: str = Field(description="Parent case identifier.")
    entity_id: str = Field(description="Entity associated with both events.")
    lead_type: ContradictionLeadType = Field(description="Classification of the lead.")
    origin_event_id: str = Field(description="Earlier strictly ordered event ID.")
    destination_event_id: str = Field(description="Later strictly ordered event ID.")
    evidence_basis: list[str] = Field(
        default_factory=list,
        description="Deduplicated Evidence IDs supporting the two events.",
    )
    observed_facts: ObservedLeadBasis = Field(
        description="Hard observed facts extracted from the underlying events and evidence."
    )
    derived_inference: DerivedLeadInference = Field(
        description="Derived spatial-temporal calculations."
    )
    explanation: str = Field(
        description="Neutral, factual description of the observations and derived metrics."
    )
    limitations: list[str] = Field(
        default_factory=list,
        description="Explicit analytical caveats and alternative explanations for investigator review.",
    )
    relationship: DerivedContradictionRelationship = Field(
        description="Canonical Relationship representation (relation_type='CONTRADICTS', is_observed=False)."
    )


class ContradictionAnalysisResult(BaseModel):
    """Complete output of the Phase 2C contradiction detection engine."""

    model_config = ConfigDict(frozen=True)

    case_id: str = Field(description="Case identifier.")
    configured_speed_threshold_kmh: float = Field(gt=0.0)
    leads: list[ContradictionLead] = Field(
        default_factory=list,
        description="Deterministically ordered investigative contradiction leads.",
    )
    relationships: list[DerivedContradictionRelationship] = Field(
        default_factory=list,
        description="Derived Relationship records corresponding 1-to-1 with leads.",
    )
    unresolved_overlaps_skipped: list[SpatialTemporalOverlap] = Field(
        default_factory=list,
        description=(
            "Pairs of located events with overlapping uncertainty windows that were "
            "intentionally excluded from contradiction leads due to unresolved event ordering."
        ),
    )
    methodology_note: str = Field(
        default=(
            "Leads are generated only for strictly ordered event pairs where conservative transit "
            "metrics exceed the configured reference speed threshold or where an explicit activity "
            "duration overlaps a subsequent sighting at a distinct location. Overlapping uncertainty "
            "windows are never treated as contradictions."
        )
    )


def extract_field(obj: dict[str, Any] | BaseModel | Any, field_name: str, default: Any = None) -> Any:
    """Safely read a field from either a mapping or an object/BaseModel without mutating it."""
    if isinstance(obj, dict):
        return obj.get(field_name, default)
    return getattr(obj, field_name, default)


class VideoStreamMetadata(BaseModel):
    """Technical stream-level metadata extracted from video evidence."""

    model_config = ConfigDict(frozen=True)

    stream_index: int = Field(ge=0, description="0-indexed position in container.")
    codec_type: str = Field(description="Stream type (e.g., 'video', 'audio', 'subtitle').")
    codec_name: Optional[str] = Field(default=None, description="Codec identifier (e.g., 'h264', 'aac').")
    codec_long_name: Optional[str] = Field(default=None, description="Full descriptive codec name.")
    profile: Optional[str] = Field(default=None, description="Stream profile if reported.")
    width: Optional[int] = Field(default=None, ge=1, description="Frame width in pixels for video.")
    height: Optional[int] = Field(default=None, ge=1, description="Frame height in pixels for video.")
    fps: Optional[float] = Field(default=None, ge=0.0, description="Evaluated frame rate as float.")
    r_frame_rate: Optional[str] = Field(default=None, description="Reported raw frame rate (e.g., '30/1').")
    avg_frame_rate: Optional[str] = Field(default=None, description="Reported average frame rate (e.g., '30/1').")
    bit_rate: Optional[int] = Field(default=None, ge=0, description="Bitrate in bits/s if reported.")
    duration_seconds: Optional[float] = Field(default=None, ge=0.0, description="Stream duration in seconds.")
    nb_frames: Optional[int] = Field(default=None, ge=0, description="Total frame count in stream if known.")
    tags: dict[str, str] = Field(default_factory=dict, description="Stream-level tags dictionary.")


class VideoMetadata(BaseModel):
    """Container and primary stream forensic metadata extracted via FFprobe."""

    model_config = ConfigDict(frozen=True)

    container_format: str = Field(description="Container format from FFprobe (e.g., 'mov,mp4,m4a,3gp,3g2,mj2').")
    format_long_name: Optional[str] = Field(default=None, description="Human-readable container format name.")
    duration_seconds: float = Field(ge=0.0, description="Total duration of the media container in seconds.")
    file_size_bytes: int = Field(ge=0, description="Original file size in bytes.")
    sha256_hash: str = Field(description="Cryptographic SHA-256 hash of the video file.")
    width: Optional[int] = Field(default=None, ge=1, description="Primary video frame width in pixels.")
    height: Optional[int] = Field(default=None, ge=1, description="Primary video frame height in pixels.")
    fps: Optional[float] = Field(default=None, ge=0.0, description="Evaluated frame rate of primary video.")
    codec_name: Optional[str] = Field(default=None, description="Codec of primary video stream.")
    has_video: bool = Field(default=True, description="True if at least one video stream is present.")
    has_audio: bool = Field(default=False, description="True if at least one audio stream is present.")
    video_streams_count: int = Field(default=1, ge=0, description="Total count of video streams.")
    audio_streams_count: int = Field(default=0, ge=0, description="Total count of audio streams.")
    creation_time_raw: Optional[str] = Field(
        default=None,
        description="Raw creation_time tag from metadata; treated as unverified camera metadata.",
    )
    streams: list[VideoStreamMetadata] = Field(default_factory=list, description="All parsed streams.")
    tags: dict[str, str] = Field(default_factory=dict, description="Container-level metadata tags.")
    tool_version: Optional[str] = Field(default=None, description="FFprobe tool version.")


class ExtractedFrameArtifact(BaseModel):
    """Derived image frame artifact extracted from video evidence."""

    model_config = ConfigDict(frozen=True)

    artifact_id: str = Field(description="Deterministic artifact identifier.")
    case_id: str = Field(description="Parent case identifier.")
    source_evidence_id: str = Field(description="Direct reference to origin CCTV Evidence ID.")
    source_video_sha256: str = Field(description="Cryptographic SHA-256 of the source video.")
    frame_index: int = Field(ge=1, description="1-indexed sequence number of the extracted frame.")
    pts_offset_seconds: float = Field(
        ge=0.0,
        description="Presentation timestamp offset in seconds from the beginning of the video.",
    )
    timestamp_utc: Optional[datetime] = Field(
        default=None,
        description="Normalized UTC timestamp if camera start time and timezone were provided.",
    )
    timestamp_precision: Optional[CanonicalTimePrecision] = Field(
        default=None,
        description="Precision of the derived UTC timestamp.",
    )
    time_window: Optional[NormalizedTimeWindow] = Field(
        default=None,
        description="Derived half-open uncertainty window [start, end) if camera start was provided.",
    )
    is_utc_anchored: bool = Field(
        default=False,
        description="True if timestamp_utc was computed; False if timestamp is purely relative PTS.",
    )
    file_path: str = Field(description="Absolute file path to the derived image artifact on disk.")
    file_name: str = Field(description="Deterministic filename of the derived image.")
    image_format: str = Field(description="Image format extension (e.g. 'jpg', 'png').")
    file_size_bytes: int = Field(ge=0, description="Size of the extracted image in bytes.")
    sha256_hash: str = Field(description="Cryptographic SHA-256 hash of the derived frame artifact.")
    width: int = Field(gt=0, description="Width of the extracted frame in pixels.")
    height: int = Field(gt=0, description="Height of the extracted frame in pixels.")
    extraction_method: str = Field(
        default="fixed_interval",
        description="Method used for extraction ('fixed_interval' or 'keyframes').",
    )


class VideoExtractionOptions(BaseModel):
    """Configuration parameters for keyframe / interval frame extraction."""

    model_config = ConfigDict(frozen=True)

    interval_seconds: Optional[float] = Field(
        default=1.0,
        gt=0.0,
        description="Interval in seconds between extracted frames (e.g. 1.0 = 1 frame/sec).",
    )
    fps: Optional[float] = Field(
        default=None,
        gt=0.0,
        description="Frame rate for extraction (e.g. 0.5 = 1 frame every 2s). Mutually consistent with interval_seconds.",
    )
    max_frames: int = Field(
        default=100,
        gt=0,
        le=10000,
        description="Maximum number of frames to extract (hard ceiling to prevent resource exhaustion).",
    )
    start_time_seconds: Optional[float] = Field(
        default=None,
        ge=0.0,
        description="Optional start time bound within the video in seconds.",
    )
    end_time_seconds: Optional[float] = Field(
        default=None,
        ge=0.0,
        description="Optional end time bound within the video in seconds.",
    )
    image_format: str = Field(
        default="jpg",
        description="Output image format ('jpg', 'jpeg', 'png', 'bmp').",
    )
    keyframes_only: bool = Field(
        default=False,
        description="If True, extracts only native keyframes (I-frames) instead of fixed intervals.",
    )
    create_unique_run_subdir: bool = Field(
        default=False,
        description="If True, creates a unique collision-free subdirectory inside output_dir for this run.",
    )
    recording_start_timestamp: Optional[str | datetime] = Field(
        default=None,
        description="Optional ISO 8601 string or datetime of camera recording start.",
    )
    source_timezone: str = Field(
        default="UTC",
        description="IANA timezone for recording_start_timestamp localization.",
    )
    clock_offset_seconds: float = Field(
        default=0.0,
        description="Signed DVR clock correction in seconds to apply to recording start.",
    )
    timeout_seconds: float = Field(
        default=30.0,
        gt=0.0,
        description="Process execution timeout limit in seconds.",
    )

    @model_validator(mode="before")
    @classmethod
    def _reject_overwrite_option(cls, data: Any) -> Any:
        if isinstance(data, dict) and "overwrite" in data:
            raise ValueError(
                "Overwriting pre-existing derived artifacts is strictly forbidden in YUKTI forensics. "
                "The 'overwrite' option has been permanently removed."
            )
        return data

    @field_validator("image_format")
    @classmethod
    def _validate_image_format(cls, value: str) -> str:
        fmt = value.lower().lstrip(".")
        if fmt not in {"jpg", "jpeg", "png", "bmp"}:
            raise ValueError(
                f"Unsupported image format '{value}'. Supported formats: jpg, jpeg, png, bmp."
            )
        return fmt

    @model_validator(mode="after")
    def _validate_options(self) -> "VideoExtractionOptions":
        if self.start_time_seconds is not None and self.end_time_seconds is not None:
            if self.end_time_seconds <= self.start_time_seconds:
                raise ValueError(
                    f"Invalid time bounds: end_time_seconds ({self.end_time_seconds}) must be "
                    f"strictly greater than start_time_seconds ({self.start_time_seconds})."
                )
        if self.fps is not None and self.interval_seconds is not None:
            expected_interval = 1.0 / self.fps
            if abs(expected_interval - self.interval_seconds) > 1e-3:
                raise ValueError(
                    f"Conflicting fps ({self.fps}) and interval_seconds ({self.interval_seconds}). "
                    f"Expected interval_seconds to be ~{expected_interval:.4f}."
                )
        return self


class VideoExtractionResult(BaseModel):
    """Complete forensic output of the video processing and frame extraction engine."""

    model_config = ConfigDict(frozen=True)

    case_id: str = Field(description="Parent case identifier.")
    source_evidence_id: str = Field(description="Evidence ID of the input video.")
    source_video_path: str = Field(description="Absolute path to the source video file.")
    source_video_sha256_before: str = Field(
        description="SHA-256 hash of the video before processing commenced."
    )
    source_video_sha256_after: str = Field(
        description="SHA-256 hash of the video after processing completed."
    )
    integrity_verified: bool = Field(
        description="True if source_video_sha256_before equals source_video_sha256_after, proving immutability."
    )
    output_directory: str = Field(
        description="Directory where derived frame artifacts were written."
    )
    video_metadata: VideoMetadata = Field(
        description="Extracted video container and stream metadata."
    )
    options_applied: VideoExtractionOptions = Field(
        description="Extraction configuration applied during the run."
    )
    frames: list[ExtractedFrameArtifact] = Field(
        default_factory=list,
        description="Deterministic list of extracted frame artifacts.",
    )
    total_frames_extracted: int = Field(
        ge=0,
        description="Total count of frames successfully extracted and verified.",
    )
    processing_time_seconds: float = Field(
        ge=0.0,
        description="Wall-clock time in seconds taken for the extraction operation.",
    )
    processed_at_utc: datetime = Field(
        description="UTC timestamp when extraction completed.",
    )
    tool_metadata: dict[str, str] = Field(
        default_factory=dict,
        description="Version and execution metadata for FFmpeg and FFprobe.",
    )
    provenance_note: str = Field(
        default=(
            "Derived frames are secondary artifacts; original video evidence remains untouched. "
            "Frame hashes and PTS offsets are cryptographically bound to the source video SHA-256."
        ),
        description="Forensic provenance and chain-of-custody note.",
    )
