"""Scenario D: malformed Member 1 JSON (missing fields, wrong types, duplicates, precision, size)."""

import json
from copy import deepcopy
from typing import Any

import pytest
from fastapi.testclient import TestClient

from app.modules.reconstruction.pipeline import PipelineStatus, ReconstructionResult
from app.modules.reporting.forensic_report import build_forensic_report
from app.modules.reporting.report_render import render_html, render_markdown
from app.schemas.reconstruction_api import MAX_ITEMS
from tests.integration.reconstruction.support import (
    BASE,
    FIXED_NOW,
    SyntheticCase,
    assert_envelope,
    make_service,
    request_body,
    run_pipeline,
    synthetic_candidate,
    use_service,
)

TARGET = "evt-02-outbound-call"


def _mutated(case: SyntheticCase, change: dict[str, Any] | None = None, drop: str | None = None) -> list[dict[str, Any]]:
    candidates = deepcopy(case.candidates)
    target = next(c for c in candidates if c["candidate_id"] == TARGET)
    if drop is not None:
        del target[drop]
    target.update(change or {})
    return candidates


def _only_target_excluded(result: ReconstructionResult, code: str = "INVALID_EVENT_CANDIDATE") -> None:
    assert result.status == PipelineStatus.COMPLETED_WITH_ERRORS
    assert [(x.subject_type, x.subject_id, x.reason_code) for x in result.excluded_inputs] == [
        ("EVENT_CANDIDATE", TARGET, code)
    ]
    assert TARGET not in {e.event_id for e in result.timeline_events}
    assert len(result.timeline_events) == 4
    assert any(d.code == code and d.subject_id == TARGET for d in result.diagnostics)


@pytest.mark.parametrize("field", ["raw_timestamp", "source_evidence_id", "event_type"])
def test_missing_required_field(burglary_case: SyntheticCase, field: str) -> None:
    _only_target_excluded(run_pipeline(burglary_case, candidates=_mutated(burglary_case, drop=field)))


@pytest.mark.parametrize("field", ["timestamp_precision", "description"])
def test_absent_producer_field_is_a_contract_gap(burglary_case: SyntheticCase, field: str) -> None:
    """Fields with no safe derivation are reported as a producer contract gap, never inferred."""
    result = run_pipeline(burglary_case, candidates=_mutated(burglary_case, drop=field))
    _only_target_excluded(result, code="PRODUCER_CONTRACT_GAP")


def test_missing_candidate_id_is_reported_by_index(burglary_case: SyntheticCase) -> None:
    candidates = deepcopy(burglary_case.candidates)
    del candidates[1]["candidate_id"]
    result = run_pipeline(burglary_case, candidates=candidates)
    assert result.status == PipelineStatus.COMPLETED_WITH_ERRORS
    assert [x.subject_id for x in result.excluded_inputs] == ["<index_1>"]


@pytest.mark.parametrize(
    "change",
    [
        {"raw_timestamp": 1731254520},
        {"raw_timestamp": "yesterday evening"},
        {"coordinates": "north of the warehouse"},
        {"coordinates": {"lat": 123.0, "lon": 77.0}},
        {"coordinates": {"lat": "28.5", "lon": 77.0}},
        {"duration_seconds": "long"},
        {"duration_seconds": -5.0},
        {"involved_entity_raw_ids": "ent-phone-01"},
        {"extraction_confidence": "high"},
        {"source_evidence_id": "ev-does-not-exist"},
    ],
    ids=lambda c: next(iter(c)),
)
def test_wrong_types_and_values(burglary_case: SyntheticCase, change: dict[str, Any]) -> None:
    _only_target_excluded(run_pipeline(burglary_case, candidates=_mutated(burglary_case, change)))


@pytest.mark.parametrize("precision", ["FORTNIGHT", "", None, 5, "exact_second "])
def test_bad_precision(burglary_case: SyntheticCase, precision: Any) -> None:
    _only_target_excluded(run_pipeline(burglary_case, candidates=_mutated(burglary_case, {"timestamp_precision": precision})))


def test_legacy_precision_is_mapped_with_a_warning(burglary_case: SyntheticCase) -> None:
    result = run_pipeline(burglary_case, candidates=_mutated(burglary_case, {"timestamp_precision": "EXACT"}))
    assert result.excluded_inputs == []
    assert any(d.code == "LEGACY_TIME_PRECISION_MAPPED" and d.subject_id == TARGET for d in result.diagnostics)


def test_duplicate_candidate_ids_exclude_every_copy(burglary_case: SyntheticCase) -> None:
    candidates = deepcopy(burglary_case.candidates)
    clone = deepcopy(next(c for c in candidates if c["candidate_id"] == TARGET))
    clone["description"] = "conflicting copy"
    candidates.append(clone)
    _only_target_excluded(run_pipeline(burglary_case, candidates=candidates), "DUPLICATE_CANDIDATE_ID")


def test_non_object_items_are_reported_by_index(burglary_case: SyntheticCase) -> None:
    candidates: list[Any] = [*deepcopy(burglary_case.candidates), "oops", 42, None]
    result = run_pipeline(burglary_case, candidates=candidates)
    assert result.status == PipelineStatus.COMPLETED_WITH_ERRORS
    assert {x.subject_id for x in result.excluded_inputs} == {"<index_5>", "<index_6>", "<index_7>"}
    assert len(result.timeline_events) == 5


def test_injected_derived_fields_are_ignored(burglary_case: SyntheticCase) -> None:
    candidates = _mutated(
        burglary_case, {"timestamp_utc": "1999-01-01T00:00:00Z", "clock_offset_seconds": 9999.0}
    )
    result = run_pipeline(burglary_case, candidates=candidates)
    event = next(e for e in result.timeline_events if e.event_id == TARGET)
    assert event.timestamp_utc.isoformat() == "2026-11-10T16:12:00+00:00"
    assert any(d.code == "DERIVED_FIELDS_IGNORED" and d.subject_id == TARGET for d in result.diagnostics)


@pytest.mark.parametrize("payload", ["{not json", '{"case_id": "x"}', "42", b"\xff\xfe"])
def test_unreadable_payload_fails_the_run_with_limitations(burglary_case: SyntheticCase, payload: Any) -> None:
    result = run_pipeline(burglary_case, candidates=payload)
    assert result.status == PipelineStatus.FAILED
    assert any(d.code == "INVALID_EVENT_CANDIDATE_PAYLOAD" for d in result.diagnostics)
    assert [lim.code for lim in result.limitations] == ["RUN_FAILED_NO_RECONSTRUCTION"]
    report = build_forensic_report(result, generated_at=FIXED_NOW)
    assert {lim.code for lim in report.limitations.limitations} == {"REPORT_SCOPE", "RUN_FAILED_NO_RECONSTRUCTION"}
    assert "failed" in render_markdown(report)


def test_unified_extraction_result_object_is_accepted(burglary_case: SyntheticCase) -> None:
    result = run_pipeline(burglary_case, candidates=burglary_case.unified_extraction_json())
    assert len(result.timeline_events) == 5


def test_large_input_with_one_bad_item_continues(burglary_case: SyntheticCase) -> None:
    candidates = [
        synthetic_candidate(f"bulk-{i:04d}", "ev-cdr-tower", f"2026-11-10T{10 + i // 3600:02d}:{i // 60 % 60:02d}:{i % 60:02d}Z")
        for i in range(2_000)
    ]
    candidates[1234]["timestamp_precision"] = "FORTNIGHT"
    result = run_pipeline(burglary_case, candidates=candidates, entities=[])
    assert result.status == PipelineStatus.COMPLETED_WITH_ERRORS
    assert [x.subject_id for x in result.excluded_inputs] == ["bulk-1234"]
    assert len(result.timeline_events) == 1_999


def test_very_long_text_is_kept_and_escaped(burglary_case: SyntheticCase) -> None:
    long_text = "<b>x</b>" * 50_000  # 400 kB
    candidates = _mutated(burglary_case, {"description": long_text})
    result = run_pipeline(burglary_case, candidates=candidates)
    event = next(e for e in result.timeline_events if e.event_id == TARGET)
    assert event.description == long_text
    report = build_forensic_report(result, generated_at=FIXED_NOW)
    assert "<b>x</b>" not in render_html(report)
    assert "<b>x</b>" not in render_markdown(report)


def test_api_rejects_more_than_max_items(client: TestClient, burglary_case: SyntheticCase) -> None:
    use_service(make_service(burglary_case))
    item = synthetic_candidate("c", "ev-cdr-tower", "2026-11-10T16:12:00Z")
    body = request_body(burglary_case, event_candidates=[item] * (MAX_ITEMS + 1))
    response = client.post(f"{BASE}/run", content=json.dumps(body), headers={"content-type": "application/json"})
    assert_envelope(response, 422, "REQUEST_VALIDATION_FAILED")


@pytest.mark.parametrize(
    "candidates",
    [["not-an-object"], "a string", {"event_candidates": "nope"}, {"no_candidates": []}],
)
def test_api_rejects_structurally_invalid_candidates(
    client: TestClient, burglary_case: SyntheticCase, candidates: Any
) -> None:
    use_service(make_service(burglary_case))
    response = client.post(f"{BASE}/run", json=request_body(burglary_case, event_candidates=candidates))
    assert_envelope(response, 422, "REQUEST_VALIDATION_FAILED")


def test_api_item_level_errors_return_201_with_exclusions(client: TestClient, burglary_case: SyntheticCase) -> None:
    use_service(make_service(burglary_case))
    candidates = _mutated(burglary_case, {"timestamp_precision": "FORTNIGHT"})
    response = client.post(f"{BASE}/run", json=request_body(burglary_case, event_candidates=candidates))
    assert response.status_code == 201
    assert response.json()["status"] == "COMPLETED_WITH_ERRORS"
    assert response.json()["counts"]["excluded_inputs"] == 1
