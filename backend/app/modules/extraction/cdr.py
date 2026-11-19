"""Call Detail Record (CDR) Extraction Engine (Member 1).

Parses telephony Call Detail Record CSV files deterministically, performs strict
row and header validation, preserves original values without mutation, extracts
caller/receiver identifiers, and synthesizes event and relationship candidates.
"""

from __future__ import annotations

import csv
import io
from pathlib import Path
from typing import Optional, Union

from app.modules.correlation.relationships import generate_cdr_relationship_candidate
from app.modules.correlation.resolution import normalize_phone_number
from app.schemas.extraction import (
    CDRExtractionResult,
    ExtractedEventCandidate,
    ExtractedIdentifier,
    ExtractionValidationIssue,
    ParsedCDRRecord,
)

REQUIRED_CDR_HEADERS = {
    "record_id",
    "caller_phone",
    "receiver_phone",
    "call_timestamp",
    "duration_seconds",
}


def parse_cdr_csv(
    csv_source: Union[str, Path, io.StringIO, io.TextIOBase],
    source_evidence_id: str,
    *,
    default_country_prefix: Optional[str] = None,
) -> CDRExtractionResult:
    """Deterministically parse a Call Detail Record (CDR) CSV into structured candidates and issues.

    Forensic guarantees:
    - Never modifies the source file.
    - Rejects invalid rows with explicit structured diagnostic issues instead of dropping them.
    - Preserves uncorrected timestamps and raw phone strings verbatim.
    - Synthesizes observed call communication relationships and telephony event candidates.
    """
    raw_text: str
    if isinstance(csv_source, Path):
        with open(csv_source, "r", encoding="utf-8-sig", errors="replace") as f:
            raw_text = f.read()
    elif isinstance(csv_source, io.TextIOBase):
        raw_text = csv_source.read()
    elif isinstance(csv_source, str):
        # Could be file path or inline CSV content
        if "\n" not in csv_source and Path(csv_source).is_file():
            with open(csv_source, "r", encoding="utf-8-sig", errors="replace") as f:
                raw_text = f.read()
        else:
            raw_text = csv_source
    else:
        raise TypeError(f"Unsupported csv_source type: {type(csv_source).__name__}")

    reader = csv.reader(io.StringIO(raw_text))
    rows = list(reader)

    if not rows:
        return CDRExtractionResult(
            source_evidence_id=source_evidence_id,
            total_rows_processed=0,
            rejected_records=[
                ExtractionValidationIssue(
                    field_name="file",
                    issue_type="EMPTY_FILE",
                    message="The provided CDR CSV file is completely empty.",
                    row_index=0,
                )
            ],
        )

    # Validate header row
    raw_header = [h.strip().lower() for h in rows[0]]
    missing_headers = REQUIRED_CDR_HEADERS - set(raw_header)
    if missing_headers:
        return CDRExtractionResult(
            source_evidence_id=source_evidence_id,
            total_rows_processed=len(rows) - 1,
            rejected_records=[
                ExtractionValidationIssue(
                    field_name="headers",
                    issue_type="MISSING_REQUIRED_HEADERS",
                    raw_value=",".join(rows[0]),
                    message=f"CSV missing mandatory columns: {sorted(missing_headers)}. Found headers: {raw_header}",
                    row_index=1,
                )
            ],
        )

    header_indices = {col: idx for idx, col in enumerate(raw_header)}
    valid_records: list[ParsedCDRRecord] = []
    rejected_records: list[ExtractionValidationIssue] = []
    identifiers: list[ExtractedIdentifier] = []
    event_candidates: list[ExtractedEventCandidate] = []
    relationship_candidates = []

    for row_idx, row in enumerate(rows[1:], start=2):
        if not any(field.strip() for field in row):
            # Empty row
            continue

        # Check column length
        if len(row) < len(header_indices):
            rejected_records.append(
                ExtractionValidationIssue(
                    field_name="row",
                    issue_type="MALFORMED_ROW_COLUMN_MISMATCH",
                    raw_value=",".join(row),
                    message=f"Row {row_idx} has {len(row)} columns, expected at least {len(header_indices)}.",
                    row_index=row_idx,
                )
            )
            continue

        def get_val(col_name: str) -> str:
            idx = header_indices.get(col_name)
            return row[idx].strip() if idx is not None and idx < len(row) else ""

        rec_id = get_val("record_id")
        caller = get_val("caller_phone")
        callee = get_val("receiver_phone")
        timestamp_raw = get_val("call_timestamp")
        duration_raw = get_val("duration_seconds")
        cell_tower = get_val("cell_tower_id") or None
        call_type = get_val("call_type") or "VOICE_CALL"

        # Validate mandatory fields
        has_error = False
        if not rec_id:
            rejected_records.append(
                ExtractionValidationIssue(
                    record_id=None,
                    field_name="record_id",
                    issue_type="MISSING_REQUIRED_FIELD",
                    raw_value="",
                    message=f"Row {row_idx} lacks mandatory 'record_id'.",
                    row_index=row_idx,
                )
            )
            has_error = True

        if not caller:
            rejected_records.append(
                ExtractionValidationIssue(
                    record_id=rec_id or f"row-{row_idx}",
                    field_name="caller_phone",
                    issue_type="MISSING_REQUIRED_FIELD",
                    raw_value="",
                    message=f"Row {row_idx} lacks mandatory 'caller_phone'.",
                    row_index=row_idx,
                )
            )
            has_error = True

        if not callee:
            rejected_records.append(
                ExtractionValidationIssue(
                    record_id=rec_id or f"row-{row_idx}",
                    field_name="receiver_phone",
                    issue_type="MISSING_REQUIRED_FIELD",
                    raw_value="",
                    message=f"Row {row_idx} lacks mandatory 'receiver_phone'.",
                    row_index=row_idx,
                )
            )
            has_error = True

        if not timestamp_raw:
            rejected_records.append(
                ExtractionValidationIssue(
                    record_id=rec_id or f"row-{row_idx}",
                    field_name="call_timestamp",
                    issue_type="MISSING_REQUIRED_FIELD",
                    raw_value="",
                    message=f"Row {row_idx} lacks mandatory 'call_timestamp'.",
                    row_index=row_idx,
                )
            )
            has_error = True

        # Validate duration
        try:
            duration = float(duration_raw)
            if duration < 0:
                raise ValueError("Duration cannot be negative.")
        except (ValueError, TypeError) as exc:
            rejected_records.append(
                ExtractionValidationIssue(
                    record_id=rec_id or f"row-{row_idx}",
                    field_name="duration_seconds",
                    issue_type="INVALID_DURATION",
                    raw_value=duration_raw,
                    message=f"Row {row_idx} has invalid duration '{duration_raw}': {exc}.",
                    row_index=row_idx,
                )
            )
            has_error = True

        if has_error:
            continue

        # Valid Record
        parsed_rec = ParsedCDRRecord(
            source_evidence_id=source_evidence_id,
            record_id=rec_id,
            caller_phone_raw=caller,
            receiver_phone_raw=callee,
            call_timestamp_raw=timestamp_raw,
            duration_seconds=duration,
            cell_tower_id=cell_tower,
            call_type=call_type,
            row_index=row_idx,
        )
        valid_records.append(parsed_rec)

        # 1. Identifier Extraction for Caller & Callee
        caller_norm = normalize_phone_number(caller, default_country_prefix=default_country_prefix)
        identifiers.append(
            ExtractedIdentifier(
                identifier_type="PHONE_NUMBER",
                raw_value=caller,
                normalized_value=caller_norm.normalized_e164 if caller_norm.is_normalized else None,
                source_evidence_id=source_evidence_id,
                source_record_id=rec_id,
                location_reference="column:caller_phone",
                extraction_method="cdr_column",
            )
        )

        callee_norm = normalize_phone_number(callee, default_country_prefix=default_country_prefix)
        identifiers.append(
            ExtractedIdentifier(
                identifier_type="PHONE_NUMBER",
                raw_value=callee,
                normalized_value=callee_norm.normalized_e164 if callee_norm.is_normalized else None,
                source_evidence_id=source_evidence_id,
                source_record_id=rec_id,
                location_reference="column:receiver_phone",
                extraction_method="cdr_column",
            )
        )

        # 2. Event Candidate Creation
        event_candidates.append(
            ExtractedEventCandidate(
                candidate_id=f"evt-cdr-{source_evidence_id}-{rec_id}",
                source_evidence_id=source_evidence_id,
                source_record_id=rec_id,
                event_type="TELEPHONY_CALL",
                raw_timestamp=timestamp_raw,
                duration_seconds=duration,
                location_name=cell_tower,
                involved_entity_raw_ids=[caller, callee],
                metadata={
                    "call_type": call_type,
                    "cell_tower_id": cell_tower,
                    "row_index": row_idx,
                },
            )
        )

        # 3. Relationship Candidate Creation
        rel_cand = generate_cdr_relationship_candidate(
            parsed_rec, default_country_prefix=default_country_prefix
        )
        relationship_candidates.append(rel_cand)

    return CDRExtractionResult(
        source_evidence_id=source_evidence_id,
        total_rows_processed=len(rows) - 1,
        valid_records=valid_records,
        rejected_records=rejected_records,
        identifiers=identifiers,
        event_candidates=event_candidates,
        relationship_candidates=relationship_candidates,
    )
