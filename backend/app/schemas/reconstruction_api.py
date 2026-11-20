"""Request and response schemas of the Member 4 reconstruction API (Phase 3G).

Item-level contents (evidence records, event candidates, entities, frame artifacts) are
validated by the reconstruction pipeline, one item at a time, so that one bad item is reported
and excluded instead of rejecting the whole request. The request schema validates structure:
arrays of JSON objects, size limits, no unknown top-level fields.
"""

from enum import StrEnum
from typing import Annotated, Any

from pydantic import BaseModel, ConfigDict, Field

from app.modules.reconstruction.adapters.diagnostics import ReconstructionDiagnostic
from app.modules.reconstruction.adapters.event_adapter import ReconstructedEvent
from app.modules.reconstruction.adapters.relationship_adapter import (
    SharedRelationshipRecord,
)
from app.modules.reconstruction.findings import EvidenceGap, Limitation
from app.modules.reconstruction.pipeline import (
    ContradictionLeadRecord,
    ExcludedInput,
    PipelineStatus,
    RelationshipExportStatus,
    SpatialObservation,
    VideoProcessingRecord,
)
from app.schemas.reconstruction import (
    MovementSegment,
    SpatialTemporalOverlap,
    TimelineOverlapPair,
)

MAX_ITEMS = 10_000
SAFE_ID_PATTERN = r"^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$"

_EXAMPLE_REQUEST: dict[str, Any] = {
    "evidence": [
        {
            "evidence_id": "ev-cctv-dock",
            "case_id": "case-wh-burglary-001",
            "source_type": "CCTV",
            "original_filename": "dock_cam_01.mp4",
            "file_size_bytes": 32,
            "sha256_hash": "e77a6607dd66c74a6f1bca53e372ce4ba9d88b77466d3ca479ca7fefee338890",
            "storage_uri": "urn:yukti:evidence:ev-cctv-dock:storage",
            "metadata": {
                "source_timezone": "Asia/Kolkata",
                "clock_offset_seconds": -120.0,
                "recording_start_timestamp": "2026-11-10T21:30:00",
                "device_coordinates": {"lat": 28.5355, "lon": 77.271},
            },
        }
    ],
    "event_candidates": [
        {
            "candidate_id": "evt-01-shutter-forced",
            "source_evidence_id": "ev-cctv-dock",
            "source_record_id": "osd:2026-11-10T21:42:00",
            "raw_timestamp": "2026-11-10T21:42:00",
            "timestamp_precision": "EXACT_SECOND",
            "event_type": "FORCED_ENTRY",
            "description": "Rear shutter forced open at loading dock",
            "involved_entity_raw_ids": ["ent-person-ravi"],
            "coordinates": {"lat": 28.5355, "lon": 77.271},
            "duration_seconds": 30.0,
            "extraction_confidence": 0.9,
        }
    ],
    "entities": [
        {
            "entity_id": "ent-person-ravi",
            "case_id": "case-wh-burglary-001",
            "entity_type": "PERSON",
            "canonical_name": "Ravi Kumar",
            "aliases": ["R. Kumar"],
            "source_evidence_ids": ["ev-cctv-dock"],
            "confidence": 1.0,
        }
    ],
    "frame_artifacts": [],
}


class UnifiedExtractionPayload(BaseModel):
    """Member 1 ``UnifiedExtractionResult`` serialization; only ``event_candidates`` is read."""

    model_config = ConfigDict(extra="allow")

    event_candidates: list[dict[str, Any]] = Field(max_length=MAX_ITEMS)


class ReconstructionRunRequest(BaseModel):
    """Inputs of one reconstruction run. ``case_id`` comes from the path."""

    model_config = ConfigDict(extra="forbid", json_schema_extra={"examples": [_EXAMPLE_REQUEST]})

    evidence: list[dict[str, Any]] = Field(
        min_length=1,
        max_length=MAX_ITEMS,
        description=(
            "Shared Evidence records. storage_uri must be the canonical key "
            "data/evidence/raw/{case_id}/{evidence_id}/{evidence_id}.raw or "
            "urn:yukti:evidence:{evidence_id}:storage. metadata may carry the DEC-006 keys "
            "source_timezone, clock_offset_seconds, recording_start_timestamp, device_coordinates."
        ),
    )
    event_candidates: Annotated[list[dict[str, Any]], Field(max_length=MAX_ITEMS)] | UnifiedExtractionPayload = Field(
        description=(
            "Member 1 event candidates: an array (at most 10,000 items), or a "
            "UnifiedExtractionResult object with an event_candidates array. raw_timestamp is "
            "preserved; UTC is derived once by Member 4."
        ),
    )
    entities: list[dict[str, Any]] = Field(default_factory=list, max_length=MAX_ITEMS)
    frame_artifacts: list[dict[str, Any]] = Field(
        default_factory=list,
        max_length=MAX_ITEMS,
        description="Optional, previously extracted frame artifacts. No filesystem paths allowed.",
    )

    def candidates_payload(self) -> list[dict[str, Any]] | dict[str, Any]:
        if isinstance(self.event_candidates, UnifiedExtractionPayload):
            return self.event_candidates.model_dump()
        return self.event_candidates


class ErrorBody(BaseModel):
    code: str
    message: str
    details: Any | None = None


class ErrorEnvelope(BaseModel):
    """Standard error envelope (``.agents/rules/project-standards.md`` §5)."""

    error: ErrorBody


class RunCounts(BaseModel):
    timeline_events: int
    movement_segments: int
    contradiction_leads: int
    relationships: int
    excluded_inputs: int
    diagnostics_by_severity: dict[str, int]


class RunLinks(BaseModel):
    result: str
    timeline: str
    spatial: str
    contradictions: str
    relationships: str
    diagnostics: str
    report: str


class ReportFormat(StrEnum):
    """Formats of GET .../runs/{run_id}/report."""

    JSON = "json"
    MARKDOWN = "markdown"
    HTML = "html"


class ReconstructionRunSummary(BaseModel):
    """Response of POST .../reconstruction/run."""

    run_id: str
    case_id: str
    status: PipelineStatus
    result_sha256: str
    algorithm_version: str
    configuration_version: str
    relationship_export_status: RelationshipExportStatus
    counts: RunCounts
    links: RunLinks


class RunViewBase(BaseModel):
    run_id: str
    case_id: str
    status: PipelineStatus
    result_sha256: str


class TimelineView(RunViewBase):
    timeline_events: list[ReconstructedEvent]
    timeline_overlaps: list[TimelineOverlapPair]


class SpatialView(RunViewBase):
    spatial_observations: list[SpatialObservation]
    movement_segments: list[MovementSegment]
    uncertain_spatial_overlaps: list[SpatialTemporalOverlap]


class ContradictionsView(RunViewBase):
    relationship_export_status: RelationshipExportStatus
    contradiction_leads: list[ContradictionLeadRecord]
    contradiction_overlaps_not_evaluated: list[SpatialTemporalOverlap]


class RelationshipsView(RunViewBase):
    relationship_export_status: RelationshipExportStatus
    relationships: list[SharedRelationshipRecord] = Field(
        description="Canonical relationship output. weight is null when not computed."
    )


class DiagnosticsView(RunViewBase):
    diagnostics: list[ReconstructionDiagnostic]
    limitations: list[Limitation]
    evidence_gaps: list[EvidenceGap]
    excluded_inputs: list[ExcludedInput]
    video: list[VideoProcessingRecord]
