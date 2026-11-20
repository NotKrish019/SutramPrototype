"""Scenario A: full burglary run through the API (timeline, movement, contradiction, clock skew)."""

from fastapi.testclient import TestClient

from tests.integration.reconstruction.support import (
    BASE,
    EXPECTED_UTC,
    SyntheticCase,
    get_view,
    make_service,
    post_run,
    use_service,
)

LEAD_EVENTS = ("evt-01-shutter-forced", "evt-03-location-shared")


def test_full_run_summary(client: TestClient, burglary_case: SyntheticCase) -> None:
    use_service(make_service(burglary_case))
    summary = post_run(client, burglary_case)
    assert summary["status"] == "COMPLETED_WITH_WARNINGS"
    assert summary["relationship_export_status"] == "EXPORTED"
    assert summary["counts"] == {
        "timeline_events": 5,
        "movement_segments": 1,
        "contradiction_leads": 1,
        "relationships": 1,
        "excluded_inputs": 0,
        "diagnostics_by_severity": {"INFO": 2, "WARNING": 2},
    }
    run_id = summary["run_id"]
    assert summary["links"]["report"] == f"{BASE}/runs/{run_id}/report"


def test_timeline_is_in_corrected_utc_order(client: TestClient, burglary_case: SyntheticCase) -> None:
    use_service(make_service(burglary_case))
    run_id = post_run(client, burglary_case)["run_id"]
    events = get_view(client, run_id, "/timeline")["timeline_events"]
    assert [e["event_id"] for e in events] == list(EXPECTED_UTC)
    assert {e["event_id"]: e["timestamp_utc"] for e in events} == EXPECTED_UTC
    starts = [e["uncertainty_window_start"] for e in events]
    assert starts == sorted(starts)
    assert all(e["ordering_status"] == "STRICTLY_ORDERED" for e in events)
    for event in events:
        assert event["uncertainty_window_start"] <= event["timestamp_utc"] < event["uncertainty_window_end"]


def test_movement_segment_and_single_impossible_travel_lead(client: TestClient, burglary_case: SyntheticCase) -> None:
    use_service(make_service(burglary_case))
    run_id = post_run(client, burglary_case)["run_id"]

    (segment,) = get_view(client, run_id, "/spatial")["movement_segments"]
    assert (segment["origin_event_id"], segment["destination_event_id"]) == LEAD_EVENTS
    assert segment["entity_id"] == "ent-person-ravi"
    assert 25.0 < segment["distance_km"] < 25.7
    assert segment["max_possible_elapsed_seconds"] == 300.0
    assert segment["threshold_evaluation"] == "EXCEEDS_CONFIGURED_THRESHOLD"

    (lead,) = get_view(client, run_id, "/contradictions")["contradiction_leads"]
    assert lead["lead_type"] == "SPEED_THRESHOLD_EXCEEDANCE_LEAD"
    assert (lead["origin_event_id"], lead["destination_event_id"]) == LEAD_EVENTS
    assert lead["derived_inference"]["is_observed"] is False
    assert lead["derived_inference"]["min_required_speed_kmh"] > lead["derived_inference"]["configured_speed_threshold_kmh"]
    assert sorted(lead["evidence_basis"]) == ["ev-cctv-dock", "ev-chat-export"]


def test_exactly_one_clock_skew_case(client: TestClient, burglary_case: SyntheticCase) -> None:
    use_service(make_service(burglary_case))
    run_id = post_run(client, burglary_case)["run_id"]
    diagnostics = get_view(client, run_id, "/diagnostics")["diagnostics"]
    skew = [d for d in diagnostics if d["code"] == "CLOCK_OFFSET_APPLIED"]
    assert [d["subject_id"] for d in skew] == ["ev-cctv-dock"]
    assert "fast by 120 s" in skew[0]["message"]
    events = {e["event_id"]: e for e in get_view(client, run_id, "/timeline")["timeline_events"]}
    cctv = events["evt-01-shutter-forced"]
    assert cctv["raw_timestamp"] == "2026-11-10T21:42:00"
    # 21:42 IST = 16:12Z, minus 120 s for the fast DVR = 16:10Z (applied once).
    assert cctv["timestamp_utc"] == "2026-11-10T16:10:00Z"
    assert cctv["clock_calibration"] == "DECLARED_NOT_INDEPENDENTLY_VERIFIED"


def test_contradicts_relationship_is_inferred_with_null_weight(client: TestClient, burglary_case: SyntheticCase) -> None:
    use_service(make_service(burglary_case))
    run_id = post_run(client, burglary_case)["run_id"]
    response = client.get(f"{BASE}/runs/{run_id}/relationships")
    (rel,) = response.json()["relationships"]
    assert rel["relation_type"] == "CONTRADICTS"
    assert rel["is_observed"] is False
    assert "weight" in rel and rel["weight"] is None
    assert '"weight":null' in response.text.replace(" ", "")
    assert (rel["source_id"], rel["target_id"]) == LEAD_EVENTS
    assert rel["metadata"]["weight_semantics"] == "NOT_COMPUTED"
    (lead,) = get_view(client, run_id, "/contradictions")["contradiction_leads"]
    assert lead["relationship_id"] == rel["relationship_id"]


def test_every_finding_has_provenance(client: TestClient, burglary_case: SyntheticCase) -> None:
    use_service(make_service(burglary_case))
    run_id = post_run(client, burglary_case)["run_id"]
    result = get_view(client, run_id)
    provenance = {(p["finding_type"], p["finding_id"]): p for p in result["finding_provenance"]}
    expected = (
        [("TIMELINE_EVENT", e["event_id"]) for e in result["timeline_events"]]
        + [("MOVEMENT_SEGMENT", s["segment_id"]) for s in result["movement_segments"]]
        + [("CONTRADICTION_LEAD", lead["lead_id"]) for lead in result["contradiction_leads"]]
        + [("RELATIONSHIP", r["relationship_id"]) for r in result["relationships"]]
    )
    for key in expected:
        record = provenance[key]
        assert record["evidence_ids"], key
        assert set(record["evidence_sha256"]) == set(record["evidence_ids"])
        assert record["is_observed"] is (key[0] == "TIMELINE_EVENT")
