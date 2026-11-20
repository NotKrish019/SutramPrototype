"""End-to-end Reconstruction Pipeline (Member 4 — Phase 3F).

One entry point, ``ReconstructionPipeline.run``, takes a case id, shared Evidence records
(with DEC-006 metadata), Member 1 event candidates (JSON), optional shared Entity records and
optional, previously extracted frame artifacts. It returns a typed ``ReconstructionResult``,
which serializes to a complete JSON snapshot. Nothing is written to any database.

Execution order:
1. Validate inputs: evidence records, canonical storage keys, SHA-256 integrity, the candidate
   payload and each candidate, entities, and supplied frames.
   **Partial failure:** an invalid evidence item, candidate or frame is excluded and reported
   (diagnostics, ``excluded_inputs``, ``evidence_gaps``, ``limitations``). The run continues
   and ends ``COMPLETED_WITH_ERRORS``. Candidates whose evidence was excluded are excluded too.
   Only run-level problems give ``FAILED``: an unreadable candidate payload, an inconsistent
   entity registry, an engine failure, or the double-correction guard tripping.
2. Video: FFmpeg frame extraction with provenance, for usable CCTV evidence only, plus
   validated supplied frames. No semantic event recognition happens, and frames never become
   events.
3. Entity resolution (exact registrations only) → timeline and spatial reconstruction
   (movement segments, travel feasibility). Every UTC value is derived once from
   ``raw_timestamp`` by the temporal engine and cross-checked against the adapter.
4. Contradiction leads → ``CONTRADICTS`` relationships (``is_observed = false``,
   ``weight = null``).
5. Provenance for every finding, limitations, evidence gaps, and ``result_sha256``.

Determinism: identical inputs and configuration give identical run ids, ordering and
``result_sha256``. The stable sort rules are listed in ``docs/api/reconstruction-api.md``.
The only per-run value is ``frame_artifacts[].artifact_relative_path`` for frames extracted in
the run, because derived artifacts are never overwritten. That field is excluded from
``result_sha256``.
"""

import hashlib
import json
import logging
import os
import re
from collections import Counter
from collections.abc import Iterable, Mapping, Sequence
from datetime import datetime
from enum import StrEnum
from pathlib import Path
from typing import Any

from pydantic import BaseModel, ConfigDict, Field

from app.modules.reconstruction.adapters.contracts import (
    APPROVAL_RECORD,
    APPROVED_CONTRACT_DECISIONS,
    ContractDecision,
    missing_decisions,
    normalize_approved_decisions,
)
from app.modules.reconstruction.adapters.diagnostics import (
    DiagnosticSeverity,
    PipelineStage,
    ReconstructionDiagnostic,
    diagnostic_from_error,
    sort_diagnostics,
)
from app.modules.reconstruction.adapters.entity_adapter import (
    EntityReferenceStatus,
    EntityRegistry,
    EventEntityResolution,
    build_entity_registry,
)
from app.modules.reconstruction.adapters.errors import (
    EventCandidatePayloadError,
    ReconstructionAdapterError,
)
from app.modules.reconstruction.adapters.event_adapter import (
    CANDIDATE_FIELDS,
    AdaptedEvent,
    ReconstructedEvent,
    adapt_event_candidate,
    build_reconstructed_event,
    build_temporal_evidence_index,
    parse_event_candidates_payload,
)
from app.modules.reconstruction.adapters.evidence_adapter import (
    TIME_METADATA_DECISIONS,
    ClockCalibrationStatus,
    EvidenceRecordView,
    VerifiedEvidenceFile,
    evidence_to_video_extraction_options,
    parse_evidence_record,
    verify_evidence_file,
)
from app.modules.reconstruction.adapters.frame_artifacts import (
    FrameArtifactInputError,
    adapt_supplied_frame,
)
from app.modules.reconstruction.adapters.frame_handoff import (
    FRAME_EXTRACTION_NOTE,
    ExtractedFrameConsumer,
    FrameArtifactReference,
    FrameHandoff,
)
from app.modules.reconstruction.adapters.relationship_adapter import (
    RELATIONSHIP_DECISIONS,
    SharedRelationshipRecord,
    contradiction_lead_to_relationship,
)
from app.modules.reconstruction.contradictions import (
    detect_contradictions_from_spatial_result,
)
from app.modules.reconstruction.findings import (
    EvidenceGap,
    EvidenceGapType,
    FindingProvenance,
    FindingType,
    Limitation,
    ProvenanceBuilder,
    build_limitations,
    link_frames_to_events,
    run_failed_limitations,
    sorted_segments,
)
from app.modules.reconstruction.spatial import (
    DEFAULT_REFERENCE_SPEED_THRESHOLD_KMH,
    reconstruct_spatial_trajectories,
)
from app.modules.reconstruction.temporal import DEFAULT_ESTIMATED_HALF_WIDTH_SECONDS
from app.modules.reconstruction.video import (
    VideoProcessingError,
    extract_video_keyframes,
)
from app.schemas.reconstruction import (
    ContradictionAnalysisResult,
    ContradictionLead,
    ContradictionLeadType,
    DerivedLeadInference,
    GeoPoint,
    MovementSegment,
    ObservedLeadBasis,
    SpatialReconstructionResult,
    SpatialTemporalOverlap,
    TimelineOverlapPair,
    VideoExtractionOptions,
    VideoExtractionResult,
)

logger = logging.getLogger(__name__)

ALGORITHM_VERSION = "yukti-m4-reconstruction/3F.2"
SNAPSHOT_SCHEMA_VERSION = "yukti-reconstruction-snapshot/3"

INTERPRETATION_NOTICE = (
    "Reconstruction output separates observed source facts from derived analytical inferences. "
    "Contradiction leads are investigative prompts for human review; they do not establish "
    "identity, deception, guilt or intent. Declared clock offsets are applied as supplied and "
    "are not independently verified."
)


class PipelineStatus(StrEnum):
    COMPLETED = "COMPLETED"
    COMPLETED_WITH_WARNINGS = "COMPLETED_WITH_WARNINGS"
    COMPLETED_WITH_ERRORS = "COMPLETED_WITH_ERRORS"
    FAILED = "FAILED"


class VideoStageStatus(StrEnum):
    EXTRACTED = "EXTRACTED"
    FAILED = "FAILED"
    SKIPPED_NO_ARTIFACT_ROOT = "SKIPPED_NO_ARTIFACT_ROOT"


class FrameHandoffStatus(StrEnum):
    SUBMITTED = "SUBMITTED"
    NOT_SUBMITTED_NO_CONSUMER = "NOT_SUBMITTED_NO_CONSUMER"
    NOT_SUBMITTED_NO_FRAMES = "NOT_SUBMITTED_NO_FRAMES"
    FAILED = "FAILED"


class RelationshipExportStatus(StrEnum):
    EXPORTED = "EXPORTED"
    NO_CONTRADICTION_LEADS = "NO_CONTRADICTION_LEADS"
    RELATIONSHIP_EXPORT_BLOCKED = "RELATIONSHIP_EXPORT_BLOCKED"
    NOT_RUN = "NOT_RUN"


class ReconstructionPipelineConfig(BaseModel):
    """Pipeline configuration.

    ``approved_contract_decisions`` defaults to the verified approvals, which are currently none
    (fail closed). Pass decisions explicitly to enable behaviour that depends on them.
    """

    model_config = ConfigDict(frozen=True)

    storage_root: Path = Field(
        description="Root against which canonical evidence storage keys are resolved."
    )
    artifact_root: Path | None = Field(
        default=None,
        description="Derived-artifact directory for video frames; None skips frame extraction.",
    )
    approved_contract_decisions: frozenset[ContractDecision] = APPROVED_CONTRACT_DECISIONS
    speed_threshold_kmh: float = Field(default=DEFAULT_REFERENCE_SPEED_THRESHOLD_KMH, gt=0.0)
    estimated_half_width_seconds: float = Field(
        default=DEFAULT_ESTIMATED_HALF_WIDTH_SECONDS, gt=0.0
    )
    video_extraction_options: VideoExtractionOptions = Field(
        default_factory=lambda: VideoExtractionOptions(create_unique_run_subdir=True)
    )
    verify_all_evidence_integrity: bool = Field(
        default=True,
        description="Verify SHA-256 of every evidence item, not only those whose bytes are processed.",
    )
    explicit_entity_aliases: dict[str, str] = Field(default_factory=dict)

    def public_configuration(self) -> dict[str, Any]:
        """Configuration without host paths, safe to embed in a snapshot."""
        data = self.model_dump(mode="json", exclude={"storage_root", "artifact_root"})
        data["approved_contract_decisions"] = sorted(data["approved_contract_decisions"])
        data["artifact_root_configured"] = self.artifact_root is not None
        return data

    def configuration_version(self) -> str:
        encoded = json.dumps(self.public_configuration(), sort_keys=True, separators=(",", ":"))
        return f"cfg-sha256-{hashlib.sha256(encoded.encode('utf-8')).hexdigest()[:32]}"


class EvidenceValidationRecord(BaseModel):
    model_config = ConfigDict(frozen=True)

    evidence_id: str
    source_type: str
    sha256_hash: str
    integrity_verified: bool
    usable: bool = Field(description="False when the item was excluded from reconstruction.")
    storage_reference: str
    storage_key: str
    time_metadata_applied: bool
    source_timezone: str | None = None
    clock_offset_seconds: float | None = None
    clock_calibration: ClockCalibrationStatus
    recording_start_timestamp: str | None = None
    device_coordinates: GeoPoint | None = None
    ignored_time_metadata_keys: list[str] = Field(default_factory=list)


class ExcludedInput(BaseModel):
    """An input item that failed validation and was left out of this run."""

    model_config = ConfigDict(frozen=True)

    subject_type: str = Field(description="EVIDENCE, EVENT_CANDIDATE or FRAME_ARTIFACT.")
    subject_id: str
    reason_code: str


class VideoProcessingRecord(BaseModel):
    model_config = ConfigDict(frozen=True)

    evidence_id: str
    status: VideoStageStatus
    processing_kind: str = "FRAME_EXTRACTION_WITH_PROVENANCE"
    processing_note: str = FRAME_EXTRACTION_NOTE
    frame_handoff_status: FrameHandoffStatus | None = None
    frames_extracted: int = 0
    timestamps_utc_anchored: bool = False
    source_video_sha256_before: str | None = None
    source_video_sha256_after: str | None = None
    source_integrity_verified: bool | None = None
    frame_artifact_ids: list[str] = Field(default_factory=list)


class SpatialObservation(BaseModel):
    """An entity placed at the coordinates of an event, exactly as extracted (no inference)."""

    model_config = ConfigDict(frozen=True)

    entity_id: str
    event_id: str
    source_evidence_id: str
    coordinates: GeoPoint
    timestamp_utc: datetime
    uncertainty_window_start: datetime | None = None
    uncertainty_window_end: datetime | None = None
    is_observed: bool = True
    basis: str = "EVENT_CANDIDATE_COORDINATES"


class ContradictionLeadRecord(BaseModel):
    """A contradiction lead, kept separate from its canonical Relationship record."""

    model_config = ConfigDict(frozen=True)

    lead_id: str
    case_id: str
    entity_id: str
    lead_type: ContradictionLeadType
    origin_event_id: str
    destination_event_id: str
    evidence_basis: list[str]
    observed_facts: ObservedLeadBasis
    derived_inference: DerivedLeadInference
    explanation: str
    limitations: list[str]
    supporting_frame_artifact_ids: list[str] = Field(default_factory=list)
    relationship_id: str | None = Field(
        default=None, description="Canonical Relationship record exported for this lead, if any."
    )

    @classmethod
    def from_lead(
        cls, lead: ContradictionLead, relationship_id: str | None, frame_ids: Sequence[str]
    ) -> "ContradictionLeadRecord":
        return cls(
            **lead.model_dump(exclude={"relationship"}),
            supporting_frame_artifact_ids=list(frame_ids),
            relationship_id=relationship_id,
        )


class ReconstructionResult(BaseModel):
    """Complete, persistence-agnostic output of one reconstruction run."""

    model_config = ConfigDict(frozen=True)

    snapshot_schema_version: str = SNAPSHOT_SCHEMA_VERSION
    algorithm_version: str = ALGORITHM_VERSION
    configuration_version: str
    configuration: dict[str, Any]
    run_id: str
    case_id: str
    status: PipelineStatus
    result_sha256: str = Field(
        default="",
        description="SHA-256 of the canonical snapshot, excluding this field and frame artifact paths.",
    )
    approved_contract_decisions: list[str] = Field(
        description="Decisions enabled for this run (explicitly configured or verified approvals)."
    )
    withheld_contract_decisions: list[str]
    unverified_contract_decisions: list[str] = Field(
        default_factory=list,
        description=(
            "Enabled decisions without a verified team sign-off (proposal only). Results that "
            "depend on them are conditional on the proposal being approved."
        ),
    )
    approval_record: str = APPROVAL_RECORD
    evidence: list[EvidenceValidationRecord] = Field(default_factory=list)
    excluded_inputs: list[ExcludedInput] = Field(default_factory=list)
    timeline_events: list[ReconstructedEvent] = Field(default_factory=list)
    timeline_overlaps: list[TimelineOverlapPair] = Field(default_factory=list)
    spatial_observations: list[SpatialObservation] = Field(default_factory=list)
    movement_segments: list[MovementSegment] = Field(default_factory=list)
    spatial: SpatialReconstructionResult | None = None
    entity_resolution: list[EventEntityResolution] = Field(default_factory=list)
    relationship_export_status: RelationshipExportStatus = RelationshipExportStatus.NOT_RUN
    relationships: list[SharedRelationshipRecord] = Field(
        default_factory=list, description="Canonical relationship output."
    )
    contradiction_leads: list[ContradictionLeadRecord] = Field(default_factory=list)
    contradiction_overlaps_not_evaluated: list[SpatialTemporalOverlap] = Field(
        default_factory=list
    )
    video: list[VideoProcessingRecord] = Field(default_factory=list)
    frame_artifacts: list[FrameArtifactReference] = Field(default_factory=list)
    finding_provenance: list[FindingProvenance] = Field(default_factory=list)
    limitations: list[Limitation] = Field(default_factory=list)
    evidence_gaps: list[EvidenceGap] = Field(default_factory=list)
    diagnostics: list[ReconstructionDiagnostic] = Field(default_factory=list)
    interpretation_notice: str = INTERPRETATION_NOTICE

    def to_snapshot(self) -> dict[str, Any]:
        """Complete JSON-compatible snapshot of this run."""
        return self.model_dump(mode="json")

    def to_snapshot_json(self) -> str:
        return json.dumps(self.to_snapshot(), sort_keys=True, ensure_ascii=False, indent=2)

    def to_event_candidates(self) -> list[dict[str, Any]]:
        """The original candidate inputs of this run, without any derived field."""
        return event_candidates_from_snapshot(self.to_snapshot())

    def compute_result_sha256(self) -> str:
        snapshot = self.to_snapshot()
        snapshot.pop("result_sha256", None)
        for frame in snapshot.get("frame_artifacts", []):
            if frame.get("origin") == "EXTRACTED_IN_RUN":
                frame.pop("artifact_relative_path", None)
        encoded = json.dumps(snapshot, sort_keys=True, separators=(",", ":"), ensure_ascii=False)
        return hashlib.sha256(encoded.encode("utf-8")).hexdigest()


def event_candidates_from_snapshot(snapshot: Mapping[str, Any]) -> list[dict[str, Any]]:
    """Extract the candidate-contract fields of a snapshot's timeline events.

    Re-running the pipeline on these candidates recomputes UTC values from ``raw_timestamp``,
    so no correction is applied twice.
    """
    events = snapshot.get("timeline_events")
    if not isinstance(events, list):
        raise EventCandidatePayloadError("Snapshot has no 'timeline_events' array.")
    candidates: list[dict[str, Any]] = []
    for event in events:
        if not isinstance(event, Mapping):
            raise EventCandidatePayloadError("Snapshot timeline event is not an object.")
        candidate = {name: event.get(name) for name in CANDIDATE_FIELDS}
        candidate["timestamp_precision"] = event.get(
            "raw_timestamp_precision", event.get("timestamp_precision")
        )
        preserved = event.get("source_fields_preserved") or {}
        if not isinstance(preserved, Mapping):
            raise EventCandidatePayloadError("Snapshot source_fields_preserved is not an object.")
        for name, value in preserved.items():
            if name in candidate:
                raise EventCandidatePayloadError(
                    f"Snapshot source_fields_preserved key '{name}' collides with a contract field."
                )
            candidate[name] = value
        candidates.append(candidate)
    return candidates


def _artifact_segment(value: str) -> str:
    """Filesystem-safe, collision-resistant directory segment for an identifier."""
    readable = re.sub(r"[^A-Za-z0-9_-]", "_", value)[:48] or "id"
    digest = hashlib.sha256(value.encode("utf-8")).hexdigest()[:12]
    return f"{readable}-{digest}"


def _require_sequence(value: Any, name: str) -> Sequence[Any]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise TypeError(f"{name} must be a sequence of {name} records.")
    return value


def _json_default(value: Any) -> Any:
    if isinstance(value, BaseModel):
        return value.model_dump(mode="json")
    return str(value)


def _input_digest(items: Any) -> list[str]:
    if isinstance(items, (str, bytes, bytearray)):
        text = items.decode("utf-8", "replace") if isinstance(items, (bytes, bytearray)) else items
        return [hashlib.sha256(text.encode("utf-8")).hexdigest()]
    if not isinstance(items, Sequence):
        return [json.dumps(items, sort_keys=True, default=_json_default)]
    return sorted(json.dumps(item, sort_keys=True, default=_json_default) for item in items)


def _raw_id(record: Any, field: str) -> str | None:
    value = record.get(field) if isinstance(record, Mapping) else getattr(record, field, None)
    return value.strip() if isinstance(value, str) and value.strip() else None


class _RunState:
    """Mutable working state of one run (never shared between runs)."""

    def __init__(self) -> None:
        self.diags: list[ReconstructionDiagnostic] = []
        self.excluded: list[ExcludedInput] = []
        self.run_level_failure = False

    def exclude(self, subject_type: str, subject_id: str, reason_code: str) -> None:
        self.excluded.append(
            ExcludedInput(subject_type=subject_type, subject_id=subject_id, reason_code=reason_code)
        )


class ReconstructionPipeline:
    """Orchestrates adapters and engines for one case. Holds no per-run mutable state."""

    def __init__(
        self,
        config: ReconstructionPipelineConfig,
        *,
        frame_consumer: ExtractedFrameConsumer | None = None,
    ) -> None:
        self._config = config
        self._approved = normalize_approved_decisions(config.approved_contract_decisions)
        self._frame_consumer = frame_consumer
        self._configuration_version = config.configuration_version()
        self._path_replacements = self._build_path_replacements()

    @property
    def configuration_version(self) -> str:
        return self._configuration_version

    def _build_path_replacements(self) -> list[tuple[str, str]]:
        pairs: list[tuple[str, str]] = []
        for root, label in (
            (self._config.storage_root, "<storage_root>"),
            (self._config.artifact_root, "<artifact_root>"),
        ):
            if root is None:
                continue
            for variant in {os.path.realpath(root), os.path.abspath(root)}:
                pairs.append((variant, label))
        return sorted(pairs, key=lambda p: len(p[0]), reverse=True)

    def _scrub(self, text: str) -> str:
        for path, label in self._path_replacements:
            text = text.replace(path, label)
        return text

    def compute_run_id(
        self,
        case_id: str,
        evidence: Any,
        event_candidates: Any,
        entities: Any,
        frame_artifacts: Any,
    ) -> str:
        """Deterministic run id: digest of algorithm, configuration and order-normalized inputs."""
        material = json.dumps(
            {
                "algorithm_version": ALGORITHM_VERSION,
                "configuration_version": self._configuration_version,
                "case_id": case_id,
                "evidence": _input_digest(evidence),
                "event_candidates": _input_digest(event_candidates),
                "entities": _input_digest(entities),
                "frame_artifacts": _input_digest(frame_artifacts),
            },
            sort_keys=True,
        )
        return f"run-{hashlib.sha256(material.encode('utf-8')).hexdigest()[:32]}"

    def run(
        self,
        *,
        case_id: str,
        evidence: Sequence[Mapping[str, Any] | BaseModel | Any],
        event_candidates: Sequence[Mapping[str, Any]] | Mapping[str, Any] | str | bytes,
        entities: Sequence[Mapping[str, Any] | BaseModel | Any] = (),
        frame_artifacts: Sequence[Mapping[str, Any]] | None = None,
        run_id: str | None = None,
    ) -> ReconstructionResult:
        """Run one end-to-end reconstruction.

        ``event_candidates`` may be a list of candidate objects, JSON text of such a list, or
        the ``UnifiedExtractionResult`` object or text. ``frame_artifacts`` are optional,
        previously extracted frames. ``run_id`` defaults to ``compute_run_id``.
        """
        if not isinstance(case_id, str) or not case_id.strip():
            raise ValueError("case_id must be a non-empty string.")
        case_id = case_id.strip()
        evidence_in = _require_sequence(evidence, "Evidence")
        entities_in = _require_sequence(entities, "Entity")
        frames_in = _require_sequence(frame_artifacts if frame_artifacts is not None else [], "FrameArtifact")
        if run_id is not None and (not isinstance(run_id, str) or not run_id.strip()):
            raise ValueError("run_id must be a non-empty string when provided.")
        resolved_run_id = (
            run_id.strip()
            if run_id is not None
            else self.compute_run_id(case_id, evidence_in, event_candidates, entities_in, frames_in)
        )

        state = _RunState()
        try:
            candidates_in = parse_event_candidates_payload(event_candidates)
        except EventCandidatePayloadError as exc:
            state.diags.append(diagnostic_from_error(exc, stage=PipelineStage.INPUT, subject_type=None))
            state.run_level_failure = True
            candidates_in = []

        views, verified, excluded_evidence = self._validate_evidence(case_id, evidence_in, state)
        usable = {eid: v for eid, v in views.items() if eid not in excluded_evidence}
        adapted = self._adapt_candidates(case_id, candidates_in, usable, excluded_evidence, state)
        registry = self._build_registry(case_id, entities_in, state)
        supplied_frames = self._adapt_frames(case_id, frames_in, usable, state)
        evidence_records = self._evidence_records(views, verified, excluded_evidence)

        def finish(status: PipelineStatus, **fields: Any) -> ReconstructionResult:
            if status == PipelineStatus.FAILED:
                fields.setdefault("limitations", run_failed_limitations())
            return self._result(case_id, resolved_run_id, status, state, evidence=evidence_records, **fields)

        if registry is None or state.run_level_failure:
            state.run_level_failure = True
            return finish(
                PipelineStatus.FAILED,
                evidence_gaps=self._gaps(state, usable, [], [], [], []),
            )

        video_records, extracted_frames = self._process_video(case_id, usable, verified, state)
        frames = self._merge_frames(extracted_frames, supplied_frames, state)
        resolutions = self._resolve_entities(adapted, registry, state)
        resolution_by_event = {r.event_id: r for r in resolutions}

        engine_events = [
            event.to_engine_record(resolution_by_event[event.event_id].canonical_entity_ids)
            for event in adapted
        ]
        try:
            spatial = reconstruct_spatial_trajectories(
                engine_events,
                evidence_by_id=build_temporal_evidence_index(usable),
                case_id=case_id,
                speed_threshold_kmh=self._config.speed_threshold_kmh,
                estimated_half_width_seconds=self._config.estimated_half_width_seconds,
            )
        except ValueError as exc:
            state.diags.append(
                ReconstructionDiagnostic(
                    severity=DiagnosticSeverity.ERROR,
                    stage=PipelineStage.SPATIAL,
                    code="SPATIAL_RECONSTRUCTION_FAILED",
                    message=str(exc),
                )
            )
            state.run_level_failure = True
            return finish(PipelineStatus.FAILED, video=video_records, entity_resolution=resolutions)

        timeline_events = self._timeline_events(adapted, spatial, resolution_by_event, usable, state)
        if any(d.code == "TIME_CORRECTION_MISMATCH" for d in state.diags):
            state.run_level_failure = True
            return finish(PipelineStatus.FAILED, video=video_records, entity_resolution=resolutions)

        frame_links = link_frames_to_events(timeline_events, frames)
        timeline_events = [
            e.model_copy(update={"supporting_frame_artifact_ids": frame_links[e.event_id]})
            for e in timeline_events
        ]
        self._clock_diagnostics(usable, timeline_events, state)

        contradictions = detect_contradictions_from_spatial_result(spatial)
        provenance = {
            e.event_id: {
                "candidate_id": e.candidate_id,
                "source_record_id": e.source_record_id,
                "evidence_sha256": e.evidence_sha256,
                "raw_timestamp": e.raw_timestamp,
                "timestamp_utc": e.timestamp_utc.isoformat(),
                "frame_artifact_ids": list(frame_links[e.event_id]),
            }
            for e in timeline_events
        }
        export_status, relationships, rel_by_lead = self._export_relationships(
            contradictions, provenance, state
        )
        leads = sorted(
            contradictions.leads,
            key=lambda lead: (lead.observed_facts.origin_time_window.nominal_utc, lead.lead_id),
        )
        lead_records = [
            ContradictionLeadRecord.from_lead(
                lead,
                rel_by_lead.get(lead.lead_id),
                sorted(
                    set(frame_links[lead.origin_event_id]) | set(frame_links[lead.destination_event_id])
                ),
            )
            for lead in leads
        ]
        segments = sorted_segments(
            seg for traj in spatial.trajectories_by_entity.values() for seg in traj.segments
        )
        observations = self._spatial_observations(spatial, timeline_events)

        sha_by_evidence = {eid: v.sha256_hash for eid, v in usable.items()}
        builder = ProvenanceBuilder(timeline_events, sha_by_evidence, frame_links)
        for event in timeline_events:
            builder.add(FindingType.TIMELINE_EVENT, event.event_id, [event.event_id], is_observed=True)
        for obs in observations:
            builder.add(
                FindingType.SPATIAL_OBSERVATION,
                f"obs:{obs.entity_id}:{obs.event_id}",
                [obs.event_id],
                is_observed=True,
            )
        for seg in segments:
            builder.add(
                FindingType.MOVEMENT_SEGMENT,
                seg.segment_id,
                [seg.origin_event_id, seg.destination_event_id],
                is_observed=False,
            )
        for lead in leads:
            builder.add(
                FindingType.CONTRADICTION_LEAD,
                lead.lead_id,
                [lead.origin_event_id, lead.destination_event_id],
                is_observed=False,
            )
        for rel in relationships:
            builder.add(
                FindingType.RELATIONSHIP,
                rel.relationship_id,
                [rel.source_id, rel.target_id],
                is_observed=False,
            )

        unresolved_events = [r.event_id for r in resolutions if r.unresolved_references]
        limitations = build_limitations(
            events=timeline_events,
            segments=segments,
            calibration_by_evidence={
                eid: v.time_settings.clock_calibration for eid, v in sorted(usable.items())
            },
            excluded_ids=[x.subject_id for x in state.excluded],
            unresolved_event_ids=unresolved_events,
            overlaps_not_evaluated=len(contradictions.unresolved_overlaps_skipped),
            frames=frames,
            has_video_evidence=any(v.is_video_evidence for v in usable.values()),
            speed_threshold_kmh=self._config.speed_threshold_kmh,
        )

        return finish(
            self._status(state.diags),
            timeline_events=timeline_events,
            timeline_overlaps=list(spatial.timeline.overlaps),
            spatial_observations=observations,
            movement_segments=segments,
            spatial=spatial,
            entity_resolution=resolutions,
            relationship_export_status=export_status,
            relationships=relationships,
            contradiction_leads=lead_records,
            contradiction_overlaps_not_evaluated=list(contradictions.unresolved_overlaps_skipped),
            video=video_records,
            frame_artifacts=frames,
            finding_provenance=builder.build(),
            limitations=limitations,
            evidence_gaps=self._gaps(state, usable, timeline_events, resolutions, segments, frames),
        )

    # --- Stage helpers -------------------------------------------------------------------

    def _validate_evidence(
        self,
        case_id: str,
        records: Sequence[Any],
        state: _RunState,
    ) -> tuple[dict[str, EvidenceRecordView], dict[str, VerifiedEvidenceFile], dict[str, str]]:
        """Return (views, verified, excluded evidence id → reason code). Never raises."""
        counts = Counter(_raw_id(r, "evidence_id") for r in records)
        views: dict[str, EvidenceRecordView] = {}
        verified: dict[str, VerifiedEvidenceFile] = {}
        excluded: dict[str, str] = {}

        for idx, record in enumerate(records):
            raw_id = _raw_id(record, "evidence_id")
            if raw_id is not None and counts[raw_id] > 1:
                if raw_id not in excluded:
                    excluded[raw_id] = "DUPLICATE_EVIDENCE_ID"
                    state.diags.append(
                        ReconstructionDiagnostic(
                            severity=DiagnosticSeverity.ERROR,
                            stage=PipelineStage.EVIDENCE,
                            code="DUPLICATE_EVIDENCE_ID",
                            message=(
                                f"evidence_id appears {counts[raw_id]} times; every record with "
                                "this id was excluded."
                            ),
                            subject_type="EVIDENCE",
                            subject_id=raw_id,
                        )
                    )
                continue
            try:
                view = parse_evidence_record(record, case_id=case_id, approved_decisions=self._approved)
            except ReconstructionAdapterError as exc:
                subject = raw_id or f"<index_{idx}>"
                excluded[subject] = exc.code
                state.diags.append(
                    diagnostic_from_error(
                        exc, stage=PipelineStage.EVIDENCE, subject_type="EVIDENCE", fallback_subject_id=subject
                    )
                )
                continue
            views[view.evidence_id] = view
            state.diags.extend(self._time_settings_diagnostics(view))

            if self._config.verify_all_evidence_integrity or view.is_video_evidence:
                try:
                    verified[view.evidence_id] = verify_evidence_file(view, self._config.storage_root)
                except ReconstructionAdapterError as exc:
                    excluded[view.evidence_id] = exc.code
                    state.diags.append(
                        diagnostic_from_error(
                            exc,
                            stage=PipelineStage.EVIDENCE,
                            subject_type="EVIDENCE",
                            fallback_subject_id=view.evidence_id,
                        )
                    )

        for eid in sorted(excluded):
            state.exclude("EVIDENCE", eid, excluded[eid])
        return views, verified, excluded

    def _time_settings_diagnostics(self, view: EvidenceRecordView) -> list[ReconstructionDiagnostic]:
        settings = view.time_settings
        out: list[ReconstructionDiagnostic] = []

        def add(severity: DiagnosticSeverity, code: str, message: str, pending: list[str]) -> None:
            out.append(
                ReconstructionDiagnostic(
                    severity=severity,
                    stage=PipelineStage.EVIDENCE,
                    code=code,
                    message=message,
                    subject_type="EVIDENCE",
                    subject_id=view.evidence_id,
                    pending_contract_decisions=pending,
                )
            )

        if not settings.applied_from_metadata:
            if settings.ignored_metadata_keys:
                add(
                    DiagnosticSeverity.WARNING,
                    "TIME_METADATA_NOT_APPLIED",
                    f"Evidence metadata keys {settings.ignored_metadata_keys} were not applied "
                    "because required contract decisions are withheld in this run.",
                    [d.value for d in missing_decisions(self._approved, *TIME_METADATA_DECISIONS)],
                )
            return out
        if settings.ingestion_default_keys_ignored:
            add(
                DiagnosticSeverity.WARNING,
                "CLOCK_METADATA_INGESTION_DEFAULT_IGNORED",
                f"Evidence metadata keys {settings.ingestion_default_keys_ignored} hold the values "
                "that Member 2 ingestion writes when nothing was declared (source_timezone 'UTC', "
                "clock_offset_seconds 0.0). They were treated as unknown, not as a declared "
                "calibration. Naive timestamps from this evidence need a declared source_timezone.",
                [],
            )
        if settings.clock_calibration == ClockCalibrationStatus.NOT_DECLARED_UNCALIBRATED:
            add(
                DiagnosticSeverity.INFO,
                "CLOCK_OFFSET_NOT_DECLARED",
                "No clock_offset_seconds declared. No correction was applied, and the source "
                "clock is reported as uncalibrated, not as accurate.",
                [],
            )
        if view.is_video_evidence and settings.recording_start_timestamp is not None and (
            not settings.recording_start_anchorable
        ):
            add(
                DiagnosticSeverity.WARNING,
                "FRAME_UTC_ANCHORING_WITHHELD",
                "recording_start_timestamp has no UTC offset and no source_timezone is declared; "
                "frames keep relative PTS offsets only.",
                [],
            )
        return out

    def _adapt_candidates(
        self,
        case_id: str,
        records: Sequence[Any],
        usable: Mapping[str, EvidenceRecordView],
        excluded_evidence: Mapping[str, str],
        state: _RunState,
    ) -> list[AdaptedEvent]:
        counts = Counter(_raw_id(r, "candidate_id") for r in records if isinstance(r, Mapping))
        adapted: dict[str, AdaptedEvent] = {}
        excluded: dict[str, str] = {}
        for idx, record in enumerate(records):
            raw_id = _raw_id(record, "candidate_id") if isinstance(record, Mapping) else None
            subject = raw_id or f"<index_{idx}>"
            if raw_id is not None and counts[raw_id] > 1:
                if raw_id not in excluded:
                    excluded[raw_id] = "DUPLICATE_CANDIDATE_ID"
                    state.diags.append(
                        ReconstructionDiagnostic(
                            severity=DiagnosticSeverity.ERROR,
                            stage=PipelineStage.EVENTS,
                            code="DUPLICATE_CANDIDATE_ID",
                            message=(
                                f"candidate_id appears {counts[raw_id]} times; every candidate "
                                "with this id was excluded."
                            ),
                            subject_type="EVENT_CANDIDATE",
                            subject_id=raw_id,
                        )
                    )
                continue
            source = _raw_id(record, "source_evidence_id") if isinstance(record, Mapping) else None
            if source is not None and source in excluded_evidence:
                excluded[subject] = "SOURCE_EVIDENCE_EXCLUDED"
                state.diags.append(
                    ReconstructionDiagnostic(
                        severity=DiagnosticSeverity.WARNING,
                        stage=PipelineStage.EVENTS,
                        code="SOURCE_EVIDENCE_EXCLUDED",
                        message=(
                            f"Candidate excluded because its source evidence '{source}' was "
                            f"excluded ({excluded_evidence[source]})."
                        ),
                        subject_type="EVENT_CANDIDATE",
                        subject_id=subject,
                    )
                )
                continue
            try:
                adaptation = adapt_event_candidate(
                    record,
                    usable,
                    case_id=case_id,
                    approved_decisions=self._approved,
                    estimated_half_width_seconds=self._config.estimated_half_width_seconds,
                )
            except ReconstructionAdapterError as exc:
                excluded[exc.subject_id or subject] = exc.code
                state.diags.append(
                    diagnostic_from_error(
                        exc, stage=PipelineStage.EVENTS, subject_type="EVENT_CANDIDATE", fallback_subject_id=subject
                    )
                )
                continue
            adapted[adaptation.event.event_id] = adaptation.event
            state.diags.extend(adaptation.diagnostics)
        for cid in sorted(excluded):
            state.exclude("EVENT_CANDIDATE", cid, excluded[cid])
        return [adapted[eid] for eid in sorted(adapted)]

    def _build_registry(
        self, case_id: str, records: Sequence[Any], state: _RunState
    ) -> EntityRegistry | None:
        try:
            registry = build_entity_registry(
                records, case_id=case_id, explicit_aliases=self._config.explicit_entity_aliases
            )
        except ReconstructionAdapterError as exc:
            state.diags.append(diagnostic_from_error(exc, stage=PipelineStage.ENTITIES, subject_type="ENTITY"))
            state.run_level_failure = True
            return None
        for alias, candidates in registry.ambiguous_aliases().items():
            state.diags.append(
                ReconstructionDiagnostic(
                    severity=DiagnosticSeverity.WARNING,
                    stage=PipelineStage.ENTITIES,
                    code="AMBIGUOUS_ENTITY_ALIAS",
                    message=(
                        f"Alias is registered for multiple entities {candidates}; references "
                        "using it will not be resolved."
                    ),
                    subject_type="ENTITY_REFERENCE",
                    subject_id=alias,
                )
            )
        return registry

    def _adapt_frames(
        self,
        case_id: str,
        records: Sequence[Any],
        usable: Mapping[str, EvidenceRecordView],
        state: _RunState,
    ) -> list[FrameArtifactReference]:
        frames: dict[str, FrameArtifactReference] = {}
        excluded: dict[str, str] = {}
        for idx, record in enumerate(records):
            try:
                frame, diags = adapt_supplied_frame(record, index=idx, case_id=case_id, evidence_by_id=usable)
            except FrameArtifactInputError as exc:
                excluded[exc.subject_id] = exc.code
                state.diags.append(
                    ReconstructionDiagnostic(
                        severity=DiagnosticSeverity.ERROR,
                        stage=PipelineStage.FRAME_HANDOFF,
                        code=exc.code,
                        message=str(exc),
                        subject_type="FRAME_ARTIFACT",
                        subject_id=exc.subject_id,
                    )
                )
                continue
            if frame.artifact_id in frames:
                excluded[frame.artifact_id] = "DUPLICATE_FRAME_ARTIFACT_ID"
                state.diags.append(
                    ReconstructionDiagnostic(
                        severity=DiagnosticSeverity.ERROR,
                        stage=PipelineStage.FRAME_HANDOFF,
                        code="DUPLICATE_FRAME_ARTIFACT_ID",
                        message="Supplied frame artifact_id appears more than once; all copies were excluded.",
                        subject_type="FRAME_ARTIFACT",
                        subject_id=frame.artifact_id,
                    )
                )
                continue
            frames[frame.artifact_id] = frame
            state.diags.extend(diags)
        for fid in sorted(excluded):
            frames.pop(fid, None)
            state.exclude("FRAME_ARTIFACT", fid, excluded[fid])
        return [frames[k] for k in sorted(frames)]

    def _merge_frames(
        self,
        extracted: Sequence[FrameArtifactReference],
        supplied: Sequence[FrameArtifactReference],
        state: _RunState,
    ) -> list[FrameArtifactReference]:
        merged = {f.artifact_id: f for f in extracted}
        for frame in supplied:
            existing = merged.get(frame.artifact_id)
            if existing is None:
                merged[frame.artifact_id] = frame
            elif existing.sha256_hash != frame.sha256_hash:
                state.exclude("FRAME_ARTIFACT", frame.artifact_id, "FRAME_ARTIFACT_CONFLICT")
                state.diags.append(
                    ReconstructionDiagnostic(
                        severity=DiagnosticSeverity.ERROR,
                        stage=PipelineStage.FRAME_HANDOFF,
                        code="FRAME_ARTIFACT_CONFLICT",
                        message=(
                            "A supplied frame has the same artifact_id as a frame extracted in "
                            "this run but a different SHA-256; the supplied frame was excluded."
                        ),
                        subject_type="FRAME_ARTIFACT",
                        subject_id=frame.artifact_id,
                    )
                )
        return sorted(
            merged.values(), key=lambda f: (f.source_evidence_id, f.pts_offset_seconds, f.artifact_id)
        )

    @staticmethod
    def _evidence_records(
        views: Mapping[str, EvidenceRecordView],
        verified: Mapping[str, VerifiedEvidenceFile],
        excluded: Mapping[str, str],
    ) -> list[EvidenceValidationRecord]:
        return [
            EvidenceValidationRecord(
                evidence_id=eid,
                source_type=view.source_type,
                sha256_hash=view.sha256_hash,
                integrity_verified=eid in verified,
                usable=eid not in excluded,
                storage_reference=view.storage_reference,
                storage_key=view.storage_key,
                time_metadata_applied=view.time_settings.applied_from_metadata,
                source_timezone=view.time_settings.source_timezone,
                clock_offset_seconds=view.time_settings.clock_offset_seconds,
                clock_calibration=view.time_settings.clock_calibration,
                recording_start_timestamp=view.time_settings.recording_start_timestamp,
                device_coordinates=view.device_coordinates,
                ignored_time_metadata_keys=list(view.time_settings.ignored_metadata_keys),
            )
            for eid, view in sorted(views.items())
        ]

    def _process_video(
        self,
        case_id: str,
        usable: Mapping[str, EvidenceRecordView],
        verified: Mapping[str, VerifiedEvidenceFile],
        state: _RunState,
    ) -> tuple[list[VideoProcessingRecord], list[FrameArtifactReference]]:
        records: list[VideoProcessingRecord] = []
        frames: list[FrameArtifactReference] = []
        artifact_root = self._config.artifact_root
        for evidence_id in sorted(usable):
            view = usable[evidence_id]
            if not view.is_video_evidence:
                continue

            if artifact_root is None:
                state.diags.append(
                    ReconstructionDiagnostic(
                        severity=DiagnosticSeverity.WARNING,
                        stage=PipelineStage.VIDEO,
                        code="VIDEO_STAGE_SKIPPED",
                        message="No artifact_root configured; frames were not extracted.",
                        subject_type="EVIDENCE",
                        subject_id=evidence_id,
                    )
                )
                records.append(
                    VideoProcessingRecord(evidence_id=evidence_id, status=VideoStageStatus.SKIPPED_NO_ARTIFACT_ROOT)
                )
                continue

            try:
                options = evidence_to_video_extraction_options(
                    view, base_options=self._config.video_extraction_options
                )
                output_dir = Path(artifact_root) / _artifact_segment(case_id) / _artifact_segment(evidence_id)
                extraction = extract_video_keyframes(
                    verified[evidence_id].resolved_path,
                    output_dir,
                    evidence_id=evidence_id,
                    case_id=case_id,
                    options=options,
                    expected_sha256=view.sha256_hash,
                    allowed_evidence_dir=self._config.storage_root,
                    allowed_artifact_dir=artifact_root,
                )
                frame_refs = [FrameArtifactReference.from_artifact(f, artifact_root) for f in extraction.frames]
            except (VideoProcessingError, ReconstructionAdapterError, ValueError) as exc:
                state.diags.append(
                    ReconstructionDiagnostic(
                        severity=DiagnosticSeverity.ERROR,
                        stage=PipelineStage.VIDEO,
                        code="VIDEO_PROCESSING_FAILED",
                        message=f"{type(exc).__name__}: {exc}",
                        subject_type="EVIDENCE",
                        subject_id=evidence_id,
                    )
                )
                records.append(VideoProcessingRecord(evidence_id=evidence_id, status=VideoStageStatus.FAILED))
                continue

            anchored = bool(extraction.frames) and all(f.is_utc_anchored for f in extraction.frames)
            handoff_status = self._handoff_frames(case_id, view, extraction, anchored, state)
            frames.extend(frame_refs)
            records.append(
                VideoProcessingRecord(
                    evidence_id=evidence_id,
                    status=VideoStageStatus.EXTRACTED,
                    frame_handoff_status=handoff_status,
                    frames_extracted=extraction.total_frames_extracted,
                    timestamps_utc_anchored=anchored,
                    source_video_sha256_before=extraction.source_video_sha256_before,
                    source_video_sha256_after=extraction.source_video_sha256_after,
                    source_integrity_verified=extraction.integrity_verified,
                    frame_artifact_ids=sorted(f.artifact_id for f in frame_refs),
                )
            )
        return records, frames

    def _handoff_frames(
        self,
        case_id: str,
        view: EvidenceRecordView,
        extraction: VideoExtractionResult,
        anchored: bool,
        state: _RunState,
    ) -> FrameHandoffStatus:
        if not extraction.frames:
            return FrameHandoffStatus.NOT_SUBMITTED_NO_FRAMES
        if self._frame_consumer is None:
            state.diags.append(
                ReconstructionDiagnostic(
                    severity=DiagnosticSeverity.INFO,
                    stage=PipelineStage.FRAME_HANDOFF,
                    code="FRAME_CONSUMER_UNAVAILABLE",
                    message=(
                        "No Member 1 frame consumer is configured. Frames were extracted with "
                        "provenance but not submitted. No semantic event recognition was "
                        "performed and no events were derived from them."
                    ),
                    subject_type="EVIDENCE",
                    subject_id=view.evidence_id,
                )
            )
            return FrameHandoffStatus.NOT_SUBMITTED_NO_CONSUMER

        handoff = FrameHandoff(
            case_id=case_id,
            source_evidence_id=view.evidence_id,
            source_video_sha256=extraction.source_video_sha256_after,
            frames=list(extraction.frames),
            timestamps_utc_anchored=anchored,
        )
        try:
            self._frame_consumer.submit_frames(handoff)
        except Exception as exc:  # noqa: BLE001 — external boundary: recorded, never hidden.
            state.diags.append(
                ReconstructionDiagnostic(
                    severity=DiagnosticSeverity.ERROR,
                    stage=PipelineStage.FRAME_HANDOFF,
                    code="FRAME_HANDOFF_FAILED",
                    message=f"{type(exc).__name__}: {exc}",
                    subject_type="EVIDENCE",
                    subject_id=view.evidence_id,
                )
            )
            return FrameHandoffStatus.FAILED
        return FrameHandoffStatus.SUBMITTED

    def _resolve_entities(
        self,
        events: Sequence[AdaptedEvent],
        registry: EntityRegistry,
        state: _RunState,
    ) -> list[EventEntityResolution]:
        resolutions: list[EventEntityResolution] = []
        for event in events:
            resolution = registry.resolve_event(event.event_id, event.involved_entity_raw_ids)
            resolutions.append(resolution)
            for item in resolution.resolutions:
                if item.status == EntityReferenceStatus.RESOLVED:
                    continue
                code = (
                    "ENTITY_REFERENCE_AMBIGUOUS"
                    if item.status == EntityReferenceStatus.UNRESOLVED_AMBIGUOUS
                    else "ENTITY_REFERENCE_UNRESOLVED"
                )
                state.diags.append(
                    ReconstructionDiagnostic(
                        severity=DiagnosticSeverity.WARNING,
                        stage=PipelineStage.ENTITY_RESOLUTION,
                        code=code,
                        message=f"Event '{event.event_id}' reference '{item.reference}': {item.note}",
                        subject_type="EVENT_CANDIDATE",
                        subject_id=event.event_id,
                    )
                )
        return resolutions

    @staticmethod
    def _timeline_events(
        adapted: Sequence[AdaptedEvent],
        spatial: SpatialReconstructionResult,
        resolution_by_event: Mapping[str, EventEntityResolution],
        usable: Mapping[str, EvidenceRecordView],
        state: _RunState,
    ) -> list[ReconstructedEvent]:
        """Build shared-facing events in timeline order and guard against double correction."""
        by_id = {e.event_id: e for e in adapted}
        out: list[ReconstructedEvent] = []
        for entry in spatial.timeline.entries:
            event = by_id[entry.event_id]
            if entry.time_window != event.time_window:
                state.diags.append(
                    ReconstructionDiagnostic(
                        severity=DiagnosticSeverity.ERROR,
                        stage=PipelineStage.SPATIAL,
                        code="TIME_CORRECTION_MISMATCH",
                        message=(
                            "The timeline engine and the event adapter derived different UTC "
                            f"windows ({entry.time_window.nominal_utc.isoformat()} vs "
                            f"{event.time_window.nominal_utc.isoformat()}); a correction may have "
                            "been applied more than once."
                        ),
                        subject_type="EVENT_CANDIDATE",
                        subject_id=entry.event_id,
                    )
                )
                continue
            resolution = resolution_by_event[entry.event_id]
            out.append(
                build_reconstructed_event(
                    event,
                    time_window=entry.time_window,
                    involved_entity_ids=resolution.canonical_entity_ids,
                    unresolved_entity_raw_ids=resolution.unresolved_references,
                    ordering_status=entry.ordering_status,
                    concurrent_event_ids=entry.concurrent_event_ids,
                    evidence_sha256=usable[event.source_evidence_id].sha256_hash,
                )
            )
        return out

    @staticmethod
    def _clock_diagnostics(
        usable: Mapping[str, EvidenceRecordView],
        events: Sequence[ReconstructedEvent],
        state: _RunState,
    ) -> None:
        """Report each applied, non-zero declared clock offset (clock skew correction)."""
        for eid in sorted(usable):
            offset = usable[eid].time_settings.clock_offset_seconds
            if offset is None or offset == 0.0:
                continue
            count = sum(1 for e in events if e.source_evidence_id == eid)
            direction = "fast" if offset < 0 else "slow"
            state.diags.append(
                ReconstructionDiagnostic(
                    severity=DiagnosticSeverity.INFO,
                    stage=PipelineStage.EVENTS,
                    code="CLOCK_OFFSET_APPLIED",
                    message=(
                        f"Declared clock offset {offset:+g} s (device clock {direction} by "
                        f"{abs(offset):g} s) was applied once to {count} event(s). The offset is "
                        "as declared at ingestion and not independently verified."
                    ),
                    subject_type="EVIDENCE",
                    subject_id=eid,
                )
            )

    @staticmethod
    def _spatial_observations(
        spatial: SpatialReconstructionResult,
        timeline_events: Sequence[ReconstructedEvent],
    ) -> list[SpatialObservation]:
        by_id = {e.event_id: e for e in timeline_events}
        observations: list[SpatialObservation] = []
        for entity_id in sorted(spatial.trajectories_by_entity):
            trajectory = spatial.trajectories_by_entity[entity_id]
            for event_id in trajectory.located_event_ids:
                event = by_id.get(event_id)
                if event is None or event.coordinates is None:
                    continue
                observations.append(
                    SpatialObservation(
                        entity_id=entity_id,
                        event_id=event_id,
                        source_evidence_id=event.source_evidence_id,
                        coordinates=event.coordinates,
                        timestamp_utc=event.timestamp_utc,
                        uncertainty_window_start=event.uncertainty_window_start,
                        uncertainty_window_end=event.uncertainty_window_end,
                    )
                )
        return observations

    def _export_relationships(
        self,
        contradictions: ContradictionAnalysisResult,
        provenance: Mapping[str, Mapping[str, Any]],
        state: _RunState,
    ) -> tuple[RelationshipExportStatus, list[SharedRelationshipRecord], dict[str, str]]:
        if not contradictions.leads:
            return RelationshipExportStatus.NO_CONTRADICTION_LEADS, [], {}
        missing = missing_decisions(self._approved, *RELATIONSHIP_DECISIONS)
        if missing:
            state.diags.append(
                ReconstructionDiagnostic(
                    severity=DiagnosticSeverity.WARNING,
                    stage=PipelineStage.RELATIONSHIPS,
                    code="RELATIONSHIP_EXPORT_BLOCKED",
                    message=(
                        f"{len(contradictions.leads)} contradiction lead(s) were detected but not "
                        "exported as Relationship records because required decisions are "
                        "withheld in this run. Leads remain in 'contradiction_leads'."
                    ),
                    pending_contract_decisions=[d.value for d in missing],
                )
            )
            return RelationshipExportStatus.RELATIONSHIP_EXPORT_BLOCKED, [], {}

        relationships: list[SharedRelationshipRecord] = []
        by_lead: dict[str, str] = {}
        for lead in contradictions.leads:
            try:
                rel = contradiction_lead_to_relationship(
                    lead, approved_decisions=self._approved, event_provenance=provenance
                )
            except ReconstructionAdapterError as exc:
                state.diags.append(
                    diagnostic_from_error(
                        exc, stage=PipelineStage.RELATIONSHIPS, subject_type="LEAD", fallback_subject_id=lead.lead_id
                    )
                )
                continue
            if lead.lead_id in by_lead:
                # Lead IDs are SHA-256 over a canonical identity tuple and detection dedups that
                # tuple, so this means an upstream defect. Fail loudly, never overwrite.
                raise RuntimeError(f"Duplicate contradiction lead_id '{lead.lead_id}' in one run.")
            relationships.append(rel)
            by_lead[lead.lead_id] = rel.relationship_id
        return (
            RelationshipExportStatus.EXPORTED,
            sorted(relationships, key=lambda r: r.relationship_id),
            by_lead,
        )

    @staticmethod
    def _gaps(
        state: _RunState,
        usable: Mapping[str, EvidenceRecordView],
        events: Sequence[ReconstructedEvent],
        resolutions: Sequence[EventEntityResolution],
        segments: Sequence[MovementSegment],
        frames: Sequence[FrameArtifactReference],
    ) -> list[EvidenceGap]:
        gaps: list[EvidenceGap] = []
        kind = {
            "EVIDENCE": EvidenceGapType.EVIDENCE_EXCLUDED,
            "EVENT_CANDIDATE": EvidenceGapType.CANDIDATE_EXCLUDED,
            "FRAME_ARTIFACT": EvidenceGapType.FRAME_EXCLUDED,
        }
        for item in state.excluded:
            gaps.append(
                EvidenceGap(
                    gap_type=kind[item.subject_type],
                    subject_type=item.subject_type,
                    subject_id=item.subject_id,
                    description="Excluded after failing validation; see diagnostics.",
                    reason_code=item.reason_code,
                )
            )
        complete = not state.run_level_failure
        if complete:
            with_events = {e.source_evidence_id for e in events}
            for eid in sorted(usable):
                if eid not in with_events:
                    gaps.append(
                        EvidenceGap(
                            gap_type=EvidenceGapType.EVIDENCE_WITHOUT_EVENTS,
                            subject_type="EVIDENCE",
                            subject_id=eid,
                            description="No accepted event candidate references this evidence item.",
                        )
                    )
        for event in events:
            if event.coordinates is None:
                gaps.append(
                    EvidenceGap(
                        gap_type=EvidenceGapType.EVENT_WITHOUT_COORDINATES,
                        subject_type="EVENT",
                        subject_id=event.event_id,
                        description="Event has no coordinates; it cannot place entities spatially.",
                    )
                )
        for resolution in resolutions:
            for ref in resolution.unresolved_references:
                gaps.append(
                    EvidenceGap(
                        gap_type=EvidenceGapType.UNRESOLVED_ENTITY_REFERENCE,
                        subject_type="EVENT",
                        subject_id=resolution.event_id,
                        description=f"Entity reference '{ref}' has no exact registration.",
                    )
                )
        for seg in segments:
            if seg.has_unlocated_intervening_events:
                gaps.append(
                    EvidenceGap(
                        gap_type=EvidenceGapType.SPATIAL_CONTINUITY_GAP,
                        subject_type="MOVEMENT_SEGMENT",
                        subject_id=seg.segment_id,
                        description=seg.continuity_note or "Intervening events lack coordinates.",
                    )
                )
        framed = {f.source_evidence_id for f in frames}
        for eid in sorted(usable):
            if complete and usable[eid].is_video_evidence and eid not in framed:
                gaps.append(
                    EvidenceGap(
                        gap_type=EvidenceGapType.VIDEO_WITHOUT_FRAMES,
                        subject_type="EVIDENCE",
                        subject_id=eid,
                        description="No frame artifacts were extracted or supplied for this video evidence.",
                    )
                )
        order = {t: i for i, t in enumerate(EvidenceGapType)}
        return sorted(gaps, key=lambda g: (order[g.gap_type], g.subject_type, g.subject_id, g.description))

    # --- Result assembly -----------------------------------------------------------------

    @staticmethod
    def _status(diags: Iterable[ReconstructionDiagnostic]) -> PipelineStatus:
        severities = {d.severity for d in diags}
        if DiagnosticSeverity.ERROR in severities:
            return PipelineStatus.COMPLETED_WITH_ERRORS
        if DiagnosticSeverity.WARNING in severities:
            return PipelineStatus.COMPLETED_WITH_WARNINGS
        return PipelineStatus.COMPLETED

    def _result(
        self,
        case_id: str,
        run_id: str,
        status: PipelineStatus,
        state: _RunState,
        **fields: Any,
    ) -> ReconstructionResult:
        scrubbed = [d.model_copy(update={"message": self._scrub(d.message)}) for d in state.diags]
        result = ReconstructionResult(
            configuration_version=self._configuration_version,
            configuration=self._config.public_configuration(),
            run_id=run_id,
            case_id=case_id,
            status=status,
            approved_contract_decisions=sorted(d.value for d in self._approved),
            withheld_contract_decisions=sorted(d.value for d in ContractDecision if d not in self._approved),
            unverified_contract_decisions=sorted(
                d.value for d in self._approved if d not in APPROVED_CONTRACT_DECISIONS
            ),
            excluded_inputs=sorted(state.excluded, key=lambda x: (x.subject_type, x.subject_id)),
            diagnostics=sort_diagnostics(scrubbed),
            **fields,
        )
        result = result.model_copy(update={"result_sha256": result.compute_result_sha256()})
        logger.info(
            "reconstruction run finished run_id=%s case_id=%s status=%s events=%d leads=%d",
            run_id,
            case_id,
            status.value,
            len(result.timeline_events),
            len(result.contradiction_leads),
        )
        return result
