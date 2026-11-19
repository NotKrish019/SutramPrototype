"""Evidence Ingestion Service & Idempotent Extraction Run Coordinator (Member 2).

Orchestrates streaming upload, cryptographic hashing, content sniffing, safe file
storage, duplicate detection, and non-destructive versioned extraction runs.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
from typing import Any, BinaryIO, Optional
import uuid

from app.core.config import settings
from app.modules.ingestion.storage import (
    get_safe_storage_path,
    make_file_read_only,
    safe_unlink,
    save_evidence_file,
    verify_evidence_integrity,
)
from app.modules.ingestion.validation import sniff_file_content
from app.schemas.ingestion import (
    EvidenceUploadResponse,
    ExtractionRunDTO,
    ExtractionRunStatus,
    IngestionStatus,
)


@dataclass
class EvidenceRecord:
    """In-memory representation of an ingested evidence item."""

    evidence_id: str
    case_id: str
    original_filename: str
    source_type: str
    mime_type: str
    file_size_bytes: int
    sha256_hash: str
    storage_path: Path
    status: IngestionStatus
    metadata: dict[str, Any]
    created_at: str
    active_run_id: Optional[str] = None


@dataclass
class ExtractionRunRecord:
    """In-memory representation of an individual extraction execution."""

    run_id: str
    evidence_id: str
    source_sha256: str
    extractor_name: str
    extractor_version: str
    config_hash: str
    status: ExtractionRunStatus
    started_at: str
    completed_at: Optional[str] = None
    records_processed: int = 0
    valid_count: int = 0
    rejected_count: int = 0
    is_active: bool = True
    error_message: Optional[str] = None
    result_payload: Optional[dict[str, Any]] = None


class IngestionRepository:
    """In-memory repository for evidence items and extraction runs (Phase 1D).
    
    Provides thread-safe, transactional-like behavior for local development
    and testing prior to full PostgreSQL / Neo4j container provisioning.
    """

    def __init__(self) -> None:
        self._evidence: dict[str, EvidenceRecord] = {}
        self._runs: dict[str, ExtractionRunRecord] = {}
        # case_id -> {sha256_hash -> evidence_id}
        self._hash_index: dict[str, dict[str, str]] = {}

    def save_evidence(self, record: EvidenceRecord) -> None:
        self._evidence[record.evidence_id] = record
        if record.case_id not in self._hash_index:
            self._hash_index[record.case_id] = {}
        self._hash_index[record.case_id][record.sha256_hash] = record.evidence_id

    def get_evidence(self, evidence_id: str) -> Optional[EvidenceRecord]:
        return self._evidence.get(evidence_id)

    def find_by_hash(self, case_id: str, sha256_hash: str) -> Optional[EvidenceRecord]:
        ev_id = self._hash_index.get(case_id, {}).get(sha256_hash)
        if ev_id:
            return self._evidence.get(ev_id)
        return None

    def save_run(self, run: ExtractionRunRecord) -> None:
        self._runs[run.run_id] = run

    def get_run(self, run_id: str) -> Optional[ExtractionRunRecord]:
        return self._runs.get(run_id)

    def get_runs_for_evidence(self, evidence_id: str) -> list[ExtractionRunRecord]:
        return [r for r in self._runs.values() if r.evidence_id == evidence_id]

    def supersede_prior_runs(self, evidence_id: str, current_active_run_id: str) -> None:
        """Mark all prior successful runs for this evidence item as SUPERSEDED."""
        for run in self._runs.values():
            if run.evidence_id == evidence_id and run.run_id != current_active_run_id:
                if run.is_active:
                    run.is_active = False
                    if run.status in {ExtractionRunStatus.SUCCESS, ExtractionRunStatus.PARTIAL_SUCCESS}:
                        run.status = ExtractionRunStatus.SUPERSEDED

    def clear(self) -> None:
        """Clear registry (used between unit tests)."""
        self._evidence.clear()
        self._runs.clear()
        self._hash_index.clear()


# Global in-memory singleton registry for Phase 1D
default_registry = IngestionRepository()


class EvidenceIngestionService:
    """Core backend ingestion engine managing evidence intake and extraction runs."""

    def __init__(
        self,
        storage_base_dir: Optional[Path | str] = None,
        registry: Optional[IngestionRepository] = None,
    ) -> None:
        self.storage_base_dir = Path(storage_base_dir or settings.EVIDENCE_STORAGE_PATH)
        self.registry = registry or default_registry

    async def ingest_evidence(
        self,
        case_id: str,
        file_stream: BinaryIO,
        original_filename: str,
        *,
        source_type: Optional[str] = None,
        source_timezone: str = "UTC",
        clock_offset_seconds: float = 0.0,
        metadata: Optional[dict[str, Any]] = None,
        trigger_extraction: bool = False,
    ) -> EvidenceUploadResponse:
        """Ingest evidence file with cryptographic hashing, content sniffing, and safe storage."""
        evidence_id = str(uuid.uuid4())
        now_utc = datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")

        # 1. Resolve safe target path
        dest_path = get_safe_storage_path(
            self.storage_base_dir,
            case_id=case_id,
            evidence_id=evidence_id,
            filename=original_filename,
        )

        # 2. Stream bytes to storage and compute SHA-256 incrementally
        bytes_written, sha256_hash = save_evidence_file(
            file_stream,
            dest_path,
            max_bytes=settings.MAX_UPLOAD_SIZE_BYTES,
        )

        # 3. Content sniffing verification
        is_valid, detected_mime, inferred_type, reason = sniff_file_content(
            dest_path, original_filename
        )
        if not is_valid:
            # Forensic clean-up of rejected unverified file
            safe_unlink(dest_path)
            raise ValueError(f"Content validation rejected evidence '{original_filename}': {reason}")

        resolved_source_type = source_type or inferred_type

        # 4. Check for duplicate upload within same case
        existing = self.registry.find_by_hash(case_id, sha256_hash)
        if existing:
            # Clean up the redundant write to avoid waste
            safe_unlink(dest_path)
            raise ValueError(
                f"Duplicate evidence detected: file with identical SHA-256 ({sha256_hash}) "
                f"is already registered under evidence_id '{existing.evidence_id}'."
            )

        # 5. Build metadata payload
        merged_metadata = dict(metadata or {})
        merged_metadata.update({
            "source_timezone": source_timezone,
            "clock_offset_seconds": clock_offset_seconds,
            "content_detection_reason": reason,
        })

        # 6. Register Evidence Record
        record = EvidenceRecord(
            evidence_id=evidence_id,
            case_id=case_id,
            original_filename=original_filename,
            source_type=resolved_source_type,
            mime_type=detected_mime,
            file_size_bytes=bytes_written,
            sha256_hash=sha256_hash,
            storage_path=dest_path,
            status=IngestionStatus.INGESTED,
            metadata=merged_metadata,
            created_at=now_utc,
        )
        self.registry.save_evidence(record)

        # Seal file as read-only now that it is validated and registered
        make_file_read_only(dest_path)

        # 7. Optionally trigger extraction
        active_run_id = None
        extraction_summary = None
        if trigger_extraction:
            run_dto = await self.execute_extraction_run(
                record,
                source_timezone=source_timezone,
                clock_offset_seconds=clock_offset_seconds,
            )
            active_run_id = run_dto.extraction_run_id
            extraction_summary = {
                "status": run_dto.status.value,
                "records_processed": run_dto.records_processed,
                "valid_records_count": run_dto.valid_records_count,
                "rejected_records_count": run_dto.rejected_records_count,
            }

        return EvidenceUploadResponse(
            evidence_id=evidence_id,
            case_id=case_id,
            original_filename=dest_path.name,
            source_type=resolved_source_type,
            mime_type=detected_mime,
            file_size_bytes=bytes_written,
            sha256_hash=sha256_hash,
            storage_uri=str(dest_path.as_posix()),
            status=record.status,
            metadata=merged_metadata,
            created_at=now_utc,
            active_extraction_run_id=active_run_id,
            extraction_summary=extraction_summary,
        )

    async def execute_extraction_run(
        self,
        record: EvidenceRecord,
        *,
        source_timezone: str = "UTC",
        clock_offset_seconds: float = 0.0,
        custom_params: Optional[dict[str, Any]] = None,
    ) -> ExtractionRunDTO:
        """Execute a versioned, non-destructive extraction run for an evidence item."""
        run_id = str(uuid.uuid4())
        started_at = datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")

        # Deterministic configuration hash
        config_dict = {
            "source_timezone": source_timezone,
            "clock_offset_seconds": clock_offset_seconds,
            "custom_params": custom_params or {},
        }
        config_hash = hashlib.sha256(
            json.dumps(config_dict, sort_keys=True).encode("utf-8")
        ).hexdigest()[:16]

        # 1. Pre-extraction cryptographic verification
        is_intact = verify_evidence_integrity(record.storage_path, record.sha256_hash)
        if not is_intact:
            completed_at = datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")
            run_record = ExtractionRunRecord(
                run_id=run_id,
                evidence_id=record.evidence_id,
                source_sha256=record.sha256_hash,
                extractor_name="integrity_guard",
                extractor_version="1.0.0",
                config_hash=config_hash,
                status=ExtractionRunStatus.FAILED,
                started_at=started_at,
                completed_at=completed_at,
                is_active=False,
                error_message="Cryptographic integrity breach: storage file SHA-256 does not match ingestion hash.",
            )
            self.registry.save_run(run_record)
            record.status = IngestionStatus.CORRUPTED
            return self._run_to_dto(run_record)

        # 2. Dispatch to Member 1's extractors
        extractor_name = "unknown"
        extractor_version = "1.0.0"
        records_processed = 0
        valid_count = 0
        rejected_count = 0
        run_status = ExtractionRunStatus.SUCCESS
        error_msg = None
        result_dict = None

        try:
            if record.source_type == "CALL_DETAIL_RECORD":
                from app.modules.extraction.cdr import parse_cdr_csv

                extractor_name = "cdr_extractor"
                cdr_res = parse_cdr_csv(record.storage_path, record.evidence_id)
                records_processed = cdr_res.total_rows_processed
                valid_count = len(cdr_res.valid_records)
                rejected_count = len(cdr_res.rejected_records)
                if valid_count > 0 and rejected_count > 0:
                    run_status = ExtractionRunStatus.PARTIAL_SUCCESS
                elif valid_count > 0 and rejected_count == 0:
                    run_status = ExtractionRunStatus.SUCCESS
                else:
                    run_status = ExtractionRunStatus.FAILED
                    error_msg = f"Extraction parsed 0 valid records ({rejected_count} rejected issues)."

                result_dict = {
                    "total_identifiers": len(cdr_res.identifiers),
                    "total_events": len(cdr_res.event_candidates),
                    "total_relationships": len(cdr_res.relationship_candidates),
                }

            elif record.source_type == "CHAT_EXPORT":
                from app.modules.extraction.chat import parse_chat_json

                extractor_name = "chat_extractor"
                chat_res = parse_chat_json(record.storage_path, record.evidence_id)
                records_processed = chat_res.total_messages_processed
                valid_count = len(chat_res.valid_messages)
                rejected_count = len(chat_res.rejected_messages)
                if valid_count > 0 and rejected_count > 0:
                    run_status = ExtractionRunStatus.PARTIAL_SUCCESS
                elif valid_count > 0 and rejected_count == 0:
                    run_status = ExtractionRunStatus.SUCCESS
                else:
                    run_status = ExtractionRunStatus.FAILED
                    error_msg = f"Extraction parsed 0 valid messages ({rejected_count} rejected issues)."
                result_dict = {
                    "total_identifiers": len(chat_res.identifiers),
                    "total_events": len(chat_res.event_candidates),
                    "total_relationships": len(chat_res.relationship_candidates),
                }

            else:
                # Modality not yet implemented by Member 1 (e.g. video/pdf)
                extractor_name = "noop_extractor"
                run_status = ExtractionRunStatus.FAILED
                error_msg = f"Extraction for source_type '{record.source_type}' is not yet supported."

        except Exception as exc:
            run_status = ExtractionRunStatus.FAILED
            error_msg = f"Extraction logic raised unhandled exception: {str(exc)}"

        completed_at = datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")

        # 3. Create run record
        is_successful = run_status in {
            ExtractionRunStatus.SUCCESS,
            ExtractionRunStatus.PARTIAL_SUCCESS,
        }

        run_record = ExtractionRunRecord(
            run_id=run_id,
            evidence_id=record.evidence_id,
            source_sha256=record.sha256_hash,
            extractor_name=extractor_name,
            extractor_version=extractor_version,
            config_hash=config_hash,
            status=run_status,
            started_at=started_at,
            completed_at=completed_at,
            records_processed=records_processed,
            valid_count=valid_count,
            rejected_count=rejected_count,
            is_active=is_successful,
            error_message=error_msg,
            result_payload=result_dict,
        )
        self.registry.save_run(run_record)

        # 4. If run succeeded, supersede prior active runs and mark evidence EXTRACTED
        if is_successful:
            self.registry.supersede_prior_runs(record.evidence_id, current_active_run_id=run_id)
            record.active_run_id = run_id
            record.status = IngestionStatus.EXTRACTED
        elif not record.active_run_id:
            # If no prior active run existed and this one failed
            record.status = IngestionStatus.EXTRACTION_FAILED

        return self._run_to_dto(run_record)

    def get_evidence(self, evidence_id: str) -> Optional[EvidenceRecord]:
        return self.registry.get_evidence(evidence_id)

    def get_runs_for_evidence(self, evidence_id: str) -> list[ExtractionRunDTO]:
        records = self.registry.get_runs_for_evidence(evidence_id)
        return [self._run_to_dto(r) for r in records]

    @staticmethod
    def _run_to_dto(r: ExtractionRunRecord) -> ExtractionRunDTO:
        return ExtractionRunDTO(
            extraction_run_id=r.run_id,
            evidence_id=r.evidence_id,
            source_sha256=r.source_sha256,
            extractor_name=r.extractor_name,
            extractor_version=r.extractor_version,
            configuration_hash=r.config_hash,
            status=r.status,
            started_at=r.started_at,
            completed_at=r.completed_at,
            records_processed=r.records_processed,
            valid_records_count=r.valid_count,
            rejected_records_count=r.rejected_count,
            is_active=r.is_active,
            error_message=r.error_message,
        )
