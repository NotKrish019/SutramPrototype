"""Phase 3D.2: contradiction lead / relationship IDs are collision-safe and stable.

Adversarial inputs are the exact collisions found in the Phase 3D.1 audit, where hyphen-joined
IDs ``lead-{case}-{entity}-{origin}-to-{destination}`` were identical for different identities.
"""

import re
from pathlib import Path
from typing import Any

import pytest

from app.modules.reconstruction.adapters.contracts import PROPOSED_CONTRACT_DECISIONS
from app.modules.reconstruction.adapters.relationship_adapter import build_contradiction_relationship_id
from app.modules.reconstruction.contradictions import (
    contradiction_lead_id,
    contradiction_relationship_id,
    detect_contradictions,
)
from app.modules.reconstruction.pipeline import ReconstructionPipeline, ReconstructionPipelineConfig
from tests.reconstruction.adapters.synthetic_burglary import CASE_ID, build_case

DELHI = {"lat": 28.6139, "lon": 77.2090}
MUMBAI = {"lat": 19.0760, "lon": 72.8777}

# (case, entity, origin, destination) pairs that collided under the old hyphen-joined scheme.
AUDIT_COLLISIONS: list[tuple[tuple[str, str, str, str], tuple[str, str, str, str]]] = [
    (("case-1", "ent", "ev-a-to-ev-b", "ev-c"), ("case-1", "ent", "ev-a", "ev-b-to-ev-c")),
    (("case-1", "person-x", "e1", "e2"), ("case-1", "person", "x-e1", "e2")),
]


def _old_lead_id(case: str, entity: str, origin: str, destination: str) -> str:
    return f"lead-{case}-{entity}-{origin}-to-{destination}"


@pytest.mark.parametrize(("left", "right"), AUDIT_COLLISIONS)
def test_audit_collisions_were_real_and_are_now_distinct(
    left: tuple[str, str, str, str], right: tuple[str, str, str, str]
) -> None:
    assert _old_lead_id(*left) == _old_lead_id(*right)  # the defect being fixed
    assert contradiction_lead_id(*left) != contradiction_lead_id(*right)
    assert contradiction_relationship_id(*left) != contradiction_relationship_id(*right)


def test_ids_are_well_formed_deterministic_and_order_sensitive() -> None:
    ident = ("case-1", "ent", "ev-a", "ev-b")
    assert contradiction_lead_id(*ident) == contradiction_lead_id(*ident)
    assert re.fullmatch(r"lead-[0-9a-f]{64}", contradiction_lead_id(*ident))
    assert re.fullmatch(r"rel-contra-[0-9a-f]{32}", contradiction_relationship_id(*ident))
    # Direction is part of the identity.
    assert contradiction_lead_id("case-1", "ent", "ev-b", "ev-a") != contradiction_lead_id(*ident)
    # Characters that broke delimiter schemes: quotes, commas, brackets, unicode, empty parts.
    tricky = [("c", 'a","b', "x", "y"), ("c", "a", '","b"x', "y"), ("c", "ä", "[", "]"), ("c", "", "", "")]
    assert len({contradiction_lead_id(*t) for t in tricky}) == len(tricky)


def _event(event_id: str, entity: str, ts: str, coords: dict[str, float]) -> dict[str, Any]:
    return {
        "event_id": event_id,
        "case_id": "case-1",
        "title": f"synthetic {event_id}",
        "timestamp": ts,
        "timestamp_precision": "EXACT_SECOND",
        "location_name": None,
        "coordinates": coords,
        "involved_entity_ids": [entity],
        "source_evidence_id": f"ev-src-{event_id}",
    }


def _within_entity_collision_events() -> list[dict[str, Any]]:
    # One entity whose consecutive pairs are (ev-a, ev-b-to-ev-c) and (ev-a-to-ev-b, ev-c),
    # alternating Delhi / Mumbai ten minutes apart, so every hop exceeds the speed threshold.
    return [
        _event("ev-a", "ent", "2026-11-10T10:00:00Z", DELHI),
        _event("ev-b-to-ev-c", "ent", "2026-11-10T10:10:00Z", MUMBAI),
        _event("ev-a-to-ev-b", "ent", "2026-11-10T10:20:00Z", DELHI),
        _event("ev-c", "ent", "2026-11-10T10:30:00Z", MUMBAI),
    ]


def _cross_entity_collision_events() -> list[dict[str, Any]]:
    e2 = _event("e2", "person-x", "2026-11-10T10:10:00Z", MUMBAI)
    e2["involved_entity_ids"] = ["person-x", "person"]
    return [
        _event("e1", "person-x", "2026-11-10T10:00:00Z", DELHI),
        _event("x-e1", "person", "2026-11-10T10:00:00Z", DELHI),
        e2,
    ]


@pytest.mark.parametrize(
    ("events", "expected_pairs"),
    [
        (
            _within_entity_collision_events,
            {("ent", "ev-a", "ev-b-to-ev-c"), ("ent", "ev-a-to-ev-b", "ev-c")},
        ),
        (_cross_entity_collision_events, {("person-x", "e1", "e2"), ("person", "x-e1", "e2")}),
    ],
)
def test_engine_leads_from_audit_collisions_get_distinct_ids(
    events: Any, expected_pairs: set[tuple[str, str, str]]
) -> None:
    result = detect_contradictions(events(), speed_threshold_kmh=180.0)
    pairs = {(lead.entity_id, lead.origin_event_id, lead.destination_event_id) for lead in result.leads}
    assert expected_pairs <= pairs
    assert len({lead.lead_id for lead in result.leads}) == len(result.leads)
    assert len({rel.relationship_id for rel in result.relationships}) == len(result.relationships)
    for lead in result.leads:
        ident = ("case-1", lead.entity_id, lead.origin_event_id, lead.destination_event_id)
        assert lead.lead_id == contradiction_lead_id(*ident)
        # The engine relationship and the exported shared relationship use one ID.
        assert lead.relationship.relationship_id == contradiction_relationship_id(*ident)
        assert build_contradiction_relationship_id(lead) == lead.relationship.relationship_id


# --- Through the pipeline: every consumer maps leads to the right relationship --------------


def _candidate(cid: str, raw: str, entity: str, coords: dict[str, float]) -> dict[str, Any]:
    return {
        "candidate_id": cid,
        "source_evidence_id": "ev-chat-export",
        "source_record_id": f"rec:{cid}",
        "raw_timestamp": raw,
        "timestamp_precision": "EXACT_SECOND",
        "event_type": "SYNTHETIC",
        "description": f"synthetic {cid}",
        "involved_entity_raw_ids": [entity],
        "coordinates": coords,
        "duration_seconds": None,
        "extraction_confidence": None,
    }


def _entity(eid: str) -> dict[str, Any]:
    return {
        "entity_id": eid,
        "case_id": CASE_ID,
        "entity_type": "PERSON",
        "canonical_name": eid,
        "aliases": [],
        "source_evidence_ids": ["ev-chat-export"],
        "confidence": 1.0,
    }


def _pipeline_run(tmp_path: Path) -> Any:
    case = build_case(tmp_path)
    candidates = [
        _candidate("ev-a", "2026-11-10T10:00:00+05:30", "ent", DELHI),
        _candidate("ev-b-to-ev-c", "2026-11-10T10:10:00+05:30", "ent", MUMBAI),
        _candidate("ev-a-to-ev-b", "2026-11-10T10:20:00+05:30", "ent", DELHI),
        _candidate("ev-c", "2026-11-10T10:30:00+05:30", "ent", MUMBAI),
    ]
    config = ReconstructionPipelineConfig(
        storage_root=case.storage_root, approved_contract_decisions=PROPOSED_CONTRACT_DECISIONS
    )
    return ReconstructionPipeline(config).run(
        case_id=CASE_ID,
        evidence=case.evidence,
        event_candidates=candidates,
        entities=[_entity("ent")],
    )


def test_pipeline_maps_each_colliding_lead_to_its_own_relationship(tmp_path: Path) -> None:
    result = _pipeline_run(tmp_path)
    pairs = {(lead.origin_event_id, lead.destination_event_id) for lead in result.contradiction_leads}
    assert {("ev-a", "ev-b-to-ev-c"), ("ev-a-to-ev-b", "ev-c")} <= pairs
    lead_ids = [lead.lead_id for lead in result.contradiction_leads]
    assert len(set(lead_ids)) == len(lead_ids)
    rel_by_id = {r.relationship_id: r for r in result.relationships}
    assert len(rel_by_id) == len(result.relationships) == len(result.contradiction_leads)
    for lead in result.contradiction_leads:
        rel = rel_by_id[lead.relationship_id]
        assert (rel.source_id, rel.target_id) == (lead.origin_event_id, lead.destination_event_id)
        assert rel.metadata["lead_id"] == lead.lead_id


def test_pipeline_lead_ids_are_stable_across_runs(tmp_path: Path) -> None:
    runs = []
    for i in range(3):
        root = tmp_path / f"run{i}"
        root.mkdir()
        result = _pipeline_run(root)
        runs.append(
            (
                [(lead.lead_id, lead.relationship_id) for lead in result.contradiction_leads],
                result.result_sha256,
            )
        )
    assert runs[0] == runs[1] == runs[2]
