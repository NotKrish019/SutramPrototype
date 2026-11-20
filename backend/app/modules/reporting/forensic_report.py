"""Evidence-linked investigation report built from a ``ReconstructionResult`` (Member 4 — Phase 3I).

``build_forensic_report`` turns one stored reconstruction run into a structured
``ForensicReport``. ``report_render`` renders that same object as Markdown or print-ready HTML,
so all three formats always carry identical content.

Rules enforced here (and checked by tests):

- **Every finding cites its evidence.** Timeline events, spatial observations, movement
  segments, contradiction leads and relationships each carry ``citations``: evidence id,
  evidence SHA-256, events, source records and temporally aligned frames, taken from the run's
  ``finding_provenance``.
- **Observed vs inferred.** Each finding has ``basis`` ``OBSERVED`` or ``INFERRED``. Movement
  segments, contradiction leads and relationships are always ``INFERRED``.
- **Contradictions are analytical leads,** never findings of fact.
- **Null weight stays null.** ``weight`` is copied as is; ``weight_display`` is
  ``"Not provided"`` when it is null, never ``0%``.
- **No claim of guilt or identity.** Entities are referred to by their registry ids only.
- **Limitations are always present.** The scope limitation is unconditional.
- **No host paths.** Storage is shown as the canonical key or URN; frame artifacts by URN.
- **Determinism.** The only time-dependent value is ``generated_at_utc``. ``content_sha256``
  covers everything else, so the same run always gives the same ``content_sha256``.
"""

import hashlib
import json
from collections import defaultdict
from collections.abc import Iterable, Mapping, Sequence
from datetime import UTC, datetime
from enum import StrEnum
from typing import Any

from pydantic import BaseModel, ConfigDict, Field

from app.modules.reconstruction.adapters.diagnostics import DiagnosticSeverity
from app.modules.reconstruction.adapters.event_adapter import ReconstructedEvent
from app.modules.reconstruction.adapters.evidence_adapter import ClockCalibrationStatus
from app.modules.reconstruction.findings import FindingProvenance, FindingType
from app.modules.reconstruction.pipeline import (
    FrameHandoffStatus,
    PipelineStatus,
    ReconstructionResult,
    RelationshipExportStatus,
    VideoStageStatus,
)
from app.schemas.reconstruction import GeoPoint, SpeedThresholdEvaluation

REPORT_SCHEMA_VERSION = "yukti-forensic-report/1"
REPORT_GENERATOR_VERSION = "yukti-m4-report/3I.1"
REPORT_KIND = "RECONSTRUCTION_DECISION_SUPPORT_REPORT"
NOT_PROVIDED = "Not provided"

DISCLAIMER = (
    "DECISION SUPPORT FOR INVESTIGATORS. This report is generated automatically from a "
    "reconstruction run over the evidence and extraction inputs listed in it. It organises "
    "those inputs and the analytical inferences derived from them for review by a qualified "
    "investigator. It does not establish the identity of any person, nor guilt, intent, "
    "deception or the occurrence of any offence. Contradictions are analytical leads to be "
    "checked against the original evidence, not findings of fact. Entity identifiers are "
    "registry references, not identifications. Every finding must be verified against the "
    "cited source evidence before it is relied upon."
)

CONTRADICTION_NOTICE = (
    "Each item below is an ANALYTICAL LEAD produced by a configured rule, not a proven fact. "
    "It indicates that two observations are hard to reconcile under the stated assumptions. "
    "Possible explanations include clock error, location error, extraction error, shared "
    "devices or accounts, and wrong entity linkage. Verify against the cited evidence."
)

FRAME_NOTICE = (
    "Frame references are temporal alignment only: the frame was captured during the event's "
    "uncertainty window from the same source. They do not show or confirm the event. No "
    "semantic recognition was performed on any frame."
)

SCOPE_LIMITATION_MESSAGE = (
    "This report covers only the inputs supplied to this run, processed by the stated "
    "algorithm and configuration versions. Evidence not supplied to the run is not reflected, "
    "and the absence of a finding does not mean the absence of an event."
)


class FindingBasis(StrEnum):
    OBSERVED = "OBSERVED"
    INFERRED = "INFERRED"


class IntegrityStatus(StrEnum):
    VERIFIED = "VERIFIED"
    SHA256_MISMATCH = "SHA256_MISMATCH"
    FILE_UNAVAILABLE = "FILE_UNAVAILABLE"
    STORAGE_REFERENCE_REJECTED = "STORAGE_REFERENCE_REJECTED"
    RECORD_INVALID = "RECORD_INVALID"
    DUPLICATE_EVIDENCE_ID = "DUPLICATE_EVIDENCE_ID"
    NOT_VERIFIED = "NOT_VERIFIED"


_INTEGRITY_BY_REASON: dict[str, IntegrityStatus] = {
    "EVIDENCE_INTEGRITY_MISMATCH": IntegrityStatus.SHA256_MISMATCH,
    "EVIDENCE_STORAGE_UNAVAILABLE": IntegrityStatus.FILE_UNAVAILABLE,
    "EVIDENCE_PATH_REJECTED": IntegrityStatus.STORAGE_REFERENCE_REJECTED,
    "CLIENT_STORAGE_PATH_REJECTED": IntegrityStatus.STORAGE_REFERENCE_REJECTED,
    "REMOTE_STORAGE_URI_REJECTED": IntegrityStatus.STORAGE_REFERENCE_REJECTED,
    "INVALID_EVIDENCE_RECORD": IntegrityStatus.RECORD_INVALID,
    "DUPLICATE_EVIDENCE_ID": IntegrityStatus.DUPLICATE_EVIDENCE_ID,
}

_INTEGRITY_NOTES: dict[IntegrityStatus, str] = {
    IntegrityStatus.VERIFIED: (
        "SHA-256 and size recomputed from the stored bytes during the run and matched the "
        "ingestion record."
    ),
    IntegrityStatus.SHA256_MISMATCH: (
        "Stored bytes do not match the recorded SHA-256 or size. Excluded from the run."
    ),
    IntegrityStatus.FILE_UNAVAILABLE: "No stored file was found at the canonical key. Excluded.",
    IntegrityStatus.STORAGE_REFERENCE_REJECTED: (
        "The storage reference was not a canonical key or evidence URN (client path, path "
        "traversal or remote URI). Excluded; no file was read."
    ),
    IntegrityStatus.RECORD_INVALID: "The evidence record failed validation. Excluded.",
    IntegrityStatus.DUPLICATE_EVIDENCE_ID: (
        "The evidence id appears more than once in the inputs. All copies were excluded."
    ),
    IntegrityStatus.NOT_VERIFIED: "Integrity was not verified in this run.",
}

_STATUS_EXPLANATIONS: dict[PipelineStatus, str] = {
    PipelineStatus.COMPLETED: "The run completed without warnings.",
    PipelineStatus.COMPLETED_WITH_WARNINGS: (
        "The run completed. Warnings were recorded; see Limitations and the diagnostics list."
    ),
    PipelineStatus.COMPLETED_WITH_ERRORS: (
        "The run completed, but some inputs failed validation or processing and were excluded. "
        "Findings that would involve them cannot appear in this report."
    ),
    PipelineStatus.FAILED: (
        "The run failed before reconstruction completed. This report lists only the evidence "
        "inventory, diagnostics and limitations."
    ),
}


# --- Report model ---------------------------------------------------------------------------


class _Frozen(BaseModel):
    model_config = ConfigDict(frozen=True)


class Citation(_Frozen):
    """One evidence item supporting a finding, with the records and frames used from it."""

    evidence_id: str
    evidence_sha256: str | None = Field(
        default=None, description="SHA-256 of the evidence item as verified in the run."
    )
    event_ids: list[str] = Field(default_factory=list)
    source_record_ids: list[str] = Field(default_factory=list)
    frame_artifact_ids: list[str] = Field(
        default_factory=list, description="Frames captured during the event window (alignment only)."
    )


class ReportMetadata(_Frozen):
    case_id: str
    run_id: str
    run_status: PipelineStatus
    run_status_explanation: str
    algorithm_version: str
    configuration_version: str
    snapshot_schema_version: str
    result_sha256: str
    report_schema_version: str = REPORT_SCHEMA_VERSION
    report_generator_version: str = REPORT_GENERATOR_VERSION
    generated_at_utc: str = Field(description="Report generation time, ISO 8601 UTC ('Z').")
    relationship_export_status: RelationshipExportStatus
    approved_contract_decisions: list[str]
    withheld_contract_decisions: list[str]
    unverified_contract_decisions: list[str] = Field(default_factory=list)
    approval_record: str
    configured_speed_threshold_kmh: float | None = None
    estimated_half_width_seconds: float | None = None


class LeadSummary(_Frozen):
    ref: str
    lead_id: str
    statement: str
    basis: FindingBasis = FindingBasis.INFERRED
    citations: list[Citation]


class CaseSummary(_Frozen):
    counts: dict[str, int]
    timeline_span_start_utc: str | None = None
    timeline_span_end_utc: str | None = None
    key_leads: list[LeadSummary] = Field(default_factory=list)
    interpretation_notice: str


class EvidenceInventoryItem(_Frozen):
    evidence_id: str
    source_type: str | None = None
    sha256: str | None = None
    integrity_status: IntegrityStatus
    integrity_note: str
    usable: bool
    exclusion_reason_code: str | None = None
    storage_reference: str | None = None
    source_timezone: str | None = None
    clock_offset_seconds: float | None = None
    clock_calibration: ClockCalibrationStatus | None = None
    timeline_events_cited: int = 0


class TimelineEntry(_Frozen):
    ref: str
    event_id: str
    basis: FindingBasis = FindingBasis.OBSERVED
    basis_note: str = (
        "Event extracted from the source by Member 1; corrected UTC and window derived once by "
        "Member 4 from raw_timestamp."
    )
    event_type: str
    description: str
    raw_timestamp: str
    raw_precision: str
    timestamp_utc: str
    precision: str
    uncertainty_window_start_utc: str | None = None
    uncertainty_window_end_utc: str | None = None
    is_estimated_window_assumption: bool
    timezone_basis: str
    source_timezone: str | None = None
    clock_offset_seconds: float | None = None
    clock_calibration: ClockCalibrationStatus
    ordering_status: str
    concurrent_event_ids: list[str] = Field(default_factory=list)
    involved_entity_ids: list[str] = Field(default_factory=list)
    unresolved_entity_references: list[str] = Field(default_factory=list)
    coordinates: GeoPoint | None = None
    duration_seconds: float | None = None
    extraction_confidence: float | None = None
    source_evidence_id: str
    source_record_id: str | None = None
    frame_artifact_ids: list[str] = Field(default_factory=list)
    citations: list[Citation]


class SpatialObservationEntry(_Frozen):
    ref: str
    observation_id: str
    basis: FindingBasis = FindingBasis.OBSERVED
    entity_id: str
    event_id: str
    coordinates: GeoPoint
    timestamp_utc: str
    uncertainty_window_start_utc: str | None = None
    uncertainty_window_end_utc: str | None = None
    citations: list[Citation]


class MovementEntry(_Frozen):
    ref: str
    segment_id: str
    basis: FindingBasis = FindingBasis.INFERRED
    entity_id: str
    origin_event_id: str
    destination_event_id: str
    origin_coordinates: GeoPoint
    destination_coordinates: GeoPoint
    distance_km: float
    nominal_elapsed_seconds: float
    min_possible_elapsed_seconds: float
    max_possible_elapsed_seconds: float
    min_required_speed_kmh: float | None = None
    configured_speed_threshold_kmh: float
    threshold_evaluation: SpeedThresholdEvaluation
    feasibility_statement: str
    threshold_assumption_note: str
    continuity_note: str | None = None
    intervening_unlocated_event_ids: list[str] = Field(default_factory=list)
    citations: list[Citation]


class ContradictionEntry(_Frozen):
    ref: str
    lead_id: str
    basis: FindingBasis = FindingBasis.INFERRED
    classification: str = "ANALYTICAL_LEAD_NOT_A_FINDING_OF_FACT"
    lead_type: str
    entity_id: str
    origin_event_id: str
    destination_event_id: str
    origin_window_utc: list[str]
    destination_window_utc: list[str]
    distance_km: float | None = None
    max_possible_elapsed_seconds: float
    min_required_speed_kmh: float | None = None
    configured_speed_threshold_kmh: float
    threshold_evaluation: str | None = None
    explanation: str
    lead_limitations: list[str]
    relationship_id: str | None = None
    frame_artifact_ids: list[str] = Field(default_factory=list)
    citations: list[Citation]


class UnevaluatedPair(_Frozen):
    entity_id: str
    event_a_id: str
    event_b_id: str
    note: str = "Uncertainty windows overlap; order unknown, so travel feasibility was not evaluated."


class RelationshipEntry(_Frozen):
    ref: str
    relationship_id: str
    basis: FindingBasis
    relation_type: str
    source_id: str
    target_id: str
    is_observed: bool
    weight: float | None = Field(default=None, description="Copied as is; null means not computed.")
    weight_display: str
    evidence_basis: list[str]
    lead_id: str | None = None
    citations: list[Citation]


class LimitationEntry(_Frozen):
    code: str
    message: str
    subject_ids: list[str] = Field(default_factory=list)


class EvidenceGapEntry(_Frozen):
    gap_type: str
    subject_type: str
    subject_id: str
    description: str
    reason_code: str | None = None


class ClockCalibrationEntry(_Frozen):
    evidence_id: str
    clock_calibration: ClockCalibrationStatus
    clock_offset_seconds: float | None = None
    source_timezone: str | None = None
    note: str


class PendingItem(_Frozen):
    code: str
    subject_id: str | None = None
    description: str


class DiagnosticEntry(_Frozen):
    severity: str
    stage: str
    code: str
    subject_type: str | None = None
    subject_id: str | None = None
    message: str


class LimitationsSection(_Frozen):
    limitations: list[LimitationEntry]
    evidence_gaps: list[EvidenceGapEntry]
    unverified_clock_calibrations: list[ClockCalibrationEntry]
    pending_items: list[PendingItem]
    excluded_inputs: list[dict[str, str]]
    diagnostics: list[DiagnosticEntry] = Field(
        description="ERROR and WARNING diagnostics of the run (INFO diagnostics are omitted)."
    )


class EvidenceCustodyEntry(_Frozen):
    evidence_id: str
    sha256: str | None = None
    storage_reference: str | None = None
    storage_key: str | None = None
    integrity_status: IntegrityStatus
    video_sha256_before_processing: str | None = None
    video_sha256_after_processing: str | None = None
    video_source_unchanged: bool | None = None


class FrameCustodyEntry(_Frozen):
    artifact_id: str
    artifact_reference: str
    origin: str
    source_evidence_id: str
    source_video_sha256: str
    frame_index: int
    pts_offset_seconds: float
    timestamp_utc: str | None = None
    timestamp_utc_anchored: bool
    sha256: str


class VideoCustodyEntry(_Frozen):
    evidence_id: str
    status: str
    processing_kind: str
    processing_note: str
    frame_handoff_status: str | None = None
    frames_extracted: int


class CustodyAppendix(_Frozen):
    statement: str = (
        "Original evidence is read through a read-only, no-follow descriptor and never "
        "modified. Hashes below are those recorded at ingestion and recomputed in the run. "
        "Derived frames are written to new directories and never overwrite earlier artifacts."
    )
    evidence: list[EvidenceCustodyEntry]
    video_processing: list[VideoCustodyEntry]
    frame_artifacts: list[FrameCustodyEntry]
    finding_provenance: list[FindingProvenance]
    processing_configuration: dict[str, Any]
    run_interpretation_notice: str


class ForensicReport(_Frozen):
    """Structured, evidence-linked investigation report for one reconstruction run."""

    report_schema_version: str = REPORT_SCHEMA_VERSION
    report_kind: str = REPORT_KIND
    title: str
    disclaimer: str = DISCLAIMER
    metadata: ReportMetadata
    summary: CaseSummary
    evidence_inventory: list[EvidenceInventoryItem]
    timeline: list[TimelineEntry]
    frame_notice: str = FRAME_NOTICE
    spatial_observations: list[SpatialObservationEntry]
    movement_segments: list[MovementEntry]
    contradiction_notice: str = CONTRADICTION_NOTICE
    contradictions: list[ContradictionEntry]
    contradiction_pairs_not_evaluated: list[UnevaluatedPair]
    relationships: list[RelationshipEntry]
    limitations: LimitationsSection
    chain_of_custody: CustodyAppendix
    content_sha256: str = Field(
        default="",
        description="SHA-256 of the canonical report JSON, excluding generated_at_utc and this field.",
    )

    def to_json(self) -> str:
        return json.dumps(self.model_dump(mode="json"), ensure_ascii=False, indent=2, sort_keys=False)

    def compute_content_sha256(self) -> str:
        data = self.model_dump(mode="json")
        data.pop("content_sha256", None)
        data["metadata"].pop("generated_at_utc", None)
        encoded = json.dumps(data, sort_keys=True, separators=(",", ":"), ensure_ascii=False)
        return hashlib.sha256(encoded.encode("utf-8")).hexdigest()


# --- Builder --------------------------------------------------------------------------------


def iso_z(value: datetime | None) -> str | None:
    """ISO 8601 UTC with a 'Z' suffix (timezone-aware values are converted to UTC)."""
    if value is None:
        return None
    if value.tzinfo is not None:
        value = value.astimezone(UTC)
    return value.replace(tzinfo=None).isoformat() + "Z"


def format_weight(weight: float | None) -> str:
    """Display text for a relationship weight. Null is 'Not provided', never 0%."""
    if weight is None:
        return NOT_PROVIDED
    return f"{weight:.2f}"


def _feasibility_statement(evaluation: SpeedThresholdEvaluation, min_speed: float | None, threshold: float) -> str:
    speed = f"{min_speed:.1f} km/h" if min_speed is not None else "an undetermined speed"
    if evaluation == SpeedThresholdEvaluation.EXCEEDS_CONFIGURED_THRESHOLD:
        return (
            f"Inferred: covering the straight-line distance in the longest possible elapsed time "
            f"needs at least {speed}, above the configured reference of {threshold:g} km/h. "
            "Analytical indicator only; see the contradiction leads."
        )
    if evaluation == SpeedThresholdEvaluation.WITHIN_CONFIGURED_THRESHOLD:
        return (
            f"Inferred: needs at least {speed}, within the configured reference of "
            f"{threshold:g} km/h. Feasible under the stated assumptions."
        )
    return "Inferred: no displacement between the two observations."


class _CitationIndex:
    def __init__(self, result: ReconstructionResult) -> None:
        self._events: dict[str, ReconstructedEvent] = {e.event_id: e for e in result.timeline_events}
        self._prov: dict[tuple[FindingType, str], FindingProvenance] = {
            (p.finding_type, p.finding_id): p for p in result.finding_provenance
        }
        self._sha = {e.evidence_id: e.sha256_hash for e in result.evidence if e.usable}

    def cite(self, finding_type: FindingType, finding_id: str, fallback_event_ids: Iterable[str]) -> list[Citation]:
        prov = self._prov.get((finding_type, finding_id))
        event_ids = prov.event_ids if prov is not None else sorted(set(fallback_event_ids))
        frames_allowed = set(prov.frame_artifact_ids) if prov is not None else None
        by_evidence: dict[str, list[ReconstructedEvent]] = defaultdict(list)
        for eid in event_ids:
            event = self._events.get(eid)
            if event is not None:
                by_evidence[event.source_evidence_id].append(event)
        citations: list[Citation] = []
        for evidence_id in sorted(by_evidence):
            events = by_evidence[evidence_id]
            frames = sorted(
                {
                    f
                    for e in events
                    for f in e.supporting_frame_artifact_ids
                    if frames_allowed is None or f in frames_allowed
                }
            )
            sha = (prov.evidence_sha256.get(evidence_id) if prov is not None else None) or self._sha.get(evidence_id)
            citations.append(
                Citation(
                    evidence_id=evidence_id,
                    evidence_sha256=sha,
                    event_ids=sorted(e.event_id for e in events),
                    source_record_ids=sorted({e.source_record_id for e in events if e.source_record_id}),
                    frame_artifact_ids=frames,
                )
            )
        return citations


def _inventory(result: ReconstructionResult) -> list[EvidenceInventoryItem]:
    excluded = {x.subject_id: x.reason_code for x in result.excluded_inputs if x.subject_type == "EVIDENCE"}
    cited: defaultdict[str, int] = defaultdict(int)
    for event in result.timeline_events:
        cited[event.source_evidence_id] += 1
    items: dict[str, EvidenceInventoryItem] = {}
    for record in result.evidence:
        reason = excluded.get(record.evidence_id)
        if reason is not None:
            status = _INTEGRITY_BY_REASON.get(reason, IntegrityStatus.NOT_VERIFIED)
        elif record.integrity_verified:
            status = IntegrityStatus.VERIFIED
        else:
            status = IntegrityStatus.NOT_VERIFIED
        items[record.evidence_id] = EvidenceInventoryItem(
            evidence_id=record.evidence_id,
            source_type=record.source_type,
            sha256=record.sha256_hash,
            integrity_status=status,
            integrity_note=_INTEGRITY_NOTES[status],
            usable=record.usable,
            exclusion_reason_code=reason,
            storage_reference=record.storage_reference,
            source_timezone=record.source_timezone,
            clock_offset_seconds=record.clock_offset_seconds,
            clock_calibration=record.clock_calibration,
            timeline_events_cited=cited[record.evidence_id],
        )
    for evidence_id, reason in excluded.items():
        if evidence_id in items:
            continue
        status = _INTEGRITY_BY_REASON.get(reason, IntegrityStatus.RECORD_INVALID)
        items[evidence_id] = EvidenceInventoryItem(
            evidence_id=evidence_id,
            integrity_status=status,
            integrity_note=_INTEGRITY_NOTES[status],
            usable=False,
            exclusion_reason_code=reason,
        )
    return [items[k] for k in sorted(items)]


def _timeline(result: ReconstructionResult, index: _CitationIndex) -> list[TimelineEntry]:
    out: list[TimelineEntry] = []
    for n, e in enumerate(result.timeline_events, start=1):
        out.append(
            TimelineEntry(
                ref=f"E{n}",
                event_id=e.event_id,
                event_type=e.event_type,
                description=e.description,
                raw_timestamp=e.raw_timestamp,
                raw_precision=e.raw_timestamp_precision,
                timestamp_utc=iso_z(e.timestamp_utc) or "",
                precision=e.timestamp_precision.value,
                uncertainty_window_start_utc=iso_z(e.uncertainty_window_start),
                uncertainty_window_end_utc=iso_z(e.uncertainty_window_end),
                is_estimated_window_assumption=e.is_estimated_window_assumption,
                timezone_basis=e.timezone_basis.value,
                source_timezone=e.source_timezone,
                clock_offset_seconds=e.clock_offset_seconds,
                clock_calibration=e.clock_calibration,
                ordering_status=str(e.ordering_status),
                concurrent_event_ids=list(e.concurrent_event_ids),
                involved_entity_ids=list(e.involved_entity_ids),
                unresolved_entity_references=list(e.unresolved_entity_raw_ids),
                coordinates=e.coordinates,
                duration_seconds=e.duration_seconds,
                extraction_confidence=e.extraction_confidence,
                source_evidence_id=e.source_evidence_id,
                source_record_id=e.source_record_id,
                frame_artifact_ids=list(e.supporting_frame_artifact_ids),
                citations=index.cite(FindingType.TIMELINE_EVENT, e.event_id, [e.event_id]),
            )
        )
    return out


def _observations(result: ReconstructionResult, index: _CitationIndex) -> list[SpatialObservationEntry]:
    out: list[SpatialObservationEntry] = []
    for n, obs in enumerate(result.spatial_observations, start=1):
        obs_id = f"obs:{obs.entity_id}:{obs.event_id}"
        out.append(
            SpatialObservationEntry(
                ref=f"S{n}",
                observation_id=obs_id,
                entity_id=obs.entity_id,
                event_id=obs.event_id,
                coordinates=obs.coordinates,
                timestamp_utc=iso_z(obs.timestamp_utc) or "",
                uncertainty_window_start_utc=iso_z(obs.uncertainty_window_start),
                uncertainty_window_end_utc=iso_z(obs.uncertainty_window_end),
                citations=index.cite(FindingType.SPATIAL_OBSERVATION, obs_id, [obs.event_id]),
            )
        )
    return out


def _movements(result: ReconstructionResult, index: _CitationIndex) -> list[MovementEntry]:
    out: list[MovementEntry] = []
    for n, seg in enumerate(result.movement_segments, start=1):
        out.append(
            MovementEntry(
                ref=f"M{n}",
                segment_id=seg.segment_id,
                entity_id=seg.entity_id,
                origin_event_id=seg.origin_event_id,
                destination_event_id=seg.destination_event_id,
                origin_coordinates=seg.origin_coordinates,
                destination_coordinates=seg.destination_coordinates,
                distance_km=seg.distance_km,
                nominal_elapsed_seconds=seg.nominal_elapsed_seconds,
                min_possible_elapsed_seconds=seg.min_possible_elapsed_seconds,
                max_possible_elapsed_seconds=seg.max_possible_elapsed_seconds,
                min_required_speed_kmh=seg.min_required_speed_kmh,
                configured_speed_threshold_kmh=seg.configured_speed_threshold_kmh,
                threshold_evaluation=seg.threshold_evaluation,
                feasibility_statement=_feasibility_statement(
                    seg.threshold_evaluation, seg.min_required_speed_kmh, seg.configured_speed_threshold_kmh
                ),
                threshold_assumption_note=seg.threshold_assumption_note,
                continuity_note=seg.continuity_note,
                intervening_unlocated_event_ids=list(seg.intervening_unlocated_event_ids),
                citations=index.cite(
                    FindingType.MOVEMENT_SEGMENT,
                    seg.segment_id,
                    [seg.origin_event_id, seg.destination_event_id],
                ),
            )
        )
    return out


def _contradictions(result: ReconstructionResult, index: _CitationIndex) -> list[ContradictionEntry]:
    out: list[ContradictionEntry] = []
    for n, lead in enumerate(result.contradiction_leads, start=1):
        facts, derived = lead.observed_facts, lead.derived_inference
        out.append(
            ContradictionEntry(
                ref=f"L{n}",
                lead_id=lead.lead_id,
                lead_type=lead.lead_type.value,
                entity_id=lead.entity_id,
                origin_event_id=lead.origin_event_id,
                destination_event_id=lead.destination_event_id,
                origin_window_utc=[
                    iso_z(facts.origin_time_window.window_start_utc) or "",
                    iso_z(facts.origin_time_window.window_end_utc) or "",
                ],
                destination_window_utc=[
                    iso_z(facts.destination_time_window.window_start_utc) or "",
                    iso_z(facts.destination_time_window.window_end_utc) or "",
                ],
                distance_km=derived.distance_km,
                max_possible_elapsed_seconds=derived.max_possible_elapsed_seconds,
                min_required_speed_kmh=derived.min_required_speed_kmh,
                configured_speed_threshold_kmh=derived.configured_speed_threshold_kmh,
                threshold_evaluation=(
                    derived.threshold_evaluation.value if derived.threshold_evaluation is not None else None
                ),
                explanation=lead.explanation,
                lead_limitations=list(lead.limitations),
                relationship_id=lead.relationship_id,
                frame_artifact_ids=list(lead.supporting_frame_artifact_ids),
                citations=index.cite(
                    FindingType.CONTRADICTION_LEAD,
                    lead.lead_id,
                    [lead.origin_event_id, lead.destination_event_id],
                ),
            )
        )
    return out


def _relationships(result: ReconstructionResult, index: _CitationIndex) -> list[RelationshipEntry]:
    out: list[RelationshipEntry] = []
    for n, rel in enumerate(result.relationships, start=1):
        lead_id = rel.metadata.get("lead_id")
        out.append(
            RelationshipEntry(
                ref=f"R{n}",
                relationship_id=rel.relationship_id,
                basis=FindingBasis.OBSERVED if rel.is_observed else FindingBasis.INFERRED,
                relation_type=rel.relation_type,
                source_id=rel.source_id,
                target_id=rel.target_id,
                is_observed=rel.is_observed,
                weight=rel.weight,
                weight_display=format_weight(rel.weight),
                evidence_basis=list(rel.evidence_basis),
                lead_id=lead_id if isinstance(lead_id, str) else None,
                citations=index.cite(FindingType.RELATIONSHIP, rel.relationship_id, [rel.source_id, rel.target_id]),
            )
        )
    return out


def _pending_items(result: ReconstructionResult) -> list[PendingItem]:
    items: list[PendingItem] = []
    for decision in result.withheld_contract_decisions:
        items.append(
            PendingItem(
                code="CONTRACT_DECISION_WITHHELD",
                subject_id=decision,
                description="This shared contract decision was not approved for the run; dependent steps were skipped.",
            )
        )
    for decision in result.unverified_contract_decisions:
        items.append(
            PendingItem(
                code="CONTRACT_DECISION_NOT_TEAM_APPROVED",
                subject_id=decision,
                description=(
                    "This decision was enabled for the run as a proposal only; no verified team "
                    "sign-off exists. Findings that depend on it are conditional on its approval."
                ),
            )
        )
    for video in result.video:
        if video.status == VideoStageStatus.SKIPPED_NO_ARTIFACT_ROOT:
            items.append(
                PendingItem(
                    code="VIDEO_FRAMES_NOT_EXTRACTED",
                    subject_id=video.evidence_id,
                    description="Frame extraction was skipped (no derived-artifact store configured).",
                )
            )
        elif video.status == VideoStageStatus.FAILED:
            items.append(
                PendingItem(
                    code="VIDEO_PROCESSING_FAILED",
                    subject_id=video.evidence_id,
                    description="Frame extraction failed; see diagnostics.",
                )
            )
        if video.frame_handoff_status == FrameHandoffStatus.NOT_SUBMITTED_NO_CONSUMER:
            items.append(
                PendingItem(
                    code="FRAMES_NOT_SUBMITTED_NO_CONSUMER",
                    subject_id=video.evidence_id,
                    description=(
                        "Frames were extracted with provenance but not submitted for analysis: no "
                        "Member 1 frame consumer is configured. No semantic recognition was performed."
                    ),
                )
            )
        elif video.frame_handoff_status == FrameHandoffStatus.FAILED:
            items.append(
                PendingItem(
                    code="FRAME_HANDOFF_FAILED",
                    subject_id=video.evidence_id,
                    description="Frames could not be handed off for analysis; see diagnostics.",
                )
            )
    if result.relationship_export_status == RelationshipExportStatus.RELATIONSHIP_EXPORT_BLOCKED:
        items.append(
            PendingItem(
                code="RELATIONSHIP_EXPORT_BLOCKED",
                description="Contradiction leads exist but were not exported as relationships.",
            )
        )
    if result.contradiction_overlaps_not_evaluated:
        items.append(
            PendingItem(
                code="CONCURRENT_PAIRS_NOT_EVALUATED",
                description=(
                    f"{len(result.contradiction_overlaps_not_evaluated)} pair(s) of located events "
                    "have overlapping windows and were not evaluated for travel feasibility."
                ),
            )
        )
    return items


_CALIBRATION_NOTES: dict[ClockCalibrationStatus, str] = {
    ClockCalibrationStatus.DECLARED_NOT_INDEPENDENTLY_VERIFIED: (
        "Declared offset applied once as supplied at ingestion; not independently verified."
    ),
    ClockCalibrationStatus.NOT_DECLARED_UNCALIBRATED: (
        "No offset declared; timestamps used as recorded. The clock is not known to be accurate."
    ),
    ClockCalibrationStatus.NOT_APPLIED_DECISION_WITHHELD: (
        "Clock metadata not applied because a required contract decision was withheld."
    ),
}


def _limitations(result: ReconstructionResult) -> LimitationsSection:
    limitations = [LimitationEntry(code="REPORT_SCOPE", message=SCOPE_LIMITATION_MESSAGE)]
    limitations.extend(
        LimitationEntry(code=lim.code, message=lim.message, subject_ids=list(lim.subject_ids))
        for lim in result.limitations
    )
    clocks = [
        ClockCalibrationEntry(
            evidence_id=e.evidence_id,
            clock_calibration=e.clock_calibration,
            clock_offset_seconds=e.clock_offset_seconds,
            source_timezone=e.source_timezone,
            note=_CALIBRATION_NOTES[e.clock_calibration],
        )
        for e in result.evidence
        if e.usable
    ]
    diagnostics = [
        DiagnosticEntry(
            severity=d.severity.value,
            stage=d.stage.value,
            code=d.code,
            subject_type=d.subject_type,
            subject_id=d.subject_id,
            message=d.message,
        )
        for d in result.diagnostics
        if d.severity in (DiagnosticSeverity.ERROR, DiagnosticSeverity.WARNING)
    ]
    return LimitationsSection(
        limitations=limitations,
        evidence_gaps=[
            EvidenceGapEntry(
                gap_type=g.gap_type.value,
                subject_type=g.subject_type,
                subject_id=g.subject_id,
                description=g.description,
                reason_code=g.reason_code,
            )
            for g in result.evidence_gaps
        ],
        unverified_clock_calibrations=clocks,
        pending_items=_pending_items(result),
        excluded_inputs=[
            {"subject_type": x.subject_type, "subject_id": x.subject_id, "reason_code": x.reason_code}
            for x in result.excluded_inputs
        ],
        diagnostics=diagnostics,
    )


def _custody(result: ReconstructionResult, inventory: Sequence[EvidenceInventoryItem]) -> CustodyAppendix:
    video = {v.evidence_id: v for v in result.video}
    keys = {e.evidence_id: e.storage_key for e in result.evidence}
    evidence: list[EvidenceCustodyEntry] = []
    for item in inventory:
        v = video.get(item.evidence_id)
        unchanged = None
        if v is not None and v.source_video_sha256_before and v.source_video_sha256_after:
            unchanged = v.source_video_sha256_before == v.source_video_sha256_after
        evidence.append(
            EvidenceCustodyEntry(
                evidence_id=item.evidence_id,
                sha256=item.sha256,
                storage_reference=item.storage_reference,
                storage_key=keys.get(item.evidence_id),
                integrity_status=item.integrity_status,
                video_sha256_before_processing=v.source_video_sha256_before if v else None,
                video_sha256_after_processing=v.source_video_sha256_after if v else None,
                video_source_unchanged=unchanged,
            )
        )
    frames = [
        FrameCustodyEntry(
            artifact_id=f.artifact_id,
            artifact_reference=f.artifact_reference,
            origin=f.origin.value,
            source_evidence_id=f.source_evidence_id,
            source_video_sha256=f.source_video_sha256,
            frame_index=f.frame_index,
            pts_offset_seconds=f.pts_offset_seconds,
            timestamp_utc=iso_z(f.timestamp_utc),
            timestamp_utc_anchored=f.timestamp_utc_anchored,
            sha256=f.sha256_hash,
        )
        for f in result.frame_artifacts
    ]
    return CustodyAppendix(
        evidence=evidence,
        video_processing=[
            VideoCustodyEntry(
                evidence_id=v.evidence_id,
                status=v.status.value,
                processing_kind=v.processing_kind,
                processing_note=v.processing_note,
                frame_handoff_status=v.frame_handoff_status.value if v.frame_handoff_status else None,
                frames_extracted=v.frames_extracted,
            )
            for v in result.video
        ],
        frame_artifacts=frames,
        finding_provenance=list(result.finding_provenance),
        processing_configuration=dict(result.configuration),
        run_interpretation_notice=result.interpretation_notice,
    )


def _lead_statement(entry: ContradictionEntry) -> str:
    speed = f"{entry.min_required_speed_kmh:.1f} km/h" if entry.min_required_speed_kmh is not None else "n/a"
    distance = f"{entry.distance_km:.2f} km" if entry.distance_km is not None else "an unknown distance"
    return (
        f"Analytical lead ({entry.lead_type}) for entity reference '{entry.entity_id}': events "
        f"'{entry.origin_event_id}' and '{entry.destination_event_id}' are {distance} apart; "
        f"reconciling them needs at least {speed} against a configured reference of "
        f"{entry.configured_speed_threshold_kmh:g} km/h. Not a finding of fact."
    )


def _float_config(configuration: Mapping[str, Any], key: str) -> float | None:
    value = configuration.get(key)
    return float(value) if isinstance(value, (int, float)) and not isinstance(value, bool) else None


def build_forensic_report(result: ReconstructionResult, *, generated_at: datetime) -> ForensicReport:
    """Build the structured report for one run. ``generated_at`` must be timezone-aware."""
    if generated_at.tzinfo is None:
        raise ValueError("generated_at must be timezone-aware.")
    index = _CitationIndex(result)
    inventory = _inventory(result)
    timeline = _timeline(result, index)
    contradictions = _contradictions(result, index)
    limitations = _limitations(result)
    severities: defaultdict[str, int] = defaultdict(int)
    for d in result.diagnostics:
        severities[d.severity.value] += 1

    windows = [e for e in result.timeline_events if e.uncertainty_window_start and e.uncertainty_window_end]
    span_start = min((e.uncertainty_window_start for e in windows if e.uncertainty_window_start), default=None)
    span_end = max((e.uncertainty_window_end for e in windows if e.uncertainty_window_end), default=None)

    observations = _observations(result, index)
    movements = _movements(result, index)
    relationships = _relationships(result, index)
    counts = {
        "evidence_items": len(inventory),
        "evidence_verified": sum(1 for i in inventory if i.integrity_status == IntegrityStatus.VERIFIED),
        "evidence_excluded": sum(1 for i in inventory if not i.usable),
        "timeline_events": len(timeline),
        "spatial_observations": len(observations),
        "movement_segments": len(movements),
        "contradiction_leads": len(contradictions),
        "relationships": len(relationships),
        "excluded_inputs": len(result.excluded_inputs),
        "evidence_gaps": len(result.evidence_gaps),
        "limitations": len(limitations.limitations),
        "diagnostics_error": severities["ERROR"],
        "diagnostics_warning": severities["WARNING"],
        "diagnostics_info": severities["INFO"],
    }
    report = ForensicReport(
        title=f"Reconstruction report — case {result.case_id}",
        metadata=ReportMetadata(
            case_id=result.case_id,
            run_id=result.run_id,
            run_status=result.status,
            run_status_explanation=_STATUS_EXPLANATIONS[result.status],
            algorithm_version=result.algorithm_version,
            configuration_version=result.configuration_version,
            snapshot_schema_version=result.snapshot_schema_version,
            result_sha256=result.result_sha256,
            generated_at_utc=iso_z(generated_at) or "",
            relationship_export_status=result.relationship_export_status,
            approved_contract_decisions=list(result.approved_contract_decisions),
            withheld_contract_decisions=list(result.withheld_contract_decisions),
            unverified_contract_decisions=list(result.unverified_contract_decisions),
            approval_record=result.approval_record,
            configured_speed_threshold_kmh=_float_config(result.configuration, "speed_threshold_kmh"),
            estimated_half_width_seconds=_float_config(result.configuration, "estimated_half_width_seconds"),
        ),
        summary=CaseSummary(
            counts=counts,
            timeline_span_start_utc=iso_z(span_start),
            timeline_span_end_utc=iso_z(span_end),
            key_leads=[
                LeadSummary(ref=c.ref, lead_id=c.lead_id, statement=_lead_statement(c), citations=c.citations)
                for c in contradictions
            ],
            interpretation_notice=result.interpretation_notice,
        ),
        evidence_inventory=inventory,
        timeline=timeline,
        spatial_observations=observations,
        movement_segments=movements,
        contradictions=contradictions,
        contradiction_pairs_not_evaluated=[
            UnevaluatedPair(entity_id=o.entity_id, event_a_id=o.event_a_id, event_b_id=o.event_b_id)
            for o in result.contradiction_overlaps_not_evaluated
        ],
        relationships=relationships,
        limitations=limitations,
        chain_of_custody=_custody(result, inventory),
    )
    return report.model_copy(update={"content_sha256": report.compute_content_sha256()})

