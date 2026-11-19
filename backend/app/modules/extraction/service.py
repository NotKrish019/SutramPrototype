"""Unified Evidence Extraction Dispatcher Service (Member 1).

Serves as the canonical public entrypoint for evidence extraction across supported
modalities (Call Detail Records, Chat/Instant Messaging Exports).

Performs:
1. File existence validation.
2. Pre-extraction cryptographic SHA-256 integrity verification.
3. Modality dispatch to specialized parsers (parse_cdr_csv, parse_chat_json).
4. Defense against premature CSV classification (validates required CDR headers).
5. Cross-source identifier resolution and entity candidate deduplication.
6. Unified result assembly with execution metrics, structured validation issues,
   and deterministic status computation (SUCCESS, PARTIAL_SUCCESS, FAILED).
7. Strict preservation of forensic provenance and raw evidence immutability.
"""

from __future__ import annotations

import hashlib
import time
from pathlib import Path
from typing import Any

from app.modules.correlation.resolution import deduplicate_identifiers
from app.modules.extraction.cdr import parse_cdr_csv
from app.modules.extraction.chat import parse_chat_json
from app.schemas.extraction import (
    EvidenceExtractionRequest,
    ExtractionStatus,
    ExtractionValidationIssue,
    UnifiedExtractionResult,
)

SUPPORTED_SOURCE_TYPES: dict[str, str] = {
    "CALL_DETAIL_RECORD": "CALL_DETAIL_RECORD",
    "CDR": "CALL_DETAIL_RECORD",
    "CHAT_EXPORT": "CHAT_EXPORT",
    "CHAT": "CHAT_EXPORT",
}


def compute_sha256(file_path: Path) -> str:
    """Compute the SHA-256 cryptographic digest of a file in streaming chunks."""
    hasher = hashlib.sha256()
    with open(file_path, "rb") as f:
        while chunk := f.read(65536):
            hasher.update(chunk)
    return hasher.hexdigest().lower()


def extract_evidence(request: EvidenceExtractionRequest) -> UnifiedExtractionResult:
    """Canonical entrypoint to execute extraction across supported evidence modalities.

    Forensic invariants:
    - Never mutates the raw evidence artifact.
    - Verifies file existence and SHA-256 digest before initiating parser logic.
    - Rejects unsupported evidence modalities with structured diagnostic issues.
    - Protects against generic CSV misclassification as CDR.
    - Preserves uncorrected timestamps and original observed identifiers.
    - Decouples observed facts from inferred entity candidates.
    - Decouples mathematical checksum telemetry from probabilistic confidence.
    - Traps unexpected exceptions to prevent API crashes.
    """
    start_time = time.perf_counter()
    target_path = Path(request.file_path)

    # 1. File existence verification
    if not target_path.exists() or not target_path.is_file():
        processing_time_ms = round((time.perf_counter() - start_time) * 1000.0, 2)
        issue = ExtractionValidationIssue(
            field_name="file_path",
            issue_type="FILE_NOT_FOUND",
            raw_value=str(request.file_path),
            message=f"Evidence file not found on disk: {target_path}",
        )
        return UnifiedExtractionResult(
            evidence_id=request.evidence_id,
            case_id=request.case_id,
            source_type=request.source_type,
            status=ExtractionStatus.FAILED,
            validation_issues=[issue],
            total_records_processed=0,
            valid_records_count=0,
            rejected_records_count=1,
            processing_time_ms=processing_time_ms,
            extraction_metadata={"error": "FILE_NOT_FOUND"},
        )

    # 2. Cryptographic SHA-256 pre-extraction integrity verification
    try:
        computed_sha256 = compute_sha256(target_path)
    except Exception as exc:
        processing_time_ms = round((time.perf_counter() - start_time) * 1000.0, 2)
        issue = ExtractionValidationIssue(
            field_name="file_path",
            issue_type="FILE_READ_ERROR",
            raw_value=str(request.file_path),
            message=f"Failed to read file for cryptographic verification: {exc}",
        )
        return UnifiedExtractionResult(
            evidence_id=request.evidence_id,
            case_id=request.case_id,
            source_type=request.source_type,
            status=ExtractionStatus.FAILED,
            validation_issues=[issue],
            total_records_processed=0,
            valid_records_count=0,
            rejected_records_count=1,
            processing_time_ms=processing_time_ms,
            extraction_metadata={"error": "FILE_READ_ERROR"},
        )

    expected_sha256_clean = request.expected_sha256.strip().lower()
    if computed_sha256 != expected_sha256_clean:
        processing_time_ms = round((time.perf_counter() - start_time) * 1000.0, 2)
        issue = ExtractionValidationIssue(
            field_name="sha256_hash",
            issue_type="HASH_MISMATCH",
            raw_value=computed_sha256,
            message=(
                f"Cryptographic hash mismatch for evidence '{request.evidence_id}': "
                f"expected {expected_sha256_clean}, computed {computed_sha256}."
            ),
        )
        return UnifiedExtractionResult(
            evidence_id=request.evidence_id,
            case_id=request.case_id,
            source_type=request.source_type,
            status=ExtractionStatus.FAILED,
            validation_issues=[issue],
            total_records_processed=0,
            valid_records_count=0,
            rejected_records_count=1,
            processing_time_ms=processing_time_ms,
            extraction_metadata={
                "error": "HASH_MISMATCH",
                "computed_sha256": computed_sha256,
                "expected_sha256": expected_sha256_clean,
            },
        )

    # 3. Source-type dispatch & validation
    normalized_source_type = request.source_type.strip().upper()
    canonical_modality = SUPPORTED_SOURCE_TYPES.get(normalized_source_type)

    if not canonical_modality:
        processing_time_ms = round((time.perf_counter() - start_time) * 1000.0, 2)
        issue = ExtractionValidationIssue(
            field_name="source_type",
            issue_type="UNSUPPORTED_SOURCE_TYPE",
            raw_value=request.source_type,
            message=(
                f"Unsupported evidence source type: '{request.source_type}'. "
                f"Supported modalities in Phase 1E: CALL_DETAIL_RECORD, CHAT_EXPORT."
            ),
        )
        return UnifiedExtractionResult(
            evidence_id=request.evidence_id,
            case_id=request.case_id,
            source_type=request.source_type,
            status=ExtractionStatus.FAILED,
            validation_issues=[issue],
            total_records_processed=0,
            valid_records_count=0,
            rejected_records_count=1,
            processing_time_ms=processing_time_ms,
            extraction_metadata={"error": "UNSUPPORTED_SOURCE_TYPE"},
        )

    # 4. Modality parser execution with exception safety
    try:
        if canonical_modality == "CALL_DETAIL_RECORD":
            parser_name = "cdr_parser"
            parser_version = "1.0.0"
            cdr_result = parse_cdr_csv(
                target_path,
                source_evidence_id=request.evidence_id,
                default_country_prefix=request.default_country_prefix,
            )
            raw_identifiers = cdr_result.identifiers
            event_candidates = cdr_result.event_candidates
            relationship_candidates = cdr_result.relationship_candidates
            validation_issues = cdr_result.rejected_records
            valid_records_count = len(cdr_result.valid_records)
            rejected_records_count = len(cdr_result.rejected_records)
            total_records_processed = cdr_result.total_rows_processed

        elif canonical_modality == "CHAT_EXPORT":
            parser_name = "chat_parser"
            parser_version = "1.0.0"
            chat_result = parse_chat_json(
                target_path,
                source_evidence_id=request.evidence_id,
                default_country_prefix=request.default_country_prefix,
            )
            raw_identifiers = chat_result.identifiers
            event_candidates = chat_result.event_candidates
            relationship_candidates = chat_result.relationship_candidates
            validation_issues = chat_result.rejected_messages
            valid_records_count = len(chat_result.valid_messages)
            rejected_records_count = len(chat_result.rejected_messages)
            total_records_processed = chat_result.total_messages_processed

        else:
            raise ValueError(f"Unhandled canonical modality: {canonical_modality}")

    except Exception as exc:
        processing_time_ms = round((time.perf_counter() - start_time) * 1000.0, 2)
        issue = ExtractionValidationIssue(
            field_name="extraction",
            issue_type="PARSER_EXCEPTION",
            raw_value=None,
            message=f"Parser execution failed with unhandled exception: {type(exc).__name__}: {str(exc)}",
        )
        return UnifiedExtractionResult(
            evidence_id=request.evidence_id,
            case_id=request.case_id,
            source_type=request.source_type,
            status=ExtractionStatus.FAILED,
            validation_issues=[issue],
            total_records_processed=0,
            valid_records_count=0,
            rejected_records_count=1,
            processing_time_ms=processing_time_ms,
            extraction_metadata={
                "error": "PARSER_EXCEPTION",
                "exception_type": type(exc).__name__,
                "exception_message": str(exc),
            },
        )

    # 5. Entity resolution & deduplication across extracted identifiers
    resolved_entities = deduplicate_identifiers(
        raw_identifiers,
        default_country_prefix=request.default_country_prefix,
    )

    # 6. Status determination
    if valid_records_count > 0 and rejected_records_count == 0:
        status = ExtractionStatus.SUCCESS
    elif valid_records_count > 0 and rejected_records_count > 0:
        status = ExtractionStatus.PARTIAL_SUCCESS
    else:
        status = ExtractionStatus.FAILED

    processing_time_ms = round((time.perf_counter() - start_time) * 1000.0, 2)

    extraction_meta: dict[str, Any] = {
        "parser_name": parser_name,
        "parser_version": parser_version,
        "source_timezone": request.source_timezone,
        "clock_offset_seconds": request.clock_offset_seconds,
        "default_country_prefix": request.default_country_prefix,
        "verified_sha256": computed_sha256,
        **request.metadata,
    }

    return UnifiedExtractionResult(
        evidence_id=request.evidence_id,
        case_id=request.case_id,
        source_type=request.source_type,
        status=status,
        identifiers=raw_identifiers,
        resolved_entities=resolved_entities,
        event_candidates=event_candidates,
        relationship_candidates=relationship_candidates,
        validation_issues=validation_issues,
        total_records_processed=total_records_processed,
        valid_records_count=valid_records_count,
        rejected_records_count=rejected_records_count,
        processing_time_ms=processing_time_ms,
        extraction_metadata=extraction_meta,
    )
