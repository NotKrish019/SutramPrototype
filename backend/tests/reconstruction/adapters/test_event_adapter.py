"""Phase 3E — Event Candidate Adapter tests (Member 1 contract, DEC-001/002/005/007)."""

import json
from copy import deepcopy
from datetime import UTC, datetime, timedelta
from typing import Any

import pytest
from tests.reconstruction.adapters.synthetic_burglary import CASE_ID, SyntheticCase

from app.modules.reconstruction.adapters.contracts import (
    PROPOSED_CONTRACT_DECISIONS,
    ContractDecision,
)
from app.modules.reconstruction.adapters.errors import (
    ContractDecisionPendingError,
    EventCandidateError,
    EventCandidatePayloadError,
    SourceTimezoneUnknownError,
)
from app.modules.reconstruction.adapters.event_adapter import (
    CANDIDATE_FIELDS,
    CandidateAdaptation,
    ReconstructedEvent,
    TimezoneBasis,
    adapt_event_candidate,
    build_temporal_evidence_index,
    parse_event_candidates_payload,
)
from app.modules.reconstruction.adapters.evidence_adapter import (
    ClockCalibrationStatus,
    EvidenceRecordView,
    parse_evidence_record,
)
from app.modules.reconstruction.temporal import reconstruct_timeline
from app.schemas.reconstruction import CanonicalTimePrecision, TemporalOrderingStatus

# Behaviour conditional on the PROPOSED decisions (none is approved by default).
ALL = PROPOSED_CONTRACT_DECISIONS


def _views(case: SyntheticCase, approved: Any = ALL) -> dict[str, EvidenceRecordView]:
    return {
        r["evidence_id"]: parse_evidence_record(r, case_id=CASE_ID, approved_decisions=approved)
        for r in case.evidence
    }


def _cand(case: SyntheticCase, cid: str, /, **overrides: object) -> dict[str, Any]:
    record = deepcopy(case.candidate_by_id(cid))
    record.update(overrides)
    return record


def _adapt(case: SyntheticCase, record: Any, approved: Any = ALL) -> CandidateAdaptation:
    return adapt_event_candidate(
        record, _views(case, approved), case_id=CASE_ID, approved_decisions=approved
    )


# --- Payload parsing ----------------------------------------------------------------------


def test_payload_accepts_unified_extraction_json(burglary_case: SyntheticCase) -> None:
    items = parse_event_candidates_payload(burglary_case.unified_extraction_json())
    assert items == burglary_case.candidates


@pytest.mark.parametrize(
    "payload_factory",
    [
        lambda c: c.candidates,
        lambda c: json.dumps(c.candidates),
        lambda c: json.dumps(c.candidates).encode("utf-8"),
        lambda c: {"event_candidates": c.candidates},
    ],
)
def test_payload_shapes(burglary_case: SyntheticCase, payload_factory: Any) -> None:
    assert parse_event_candidates_payload(payload_factory(burglary_case)) == burglary_case.candidates


@pytest.mark.parametrize(
    ("payload", "match"),
    [
        ("{not json", "malformed"),
        ('[{"duration_seconds": NaN}]', "malformed"),
        ('{"entity_candidates": []}', "event_candidates"),
        ('{"event_candidates": null}', "null"),
        ('"just a string"', "JSON array"),
        (42, "JSON array"),
    ],
)
def test_malformed_payload_rejected(payload: Any, match: str) -> None:
    with pytest.raises(EventCandidatePayloadError, match=match):
        parse_event_candidates_payload(payload)


# --- Valid candidates ---------------------------------------------------------------------


def test_valid_candidate_preserves_contract_fields(burglary_case: SyntheticCase) -> None:
    record = burglary_case.candidate_by_id("evt-01-shutter-forced")
    original = deepcopy(record)
    adaptation = _adapt(burglary_case, record)
    event = adaptation.event
    assert record == original  # never mutated
    assert event.event_id == event.candidate_id == "evt-01-shutter-forced"
    assert event.source_record_id == "osd:2026-11-10T21:42:00"
    assert event.raw_timestamp == "2026-11-10T21:42:00"  # raw never overwritten
    assert event.event_type == "FORCED_ENTRY"
    assert event.duration_seconds == 30.0
    assert event.extraction_confidence == 0.9
    assert event.coordinates is not None and event.coordinates.lon == 77.2710
    assert adaptation.diagnostics == []


# --- DEC-001 / DEC-007: corrected exactly once ----------------------------------------------


def test_naive_raw_localized_once_and_fast_clock_offset_applied_once(
    burglary_case: SyntheticCase,
) -> None:
    event = _adapt(burglary_case, burglary_case.candidate_by_id("evt-01-shutter-forced")).event
    # 21:42:00 IST = 16:12:00Z; DVR 120 s fast → offset -120 → 16:10:00Z (not 16:08:00Z).
    raw_as_utc = datetime(2026, 11, 10, 16, 12, tzinfo=UTC)
    assert event.time_window.nominal_utc == raw_as_utc + timedelta(seconds=-120)
    assert event.time_window.clock_offset_seconds == -120.0
    assert event.timezone_basis == TimezoneBasis.SOURCE_TIMEZONE_METADATA


@pytest.mark.parametrize(
    "raw",
    ["2026-11-10T16:12:00Z", "2026-11-10T16:12:00+00:00", "2026-11-10T21:42:00+05:30"],
)
def test_aware_raw_is_not_localized_again(burglary_case: SyntheticCase, raw: str) -> None:
    record = _cand(burglary_case, "evt-01-shutter-forced", raw_timestamp=raw)
    event = _adapt(burglary_case, record).event
    # Same instant 16:12Z; only the -120 s clock correction is applied (once). If the Z value
    # were re-localized as IST, the result would be 10:40Z.
    assert event.time_window.nominal_utc == datetime(2026, 11, 10, 16, 10, tzinfo=UTC)
    assert event.timezone_basis == TimezoneBasis.TIMESTAMP_UTC_OFFSET
    assert event.raw_timestamp == raw


def test_engine_rederivation_matches_adapter(burglary_case: SyntheticCase) -> None:
    views = _views(burglary_case)
    event = _adapt(burglary_case, burglary_case.candidate_by_id("evt-01-shutter-forced")).event
    timeline = reconstruct_timeline(
        [event.to_engine_record(["ent-person-ravi"])],
        evidence_by_id=build_temporal_evidence_index(views),
        case_id=CASE_ID,
    )
    assert timeline.entries[0].time_window == event.time_window
    assert event.to_engine_record([])["timestamp"] == event.raw_timestamp


def test_naive_raw_without_declared_timezone_rejected(burglary_case: SyntheticCase) -> None:
    record = _cand(burglary_case, "evt-02-outbound-call", raw_timestamp="2026-11-10T16:12:00")
    with pytest.raises(SourceTimezoneUnknownError, match="not assumed to be UTC") as excinfo:
        _adapt(burglary_case, record)
    assert excinfo.value.code == "SOURCE_TIMEZONE_UNKNOWN"


def test_undeclared_clock_offset_is_reported_uncalibrated(burglary_case: SyntheticCase) -> None:
    event = _adapt(burglary_case, burglary_case.candidate_by_id("evt-02-outbound-call")).event
    assert event.time_settings.clock_offset_seconds is None
    assert event.time_settings.clock_calibration == ClockCalibrationStatus.NOT_DECLARED_UNCALIBRATED
    assert event.time_window.nominal_utc == datetime(2026, 11, 10, 16, 12, tzinfo=UTC)


def test_derived_fields_in_input_are_ignored(burglary_case: SyntheticCase) -> None:
    record = _cand(
        burglary_case,
        "evt-01-shutter-forced",
        timestamp_utc="2026-11-10T16:08:00Z",
        timestamp="2026-11-10T16:08:00Z",
        clock_offset_seconds=-240.0,
        source_timezone="UTC",
        uncertainty_window_start="2026-11-10T16:00:00Z",
        uncertainty_window_end="2026-11-10T16:30:00Z",
    )
    adaptation = _adapt(burglary_case, record)
    assert adaptation.event.time_window.nominal_utc == datetime(2026, 11, 10, 16, 10, tzinfo=UTC)
    assert adaptation.event.time_window.has_explicit_bounds is False
    (diag,) = [d for d in adaptation.diagnostics if d.code == "DERIVED_FIELDS_IGNORED"]
    assert "timestamp_utc" in diag.message and "clock_offset_seconds" in diag.message


def test_unknown_fields_preserved_verbatim_and_reported(burglary_case: SyntheticCase) -> None:
    meta = {"call_type": "VOICE_CALL", "row_index": 3, "nested": {"a": [1, None]}}
    record = _cand(
        burglary_case, "evt-02-outbound-call", location_name="Tower 7B", metadata=meta, ocr_engine="x"
    )
    adaptation = _adapt(burglary_case, record)
    (diag,) = [d for d in adaptation.diagnostics if d.code == "SOURCE_FIELDS_PRESERVED"]
    assert "location_name" in diag.message and "ocr_engine" in diag.message
    assert adaptation.event.source_fields_preserved == {
        "location_name": "Tower 7B",
        "metadata": meta,
        "ocr_engine": "x",
    }
    assert record["metadata"] == meta  # input not mutated


def test_non_json_source_fields_are_rejected_not_dropped(burglary_case: SyntheticCase) -> None:
    record = _cand(burglary_case, "evt-02-outbound-call", metadata={"bad": float("nan")})
    with pytest.raises(EventCandidateError, match="cannot be preserved"):
        _adapt(burglary_case, record)


@pytest.mark.parametrize(
    "withheld",
    [
        ContractDecision.CLOCK_OFFSET_CONVENTION,
        ContractDecision.EVIDENCE_METADATA_KEYS,
        ContractDecision.RAW_TIMESTAMP_PRESERVATION,
    ],
)
def test_withheld_temporal_decision_refuses_normalization(
    burglary_case: SyntheticCase, withheld: ContractDecision
) -> None:
    approved = ALL - {withheld}
    record = burglary_case.candidate_by_id("evt-02-outbound-call")  # Z timestamp
    with pytest.raises(ContractDecisionPendingError) as excinfo:
        _adapt(burglary_case, record, approved)
    assert withheld in excinfo.value.decisions


# --- DEC-002 precision ---------------------------------------------------------------------


@pytest.mark.parametrize("token", ["EXACT_SECOND", "MINUTE", "HOUR", "DAY", "ESTIMATED"])
def test_approved_precisions_give_half_open_windows(burglary_case: SyntheticCase, token: str) -> None:
    record = _cand(burglary_case, "evt-03-location-shared", timestamp_precision=token)
    window = _adapt(burglary_case, record).event.time_window
    assert window.precision == CanonicalTimePrecision(token)
    assert window.window_start_utc <= window.nominal_utc < window.window_end_utc


def test_day_precision_floored_in_local_calendar(burglary_case: SyntheticCase) -> None:
    record = _cand(
        burglary_case, "evt-04-vehicle-message", raw_timestamp="2026-11-10T01:30:00", timestamp_precision="DAY"
    )
    window = _adapt(burglary_case, record).event.time_window
    # Local IST day 2026-11-10 = [2026-11-09T18:30Z, 2026-11-10T18:30Z).
    assert window.window_start_utc == datetime(2026, 11, 9, 18, 30, tzinfo=UTC)
    assert window.window_end_utc == datetime(2026, 11, 10, 18, 30, tzinfo=UTC)


@pytest.mark.parametrize(
    ("legacy", "canonical"),
    [("EXACT", CanonicalTimePrecision.EXACT_SECOND), ("APPROXIMATE", CanonicalTimePrecision.ESTIMATED)],
)
def test_legacy_precision_mapped_with_diagnostic(
    burglary_case: SyntheticCase, legacy: str, canonical: CanonicalTimePrecision
) -> None:
    record = _cand(burglary_case, "evt-02-outbound-call", timestamp_precision=legacy)
    adaptation = _adapt(burglary_case, record)
    assert adaptation.event.canonical_precision == canonical
    assert adaptation.event.raw_timestamp_precision == legacy
    (diag,) = [d for d in adaptation.diagnostics if d.code == "LEGACY_TIME_PRECISION_MAPPED"]
    assert legacy in diag.message and canonical.value in diag.message


def test_legacy_precision_rejected_when_dec002_withheld(burglary_case: SyntheticCase) -> None:
    approved = ALL - {ContractDecision.TIME_PRECISION_SEMANTICS}
    record = _cand(burglary_case, "evt-02-outbound-call", timestamp_precision="EXACT")
    with pytest.raises(ContractDecisionPendingError) as excinfo:
        _adapt(burglary_case, record, approved)
    assert excinfo.value.decisions == (ContractDecision.TIME_PRECISION_SEMANTICS,)


@pytest.mark.parametrize("token", ["SECOND", "minute", " MINUTE", "", None, 5])
def test_unsupported_precision_rejected(burglary_case: SyntheticCase, token: object) -> None:
    record = _cand(burglary_case, "evt-02-outbound-call", timestamp_precision=token)
    with pytest.raises(EventCandidateError, match="timestamp_precision"):
        _adapt(burglary_case, record)


# --- DEC-005 duration --------------------------------------------------------------------


def test_missing_optional_values_stay_null(burglary_case: SyntheticCase) -> None:
    record = burglary_case.candidate_by_id("evt-04-vehicle-message")
    event = _adapt(burglary_case, record).event
    assert event.coordinates is None
    assert event.duration_seconds is None
    assert event.extraction_confidence is None
    assert event.time_window.duration_seconds is None
    assert "duration_seconds" not in event.to_engine_record([])


def test_duration_kept_separate_from_uncertainty(burglary_case: SyntheticCase) -> None:
    event = _adapt(burglary_case, burglary_case.candidate_by_id("evt-01-shutter-forced")).event
    assert event.time_window.duration_seconds == 30.0
    assert event.time_window.uncertainty_span_seconds == 1.0


@pytest.mark.parametrize("duration", [-1.0, float("nan"), float("inf"), True, "30"])
def test_invalid_duration_rejected(burglary_case: SyntheticCase, duration: object) -> None:
    record = _cand(burglary_case, "evt-01-shutter-forced", duration_seconds=duration)
    with pytest.raises(EventCandidateError, match="duration_seconds"):
        _adapt(burglary_case, record)


def test_duration_rejected_when_dec005_withheld(burglary_case: SyntheticCase) -> None:
    approved = ALL - {ContractDecision.EVENT_DURATION_UNCERTAINTY}
    with pytest.raises(ContractDecisionPendingError) as excinfo:
        _adapt(burglary_case, burglary_case.candidate_by_id("evt-01-shutter-forced"), approved)
    assert excinfo.value.decisions == (ContractDecision.EVENT_DURATION_UNCERTAINTY,)


# --- Malformed candidates -----------------------------------------------------------------


@pytest.mark.parametrize(
    ("overrides", "match"),
    [
        ({"candidate_id": ""}, "candidate_id"),
        ({"source_evidence_id": None}, "source_evidence_id"),
        ({"source_evidence_id": "ev-not-ingested"}, "not a validated Evidence"),
        ({"event_type": 7}, "event_type"),
        ({"description": "  "}, "description"),
        ({"raw_timestamp": "yesterday evening"}, "raw_timestamp"),
        ({"raw_timestamp": "2026-11-10"}, "no time component"),
        ({"raw_timestamp": None}, "raw_timestamp"),
        ({"coordinates": {"lat": 128.5, "lon": 77.2}}, "coordinates"),
        ({"coordinates": [28.5, 77.2]}, "coordinates"),
        ({"involved_entity_raw_ids": "ent-person-ravi"}, "involved_entity_raw_ids"),
        ({"involved_entity_raw_ids": ["ent-person-ravi", ""]}, "involved_entity_raw_ids"),
        ({"extraction_confidence": 1.5}, "extraction_confidence"),
        ({"source_record_id": 12}, "source_record_id"),
    ],
)
def test_malformed_candidates_rejected(
    burglary_case: SyntheticCase, overrides: dict[str, Any], match: str
) -> None:
    record = _cand(burglary_case, "evt-03-location-shared", **overrides)
    with pytest.raises(EventCandidateError, match=match):
        _adapt(burglary_case, record)


def test_all_structural_problems_listed_together(burglary_case: SyntheticCase) -> None:
    record = _cand(
        burglary_case,
        "evt-03-location-shared",
        event_type="",
        coordinates={"lat": 999, "lon": 0},
        extraction_confidence=-0.1,
    )
    with pytest.raises(EventCandidateError) as excinfo:
        _adapt(burglary_case, record)
    message = str(excinfo.value)
    for name in ("event_type", "coordinates", "extraction_confidence"):
        assert name in message
    assert excinfo.value.subject_id == "evt-03-location-shared"


@pytest.mark.parametrize("record", [None, ["candidate_id", "x"], "evt-01"])
def test_non_object_candidate_rejected(burglary_case: SyntheticCase, record: object) -> None:
    with pytest.raises(EventCandidateError, match="JSON object"):
        _adapt(burglary_case, record)


def test_null_entity_refs_reported_not_fabricated(burglary_case: SyntheticCase) -> None:
    record = _cand(burglary_case, "evt-04-vehicle-message", involved_entity_raw_ids=None)
    adaptation = _adapt(burglary_case, record)
    assert adaptation.event.involved_entity_raw_ids == []
    assert "CANDIDATE_HAS_NO_ENTITY_REFERENCES" in [d.code for d in adaptation.diagnostics]


def test_candidate_fields_match_member1_contract() -> None:
    assert set(CANDIDATE_FIELDS) == {
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
    }


# --- ReconstructedEvent DEC-005 validation --------------------------------------------------


def _reconstructed(**overrides: object) -> ReconstructedEvent:
    base: dict[str, Any] = {
        "candidate_id": "c1",
        "source_evidence_id": "ev-1",
        "raw_timestamp": "2026-11-10T16:10:00Z",
        "timestamp_precision": "MINUTE",
        "event_type": "T",
        "description": "d",
        "event_id": "c1",
        "case_id": CASE_ID,
        "raw_timestamp_precision": "MINUTE",
        "timestamp_utc": "2026-11-10T16:10:00Z",
        "uncertainty_window_start": "2026-11-10T16:10:00Z",
        "uncertainty_window_end": "2026-11-10T16:11:00Z",
        "timezone_basis": "TIMESTAMP_UTC_OFFSET",
        "clock_calibration": "NOT_DECLARED_UNCALIBRATED",
        "ordering_status": TemporalOrderingStatus.STRICTLY_ORDERED,
    }
    base.update(overrides)
    return ReconstructedEvent.model_validate(base)


def test_reconstructed_event_valid_window_accepted() -> None:
    event = _reconstructed()
    assert event.clock_offset_seconds is None and event.duration_seconds is None


@pytest.mark.parametrize(
    "overrides",
    [
        {"uncertainty_window_end": None},
        {"uncertainty_window_start": None},
        {"uncertainty_window_end": "2026-11-10T16:10:00Z"},
        {"timestamp_utc": "2026-11-10T16:11:00Z"},  # nominal == end is outside [start, end)
        {"timestamp_utc": "2026-11-10T16:09:59Z"},
        {"duration_seconds": -5.0},
    ],
)
def test_reconstructed_event_dec005_violations_rejected(overrides: dict[str, Any]) -> None:
    with pytest.raises(ValueError):
        _reconstructed(**overrides)
