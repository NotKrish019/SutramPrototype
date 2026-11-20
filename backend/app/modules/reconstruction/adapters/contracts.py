"""Shared-contract decision gate for the Member 4 integration adapters.

The Phase 3C decision package (``docs/member-4/phase-3c-contract-decisions.md``) proposes
DEC-001 … DEC-006, and DEC-007 was added later. **No verifiable team sign-off exists for any of
them.** The only record (``docs/member-4/team-contract-approvals-2026-11-19.md``) is a copy of a
task brief, not a team approval, so it does not count as approval evidence (Phase 3D.2 audit).

The gate therefore fails closed: ``APPROVED_CONTRACT_DECISIONS`` is empty, and every behaviour
that depends on a decision is refused with a diagnostic that names it. A caller can enable
decisions explicitly through ``ReconstructionPipelineConfig.approved_contract_decisions``.
``PROPOSED_CONTRACT_DECISIONS`` lists the full proposal, for tests and demos that exercise
behaviour *conditional on* approval. Using it never makes a decision approved.

Withholding a decision never brings back pre-proposal assumptions:

- DEC-001 / DEC-006 / DEC-007 withheld → event candidates are rejected. No UTC timestamp is
  produced, and raw timestamps are never treated as already-normalized UTC. Video frames are
  not anchored to UTC.
- DEC-002 withheld → candidates are rejected (precision semantics are undefined).
- DEC-003 / DEC-004 withheld → contradiction leads are not exported as Relationship records.
- DEC-005 withheld → candidates that carry ``duration_seconds`` are rejected.
"""

from collections.abc import Iterable
from enum import StrEnum

from app.modules.reconstruction.adapters.errors import ContractDecisionPendingError


class ContractDecision(StrEnum):
    """Identifiers of the proposed shared-contract decisions."""

    CLOCK_OFFSET_CONVENTION = "DEC-001"
    TIME_PRECISION_SEMANTICS = "DEC-002"
    RELATION_TYPE_TAXONOMY = "DEC-003"
    RELATIONSHIP_WEIGHT_NULLABLE = "DEC-004"
    EVENT_DURATION_UNCERTAINTY = "DEC-005"
    EVIDENCE_METADATA_KEYS = "DEC-006"
    RAW_TIMESTAMP_PRESERVATION = "DEC-007"


# Where a genuine team sign-off must be recorded. The file currently states that no verified
# approval exists; it is not approval evidence.
APPROVAL_RECORD = "docs/member-4/team-contract-approvals-2026-11-19.md"

# Decisions with a verified team sign-off. Empty until one is recorded in APPROVAL_RECORD.
APPROVED_CONTRACT_DECISIONS: frozenset[ContractDecision] = frozenset()

# The full proposal (DEC-001 … DEC-007). Proposed only, NOT approved. Pass it explicitly to run
# behaviour conditional on approval (tests, demos); results then list it as the approved set.
PROPOSED_CONTRACT_DECISIONS: frozenset[ContractDecision] = frozenset(ContractDecision)

# Decisions needed to turn a raw device timestamp into a corrected UTC instant.
TEMPORAL_CORRECTION_DECISIONS: tuple[ContractDecision, ...] = (
    ContractDecision.CLOCK_OFFSET_CONVENTION,
    ContractDecision.EVIDENCE_METADATA_KEYS,
    ContractDecision.RAW_TIMESTAMP_PRESERVATION,
)


def normalize_approved_decisions(
    approved: Iterable[ContractDecision | str] | None,
) -> frozenset[ContractDecision]:
    """Validate an approval set; ``None`` means the verified approvals (currently none)."""
    if approved is None:
        return APPROVED_CONTRACT_DECISIONS
    if isinstance(approved, (str, bytes)):
        raise TypeError("approved_decisions must be an iterable of decision IDs, not a string.")
    result: set[ContractDecision] = set()
    for item in approved:
        try:
            result.add(ContractDecision(item))
        except ValueError:
            raise ValueError(f"Unknown shared contract decision id {item!r}.") from None
    return frozenset(result)


def missing_decisions(
    approved: frozenset[ContractDecision],
    *required: ContractDecision,
) -> tuple[ContractDecision, ...]:
    """Return the required decisions absent from ``approved`` in deterministic order."""
    return tuple(sorted(d for d in set(required) if d not in approved))


def require_decisions(
    approved: frozenset[ContractDecision],
    *required: ContractDecision,
    context: str,
    subject_id: str | None = None,
) -> None:
    """Raise ContractDecisionPendingError when any required decision is withheld."""
    missing = missing_decisions(approved, *required)
    if missing:
        raise ContractDecisionPendingError(missing, context, subject_id=subject_id)
