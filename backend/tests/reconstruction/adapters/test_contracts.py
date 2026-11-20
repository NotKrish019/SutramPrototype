"""Contract decision gate. No decision has a verified team sign-off, so the gate fails closed."""

import pytest

from app.modules.reconstruction.adapters.contracts import (
    APPROVAL_RECORD,
    APPROVED_CONTRACT_DECISIONS,
    PROPOSED_CONTRACT_DECISIONS,
    ContractDecision,
    normalize_approved_decisions,
    require_decisions,
)
from app.modules.reconstruction.adapters.errors import ContractDecisionPendingError


def test_no_decision_is_approved_by_default() -> None:
    """Phase 3D.2: the approval record is a copied task brief, not a team sign-off."""
    assert APPROVED_CONTRACT_DECISIONS == frozenset()
    assert normalize_approved_decisions(None) == frozenset()
    assert APPROVAL_RECORD.endswith("team-contract-approvals-2026-11-19.md")


def test_proposal_lists_all_seven_decisions_but_is_not_the_default() -> None:
    assert sorted(d.value for d in PROPOSED_CONTRACT_DECISIONS) == [
        f"DEC-00{i}" for i in range(1, 8)
    ]
    assert PROPOSED_CONTRACT_DECISIONS.isdisjoint(APPROVED_CONTRACT_DECISIONS)
    assert ContractDecision("DEC-007") == ContractDecision.RAW_TIMESTAMP_PRESERVATION


def test_gate_can_still_withhold_a_decision() -> None:
    approved = normalize_approved_decisions({"DEC-001", "DEC-006"})
    with pytest.raises(ContractDecisionPendingError) as excinfo:
        require_decisions(
            approved,
            ContractDecision.CLOCK_OFFSET_CONVENTION,
            ContractDecision.RAW_TIMESTAMP_PRESERVATION,
            context="test",
        )
    assert excinfo.value.decisions == (ContractDecision.RAW_TIMESTAMP_PRESERVATION,)
    assert "withheld" in str(excinfo.value)


def test_invalid_approval_sets_rejected() -> None:
    with pytest.raises(ValueError, match="Unknown shared contract decision"):
        normalize_approved_decisions({"DEC-042"})
    with pytest.raises(TypeError):
        normalize_approved_decisions("DEC-001")
