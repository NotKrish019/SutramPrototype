"""Data Transfer Objects for Evidence Ingestion and Extraction Run Tracking (Member 2).

These schemas govern the ingestion API request/response contracts, cryptographic
metadata, safe storage verification, and versioned extraction-run tracking.
"""

from __future__ import annotations

from enum import Enum
from typing import Any, Optional
from pydantic import BaseModel, ConfigDict, Field


class IngestionStatus(str, Enum):
    """Lifecycle status of an ingested evidence record."""

    INGESTED = "INGESTED"
    EXTRACTION_PENDING = "EXTRACTION_PENDING"
    EXTRACTED = "EXTRACTED"
    EXTRACTION_FAILED = "EXTRACTION_FAILED"
    CORRUPTED = "CORRUPTED"


class ExtractionRunStatus(str, Enum):
    """Lifecycle status of an individual, versioned extraction run."""

    PENDING = "PENDING"
    RUNNING = "RUNNING"
    SUCCESS = "SUCCESS"
    PARTIAL_SUCCESS = "PARTIAL_SUCCESS"
    FAILED = "FAILED"
    SUPERSEDED = "SUPERSEDED"


class ExtractionRunDTO(BaseModel):
    """Record of an individual extraction execution.
    
    Prevents destructive blind deletion of forensic history by tracking multiple
    runs per evidence item with explicit versioning and status.
    """

    model_config = ConfigDict(frozen=True)

    extraction_run_id: str = Field(description="Unique UUIDv4 identifier for this extraction execution.")
    evidence_id: str = Field(description="Target Evidence item UUIDv4.")
    source_sha256: str = Field(description="SHA-256 hash of the evidence file verified prior to run.")
    extractor_name: str = Field(description="Name of the extractor module (e.g. 'cdr_extractor', 'chat_extractor').")
    extractor_version: str = Field(description="Version string of the extraction logic (e.g. '1.0.0').")
    configuration_hash: str = Field(
        description="Deterministic hash of extraction parameters (e.g. timezone, offsets, keywords)."
    )
    status: ExtractionRunStatus = Field(description="Execution outcome.")
    started_at: str = Field(description="UTC ISO 8601 timestamp with trailing 'Z' when run commenced.")
    completed_at: Optional[str] = Field(
        default=None,
        description="UTC ISO 8601 timestamp with trailing 'Z' when run terminated.",
    )
    records_processed: int = Field(default=0, ge=0, description="Total records/rows examined.")
    valid_records_count: int = Field(default=0, ge=0, description="Successfully parsed records.")
    rejected_records_count: int = Field(default=0, ge=0, description="Rejected/malformed records.")
    is_active: bool = Field(
        default=True,
        description="True if this is the currently accepted, active extraction run for the evidence item.",
    )
    error_message: Optional[str] = Field(
        default=None,
        description="Explanatory error diagnostic if status is FAILED.",
    )


class EvidenceUploadResponse(BaseModel):
    """Response returned upon successful evidence file upload and ingestion."""

    model_config = ConfigDict(frozen=True)

    evidence_id: str = Field(description="Generated canonical UUIDv4 for this evidence item.")
    case_id: str = Field(description="Forensic case identifier.")
    original_filename: str = Field(description="Original uploaded filename (sanitized).")
    source_type: str = Field(description="Categorical modality (e.g. CALL_DETAIL_RECORD, CHAT_EXPORT).")
    mime_type: str = Field(description="Validated MIME type determined via content inspection.")
    file_size_bytes: int = Field(ge=0, description="Exact file size in bytes.")
    sha256_hash: str = Field(
        min_length=64,
        max_length=64,
        description="Cryptographic SHA-256 hash computed incrementally from upload bytes.",
    )
    storage_uri: str = Field(description="Safe storage path URI within the evidence repository.")
    status: IngestionStatus = Field(description="Current lifecycle status of the evidence item.")
    metadata: dict[str, Any] = Field(
        default_factory=dict,
        description="Preserved ingestion metadata (source timezone, clock offset, etc.).",
    )
    created_at: str = Field(description="UTC ISO 8601 timestamp with trailing 'Z'.")
    active_extraction_run_id: Optional[str] = Field(
        default=None,
        description="UUID of the active extraction run if extraction was executed.",
    )
    extraction_summary: Optional[dict[str, Any]] = Field(
        default=None,
        description="Summary of extraction counts and status if extraction was triggered.",
    )
