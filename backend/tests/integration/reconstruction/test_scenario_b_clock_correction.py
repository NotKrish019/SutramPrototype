"""Scenario B: clock correction (fast/slow clocks, naive/Z/offset, DST edges, unknown timezone)."""

from copy import deepcopy
from datetime import datetime, timedelta
from typing import Any

import pytest

from app.modules.reconstruction.adapters.evidence_adapter import ClockCalibrationStatus
from app.modules.reconstruction.pipeline import PipelineStatus, ReconstructionResult
from app.modules.reporting.forensic_report import build_forensic_report
from tests.integration.reconstruction.support import (
    FIXED_NOW,
    SyntheticCase,
    run_pipeline,
    synthetic_candidate,
)

NEW_YORK_CHAT = {"source_timezone": "America/New_York", "clock_offset_seconds": 0.0}


def _z(value: datetime | None) -> str:
    assert value is not None
    return value.isoformat().replace("+00:00", "Z")


def _events(result: ReconstructionResult) -> dict[str, Any]:
    return {e.event_id: e for e in result.timeline_events}


def _with_metadata(case: SyntheticCase, evidence_id: str, metadata: dict[str, Any]) -> list[dict[str, Any]]:
    evidence = deepcopy(case.evidence)
    next(e for e in evidence if e["evidence_id"] == evidence_id)["metadata"] = metadata
    return evidence


def _chat(cid: str, raw: str, precision: str = "EXACT_SECOND") -> dict[str, Any]:
    return synthetic_candidate(cid, "ev-chat-export", raw, precision=precision)


def test_fast_clock_negative_offset_is_applied_once(burglary_case: SyntheticCase) -> None:
    result = run_pipeline(burglary_case)
    events = _events(result)
    for cid, raw, utc in (
        ("evt-01-shutter-forced", "2026-11-10T21:42:00", "2026-11-10T16:10:00Z"),
        ("evt-05-van-departs", "2026-11-10T21:57:00", "2026-11-10T16:25:00Z"),
    ):
        assert events[cid].raw_timestamp == raw
        assert _z(events[cid].timestamp_utc) == utc
        assert events[cid].clock_offset_seconds == -120.0


def test_slow_clock_positive_offset_on_z_timestamp(burglary_case: SyntheticCase) -> None:
    evidence = _with_metadata(burglary_case, "ev-cdr-tower", {"clock_offset_seconds": 90.0})
    result = run_pipeline(burglary_case, evidence=evidence)
    call = _events(result)["evt-02-outbound-call"]
    assert call.raw_timestamp == "2026-11-10T16:12:00Z"
    assert _z(call.timestamp_utc) == "2026-11-10T16:13:30Z"
    assert call.timezone_basis.value == "TIMESTAMP_UTC_OFFSET"
    assert call.clock_calibration == ClockCalibrationStatus.DECLARED_NOT_INDEPENDENTLY_VERIFIED
    skew = {d.subject_id: d.message for d in result.diagnostics if d.code == "CLOCK_OFFSET_APPLIED"}
    assert "slow by 90 s" in skew["ev-cdr-tower"]
    assert "fast by 120 s" in skew["ev-cctv-dock"]


@pytest.mark.parametrize(
    ("raw", "expected_utc", "basis"),
    [
        ("2026-11-10T21:50:00", "2026-11-10T16:20:00Z", "SOURCE_TIMEZONE_METADATA"),  # naive local (IST)
        ("2026-11-10T16:20:00Z", "2026-11-10T16:20:00Z", "TIMESTAMP_UTC_OFFSET"),  # Z
        ("2026-11-10T21:50:00+05:30", "2026-11-10T16:20:00Z", "TIMESTAMP_UTC_OFFSET"),  # offset
        ("2026-11-10T12:20:00-04:00", "2026-11-10T16:20:00Z", "TIMESTAMP_UTC_OFFSET"),  # foreign offset kept
    ],
)
def test_timestamp_forms_are_localized_exactly_once(
    burglary_case: SyntheticCase, raw: str, expected_utc: str, basis: str
) -> None:
    result = run_pipeline(burglary_case, candidates=[_chat("c-form", raw)], entities=[])
    (event,) = result.timeline_events
    assert event.raw_timestamp == raw
    assert _z(event.timestamp_utc) == expected_utc
    assert event.timezone_basis.value == basis


def test_dst_gap_and_fold_are_rejected_not_guessed(burglary_case: SyntheticCase) -> None:
    evidence = _with_metadata(burglary_case, "ev-chat-export", NEW_YORK_CHAT)
    candidates = [
        _chat("gap", "2026-03-08T02:30:00"),
        _chat("fold", "2026-11-01T01:30:00"),
        _chat("before-gap", "2026-03-08T01:59:59"),
        _chat("after-gap", "2026-03-08T03:00:00"),
        _chat("fold-edt", "2026-11-01T01:30:00-04:00"),
        _chat("fold-est", "2026-11-01T01:30:00-05:00"),
    ]
    result = run_pipeline(burglary_case, evidence=evidence, candidates=candidates, entities=[])
    assert result.status == PipelineStatus.COMPLETED_WITH_ERRORS
    excluded = {x.subject_id: x.reason_code for x in result.excluded_inputs}
    assert excluded == {"gap": "INVALID_EVENT_CANDIDATE", "fold": "INVALID_EVENT_CANDIDATE"}
    messages = {d.subject_id: d.message for d in result.diagnostics if d.code == "INVALID_EVENT_CANDIDATE"}
    assert "Nonexistent" in messages["gap"] and "spring-forward" in messages["gap"]
    assert "Ambiguous" in messages["fold"] and "fall-back" in messages["fold"]
    events = _events(result)
    assert _z(events["before-gap"].timestamp_utc) == "2026-03-08T06:59:59Z"  # EST, UTC-5
    assert _z(events["after-gap"].timestamp_utc) == "2026-03-08T07:00:00Z"  # EDT, UTC-4
    assert _z(events["fold-edt"].timestamp_utc) == "2026-11-01T05:30:00Z"
    assert _z(events["fold-est"].timestamp_utc) == "2026-11-01T06:30:00Z"


@pytest.mark.parametrize(
    ("raw", "hours"),
    [("2026-11-01T12:00:00", 25), ("2026-03-08T12:00:00", 23), ("2026-11-10T12:00:00", 24)],
)
def test_day_precision_window_spans_the_local_calendar_day(burglary_case: SyntheticCase, raw: str, hours: int) -> None:
    evidence = _with_metadata(burglary_case, "ev-chat-export", NEW_YORK_CHAT)
    result = run_pipeline(burglary_case, evidence=evidence, candidates=[_chat("day", raw, "DAY")], entities=[])
    (event,) = result.timeline_events
    assert event.uncertainty_window_start is not None and event.uncertainty_window_end is not None
    assert event.uncertainty_window_end - event.uncertainty_window_start == timedelta(hours=hours)


def test_naive_timestamp_without_timezone_is_excluded_not_assumed_utc(burglary_case: SyntheticCase) -> None:
    evidence = _with_metadata(burglary_case, "ev-chat-export", {})
    result = run_pipeline(burglary_case, evidence=evidence)
    assert result.status == PipelineStatus.COMPLETED_WITH_ERRORS
    assert "evt-04-vehicle-message" not in _events(result)
    assert {(x.subject_id, x.reason_code) for x in result.excluded_inputs} == {
        ("evt-04-vehicle-message", "SOURCE_TIMEZONE_UNKNOWN")
    }
    assert any(g.subject_id == "evt-04-vehicle-message" for g in result.evidence_gaps)
    assert "EXCLUDED_INPUTS" in {lim.code for lim in result.limitations}


def test_unknown_timezone_with_utc_offset_is_kept_but_marked_uncalibrated(burglary_case: SyntheticCase) -> None:
    evidence = _with_metadata(burglary_case, "ev-chat-export", {})
    result = run_pipeline(burglary_case, evidence=evidence)
    shared = _events(result)["evt-03-location-shared"]
    assert _z(shared.timestamp_utc) == "2026-11-10T16:14:00Z"
    assert shared.source_timezone is None
    assert shared.clock_calibration == ClockCalibrationStatus.NOT_DECLARED_UNCALIBRATED
    uncalibrated = next(lim for lim in result.limitations if lim.code == "UNCALIBRATED_CLOCKS")
    assert "ev-chat-export" in uncalibrated.subject_ids

    report = build_forensic_report(result, generated_at=FIXED_NOW)
    clocks = {c.evidence_id: c for c in report.limitations.unverified_clock_calibrations}
    assert clocks["ev-chat-export"].clock_calibration == ClockCalibrationStatus.NOT_DECLARED_UNCALIBRATED
    assert "not known to be accurate" in clocks["ev-chat-export"].note


def test_no_calibration_is_ever_reported_as_verified(burglary_case: SyntheticCase) -> None:
    assert {s.value for s in ClockCalibrationStatus} == {
        "DECLARED_NOT_INDEPENDENTLY_VERIFIED",
        "NOT_DECLARED_UNCALIBRATED",
        "NOT_APPLIED_DECISION_WITHHELD",
    }
    result = run_pipeline(burglary_case)
    report = build_forensic_report(result, generated_at=FIXED_NOW)
    assert {c.evidence_id for c in report.limitations.unverified_clock_calibrations} == {
        "ev-cctv-dock",
        "ev-cdr-tower",
        "ev-chat-export",
    }
