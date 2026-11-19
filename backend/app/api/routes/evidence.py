"""Evidence Ingestion & Management API Routes (Member 2).

Provides investigator-facing endpoints for evidence upload, cryptographic verification,
status lookup, and idempotent extraction re-processing.
"""

from __future__ import annotations

import json
from typing import Any, Optional
from fastapi import APIRouter, File, Form, HTTPException, UploadFile, status

from app.modules.ingestion.service import EvidenceIngestionService
from app.schemas.ingestion import (
    EvidenceUploadResponse,
    ExtractionRunDTO,
)


router = APIRouter(prefix="/api/v1/evidence", tags=["Evidence Ingestion"])

# Default service instance for route handlers
ingestion_service = EvidenceIngestionService()


@router.post(
    "/upload",
    response_model=EvidenceUploadResponse,
    status_code=status.HTTP_201_CREATED,
    summary="Upload and ingest digital evidence file",
    description=(
        "Streams evidence bytes to secure storage, calculates SHA-256 incrementally, "
        "validates content signatures, and optionally triggers extraction candidate generation."
    ),
)
async def upload_evidence(
    file: UploadFile = File(..., description="Binary evidence file payload"),
    case_id: str = Form(..., description="Target forensic case identifier (UUID or safe string)"),
    source_type: Optional[str] = Form(
        None,
        description="Explicit modality (e.g. CALL_DETAIL_RECORD, CHAT_EXPORT). Inferred if omitted.",
    ),
    source_timezone: str = Form(
        "UTC",
        description="IANA timezone (e.g. 'Asia/Kolkata') for device clock normalization per DEC-006.",
    ),
    clock_offset_seconds: float = Form(
        0.0,
        description="Signed additive clock correction in seconds per DEC-001 (nominal = device + offset).",
    ),
    metadata: Optional[str] = Form(
        None,
        description="Optional JSON-encoded dictionary of technical/investigator metadata.",
    ),
    trigger_extraction: bool = Form(
        False,
        description="If True, immediately invokes modality extraction and generates candidates.",
    ),
) -> EvidenceUploadResponse:
    """Handle multipart file upload, storage, hashing, and optional extraction."""
    # 1. Parse optional metadata string
    parsed_metadata: dict[str, Any] = {}
    if metadata:
        try:
            parsed = json.loads(metadata)
            if isinstance(parsed, dict):
                parsed_metadata = parsed
            else:
                raise HTTPException(
                    status_code=status.HTTP_400_BAD_REQUEST,
                    detail="Form field 'metadata' must be a valid JSON object/dictionary.",
                )
        except json.JSONDecodeError as exc:
            raise HTTPException(
                status_code=status.HTTP_400_BAD_REQUEST,
                detail=f"Form field 'metadata' contains invalid JSON: {exc.msg}.",
            ) from exc

    # 2. Invoke ingestion service
    try:
        response = await ingestion_service.ingest_evidence(
            case_id=case_id,
            file_stream=file.file,
            original_filename=file.filename or "uploaded_evidence.bin",
            source_type=source_type,
            source_timezone=source_timezone,
            clock_offset_seconds=clock_offset_seconds,
            metadata=parsed_metadata,
            trigger_extraction=trigger_extraction,
        )
        return response

    except ValueError as exc:
        err_msg = str(exc)
        if "maximum allowable evidence size" in err_msg:
            raise HTTPException(
                status_code=status.HTTP_413_REQUEST_ENTITY_TOO_LARGE,
                detail=err_msg,
            ) from exc
        if "Duplicate evidence detected" in err_msg:
            raise HTTPException(
                status_code=status.HTTP_409_CONFLICT,
                detail=err_msg,
            ) from exc
        if "Path traversal detected" in err_msg or "Invalid case_id" in err_msg:
            raise HTTPException(
                status_code=status.HTTP_400_BAD_REQUEST,
                detail=err_msg,
            ) from exc
        if "Content validation rejected" in err_msg:
            raise HTTPException(
                status_code=status.HTTP_422_UNPROCESSABLE_ENTITY,
                detail=err_msg,
            ) from exc

        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail=err_msg,
        ) from exc

    except Exception as exc:
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Unexpected failure during evidence intake: {str(exc)}",
        ) from exc


@router.get(
    "/{evidence_id}",
    response_model=EvidenceUploadResponse,
    summary="Retrieve evidence item metadata",
    description="Returns metadata and current status for an ingested evidence item.",
)
async def get_evidence_item(evidence_id: str) -> EvidenceUploadResponse:
    """Lookup an ingested evidence record by UUID."""
    record = ingestion_service.get_evidence(evidence_id)
    if not record:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail=f"Evidence item with id '{evidence_id}' was not found.",
        )

    return EvidenceUploadResponse(
        evidence_id=record.evidence_id,
        case_id=record.case_id,
        original_filename=record.original_filename,
        source_type=record.source_type,
        mime_type=record.mime_type,
        file_size_bytes=record.file_size_bytes,
        sha256_hash=record.sha256_hash,
        storage_uri=str(record.storage_path.as_posix()),
        status=record.status,
        metadata=record.metadata,
        created_at=record.created_at,
        active_extraction_run_id=record.active_run_id,
    )


@router.get(
    "/{evidence_id}/runs",
    response_model=list[ExtractionRunDTO],
    summary="List all versioned extraction runs for an evidence item",
    description="Returns the full audit history of all extraction runs executed for this evidence.",
)
async def list_extraction_runs(evidence_id: str) -> list[ExtractionRunDTO]:
    """Retrieve full history of extraction runs for an evidence item."""
    record = ingestion_service.get_evidence(evidence_id)
    if not record:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail=f"Evidence item with id '{evidence_id}' was not found.",
        )

    return ingestion_service.get_runs_for_evidence(evidence_id)


@router.post(
    "/{evidence_id}/reprocess",
    response_model=ExtractionRunDTO,
    summary="Re-execute extraction for an evidence item",
    description=(
        "Executes a new extraction run without deleting or modifying raw evidence. "
        "Upon success, marks previous active run as SUPERSEDED."
    ),
)
async def reprocess_evidence(
    evidence_id: str,
    source_timezone: str = Form("UTC"),
    clock_offset_seconds: float = Form(0.0),
) -> ExtractionRunDTO:
    """Execute a non-destructive re-extraction run."""
    record = ingestion_service.get_evidence(evidence_id)
    if not record:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail=f"Evidence item with id '{evidence_id}' was not found.",
        )

    run_dto = await ingestion_service.execute_extraction_run(
        record,
        source_timezone=source_timezone,
        clock_offset_seconds=clock_offset_seconds,
    )
    return run_dto
