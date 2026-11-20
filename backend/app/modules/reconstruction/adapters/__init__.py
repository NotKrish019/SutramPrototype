"""Member 4 integration adapters between shared data contracts and reconstruction engines.

Shared contract decisions DEC-001 … DEC-007 are PROPOSED, not approved: no verifiable team
sign-off exists. The gate fails closed (``contracts.APPROVED_CONTRACT_DECISIONS`` is empty).
Callers enable decisions explicitly; ``contracts.PROPOSED_CONTRACT_DECISIONS`` is the proposal.
"""

from app.modules.reconstruction.adapters.contracts import (
    APPROVED_CONTRACT_DECISIONS,
    PROPOSED_CONTRACT_DECISIONS,
    ContractDecision,
)
from app.modules.reconstruction.adapters.diagnostics import (
    DiagnosticSeverity,
    PipelineStage,
    ReconstructionDiagnostic,
)
from app.modules.reconstruction.adapters.entity_adapter import (
    EntityReferenceStatus,
    EntityRegistry,
    build_entity_registry,
)
from app.modules.reconstruction.adapters.errors import (
    ClientStoragePathRejectedError,
    ContractDecisionPendingError,
    EntityRegistryError,
    EventCandidateError,
    EventCandidatePayloadError,
    EvidenceIntegrityError,
    EvidencePathSecurityError,
    EvidenceRecordError,
    EvidenceStorageError,
    ReconstructionAdapterError,
    RelationshipAdapterError,
    RemoteStorageNotSupportedError,
    SourceTimezoneUnknownError,
)
from app.modules.reconstruction.adapters.event_adapter import (
    AdaptedEvent,
    ReconstructedEvent,
    TimezoneBasis,
    adapt_event_candidate,
    parse_event_candidates_payload,
)
from app.modules.reconstruction.adapters.evidence_adapter import (
    ClockCalibrationStatus,
    EvidenceRecordView,
    VerifiedEvidenceFile,
    evidence_to_video_extraction_options,
    parse_evidence_record,
    resolve_evidence_storage_path,
    storage_key_for,
    storage_reference_for,
    verify_evidence_file,
)
from app.modules.reconstruction.adapters.frame_handoff import (
    ExtractedFrameConsumer,
    FrameArtifactReference,
    FrameHandoff,
)
from app.modules.reconstruction.adapters.relationship_adapter import (
    SharedRelationshipRecord,
    build_contradiction_relationship_id,
    contradiction_lead_to_relationship,
)

__all__ = [
    "APPROVED_CONTRACT_DECISIONS",
    "PROPOSED_CONTRACT_DECISIONS",
    "AdaptedEvent",
    "ClientStoragePathRejectedError",
    "ClockCalibrationStatus",
    "ContractDecision",
    "ContractDecisionPendingError",
    "DiagnosticSeverity",
    "EntityReferenceStatus",
    "EntityRegistry",
    "EntityRegistryError",
    "EventCandidateError",
    "EventCandidatePayloadError",
    "EvidenceIntegrityError",
    "EvidencePathSecurityError",
    "EvidenceRecordError",
    "EvidenceRecordView",
    "EvidenceStorageError",
    "ExtractedFrameConsumer",
    "FrameArtifactReference",
    "FrameHandoff",
    "PipelineStage",
    "ReconstructedEvent",
    "ReconstructionAdapterError",
    "ReconstructionDiagnostic",
    "RelationshipAdapterError",
    "RemoteStorageNotSupportedError",
    "SharedRelationshipRecord",
    "SourceTimezoneUnknownError",
    "TimezoneBasis",
    "VerifiedEvidenceFile",
    "adapt_event_candidate",
    "build_contradiction_relationship_id",
    "build_entity_registry",
    "contradiction_lead_to_relationship",
    "evidence_to_video_extraction_options",
    "parse_event_candidates_payload",
    "parse_evidence_record",
    "resolve_evidence_storage_path",
    "storage_key_for",
    "storage_reference_for",
    "verify_evidence_file",
]
