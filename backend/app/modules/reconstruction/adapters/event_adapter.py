"""Event Candidate Adapter (Member 4 — Phase 3E).

Consumes Member 1 event candidates, i.e. the JSON serialization of the Rust
``UnifiedExtractionResult.event_candidates``, and prepares them for the temporal, spatial and
contradiction engines.

Candidate contract (Member 1):

    candidate_id, source_evidence_id, source_record_id, raw_timestamp, timestamp_precision,
    event_type, description, involved_entity_raw_ids, coordinates, duration_seconds,
    extraction_confidence

Approved temporal rules:

- DEC-007: extraction preserves ``raw_timestamp`` unchanged. Only the Member 4 temporal engine
  produces the corrected UTC instant (``timestamp_utc``, which fills shared ``Event.timestamp``).
  Localization and the clock offset are applied exactly once, from the raw value. A raw value
  that already carries ``Z`` or a numeric offset is parsed as that instant and never localized
  again. A naive raw value needs a declared ``source_timezone``; without one it is rejected
  (``SOURCE_TIMEZONE_UNKNOWN``), never assumed to be UTC.
- DEC-001: ``timestamp_utc = localize(raw).astimezone(UTC) + clock_offset_seconds``. A fast
  clock has a negative offset.
- DEC-002: precision is one of EXACT_SECOND, MINUTE, HOUR, DAY, ESTIMATED. Calendar units are
  floored in local time before conversion. Windows are half open: start <= nominal < end.
  Legacy ``EXACT`` / ``APPROXIMATE`` are mapped, with a warning diagnostic.
- DEC-005: ``duration_seconds`` is optional, finite and >= 0. It is kept separate from the
  uncertainty window. Missing values stay null.

Fields that are not part of the candidate contract are never used as inputs. This includes
derived output fields such as ``timestamp_utc``, ``uncertainty_window_*``,
``clock_offset_seconds`` and ``source_timezone``, which appear when a reconstruction snapshot is
fed back in. Such fields are ignored and reported in a diagnostic, so a re-run cannot correct
twice and a candidate cannot override evidence-level clock settings.
"""

import json
import math
from collections.abc import Iterable, Mapping, Sequence
from datetime import datetime
from enum import StrEnum
from typing import Any

from pydantic import BaseModel, ConfigDict, Field, model_validator

from app.modules.reconstruction.adapters.contracts import (
    TEMPORAL_CORRECTION_DECISIONS,
    ContractDecision,
    normalize_approved_decisions,
    require_decisions,
)
from app.modules.reconstruction.adapters.diagnostics import (
    DiagnosticSeverity,
    PipelineStage,
    ReconstructionDiagnostic,
)
from app.modules.reconstruction.adapters.errors import (
    EventCandidateError,
    EventCandidatePayloadError,
    ProducerContractGapError,
    SourceTimezoneUnknownError,
)
from app.modules.reconstruction.adapters.evidence_adapter import (
    ClockCalibrationStatus,
    EvidenceRecordView,
    EvidenceTimeSettings,
)
from app.modules.reconstruction.spatial import validate_geopoint
from app.modules.reconstruction.temporal import (
    DEFAULT_ESTIMATED_HALF_WIDTH_SECONDS,
    normalize_event_time,
)
from app.schemas.reconstruction import (
    LEGACY_TIME_PRECISION_ALIASES,
    CanonicalTimePrecision,
    GeoPoint,
    NormalizedTimeWindow,
    TemporalOrderingStatus,
)

CANONICAL_PRECISION_TOKENS: frozenset[str] = frozenset(p.value for p in CanonicalTimePrecision)

CANDIDATE_FIELDS: tuple[str, ...] = (
    "candidate_id",
    "source_evidence_id",
    "source_record_id",
    "raw_timestamp",
    "timestamp_precision",
    "event_type",
    "description",
    "involved_entity_raw_ids",
    "coordinates",
    "duration_seconds",
    "extraction_confidence",
)

# Contract fields that have no safe derivation when a producer omits them. Precision is not
# inferred from timestamp formatting, and a description is never synthesized from event_type.
PRODUCER_REQUIRED_FIELDS: tuple[str, ...] = ("timestamp_precision", "description")

TIME_DERIVATION = "YUKTI_M4_TEMPORAL_ENGINE_FROM_RAW_TIMESTAMP"


class TimezoneBasis(StrEnum):
    """How the UTC instant of a raw timestamp was established."""

    TIMESTAMP_UTC_OFFSET = "TIMESTAMP_UTC_OFFSET"
    SOURCE_TIMEZONE_METADATA = "SOURCE_TIMEZONE_METADATA"


class AdaptedEvent(BaseModel):
    """A validated candidate with its preserved raw inputs and the engine's time window."""

    model_config = ConfigDict(frozen=True)

    event_id: str = Field(description="Equal to candidate_id.")
    case_id: str
    candidate_id: str
    source_evidence_id: str
    source_record_id: str | None = None
    event_type: str
    description: str
    raw_timestamp: str = Field(description="raw_timestamp exactly as supplied.")
    raw_timestamp_precision: str = Field(description="timestamp_precision exactly as supplied.")
    canonical_precision: CanonicalTimePrecision
    precision_resolved_via_legacy_alias: bool = False
    timezone_basis: TimezoneBasis
    coordinates: GeoPoint | None = None
    involved_entity_raw_ids: list[str] = Field(default_factory=list)
    involved_entity_raw_ids_present: bool = True
    duration_seconds: float | None = None
    extraction_confidence: float | None = None
    source_fields_preserved: dict[str, Any] = Field(
        default_factory=dict,
        description="Producer fields outside the candidate contract, kept verbatim, not interpreted.",
    )
    time_settings: EvidenceTimeSettings
    time_window: NormalizedTimeWindow

    def to_engine_record(self, canonical_entity_ids: Sequence[str]) -> dict[str, Any]:
        """Plain record for the engines. The raw timestamp is passed, never a corrected one."""
        record: dict[str, Any] = {
            "event_id": self.event_id,
            "case_id": self.case_id,
            "title": self.description,
            "source_evidence_id": self.source_evidence_id,
            "timestamp": self.raw_timestamp,
            "timestamp_precision": self.canonical_precision.value,
            "location_name": None,
            "coordinates": (
                {"lat": self.coordinates.lat, "lon": self.coordinates.lon}
                if self.coordinates is not None
                else None
            ),
            "involved_entity_ids": list(canonical_entity_ids),
        }
        if self.duration_seconds is not None:
            record["duration_seconds"] = self.duration_seconds
        return record


class ReconstructedEvent(BaseModel):
    """Shared-facing timeline event produced by the Member 4 temporal engine.

    It carries the untouched candidate inputs next to the derived UTC values. ``timestamp_utc``
    is the value for shared ``Event.timestamp``. The candidate-contract fields keep their
    names, so a snapshot's events can be fed back as candidates, and every derived field is then
    ignored.
    """

    model_config = ConfigDict(frozen=True)

    # Candidate contract (inputs, preserved verbatim).
    candidate_id: str
    source_evidence_id: str
    source_record_id: str | None = None
    raw_timestamp: str
    timestamp_precision: CanonicalTimePrecision
    event_type: str
    description: str
    involved_entity_raw_ids: list[str] = Field(default_factory=list)
    coordinates: GeoPoint | None = None
    duration_seconds: float | None = None
    extraction_confidence: float | None = None
    source_fields_preserved: dict[str, Any] = Field(
        default_factory=dict,
        description=(
            "Producer fields outside the candidate contract (e.g. Member 1 location_name, "
            "metadata), kept verbatim. Member 4 does not interpret them."
        ),
    )

    # Derived by Member 4.
    event_id: str
    case_id: str
    raw_timestamp_precision: str
    timestamp_utc: datetime
    uncertainty_window_start: datetime | None = None
    uncertainty_window_end: datetime | None = None
    is_estimated_window_assumption: bool = False
    source_timezone: str | None = Field(
        default=None, description="Declared IANA timezone of the source; null when not declared."
    )
    timezone_basis: TimezoneBasis
    clock_offset_seconds: float | None = Field(
        default=None, description="Declared additive correction (DEC-001); null when not declared."
    )
    clock_calibration: ClockCalibrationStatus
    time_derivation: str = TIME_DERIVATION
    involved_entity_ids: list[str] = Field(default_factory=list)
    unresolved_entity_raw_ids: list[str] = Field(default_factory=list)
    ordering_status: TemporalOrderingStatus
    concurrent_event_ids: list[str] = Field(default_factory=list)
    evidence_sha256: str | None = Field(
        default=None, description="SHA-256 of the source evidence recorded at ingestion."
    )
    supporting_frame_artifact_ids: list[str] = Field(
        default_factory=list,
        description="Frames from the same evidence whose UTC window overlaps (temporal alignment only).",
    )

    @model_validator(mode="after")
    def _validate_dec005(self) -> "ReconstructedEvent":
        start, end = self.uncertainty_window_start, self.uncertainty_window_end
        if (start is None) != (end is None):
            raise ValueError("uncertainty_window_start and _end must both be present or both absent.")
        if start is not None and end is not None:
            if not start < end:
                raise ValueError("uncertainty_window_start must be strictly before _end.")
            if not start <= self.timestamp_utc < end:
                raise ValueError("timestamp_utc must satisfy start <= timestamp_utc < end.")
        if self.duration_seconds is not None and (
            not math.isfinite(self.duration_seconds) or self.duration_seconds < 0.0
        ):
            raise ValueError("duration_seconds must be finite and non-negative.")
        return self


DERIVED_OUTPUT_FIELDS: frozenset[str] = frozenset(
    set(ReconstructedEvent.model_fields) - set(CANDIDATE_FIELDS)
) | {"timestamp", "title"}


class CandidateAdaptation(BaseModel):
    """An adapted candidate plus the non-fatal diagnostics it produced."""

    model_config = ConfigDict(frozen=True)

    event: AdaptedEvent
    diagnostics: list[ReconstructionDiagnostic] = Field(default_factory=list)


def _reject_json_constant(token: str) -> float:
    raise ValueError(f"non-finite JSON number '{token}' is not allowed")


def parse_event_candidates_payload(payload: Any) -> list[Any]:
    """Return the candidate items from JSON text, a decoded container, or a sequence.

    Accepted shapes: a JSON array of candidates, or a ``UnifiedExtractionResult`` object with
    an ``event_candidates`` array. Items are validated individually by
    ``adapt_event_candidate``.
    """
    decoded: Any = payload
    if isinstance(payload, BaseModel):
        decoded = payload.model_dump(mode="json")
    if isinstance(payload, (str, bytes, bytearray)):
        try:
            decoded = json.loads(payload, parse_constant=_reject_json_constant)
        except (json.JSONDecodeError, UnicodeDecodeError, ValueError) as exc:
            raise EventCandidatePayloadError(f"Event candidate JSON is malformed: {exc}") from exc

    if isinstance(decoded, Mapping):
        if "event_candidates" not in decoded:
            raise EventCandidatePayloadError(
                "Event candidate payload object has no 'event_candidates' array "
                "(expected UnifiedExtractionResult serialization)."
            )
        decoded = decoded["event_candidates"]
        if decoded is None:
            raise EventCandidatePayloadError("'event_candidates' is null; expected an array.")

    if isinstance(decoded, Sequence) and not isinstance(decoded, (str, bytes, bytearray)):
        return [
            item.model_dump(mode="json") if isinstance(item, BaseModel) else item for item in decoded
        ]
    raise EventCandidatePayloadError(
        "Event candidate payload must be a JSON array of candidates or an object with an "
        f"'event_candidates' array, got {type(decoded).__name__}."
    )


def resolve_candidate_precision(
    raw_precision: Any,
    *,
    approved: frozenset[ContractDecision],
    subject_id: str | None = None,
) -> tuple[str, CanonicalTimePrecision, bool]:
    """Return (raw_token, canonical, via_legacy_alias). Exact, case-sensitive match."""
    if not isinstance(raw_precision, str) or not raw_precision:
        raise EventCandidateError(
            f"Candidate '{subject_id}' is missing a valid string 'timestamp_precision'.",
            subject_id=subject_id,
        )
    # The canonical tokens are themselves the DEC-002 proposal, so every precision value needs it.
    require_decisions(
        approved,
        ContractDecision.TIME_PRECISION_SEMANTICS,
        context=f"Candidate '{subject_id}' timestamp_precision '{raw_precision}'",
        subject_id=subject_id,
    )
    if raw_precision in CANONICAL_PRECISION_TOKENS:
        return raw_precision, CanonicalTimePrecision(raw_precision), False
    if raw_precision in LEGACY_TIME_PRECISION_ALIASES:
        return raw_precision, LEGACY_TIME_PRECISION_ALIASES[raw_precision], True
    raise EventCandidateError(
        f"Candidate '{subject_id}' has unsupported timestamp_precision '{raw_precision}'. "
        f"Approved values (DEC-002): {sorted(CANONICAL_PRECISION_TOKENS)}.",
        subject_id=subject_id,
    )


def _parse_raw_timestamp(raw: Any, subject_id: str) -> tuple[str, bool]:
    """Return (raw_timestamp, is_naive). The string is returned exactly as supplied."""
    if not isinstance(raw, str) or not raw.strip():
        raise EventCandidateError(
            f"Candidate '{subject_id}' is missing a valid ISO 8601 string 'raw_timestamp'.",
            subject_id=subject_id,
        )
    text = raw.strip()
    if not any(sep in text for sep in ("T", "t", " ")):
        raise EventCandidateError(
            f"Candidate '{subject_id}' raw_timestamp has no time component; date-only values "
            "are not supported (supply local midnight with precision DAY).",
            subject_id=subject_id,
        )
    try:
        parsed = datetime.fromisoformat(text)
    except ValueError as exc:
        raise EventCandidateError(
            f"Candidate '{subject_id}' raw_timestamp is not valid ISO 8601: {exc}",
            subject_id=subject_id,
        ) from exc
    return raw, parsed.tzinfo is None or parsed.utcoffset() is None


def _optional_str(value: Any, name: str, issues: list[str]) -> str | None:
    if value is None:
        return None
    if not isinstance(value, str) or not value.strip():
        issues.append(f"'{name}' must be a non-empty string or null")
        return None
    return value


def _required_str(value: Any, name: str, issues: list[str]) -> str:
    if not isinstance(value, str) or not value.strip():
        issues.append(f"'{name}' must be a non-empty string")
        return ""
    return value


def _optional_number(
    value: Any, name: str, issues: list[str], *, low: float, high: float | None
) -> float | None:
    if value is None:
        return None
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        issues.append(f"'{name}' must be a number or null")
        return None
    number = float(value)
    if not math.isfinite(number) or number < low or (high is not None and number > high):
        bound = f"[{low}, {high}]" if high is not None else f">= {low}"
        issues.append(f"'{name}' must be finite and {bound}")
        return None
    return number


def _diag(
    severity: DiagnosticSeverity, code: str, message: str, subject_id: str
) -> ReconstructionDiagnostic:
    return ReconstructionDiagnostic(
        severity=severity,
        stage=PipelineStage.EVENTS,
        code=code,
        message=message,
        subject_type="EVENT_CANDIDATE",
        subject_id=subject_id,
    )


def adapt_event_candidate(
    candidate: Any,
    evidence_by_id: Mapping[str, EvidenceRecordView],
    *,
    case_id: str,
    approved_decisions: Iterable[ContractDecision | str] | None = None,
    estimated_half_width_seconds: float = DEFAULT_ESTIMATED_HALF_WIDTH_SECONDS,
) -> CandidateAdaptation:
    """Validate and normalize one event candidate. Never mutates the input.

    Raises ``EventCandidateError`` (or a subclass, or ``ContractDecisionPendingError``) and
    lists every structural problem of the candidate in the message.
    """
    approved = normalize_approved_decisions(approved_decisions)
    if not isinstance(candidate, Mapping):
        raise EventCandidateError(
            f"Event candidate must be a JSON object, got {type(candidate).__name__}."
        )

    raw_id = candidate.get("candidate_id")
    subject = raw_id.strip() if isinstance(raw_id, str) and raw_id.strip() else None
    label = f"Candidate '{subject or '<missing candidate_id>'}'"

    absent = [name for name in PRODUCER_REQUIRED_FIELDS if name not in candidate]
    if absent:
        outside = sorted(set(candidate) - set(CANDIDATE_FIELDS))
        raise ProducerContractGapError(
            f"{label} does not carry the candidate-contract field(s) {absent}. Member 4 does not "
            "infer timestamp precision from timestamp formatting or synthesize a description, so "
            "the producer must supply them (see docs/member-4/member1-candidate-contract-gap.md). "
            f"Producer fields outside the contract, not processed: {outside}.",
            subject_id=subject,
        )

    issues: list[str] = []
    diagnostics: list[ReconstructionDiagnostic] = []

    candidate_id = _required_str(raw_id, "candidate_id", issues).strip()
    source_evidence_id = _required_str(
        candidate.get("source_evidence_id"), "source_evidence_id", issues
    ).strip()
    source_record_id = _optional_str(candidate.get("source_record_id"), "source_record_id", issues)
    event_type = _required_str(candidate.get("event_type"), "event_type", issues)
    description = _required_str(candidate.get("description"), "description", issues)

    raw_refs = candidate.get("involved_entity_raw_ids")
    refs: list[str] = []
    if raw_refs is not None:
        if isinstance(raw_refs, Sequence) and not isinstance(raw_refs, (str, bytes)):
            for ref in raw_refs:
                if not isinstance(ref, str) or not ref.strip():
                    issues.append("'involved_entity_raw_ids' contains an empty or non-string item")
                    break
                refs.append(ref.strip())
        else:
            issues.append("'involved_entity_raw_ids' must be an array of strings or null")

    coordinates: GeoPoint | None = None
    try:
        coordinates = validate_geopoint(candidate.get("coordinates"))
    except ValueError as exc:
        issues.append(f"'coordinates' invalid: {exc}")

    duration = _optional_number(
        candidate.get("duration_seconds"), "duration_seconds", issues, low=0.0, high=None
    )
    confidence = _optional_number(
        candidate.get("extraction_confidence"), "extraction_confidence", issues, low=0.0, high=1.0
    )

    if issues:
        raise EventCandidateError(f"{label} is malformed: {'; '.join(issues)}.", subject_id=subject)

    require_decisions(
        approved,
        *TEMPORAL_CORRECTION_DECISIONS,
        context=f"{label} UTC normalization",
        subject_id=candidate_id,
    )
    if duration is not None:
        require_decisions(
            approved,
            ContractDecision.EVENT_DURATION_UNCERTAINTY,
            context=f"{label} duration_seconds",
            subject_id=candidate_id,
        )

    evidence = evidence_by_id.get(source_evidence_id)
    if evidence is None:
        raise EventCandidateError(
            f"{label} references source_evidence_id '{source_evidence_id}', which is not a "
            "validated Evidence record of this case.",
            subject_id=candidate_id,
        )
    settings = evidence.time_settings

    raw_precision, canonical_precision, via_alias = resolve_candidate_precision(
        candidate.get("timestamp_precision"), approved=approved, subject_id=candidate_id
    )
    if via_alias:
        diagnostics.append(
            _diag(
                DiagnosticSeverity.WARNING,
                "LEGACY_TIME_PRECISION_MAPPED",
                f"Legacy timestamp_precision '{raw_precision}' was mapped to "
                f"'{canonical_precision.value}' (DEC-002). The producer should emit approved values.",
                candidate_id,
            )
        )

    raw_timestamp, is_naive = _parse_raw_timestamp(candidate.get("raw_timestamp"), candidate_id)
    if is_naive and settings.source_timezone is None:
        raise SourceTimezoneUnknownError(
            f"{label} raw_timestamp has no UTC offset and evidence '{source_evidence_id}' declares "
            "no source_timezone. The UTC instant is unknown; it is not assumed to be UTC.",
            subject_id=candidate_id,
        )
    basis = TimezoneBasis.SOURCE_TIMEZONE_METADATA if is_naive else TimezoneBasis.TIMESTAMP_UTC_OFFSET

    ignored = sorted(set(candidate) - set(CANDIDATE_FIELDS))
    derived = [name for name in ignored if name in DERIVED_OUTPUT_FIELDS]
    unknown = [name for name in ignored if name not in DERIVED_OUTPUT_FIELDS]
    if derived:
        diagnostics.append(
            _diag(
                DiagnosticSeverity.WARNING,
                "DERIVED_FIELDS_IGNORED",
                f"Derived fields {derived} were ignored. UTC values are recomputed from "
                "raw_timestamp and the evidence clock settings, so no correction is applied twice.",
                candidate_id,
            )
        )
    preserved: dict[str, Any] = {}
    if unknown:
        preserved = {name: candidate[name] for name in unknown}
        try:
            preserved = json.loads(json.dumps(preserved, allow_nan=False))
        except (TypeError, ValueError) as exc:
            raise EventCandidateError(
                f"{label} has non-JSON source fields {unknown}; they cannot be preserved: {exc}",
                subject_id=candidate_id,
            ) from exc
        diagnostics.append(
            _diag(
                DiagnosticSeverity.INFO,
                "SOURCE_FIELDS_PRESERVED",
                f"Fields {unknown} are not part of the event candidate contract. They were "
                "preserved verbatim in source_fields_preserved and not interpreted.",
                candidate_id,
            )
        )
    if raw_refs is None:
        diagnostics.append(
            _diag(
                DiagnosticSeverity.INFO,
                "CANDIDATE_HAS_NO_ENTITY_REFERENCES",
                "involved_entity_raw_ids is null; the event appears on the timeline only.",
                candidate_id,
            )
        )
    if source_record_id is None:
        diagnostics.append(
            _diag(
                DiagnosticSeverity.INFO,
                "SOURCE_RECORD_ID_MISSING",
                "source_record_id is null; provenance is limited to source_evidence_id.",
                candidate_id,
            )
        )

    probe: dict[str, Any] = {
        "event_id": candidate_id,
        "timestamp": raw_timestamp,
        "timestamp_precision": canonical_precision.value,
        "duration_seconds": duration,
    }
    try:
        window = normalize_event_time(
            probe,
            evidence_metadata=settings.as_temporal_evidence_record(),
            estimated_half_width_seconds=estimated_half_width_seconds,
        )
    except ValueError as exc:
        raise EventCandidateError(f"{label}: {exc}", subject_id=candidate_id) from exc
    if not window.window_start_utc <= window.nominal_utc < window.window_end_utc:
        raise EventCandidateError(
            f"{label}: derived window violates start <= nominal < end.", subject_id=candidate_id
        )

    event = AdaptedEvent(
        event_id=candidate_id,
        case_id=case_id,
        candidate_id=candidate_id,
        source_evidence_id=source_evidence_id,
        source_record_id=source_record_id,
        event_type=event_type,
        description=description,
        raw_timestamp=raw_timestamp,
        raw_timestamp_precision=raw_precision,
        canonical_precision=canonical_precision,
        precision_resolved_via_legacy_alias=via_alias,
        timezone_basis=basis,
        coordinates=coordinates,
        involved_entity_raw_ids=refs,
        involved_entity_raw_ids_present=raw_refs is not None,
        duration_seconds=duration,
        extraction_confidence=confidence,
        source_fields_preserved=preserved,
        time_settings=settings,
        time_window=window,
    )
    return CandidateAdaptation(event=event, diagnostics=diagnostics)


def build_reconstructed_event(
    adapted: AdaptedEvent,
    *,
    time_window: NormalizedTimeWindow,
    involved_entity_ids: Sequence[str],
    unresolved_entity_raw_ids: Sequence[str],
    ordering_status: TemporalOrderingStatus,
    concurrent_event_ids: Sequence[str],
    evidence_sha256: str | None = None,
) -> ReconstructedEvent:
    """Combine an adapted candidate with the engine's timeline output."""
    settings = adapted.time_settings
    return ReconstructedEvent(
        candidate_id=adapted.candidate_id,
        source_evidence_id=adapted.source_evidence_id,
        source_record_id=adapted.source_record_id,
        raw_timestamp=adapted.raw_timestamp,
        timestamp_precision=adapted.canonical_precision,
        event_type=adapted.event_type,
        description=adapted.description,
        involved_entity_raw_ids=list(adapted.involved_entity_raw_ids),
        coordinates=adapted.coordinates,
        duration_seconds=adapted.duration_seconds,
        extraction_confidence=adapted.extraction_confidence,
        source_fields_preserved=dict(adapted.source_fields_preserved),
        event_id=adapted.event_id,
        case_id=adapted.case_id,
        raw_timestamp_precision=adapted.raw_timestamp_precision,
        timestamp_utc=time_window.nominal_utc,
        uncertainty_window_start=time_window.window_start_utc,
        uncertainty_window_end=time_window.window_end_utc,
        is_estimated_window_assumption=time_window.is_estimated_window_assumption,
        source_timezone=settings.source_timezone,
        timezone_basis=adapted.timezone_basis,
        clock_offset_seconds=settings.clock_offset_seconds,
        clock_calibration=settings.clock_calibration,
        involved_entity_ids=list(involved_entity_ids),
        unresolved_entity_raw_ids=list(unresolved_entity_raw_ids),
        ordering_status=ordering_status,
        concurrent_event_ids=list(concurrent_event_ids),
        evidence_sha256=evidence_sha256,
    )


def build_temporal_evidence_index(
    evidence_by_id: Mapping[str, EvidenceRecordView],
) -> dict[str, dict[str, Any]]:
    """Evidence index for the engines containing only validated time settings."""
    return {
        evidence_id: view.time_settings.as_temporal_evidence_record()
        for evidence_id, view in sorted(evidence_by_id.items())
    }
