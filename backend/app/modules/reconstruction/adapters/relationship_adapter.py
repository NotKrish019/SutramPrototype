"""Contradiction Relationship Adapter (Member 4 — Phase 3E).

Converts a ``ContradictionLead`` into the canonical shared ``Relationship`` record
(``shared/schemas/README.md`` §5). Under the approved decisions:

- DEC-003: ``relation_type = "CONTRADICTS"``, and ``is_observed = False`` (an inferred link,
  not an observed fact).
- DEC-004: ``weight = None``. No numeric confidence is ever assigned, and ``0.0`` is never
  used as a substitute.

``ContradictionLead`` stays a separate record. The Relationship metadata keeps the
contradiction type, explanation, supporting evidence and provenance, and holds observed facts
and derived inference apart. Relationship IDs are SHA-256 digests over a canonical JSON
encoding of the identifying tuple. They are stable across runs and cannot collide through
delimiter ambiguity.
"""

import math
from collections.abc import Iterable, Mapping
from typing import Any

from pydantic import BaseModel, ConfigDict, Field, field_validator

from app.modules.reconstruction.adapters.contracts import (
    ContractDecision,
    normalize_approved_decisions,
    require_decisions,
)
from app.modules.reconstruction.adapters.errors import RelationshipAdapterError
from app.modules.reconstruction.contradictions import (
    CONTRADICTS_RELATION_TYPE,
    RELATIONSHIP_ID_SCHEME,
    contradiction_relationship_id,
)
from app.schemas.reconstruction import ContradictionLead

RELATIONSHIP_DECISIONS: tuple[ContractDecision, ...] = (
    ContractDecision.RELATION_TYPE_TAXONOMY,
    ContractDecision.RELATIONSHIP_WEIGHT_NULLABLE,
)

INTERPRETATION_NOTICE = (
    "Algorithmically derived investigative lead for human review. It records that two "
    "evidence-backed events are difficult to reconcile under the configured reference "
    "assumptions. It is not proof of identity, deception, guilt or intent. weight is null "
    "because no confidence score is computed; it must not be displayed as 0%."
)

SHARED_RELATIONSHIP_FIELDS: frozenset[str] = frozenset(
    {
        "relationship_id",
        "source_id",
        "target_id",
        "relation_type",
        "weight",
        "is_observed",
        "evidence_basis",
        "metadata",
    }
)


class SharedRelationshipRecord(BaseModel):
    """Backend view of a shared Relationship record (exactly the committed field set)."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    relationship_id: str
    source_id: str
    target_id: str
    relation_type: str
    weight: float | None = Field(default=None, ge=0.0, le=1.0)
    is_observed: bool
    evidence_basis: list[str] = Field(min_length=1)
    metadata: dict[str, Any] = Field(default_factory=dict)

    @field_validator("weight")
    @classmethod
    def _finite_weight(cls, value: float | None) -> float | None:
        if value is not None and not math.isfinite(value):
            raise ValueError("weight must be finite when present.")
        return value


def build_contradiction_relationship_id(lead: ContradictionLead) -> str:
    """Deterministic, delimiter-safe relationship ID for a contradiction lead (engine scheme)."""
    return contradiction_relationship_id(
        lead.case_id, lead.entity_id, lead.origin_event_id, lead.destination_event_id
    )


def contradiction_lead_to_relationship(
    lead: ContradictionLead,
    *,
    approved_decisions: Iterable[ContractDecision | str] | None = None,
    event_provenance: Mapping[str, Mapping[str, Any]] | None = None,
) -> SharedRelationshipRecord:
    """Convert a ContradictionLead into a shared Relationship record (DEC-003 / DEC-004).

    ``event_provenance`` optionally maps event_id to provenance fields (for example
    ``source_record_id`` and ``raw_timestamp``). They are copied into ``metadata.provenance``.
    """
    if not isinstance(lead, ContradictionLead):
        raise RelationshipAdapterError("Expected a ContradictionLead instance.")
    approved = normalize_approved_decisions(approved_decisions)
    require_decisions(
        approved,
        *RELATIONSHIP_DECISIONS,
        context=f"Exporting contradiction lead '{lead.lead_id}' as a shared Relationship",
        subject_id=lead.lead_id,
    )

    if lead.derived_inference.is_observed:
        raise RelationshipAdapterError(
            f"Lead '{lead.lead_id}' is marked observed; contradiction leads must be inferred.",
            subject_id=lead.lead_id,
        )
    if lead.origin_event_id == lead.destination_event_id:
        raise RelationshipAdapterError(
            f"Lead '{lead.lead_id}' links an event to itself.", subject_id=lead.lead_id
        )
    evidence_basis = [eid for eid in lead.evidence_basis if isinstance(eid, str) and eid.strip()]
    if not evidence_basis or len(evidence_basis) != len(lead.evidence_basis):
        raise RelationshipAdapterError(
            f"Lead '{lead.lead_id}' has missing or invalid evidence_basis.",
            subject_id=lead.lead_id,
        )
    expected_basis = {
        lead.observed_facts.origin_source_evidence_id,
        lead.observed_facts.destination_source_evidence_id,
    }
    if set(evidence_basis) != expected_basis:
        raise RelationshipAdapterError(
            f"Lead '{lead.lead_id}' evidence_basis does not match the source evidence of its "
            "two events.",
            subject_id=lead.lead_id,
        )

    provenance_source = event_provenance or {}
    provenance = {
        "lead_id": lead.lead_id,
        "origin_event": {
            **dict(provenance_source.get(lead.origin_event_id, {})),
            "event_id": lead.origin_event_id,
            "source_evidence_id": lead.observed_facts.origin_source_evidence_id,
        },
        "destination_event": {
            **dict(provenance_source.get(lead.destination_event_id, {})),
            "event_id": lead.destination_event_id,
            "source_evidence_id": lead.observed_facts.destination_source_evidence_id,
        },
        "derived_by": "YUKTI Member 4 contradiction engine",
    }

    metadata: dict[str, Any] = {
        "record_kind": "INVESTIGATIVE_LEAD",
        "observation_status": "INFERRED",
        "contradiction_type": lead.lead_type.value,
        "supporting_evidence": {
            "evidence_ids": list(evidence_basis),
            "event_ids": [lead.origin_event_id, lead.destination_event_id],
        },
        "provenance": provenance,
        "source_kind": "EVENT",
        "target_kind": "EVENT",
        "lead_id": lead.lead_id,
        "lead_type": lead.lead_type.value,
        "case_id": lead.case_id,
        "entity_id": lead.entity_id,
        "observed_facts": lead.observed_facts.model_dump(mode="json"),
        "derived_inference": lead.derived_inference.model_dump(mode="json"),
        "explanation": lead.explanation,
        "limitations": list(lead.limitations),
        "interpretation_notice": INTERPRETATION_NOTICE,
        "weight_semantics": "NOT_COMPUTED",
        "contract_basis": [d.value for d in RELATIONSHIP_DECISIONS],
        "relationship_id_scheme": RELATIONSHIP_ID_SCHEME,
    }

    return SharedRelationshipRecord(
        relationship_id=build_contradiction_relationship_id(lead),
        source_id=lead.origin_event_id,
        target_id=lead.destination_event_id,
        relation_type=CONTRADICTS_RELATION_TYPE,
        weight=None,
        is_observed=False,
        evidence_basis=evidence_basis,
        metadata=metadata,
    )
