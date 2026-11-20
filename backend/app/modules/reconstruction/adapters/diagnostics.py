"""Structured, deterministic diagnostics emitted by the reconstruction adapters and pipeline."""

from enum import StrEnum

from pydantic import BaseModel, ConfigDict, Field

from app.modules.reconstruction.adapters.errors import (
    ContractDecisionPendingError,
    ReconstructionAdapterError,
)


class DiagnosticSeverity(StrEnum):
    """INFO = informational; WARNING = capability withheld; ERROR = input or stage failure."""

    INFO = "INFO"
    WARNING = "WARNING"
    ERROR = "ERROR"


class PipelineStage(StrEnum):
    """Pipeline stages in execution order (used for deterministic diagnostic ordering)."""

    INPUT = "INPUT"
    EVIDENCE = "EVIDENCE"
    EVENTS = "EVENTS"
    ENTITIES = "ENTITIES"
    VIDEO = "VIDEO"
    FRAME_HANDOFF = "FRAME_HANDOFF"
    ENTITY_RESOLUTION = "ENTITY_RESOLUTION"
    SPATIAL = "SPATIAL"
    CONTRADICTIONS = "CONTRADICTIONS"
    RELATIONSHIPS = "RELATIONSHIPS"


_STAGE_ORDER: dict[PipelineStage, int] = {
    PipelineStage(stage): idx for idx, stage in enumerate(PipelineStage)
}
_SEVERITY_ORDER: dict[DiagnosticSeverity, int] = {
    DiagnosticSeverity.ERROR: 0,
    DiagnosticSeverity.WARNING: 1,
    DiagnosticSeverity.INFO: 2,
}


class ReconstructionDiagnostic(BaseModel):
    """A single explainable pipeline finding. Never contains evidence contents."""

    model_config = ConfigDict(frozen=True)

    severity: DiagnosticSeverity
    stage: PipelineStage
    code: str = Field(description="Stable machine-readable diagnostic code.")
    message: str
    subject_type: str | None = Field(
        default=None, description="EVIDENCE, EVENT, ENTITY, ENTITY_REFERENCE or LEAD."
    )
    subject_id: str | None = None
    pending_contract_decisions: list[str] = Field(
        default_factory=list,
        description="Shared contract decisions withheld in this run that caused this diagnostic.",
    )


def diagnostic_from_error(
    exc: ReconstructionAdapterError,
    *,
    stage: PipelineStage,
    subject_type: str | None,
    fallback_subject_id: str | None = None,
) -> ReconstructionDiagnostic:
    """Convert a typed adapter error into an ERROR diagnostic, preserving pending decisions."""
    pending = (
        [d.value for d in exc.decisions] if isinstance(exc, ContractDecisionPendingError) else []
    )
    return ReconstructionDiagnostic(
        severity=DiagnosticSeverity.ERROR,
        stage=stage,
        code=exc.code,
        message=str(exc),
        subject_type=subject_type,
        subject_id=exc.subject_id or fallback_subject_id,
        pending_contract_decisions=pending,
    )


def sort_diagnostics(
    diagnostics: list[ReconstructionDiagnostic],
) -> list[ReconstructionDiagnostic]:
    """Order diagnostics deterministically by stage, severity, subject, code and message."""
    return sorted(
        diagnostics,
        key=lambda d: (
            _STAGE_ORDER[d.stage],
            _SEVERITY_ORDER[d.severity],
            d.subject_type or "",
            d.subject_id or "",
            d.code,
            d.message,
        ),
    )
