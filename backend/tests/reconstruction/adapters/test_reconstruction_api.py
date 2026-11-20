"""Phase 3G — Reconstruction API tests (synthetic warehouse burglary through FastAPI)."""

import json
import os
import random
from collections.abc import Iterator
from copy import deepcopy
from pathlib import Path
from typing import Any

import pytest
from fastapi.testclient import TestClient
from tests.reconstruction.adapters.synthetic_burglary import (
    CASE_ID,
    EXPECTED_UTC,
    FFMPEG_AVAILABLE,
    SyntheticCase,
    build_case,
)

from app.api.openapi_export import DEFAULT_OUTPUT, render_openapi
from app.main import app
from app.modules.reconstruction.adapters.contracts import (
    PROPOSED_CONTRACT_DECISIONS,
    ContractDecision,
)
from app.modules.reconstruction.pipeline import ReconstructionPipelineConfig
from app.modules.reconstruction.run_store import InMemoryRunStore
from app.modules.reconstruction.service import (
    ReconstructionService,
    get_reconstruction_service,
)
from app.schemas.reconstruction import VideoExtractionOptions

BASE = f"/cases/{CASE_ID}/reconstruction"
VIEWS = ("", "/timeline", "/spatial", "/contradictions", "/relationships", "/diagnostics")
requires_ffmpeg = pytest.mark.skipif(not FFMPEG_AVAILABLE, reason="ffmpeg/ffprobe not installed")


def _service(case: SyntheticCase, **config: Any) -> ReconstructionService:
    config.setdefault("approved_contract_decisions", PROPOSED_CONTRACT_DECISIONS)
    cfg = ReconstructionPipelineConfig(
        storage_root=case.storage_root,
        video_extraction_options=VideoExtractionOptions(
            interval_seconds=1.0, max_frames=5, create_unique_run_subdir=True
        ),
        **config,
    )
    return ReconstructionService(cfg, InMemoryRunStore())


@pytest.fixture
def client() -> Iterator[TestClient]:
    with TestClient(app, raise_server_exceptions=False) as test_client:
        yield test_client
    app.dependency_overrides.clear()


def _use(service: ReconstructionService) -> None:
    app.dependency_overrides[get_reconstruction_service] = lambda: service


def _body(case: SyntheticCase, **overrides: Any) -> dict[str, Any]:
    body: dict[str, Any] = {
        "evidence": case.evidence,
        "event_candidates": case.candidates,
        "entities": case.entities,
        "frame_artifacts": [],
    }
    body.update(overrides)
    return body


def _post(client: TestClient, case: SyntheticCase, **overrides: Any) -> dict[str, Any]:
    response = client.post(f"{BASE}/run", json=_body(case, **overrides))
    assert response.status_code == 201, response.text
    data: dict[str, Any] = response.json()
    return data


def _get(client: TestClient, run_id: str, view: str = "") -> dict[str, Any]:
    response = client.get(f"{BASE}/runs/{run_id}{view}")
    assert response.status_code == 200, response.text
    data: dict[str, Any] = response.json()
    return data


def _assert_envelope(response: Any, status_code: int, code: str) -> dict[str, Any]:
    assert response.status_code == status_code, response.text
    body: dict[str, Any] = response.json()
    assert set(body) == {"error"}
    assert set(body["error"]) == {"code", "message", "details"}
    assert body["error"]["code"] == code
    return body


def _host_variants(*paths: Path) -> set[str]:
    out: set[str] = set()
    for path in paths:
        out |= {str(path), os.path.realpath(path), os.path.abspath(path)}
    return out


# --- 1. Full burglary scenario through the API ---------------------------------------------


def test_burglary_scenario_end_to_end(client: TestClient, burglary_case: SyntheticCase) -> None:
    _use(_service(burglary_case))
    summary = _post(client, burglary_case)
    assert summary["status"] == "COMPLETED_WITH_WARNINGS"
    assert summary["relationship_export_status"] == "EXPORTED"
    assert summary["counts"]["timeline_events"] == 5
    assert summary["counts"]["contradiction_leads"] == 1
    assert summary["counts"]["relationships"] == 1
    assert summary["links"]["timeline"] == f"{BASE}/runs/{summary['run_id']}/timeline"
    run_id = summary["run_id"]

    # Timeline order and corrected UTC.
    timeline = _get(client, run_id, "/timeline")
    events = timeline["timeline_events"]
    assert [e["event_id"] for e in events] == list(EXPECTED_UTC)
    assert {e["event_id"]: e["timestamp_utc"] for e in events} == EXPECTED_UTC

    # Clock skew: DVR 120 s fast, raw preserved, corrected exactly once.
    cctv = events[0]
    assert cctv["raw_timestamp"] == "2026-11-10T21:42:00"
    assert cctv["timestamp_utc"] == "2026-11-10T16:10:00Z"
    assert cctv["clock_offset_seconds"] == -120.0
    assert cctv["clock_calibration"] == "DECLARED_NOT_INDEPENDENTLY_VERIFIED"
    assert cctv["uncertainty_window_start"] == "2026-11-10T16:10:00Z"
    assert cctv["uncertainty_window_end"] == "2026-11-10T16:10:01Z"
    skew = [
        d for d in _get(client, run_id, "/diagnostics")["diagnostics"] if d["code"] == "CLOCK_OFFSET_APPLIED"
    ]
    assert [d["subject_id"] for d in skew] == ["ev-cctv-dock"]
    assert "fast by 120 s" in skew[0]["message"]

    # Exactly one impossible-travel contradiction.
    contradictions = _get(client, run_id, "/contradictions")
    (lead,) = contradictions["contradiction_leads"]
    assert lead["lead_type"] == "SPEED_THRESHOLD_EXCEEDANCE_LEAD"
    assert lead["entity_id"] == "ent-person-ravi"
    inference = lead["derived_inference"]
    assert inference["min_required_speed_kmh"] > inference["configured_speed_threshold_kmh"]
    assert lead["evidence_basis"] == ["ev-cctv-dock", "ev-chat-export"]

    # Relationship output: CONTRADICTS, inferred, weight null in raw JSON.
    raw = client.get(f"{BASE}/runs/{run_id}/relationships").text
    assert '"weight":null' in raw.replace(" ", "")
    relationships = json.loads(raw)
    assert relationships["relationship_export_status"] == "EXPORTED"
    (rel,) = relationships["relationships"]
    assert rel["relation_type"] == "CONTRADICTS"
    assert rel["is_observed"] is False
    assert rel["weight"] is None
    assert rel["relationship_id"] == lead["relationship_id"]
    assert rel["source_id"] == "evt-01-shutter-forced"
    assert rel["target_id"] == "evt-03-location-shared"

    # Spatial: one movement segment with an exceeded feasibility threshold.
    spatial = _get(client, run_id, "/spatial")
    (segment,) = spatial["movement_segments"]
    assert segment["threshold_evaluation"] == "EXCEEDS_CONFIGURED_THRESHOLD"
    assert {o["event_id"] for o in spatial["spatial_observations"]} >= {
        "evt-01-shutter-forced",
        "evt-03-location-shared",
    }

    # Provenance for every finding.
    full = _get(client, run_id)
    findings = {(p["finding_type"], p["finding_id"]) for p in full["finding_provenance"]}
    assert ("RELATIONSHIP", rel["relationship_id"]) in findings
    assert ("CONTRADICTION_LEAD", lead["lead_id"]) in findings
    assert ("MOVEMENT_SEGMENT", segment["segment_id"]) in findings
    assert {("TIMELINE_EVENT", eid) for eid in EXPECTED_UTC} <= findings
    rel_prov = next(p for p in full["finding_provenance"] if p["finding_id"] == rel["relationship_id"])
    assert rel_prov["is_observed"] is False
    assert set(rel_prov["evidence_sha256"]) == {"ev-cctv-dock", "ev-chat-export"}
    assert full["limitations"] and full["evidence_gaps"]


# --- 2. Determinism ----------------------------------------------------------------------


def test_three_runs_are_identical(client: TestClient, burglary_case: SyntheticCase) -> None:
    _use(_service(burglary_case))
    summaries = [_post(client, burglary_case) for _ in range(3)]
    assert len({s["run_id"] for s in summaries}) == 1
    assert len({s["result_sha256"] for s in summaries}) == 1
    run_id = summaries[0]["run_id"]

    full_texts = [client.get(f"{BASE}/runs/{run_id}").text for _ in range(3)]
    assert full_texts[0] == full_texts[1] == full_texts[2]

    # A fresh service (new store) with shuffled input order gives the same ids and hashes.
    shuffled = deepcopy(burglary_case)
    rng = random.Random(7)
    for seq in (shuffled.evidence, shuffled.candidates, shuffled.entities):
        rng.shuffle(seq)
    _use(_service(burglary_case))
    again = _post(client, shuffled)
    assert again["run_id"] == run_id
    assert again["result_sha256"] == summaries[0]["result_sha256"]
    assert client.get(f"{BASE}/runs/{run_id}").text == full_texts[0]


# --- 3. Partial failure and malformed input -------------------------------------------------


def test_partial_failure_reports_and_continues(client: TestClient, burglary_case: SyntheticCase) -> None:
    _use(_service(burglary_case))
    burglary_case.evidence_by_id("ev-cdr-tower")["sha256_hash"] = "0" * 64
    burglary_case.candidates.append({"candidate_id": "evt-bad", "raw_timestamp": "not-a-time"})
    summary = _post(client, burglary_case)
    assert summary["status"] == "COMPLETED_WITH_ERRORS"
    assert summary["counts"]["timeline_events"] == 4
    assert summary["counts"]["relationships"] == 1

    diagnostics = _get(client, summary["run_id"], "/diagnostics")
    codes = {(d["code"], d["subject_id"]) for d in diagnostics["diagnostics"]}
    assert ("EVIDENCE_INTEGRITY_MISMATCH", "ev-cdr-tower") in codes
    assert ("PRODUCER_CONTRACT_GAP", "evt-bad") in codes
    excluded = {(x["subject_type"], x["subject_id"]) for x in diagnostics["excluded_inputs"]}
    assert excluded == {
        ("EVIDENCE", "ev-cdr-tower"),
        ("EVENT_CANDIDATE", "evt-02-outbound-call"),
        ("EVENT_CANDIDATE", "evt-bad"),
    }
    assert any(lim["code"] == "EXCLUDED_INPUTS" for lim in diagnostics["limitations"])
    assert ("EVIDENCE_EXCLUDED", "ev-cdr-tower") in {
        (g["gap_type"], g["subject_id"]) for g in diagnostics["evidence_gaps"]
    }


@pytest.mark.parametrize(
    "body",
    [
        {},
        {"event_candidates": []},
        {"evidence": [], "event_candidates": []},
        {"evidence": "/etc/passwd", "event_candidates": []},
        {"evidence": [{"evidence_id": "e"}], "event_candidates": ["not-an-object"]},
        {"evidence": [{"evidence_id": "e"}], "event_candidates": {"no_candidates": []}},
        {"evidence": [{"evidence_id": "e"}], "event_candidates": [], "unexpected": 1},
        {"evidence": [{"evidence_id": "e"}], "event_candidates": [], "frame_artifacts": {"a": 1}},
    ],
)
def test_malformed_requests_get_422_envelope(
    client: TestClient, burglary_case: SyntheticCase, body: dict[str, Any]
) -> None:
    _use(_service(burglary_case))
    response = client.post(f"{BASE}/run", json=body)
    envelope = _assert_envelope(response, 422, "REQUEST_VALIDATION_FAILED")
    assert isinstance(envelope["error"]["details"], list)
    assert "/etc/passwd" not in response.text  # submitted values are never echoed


def test_invalid_json_and_invalid_ids(client: TestClient, burglary_case: SyntheticCase) -> None:
    _use(_service(burglary_case))
    response = client.post(
        f"{BASE}/run", content=b"{not json", headers={"content-type": "application/json"}
    )
    _assert_envelope(response, 422, "REQUEST_VALIDATION_FAILED")
    _assert_envelope(client.post("/cases/bad%20id!/reconstruction/run", json=_body(burglary_case)), 422, "REQUEST_VALIDATION_FAILED")
    _assert_envelope(client.get(f"{BASE}/runs/bad%20id!"), 422, "REQUEST_VALIDATION_FAILED")


def test_unexpected_failure_gives_500_without_details(
    client: TestClient, burglary_case: SyntheticCase
) -> None:
    class Exploding(ReconstructionService):
        def get(self, case_id: str, run_id: str) -> Any:
            raise RuntimeError(f"secret at {burglary_case.storage_root}")

    _use(Exploding(ReconstructionPipelineConfig(storage_root=burglary_case.storage_root), InMemoryRunStore()))
    response = client.get(f"{BASE}/runs/run-abc")
    body = _assert_envelope(response, 500, "INTERNAL_SERVER_ERROR")
    assert body["error"]["details"] is None
    assert "secret" not in response.text and str(burglary_case.storage_root) not in response.text


# --- 4. API contract per endpoint -----------------------------------------------------------


EXPECTED_KEYS = {
    "": {"run_id", "case_id", "status", "result_sha256", "timeline_events", "relationships",
         "contradiction_leads", "movement_segments", "finding_provenance", "limitations",
         "evidence_gaps", "diagnostics", "algorithm_version", "configuration_version"},
    "/timeline": {"run_id", "case_id", "status", "result_sha256", "timeline_events", "timeline_overlaps"},
    "/spatial": {"run_id", "case_id", "status", "result_sha256", "spatial_observations",
                 "movement_segments", "uncertain_spatial_overlaps"},
    "/contradictions": {"run_id", "case_id", "status", "result_sha256", "relationship_export_status",
                        "contradiction_leads", "contradiction_overlaps_not_evaluated"},
    "/relationships": {"run_id", "case_id", "status", "result_sha256", "relationship_export_status",
                       "relationships"},
    "/diagnostics": {"run_id", "case_id", "status", "result_sha256", "diagnostics", "limitations",
                     "evidence_gaps", "excluded_inputs", "video"},
}


@pytest.mark.parametrize("view", VIEWS)
def test_every_endpoint_contract(client: TestClient, burglary_case: SyntheticCase, view: str) -> None:
    _use(_service(burglary_case))
    run_id = _post(client, burglary_case)["run_id"]
    data = _get(client, run_id, view)
    assert EXPECTED_KEYS[view] <= set(data)
    assert data["run_id"] == run_id and data["case_id"] == CASE_ID

    _assert_envelope(client.get(f"{BASE}/runs/run-does-not-exist{view}"), 404, "RUN_NOT_FOUND")
    # A run is only visible under its own case.
    _assert_envelope(client.get(f"/cases/case-other/reconstruction/runs/{run_id}{view}"), 404, "RUN_NOT_FOUND")


def test_blocked_status_returned_verbatim(client: TestClient, burglary_case: SyntheticCase) -> None:
    _use(_service(burglary_case, approved_contract_decisions=frozenset(ContractDecision) - {ContractDecision.RELATIONSHIP_WEIGHT_NULLABLE}))
    run_id = _post(client, burglary_case)["run_id"]
    rels = _get(client, run_id, "/relationships")
    assert rels["relationship_export_status"] == "RELATIONSHIP_EXPORT_BLOCKED"
    assert rels["relationships"] == []
    (lead,) = _get(client, run_id, "/contradictions")["contradiction_leads"]
    assert lead["relationship_id"] is None
    blocked = [d for d in _get(client, run_id, "/diagnostics")["diagnostics"] if d["code"] == "RELATIONSHIP_EXPORT_BLOCKED"]
    assert blocked[0]["pending_contract_decisions"] == ["DEC-004"]


def test_supplied_frames_link_to_events_by_time_only(
    client: TestClient, burglary_case: SyntheticCase
) -> None:
    burglary_case.evidence_by_id("ev-cctv-dock")["metadata"]["recording_start_timestamp"] = "2026-11-10T21:42:00"
    sha = burglary_case.evidence_by_id("ev-cctv-dock")["sha256_hash"]
    frames = [
        {
            "artifact_id": "artifact_ev-cctv-dock_frame_000001",
            "source_evidence_id": "ev-cctv-dock",
            "source_video_sha256": sha,
            "frame_index": 1,
            "pts_offset_seconds": 0.0,
            "sha256_hash": "a" * 64,
            "timestamp_utc": "1999-01-01T00:00:00Z",  # ignored, recomputed once
        },
        {
            "artifact_id": "artifact_bad",
            "source_evidence_id": "ev-cctv-dock",
            "source_video_sha256": sha,
            "frame_index": 2,
            "pts_offset_seconds": 1.0,
            "sha256_hash": "b" * 64,
            "file_path": "/srv/frames/bad.jpg",
        },
    ]
    _use(_service(burglary_case))
    summary = _post(client, burglary_case, frame_artifacts=frames)
    assert summary["status"] == "COMPLETED_WITH_ERRORS"  # the bad frame is excluded and reported
    full = _get(client, summary["run_id"])
    (frame,) = full["frame_artifacts"]
    assert frame["origin"] == "SUPPLIED"
    # 21:42:00 IST = 16:12Z, DVR 120 s fast → 16:10:00Z (offset applied once).
    assert frame["timestamp_utc"] == "2026-11-10T16:10:00Z"
    cctv = next(e for e in full["timeline_events"] if e["event_id"] == "evt-01-shutter-forced")
    assert cctv["supporting_frame_artifact_ids"] == [frame["artifact_id"]]
    (lead,) = full["contradiction_leads"]
    assert lead["supporting_frame_artifact_ids"] == [frame["artifact_id"]]
    assert "/srv/frames" not in json.dumps(full)
    assert ("FRAME_ARTIFACT", "artifact_bad") in {
        (x["subject_type"], x["subject_id"]) for x in full["excluded_inputs"]
    }


@requires_ffmpeg
def test_video_status_verbatim_and_no_host_paths(client: TestClient, tmp_path: Path) -> None:
    case = build_case(tmp_path, real_video=True)
    _use(_service(case, artifact_root=case.artifact_root))
    run_id = _post(client, case)["run_id"]
    diagnostics = _get(client, run_id, "/diagnostics")
    (video,) = diagnostics["video"]
    assert video["frame_handoff_status"] == "NOT_SUBMITTED_NO_CONSUMER"
    assert video["processing_kind"] == "FRAME_EXTRACTION_WITH_PROVENANCE"

    texts = [client.get(f"{BASE}/runs/{run_id}{view}").text for view in VIEWS]
    for variant in _host_variants(case.storage_root, case.artifact_root, tmp_path):
        assert all(variant not in text for text in texts)


def test_openapi_documents_endpoints_and_is_committed(client: TestClient) -> None:
    schema = client.get("/openapi.json").json()
    for view in ("/run", "/runs/{run_id}", *(f"/runs/{{run_id}}{v}" for v in VIEWS if v)):
        assert f"/cases/{{case_id}}/reconstruction{view}" in schema["paths"]
    post = schema["paths"]["/cases/{case_id}/reconstruction/run"]["post"]
    assert post["responses"]["422"]["content"]["application/json"]["schema"]["$ref"].endswith("/ErrorEnvelope")
    weight = schema["components"]["schemas"]["SharedRelationshipRecord"]["properties"]["weight"]
    assert {"type": "null"} in weight["anyOf"]
    assert DEFAULT_OUTPUT.is_file(), "run: uv run python -m app.api.openapi_export"
    assert DEFAULT_OUTPUT.read_text(encoding="utf-8") == render_openapi(), (
        "docs/api/openapi.json is stale; regenerate with: uv run python -m app.api.openapi_export"
    )
