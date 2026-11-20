"""Scenarios E and F: partial failure (one bad item, run continues) and sparse evidence."""

from copy import deepcopy

from fastapi.testclient import TestClient

from app.modules.reconstruction.pipeline import PipelineStatus, RelationshipExportStatus
from app.modules.reporting.forensic_report import build_forensic_report
from app.modules.reporting.report_render import render_markdown
from tests.integration.reconstruction.support import (
    FIXED_NOW,
    SyntheticCase,
    evidence_path,
    get_view,
    make_service,
    post_run,
    run_pipeline,
    synthetic_candidate,
    use_service,
)

# --- E. Partial failure ---------------------------------------------------------------------


def test_one_bad_evidence_item_run_continues_through_api(client: TestClient, burglary_case: SyntheticCase) -> None:
    evidence_path(burglary_case.storage_root, "ev-cdr-tower").write_bytes(b"tampered")
    use_service(make_service(burglary_case))
    summary = post_run(client, burglary_case)
    assert summary["status"] == "COMPLETED_WITH_ERRORS"
    # The CCTV→chat lead does not depend on the CDR, so it is still found.
    assert summary["counts"]["contradiction_leads"] == 1
    assert summary["counts"]["timeline_events"] == 4

    diagnostics = get_view(client, summary["run_id"], "/diagnostics")
    errors = [d for d in diagnostics["diagnostics"] if d["severity"] == "ERROR"]
    assert [(d["code"], d["subject_id"]) for d in errors] == [("EVIDENCE_INTEGRITY_MISMATCH", "ev-cdr-tower")]
    assert {(x["subject_id"], x["reason_code"]) for x in diagnostics["excluded_inputs"]} == {
        ("ev-cdr-tower", "EVIDENCE_INTEGRITY_MISMATCH"),
        ("evt-02-outbound-call", "SOURCE_EVIDENCE_EXCLUDED"),
    }
    assert "EXCLUDED_INPUTS" in {lim["code"] for lim in diagnostics["limitations"]}
    gap_types = {(g["gap_type"], g["subject_id"]) for g in diagnostics["evidence_gaps"]}
    assert ("EVIDENCE_EXCLUDED", "ev-cdr-tower") in gap_types
    assert ("CANDIDATE_EXCLUDED", "evt-02-outbound-call") in gap_types


def test_one_bad_candidate_among_good_ones(burglary_case: SyntheticCase) -> None:
    candidates = deepcopy(burglary_case.candidates)
    candidates[4]["raw_timestamp"] = "not a time"
    result = run_pipeline(burglary_case, candidates=candidates)
    assert result.status == PipelineStatus.COMPLETED_WITH_ERRORS
    assert len(result.timeline_events) == 4
    assert len(result.contradiction_leads) == 1
    report = build_forensic_report(result, generated_at=FIXED_NOW)
    assert ("EVENT_CANDIDATE", "evt-05-van-departs") in {
        (x["subject_type"], x["subject_id"]) for x in report.limitations.excluded_inputs
    }
    assert any(d.code == "INVALID_EVENT_CANDIDATE" for d in report.limitations.diagnostics)


def test_bad_frame_artifact_is_excluded_and_run_continues(burglary_case: SyntheticCase) -> None:
    frame = {
        "artifact_id": "frame-bad",
        "source_evidence_id": "ev-cctv-dock",
        "source_video_sha256": burglary_case.evidence[0]["sha256_hash"],
        "frame_index": 0,
        "pts_offset_seconds": 0.0,
        "sha256_hash": "a" * 64,
        "file_path": "/tmp/frame.jpg",
    }
    result = run_pipeline(burglary_case, frames=[frame])
    assert result.status == PipelineStatus.COMPLETED_WITH_ERRORS
    assert ("FRAME_ARTIFACT", "frame-bad") in {(x.subject_type, x.subject_id) for x in result.excluded_inputs}
    assert len(result.timeline_events) == 5


# --- F. Sparse evidence ---------------------------------------------------------------------


def _chat_only(case: SyntheticCase) -> tuple[list[dict[str, object]], list[dict[str, object]]]:
    evidence = [deepcopy(e) for e in case.evidence if e["evidence_id"] == "ev-chat-export"]
    candidates = [deepcopy(c) for c in case.candidates if c["source_evidence_id"] == "ev-chat-export"]
    for c in candidates:
        c["coordinates"] = None
    return evidence, candidates


def test_single_source_without_locations_states_limitations(burglary_case: SyntheticCase) -> None:
    evidence, candidates = _chat_only(burglary_case)
    result = run_pipeline(burglary_case, evidence=evidence, candidates=candidates, entities=[])
    assert result.status == PipelineStatus.COMPLETED_WITH_WARNINGS
    assert len(result.timeline_events) == 2
    assert all(e.coordinates is None for e in result.timeline_events)
    assert result.spatial_observations == [] and result.movement_segments == []
    assert result.contradiction_leads == [] and result.relationships == []
    assert result.relationship_export_status == RelationshipExportStatus.NO_CONTRADICTION_LEADS
    codes = {lim.code for lim in result.limitations}
    assert {"SINGLE_SOURCE_NO_CORROBORATION", "NO_SPATIAL_DATA"} <= codes
    gaps = {(g.gap_type.value, g.subject_id) for g in result.evidence_gaps}
    assert {("EVENT_WITHOUT_COORDINATES", "evt-03-location-shared"), ("EVENT_WITHOUT_COORDINATES", "evt-04-vehicle-message")} <= gaps

    markdown = render_markdown(build_forensic_report(result, generated_at=FIXED_NOW))
    assert "No located observations. No locations were inferred." in markdown
    assert "No contradiction leads were produced in this run." in markdown
    assert "SINGLE_SOURCE_NO_CORROBORATION" in markdown


def test_device_coordinates_are_not_used_to_guess_event_locations(burglary_case: SyntheticCase) -> None:
    evidence = [deepcopy(e) for e in burglary_case.evidence if e["evidence_id"] == "ev-cctv-dock"]
    candidates = [deepcopy(c) for c in burglary_case.candidates if c["source_evidence_id"] == "ev-cctv-dock"]
    for c in candidates:
        c["coordinates"] = None
    result = run_pipeline(burglary_case, evidence=evidence, candidates=candidates)
    assert evidence[0]["metadata"]["device_coordinates"]  # the CCTV has device coordinates…
    assert all(e.coordinates is None for e in result.timeline_events)  # …but events are not placed there
    assert result.spatial_observations == []
    assert "NO_SPATIAL_DATA" in {lim.code for lim in result.limitations}


def test_located_but_unpaired_events_state_no_movement(burglary_case: SyntheticCase) -> None:
    candidates = [
        synthetic_candidate("only-one", "ev-cdr-tower", "2026-11-10T16:12:00Z", entity_ids=["ent-phone-01"],
                            coordinates={"lat": 28.5, "lon": 77.2}),
        synthetic_candidate("unlocated", "ev-cdr-tower", "2026-11-10T16:20:00Z", entity_ids=["ent-phone-01"]),
    ]
    result = run_pipeline(burglary_case, candidates=candidates)
    assert result.movement_segments == []
    assert {"NO_MOVEMENT_SEGMENTS", "SINGLE_SOURCE_NO_CORROBORATION"} <= {lim.code for lim in result.limitations}


def test_no_events_at_all(burglary_case: SyntheticCase) -> None:
    result = run_pipeline(burglary_case, candidates=[], entities=[])
    assert result.status != PipelineStatus.FAILED
    assert result.timeline_events == []
    assert "NO_ACCEPTED_EVENTS" in {lim.code for lim in result.limitations}
    assert {g.gap_type.value for g in result.evidence_gaps} >= {"EVIDENCE_WITHOUT_EVENTS"}
    markdown = render_markdown(build_forensic_report(result, generated_at=FIXED_NOW))
    assert "No events were accepted into the timeline." in markdown
