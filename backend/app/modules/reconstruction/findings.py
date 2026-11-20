"""Provenance, limitations and evidence gaps of a reconstruction run (Member 4 — Phase 3F).

Every finding (timeline event, spatial observation, movement segment, contradiction lead,
relationship) gets one ``FindingProvenance`` record. It lists the evidence items and their
SHA-256 hashes, the events, the source records and the frame artifacts that support the
finding. ``is_observed`` separates extracted facts from derived inferences.

Frame artifacts support a finding only through temporal alignment: same source evidence and an
overlapping UTC window. That is provenance, not semantic confirmation of what the frame shows.
"""

from collections.abc import Iterable, Mapping, Sequence
from enum import StrEnum

from pydantic import BaseModel, ConfigDict, Field

from app.modules.reconstruction.adapters.event_adapter import ReconstructedEvent
from app.modules.reconstruction.adapters.evidence_adapter import ClockCalibrationStatus
from app.modules.reconstruction.adapters.frame_handoff import FrameArtifactReference
from app.modules.reconstruction.temporal import intervals_overlap
from app.schemas.reconstruction import MovementSegment


class FindingType(StrEnum):
    TIMELINE_EVENT = "TIMELINE_EVENT"
    SPATIAL_OBSERVATION = "SPATIAL_OBSERVATION"
    MOVEMENT_SEGMENT = "MOVEMENT_SEGMENT"
    CONTRADICTION_LEAD = "CONTRADICTION_LEAD"
    RELATIONSHIP = "RELATIONSHIP"


_FINDING_ORDER = {t: i for i, t in enumerate(FindingType)}


class FindingProvenance(BaseModel):
    """Which evidence, records and frames support one finding."""

    model_config = ConfigDict(frozen=True)

    finding_type: FindingType
    finding_id: str
    is_observed: bool = Field(
        description="True for extracted facts; false for derived inferences (segments, leads, relationships)."
    )
    evidence_ids: list[str]
    evidence_sha256: dict[str, str]
    event_ids: list[str] = Field(default_factory=list)
    source_record_ids: list[str] = Field(default_factory=list)
    frame_artifact_ids: list[str] = Field(
        default_factory=list,
        description="Frames temporally aligned with the supporting events (not semantic confirmation).",
    )


class Limitation(BaseModel):
    model_config = ConfigDict(frozen=True)

    code: str
    message: str
    subject_ids: list[str] = Field(default_factory=list)


class EvidenceGapType(StrEnum):
    EVIDENCE_EXCLUDED = "EVIDENCE_EXCLUDED"
    EVIDENCE_WITHOUT_EVENTS = "EVIDENCE_WITHOUT_EVENTS"
    CANDIDATE_EXCLUDED = "CANDIDATE_EXCLUDED"
    FRAME_EXCLUDED = "FRAME_EXCLUDED"
    EVENT_WITHOUT_COORDINATES = "EVENT_WITHOUT_COORDINATES"
    UNRESOLVED_ENTITY_REFERENCE = "UNRESOLVED_ENTITY_REFERENCE"
    SPATIAL_CONTINUITY_GAP = "SPATIAL_CONTINUITY_GAP"
    VIDEO_WITHOUT_FRAMES = "VIDEO_WITHOUT_FRAMES"


class EvidenceGap(BaseModel):
    model_config = ConfigDict(frozen=True)

    gap_type: EvidenceGapType
    subject_type: str
    subject_id: str
    description: str
    reason_code: str | None = None


def link_frames_to_events(
    events: Sequence[ReconstructedEvent], frames: Sequence[FrameArtifactReference]
) -> dict[str, list[str]]:
    """event_id → sorted artifact IDs of anchored frames from the same evidence whose window overlaps."""
    links: dict[str, list[str]] = {e.event_id: [] for e in events}
    for event in events:
        if event.uncertainty_window_start is None or event.uncertainty_window_end is None:
            continue
        for frame in frames:
            if frame.source_evidence_id != event.source_evidence_id or frame.time_window is None:
                continue
            if intervals_overlap(
                event.uncertainty_window_start,
                event.uncertainty_window_end,
                frame.time_window.window_start_utc,
                frame.time_window.window_end_utc,
            ):
                links[event.event_id].append(frame.artifact_id)
        links[event.event_id].sort()
    return links


class ProvenanceBuilder:
    """Builds deterministic provenance records from event-level facts."""

    def __init__(
        self,
        events: Sequence[ReconstructedEvent],
        evidence_sha256: Mapping[str, str],
        frame_links: Mapping[str, Sequence[str]],
    ) -> None:
        self._events = {e.event_id: e for e in events}
        self._sha = dict(evidence_sha256)
        self._frames = frame_links
        self._records: list[FindingProvenance] = []

    def add(
        self,
        finding_type: FindingType,
        finding_id: str,
        event_ids: Iterable[str],
        *,
        is_observed: bool,
    ) -> None:
        ids = sorted(set(event_ids))
        events = [self._events[i] for i in ids if i in self._events]
        evidence_ids = sorted({e.source_evidence_id for e in events})
        self._records.append(
            FindingProvenance(
                finding_type=finding_type,
                finding_id=finding_id,
                is_observed=is_observed,
                evidence_ids=evidence_ids,
                evidence_sha256={eid: self._sha[eid] for eid in evidence_ids if eid in self._sha},
                event_ids=ids,
                source_record_ids=sorted({e.source_record_id for e in events if e.source_record_id}),
                frame_artifact_ids=sorted({f for i in ids for f in self._frames.get(i, [])}),
            )
        )

    def build(self) -> list[FindingProvenance]:
        return sorted(self._records, key=lambda r: (_FINDING_ORDER[r.finding_type], r.finding_id))


def sorted_segments(segments: Iterable[MovementSegment]) -> list[MovementSegment]:
    return sorted(
        segments,
        key=lambda s: (s.entity_id, s.origin_time_window.window_start_utc, s.segment_id),
    )


def build_limitations(
    *,
    events: Sequence[ReconstructedEvent],
    segments: Sequence[MovementSegment],
    calibration_by_evidence: Mapping[str, ClockCalibrationStatus],
    excluded_ids: Sequence[str],
    unresolved_event_ids: Sequence[str],
    overlaps_not_evaluated: int,
    frames: Sequence[FrameArtifactReference],
    has_video_evidence: bool,
    speed_threshold_kmh: float,
) -> list[Limitation]:
    out: list[Limitation] = []

    def add(code: str, message: str, subjects: Iterable[str] = ()) -> None:
        out.append(Limitation(code=code, message=message, subject_ids=sorted(set(subjects))))

    # Sparse-evidence limitations: state what could not be analysed instead of guessing.
    sources = {e.source_evidence_id for e in events}
    if not events:
        add(
            "NO_ACCEPTED_EVENTS",
            "No event candidate was accepted. No timeline, spatial or contradiction analysis was "
            "possible, and no events were inferred.",
        )
    elif len(sources) == 1:
        add(
            "SINGLE_SOURCE_NO_CORROBORATION",
            "All accepted events come from one evidence source. No cross-source corroboration or "
            "cross-source contradiction analysis was possible.",
            sources,
        )
    if events and all(e.coordinates is None for e in events):
        add(
            "NO_SPATIAL_DATA",
            "No accepted event carries coordinates. No spatial placement, movement or travel "
            "feasibility analysis was performed, and no locations were inferred.",
        )
    elif events and not segments:
        add(
            "NO_MOVEMENT_SEGMENTS",
            "No entity has two strictly ordered events with coordinates, so no movement segment "
            "was built and travel feasibility was not evaluated.",
        )
    if segments:
        add(
            "REFERENCE_SPEED_THRESHOLD_ASSUMPTION",
            f"Travel feasibility uses a configured reference speed of {speed_threshold_kmh:g} km/h "
            "and straight-line (haversine) distance. Both are analytical assumptions, not "
            "measurements of the actual route or vehicle.",
            [s.segment_id for s in segments],
        )
    estimated = [e.event_id for e in events if e.is_estimated_window_assumption]
    if estimated:
        add(
            "ESTIMATED_WINDOW_ASSUMPTION",
            "These events have ESTIMATED precision; their windows use a configured half-width, "
            "not a measured bound.",
            estimated,
        )
    uncalibrated = [k for k, v in calibration_by_evidence.items() if v == ClockCalibrationStatus.NOT_DECLARED_UNCALIBRATED]
    if uncalibrated:
        add(
            "UNCALIBRATED_CLOCKS",
            "No clock offset was declared for these sources. Their timestamps are used as "
            "recorded; the clocks are not known to be accurate.",
            uncalibrated,
        )
    declared = [
        k for k, v in calibration_by_evidence.items()
        if v == ClockCalibrationStatus.DECLARED_NOT_INDEPENDENTLY_VERIFIED
    ]
    if declared:
        add(
            "DECLARED_CLOCK_OFFSETS_NOT_VERIFIED",
            "Declared clock offsets were applied once, as supplied at ingestion. Member 4 does not "
            "independently verify them.",
            declared,
        )
    if excluded_ids:
        add(
            "EXCLUDED_INPUTS",
            "Some inputs failed validation and were excluded. Findings that would involve them "
            "(including contradictions) cannot be detected in this run.",
            excluded_ids,
        )
    if unresolved_event_ids:
        add(
            "UNRESOLVED_ENTITY_REFERENCES",
            "Entity references without an exact registration were withheld from trajectories "
            "and contradiction analysis.",
            unresolved_event_ids,
        )
    if overlaps_not_evaluated:
        add(
            "CONCURRENT_WINDOWS_NOT_ORDERED",
            f"{overlaps_not_evaluated} pair(s) of located events had overlapping time windows; "
            "their order is unknown, so travel feasibility was not evaluated for them.",
        )
    if has_video_evidence or frames:
        add(
            "FRAMES_NOT_SEMANTICALLY_ANALYZED",
            "Video was processed by frame extraction with provenance only. No semantic event "
            "recognition was performed; frame links to events are temporal alignment only.",
            [f.artifact_id for f in frames],
        )
    unanchored = [f.artifact_id for f in frames if not f.timestamp_utc_anchored]
    if unanchored:
        add(
            "FRAMES_NOT_UTC_ANCHORED",
            "These frames carry relative PTS offsets only and could not be aligned with events.",
            unanchored,
        )
    return out


def run_failed_limitations() -> list[Limitation]:
    """Limitation recorded on a FAILED run, which produces no reconstruction at all."""
    return [
        Limitation(
            code="RUN_FAILED_NO_RECONSTRUCTION",
            message=(
                "The run failed before reconstruction completed (see ERROR diagnostics). No "
                "timeline, spatial, contradiction or relationship findings were produced."
            ),
        )
    ]
