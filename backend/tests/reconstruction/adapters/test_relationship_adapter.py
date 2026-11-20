"""Phase 3E — Contradiction Relationship Adapter tests (DEC-003 / DEC-004 approved)."""

import json
import re
from typing import Any

import pytest
from tests.reconstruction.adapters.synthetic_burglary import CASE_ID, GURUGRAM, WAREHOUSE, engine_level_events

from app.modules.reconstruction.adapters.contracts import ContractDecision
from app.modules.reconstruction.adapters.errors import (
    ContractDecisionPendingError,
    RelationshipAdapterError,
)
from app.modules.reconstruction.adapters.relationship_adapter import (
    SHARED_RELATIONSHIP_FIELDS,
    SharedRelationshipRecord,
    build_contradiction_relationship_id,
    contradiction_lead_to_relationship,
)
from app.modules.reconstruction.contradictions import detect_contradictions
from app.schemas.reconstruction import ContradictionLead

APPROVED = {ContractDecision.RELATION_TYPE_TAXONOMY, ContractDecision.RELATIONSHIP_WEIGHT_NULLABLE}


def _burglary_lead() -> ContradictionLead:
    result = detect_contradictions(engine_level_events(), case_id=CASE_ID)
    assert len(result.leads) == 1
    return result.leads[0]


def _lead_between(entity_id: str, origin_id: str, dest_id: str) -> ContradictionLead:
    events = [
        {
            "event_id": origin_id,
            "case_id": CASE_ID,
            "title": "Sighting A",
            "timestamp": "2026-11-10T16:10:00Z",
            "timestamp_precision": "EXACT_SECOND",
            "coordinates": dict(WAREHOUSE),
            "involved_entity_ids": [entity_id],
            "source_evidence_id": "ev-cctv-dock",
        },
        {
            "event_id": dest_id,
            "case_id": CASE_ID,
            "title": "Sighting B",
            "timestamp": "2026-11-10T16:11:00Z",
            "timestamp_precision": "EXACT_SECOND",
            "coordinates": dict(GURUGRAM),
            "involved_entity_ids": [entity_id],
            "source_evidence_id": "ev-chat-export",
        },
    ]
    (lead,) = detect_contradictions(events, case_id=CASE_ID).leads
    return lead


def test_default_refuses_export_because_nothing_is_approved() -> None:
    with pytest.raises(ContractDecisionPendingError):
        contradiction_lead_to_relationship(_burglary_lead())  # no verified approvals


def test_proposed_approvals_export_contradicts_with_null_weight() -> None:
    rel = contradiction_lead_to_relationship(_burglary_lead(), approved_decisions=APPROVED)
    assert rel.relation_type == "CONTRADICTS"
    assert rel.is_observed is False
    assert rel.weight is None


def test_export_refused_when_decisions_withheld() -> None:
    with pytest.raises(ContractDecisionPendingError) as excinfo:
        contradiction_lead_to_relationship(_burglary_lead(), approved_decisions=set())
    assert excinfo.value.decisions == (
        ContractDecision.RELATION_TYPE_TAXONOMY,
        ContractDecision.RELATIONSHIP_WEIGHT_NULLABLE,
    )


def test_null_weight_requires_dec004_even_if_dec003_approved() -> None:
    with pytest.raises(ContractDecisionPendingError) as excinfo:
        contradiction_lead_to_relationship(
            _burglary_lead(), approved_decisions={ContractDecision.RELATION_TYPE_TAXONOMY}
        )
    assert excinfo.value.decisions == (ContractDecision.RELATIONSHIP_WEIGHT_NULLABLE,)


def test_contradiction_type_supporting_evidence_and_provenance_kept() -> None:
    lead = _burglary_lead()
    provenance = {
        "evt-01-shutter-forced": {"source_record_id": "osd:1", "raw_timestamp": "2026-11-10T21:42:00"},
        "evt-03-location-shared": {"source_record_id": "line:1", "event_id": "spoofed"},
    }
    rel = contradiction_lead_to_relationship(
        lead, approved_decisions=APPROVED, event_provenance=provenance
    )
    meta = rel.metadata
    assert meta["contradiction_type"] == lead.lead_type.value
    assert meta["observation_status"] == "INFERRED"
    assert meta["supporting_evidence"] == {
        "evidence_ids": ["ev-cctv-dock", "ev-chat-export"],
        "event_ids": ["evt-01-shutter-forced", "evt-03-location-shared"],
    }
    origin = meta["provenance"]["origin_event"]
    assert origin["source_record_id"] == "osd:1"
    assert origin["raw_timestamp"] == "2026-11-10T21:42:00"
    # Supplied provenance can never overwrite the lead's own identifiers.
    assert meta["provenance"]["destination_event"]["event_id"] == "evt-03-location-shared"
    assert meta["explanation"] == lead.explanation


def test_no_numeric_confidence_anywhere() -> None:
    rel = contradiction_lead_to_relationship(_burglary_lead(), approved_decisions=APPROVED)

    def keys(obj: object) -> set[str]:
        if isinstance(obj, dict):
            return set(obj) | {k for v in obj.values() for k in keys(v)}
        if isinstance(obj, list):
            return {k for v in obj for k in keys(v)}
        return set()

    dumped = rel.model_dump(mode="json")
    assert dumped["weight"] is None
    assert not {k for k in keys(dumped) if "confidence" in k or k == "score"}


def test_relationship_conforms_to_shared_contract() -> None:
    lead = _burglary_lead()
    rel = contradiction_lead_to_relationship(lead, approved_decisions=APPROVED)
    dumped = rel.model_dump(mode="json")
    assert set(dumped) == SHARED_RELATIONSHIP_FIELDS
    assert rel.relation_type == "CONTRADICTS"
    assert rel.is_observed is False
    assert rel.weight is None
    assert rel.source_id == "evt-01-shutter-forced"
    assert rel.target_id == "evt-03-location-shared"
    json.dumps(dumped)  # fully JSON-serializable for transport / persistence


def test_evidence_basis_and_provenance_preserved() -> None:
    lead = _burglary_lead()
    rel = contradiction_lead_to_relationship(lead, approved_decisions=APPROVED)
    assert rel.evidence_basis == lead.evidence_basis == ["ev-cctv-dock", "ev-chat-export"]
    meta = rel.metadata
    assert meta["lead_id"] == lead.lead_id
    assert meta["observed_facts"]["origin_source_evidence_id"] == "ev-cctv-dock"
    assert meta["observed_facts"]["destination_source_evidence_id"] == "ev-chat-export"
    assert meta["derived_inference"]["is_observed"] is False
    assert meta["derived_inference"]["min_required_speed_kmh"] > 180.0
    assert meta["weight_semantics"] == "NOT_COMPUTED"
    assert meta["contract_basis"] == ["DEC-003", "DEC-004"]


def test_metadata_never_asserts_guilt_or_proof() -> None:
    rel = contradiction_lead_to_relationship(_burglary_lead(), approved_decisions=APPROVED)
    text = json.dumps(rel.metadata).lower()
    assert "not proof" in text
    for phrase in ("is guilty", "proves", "proven guilty", "lied", "is lying", "confirmed identity", "suspect is"):
        assert phrase not in text


def test_relationship_id_is_stable_and_well_formed() -> None:
    first = contradiction_lead_to_relationship(_burglary_lead(), approved_decisions=APPROVED)
    second = contradiction_lead_to_relationship(_burglary_lead(), approved_decisions=APPROVED)
    assert first.relationship_id == second.relationship_id
    assert re.fullmatch(r"rel-contra-[0-9a-f]{32}", first.relationship_id)
    assert first == second


def test_relationship_id_resists_delimiter_collisions() -> None:
    # These two tuples collided under the old hyphen-joined engine IDs. Phase 3D.2: the engine
    # and the exported Relationship now share one canonical SHA-256 ID, distinct for each tuple.
    lead_a = _lead_between("ent-a", "b-evt", "c")
    lead_b = _lead_between("ent-a-b", "evt", "c")
    assert lead_a.lead_id != lead_b.lead_id
    assert lead_a.relationship.relationship_id != lead_b.relationship.relationship_id
    assert build_contradiction_relationship_id(lead_a) != build_contradiction_relationship_id(lead_b)
    for lead in (lead_a, lead_b):
        assert build_contradiction_relationship_id(lead) == lead.relationship.relationship_id


def test_relationship_id_distinguishes_direction() -> None:
    forward = _lead_between("ent-person-ravi", "evt-a", "evt-b")
    backward = _lead_between("ent-person-ravi", "evt-b", "evt-a")
    assert build_contradiction_relationship_id(forward) != build_contradiction_relationship_id(backward)


def test_mismatched_evidence_basis_rejected() -> None:
    lead = _burglary_lead()
    tampered = lead.model_copy(update={"evidence_basis": ["ev-cdr-tower"]})
    with pytest.raises(RelationshipAdapterError, match="evidence_basis"):
        contradiction_lead_to_relationship(tampered, approved_decisions=APPROVED)


def test_observed_lead_rejected() -> None:
    lead = _burglary_lead()
    observed = lead.model_copy(
        update={"derived_inference": lead.derived_inference.model_copy(update={"is_observed": True})}
    )
    with pytest.raises(RelationshipAdapterError, match="observed"):
        contradiction_lead_to_relationship(observed, approved_decisions=APPROVED)


def test_non_lead_input_rejected() -> None:
    with pytest.raises(RelationshipAdapterError):
        contradiction_lead_to_relationship({"lead_id": "x"}, approved_decisions=APPROVED)  # type: ignore[arg-type]


def test_shared_record_rejects_extra_fields_and_out_of_range_weight() -> None:
    base: dict[str, Any] = {
        "relationship_id": "rel-1",
        "source_id": "a",
        "target_id": "b",
        "relation_type": "CONTRADICTS",
        "is_observed": False,
        "evidence_basis": ["ev-1"],
    }
    with pytest.raises(ValueError):
        SharedRelationshipRecord.model_validate({**base, "confidence": 0.9})
    with pytest.raises(ValueError):
        SharedRelationshipRecord.model_validate({**base, "weight": 1.5})
    with pytest.raises(ValueError):
        SharedRelationshipRecord.model_validate({**base, "evidence_basis": []})


def test_lead_input_not_mutated() -> None:
    lead = _burglary_lead()
    before = lead.model_dump()
    contradiction_lead_to_relationship(lead, approved_decisions=APPROVED)
    assert lead.model_dump() == before
