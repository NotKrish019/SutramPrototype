"""Typed adapter errors for the Member 4 reconstruction integration layer.

Every error carries a stable machine-readable ``code`` and an optional ``subject_id``, so the
pipeline can turn it into a structured diagnostic without parsing message text. Messages refer
to identifiers, hashes and storage *keys* only. They never include evidence contents or absolute
host paths.
"""

from collections.abc import Iterable
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from app.modules.reconstruction.adapters.contracts import ContractDecision


class ReconstructionAdapterError(ValueError):
    """Base class for all expected, input-driven adapter failures."""

    code: str = "ADAPTER_ERROR"

    def __init__(self, message: str, *, subject_id: str | None = None) -> None:
        super().__init__(message)
        self.subject_id = subject_id


class EvidenceRecordError(ReconstructionAdapterError):
    """A shared Evidence record is structurally invalid."""

    code = "INVALID_EVIDENCE_RECORD"


class EvidenceStorageError(ReconstructionAdapterError):
    """Evidence bytes cannot be located or are not a regular local file."""

    code = "EVIDENCE_STORAGE_UNAVAILABLE"


class EvidencePathSecurityError(EvidenceStorageError):
    """The storage key crosses a symlink, escapes the storage root, or is not a safe segment."""

    code = "EVIDENCE_PATH_REJECTED"


class ClientStoragePathRejectedError(EvidencePathSecurityError):
    """storage_uri is a client-supplied path instead of the canonical key or URN."""

    code = "CLIENT_STORAGE_PATH_REJECTED"


class RemoteStorageNotSupportedError(EvidenceStorageError):
    """storage_uri points to remote storage. Remote staging is not implemented."""

    code = "REMOTE_STORAGE_URI_REJECTED"


class EvidenceIntegrityError(ReconstructionAdapterError):
    """Observed evidence bytes do not match the SHA-256 or size recorded at ingestion."""

    code = "EVIDENCE_INTEGRITY_MISMATCH"


class EventCandidatePayloadError(ReconstructionAdapterError):
    """The event-candidate payload (JSON text or container) is not well formed."""

    code = "INVALID_EVENT_CANDIDATE_PAYLOAD"


class EventCandidateError(ReconstructionAdapterError):
    """One Member 1 event candidate is malformed or cannot be normalized."""

    code = "INVALID_EVENT_CANDIDATE"


class SourceTimezoneUnknownError(EventCandidateError):
    """A naive raw timestamp has no declared source_timezone, so it has no UTC instant."""

    code = "SOURCE_TIMEZONE_UNKNOWN"


class ProducerContractGapError(EventCandidateError):
    """The producer's candidate shape lacks fields of the Member 4 candidate contract.

    Raised when a required field is absent, not merely invalid, so the gap is reported as a
    contract mismatch with the producer (for example Member 1 ``ExtractedEventCandidate``).
    Member 4 never infers or synthesizes the missing values.
    """

    code = "PRODUCER_CONTRACT_GAP"


class EntityRegistryError(ReconstructionAdapterError):
    """Shared Entity records or explicit alias registrations are inconsistent."""

    code = "INVALID_ENTITY_REGISTRY"


class RelationshipAdapterError(ReconstructionAdapterError):
    """A contradiction lead cannot be converted into a shared Relationship record."""

    code = "INVALID_CONTRADICTION_LEAD"


class ContractDecisionPendingError(ReconstructionAdapterError):
    """An operation depends on a shared contract decision that is withheld in this run."""

    code = "CONTRACT_DECISION_PENDING"

    def __init__(
        self,
        decisions: "Iterable[ContractDecision]",
        context: str,
        *,
        subject_id: str | None = None,
    ) -> None:
        self.decisions: tuple[ContractDecision, ...] = tuple(sorted(set(decisions)))
        ids = ", ".join(d.value for d in self.decisions)
        super().__init__(
            f"{context} requires shared contract decision(s) {ids}, which are withheld in this "
            "run's approved_contract_decisions.",
            subject_id=subject_id,
        )
