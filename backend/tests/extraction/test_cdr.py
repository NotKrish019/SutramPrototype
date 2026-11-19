"""Unit tests for Call Detail Record (CDR) extraction engine."""

import hashlib
from pathlib import Path
import pytest

from app.modules.extraction.cdr import parse_cdr_csv

SAMPLE_CDR_PATH = Path(__file__).resolve().parents[3] / "data" / "synthetic" / "sample_cdr.csv"


def test_valid_synthetic_cdr_file_parsing():
    """Verify parsing of valid synthetic CDR CSV file."""
    assert SAMPLE_CDR_PATH.is_file(), f"Synthetic fixture missing: {SAMPLE_CDR_PATH}"

    result = parse_cdr_csv(
        SAMPLE_CDR_PATH,
        source_evidence_id="ev-cdr-001",
        default_country_prefix="91",
    )

    assert result.source_evidence_id == "ev-cdr-001"
    assert result.total_rows_processed == 5
    assert len(result.valid_records) == 5
    assert len(result.rejected_records) == 0

    first = result.valid_records[0]
    assert first.record_id == "cdr-rec-001"
    assert first.caller_phone_raw == "+919811002233"
    assert first.receiver_phone_raw == "+919899887766"
    assert first.call_timestamp_raw == "2026-11-10T21:45:12+05:30"
    assert first.duration_seconds == 184.0
    assert first.cell_tower_id == "TOWER-DELHI-SOUTH-04B"
    assert first.call_type == "VOICE_OUTGOING"
    assert first.row_index == 2

    # Verify event candidates
    assert len(result.event_candidates) == 5
    evt = result.event_candidates[0]
    assert evt.candidate_id == "evt-cdr-ev-cdr-001-cdr-rec-001"
    assert evt.event_type == "TELEPHONY_CALL"
    assert evt.raw_timestamp == "2026-11-10T21:45:12+05:30"
    assert evt.duration_seconds == 184.0
    assert evt.location_name == "TOWER-DELHI-SOUTH-04B"
    assert evt.involved_entity_raw_ids == ["+919811002233", "+919899887766"]

    # Verify relationships
    assert len(result.relationship_candidates) == 5
    rel = result.relationship_candidates[0]
    assert rel.relation_type == "CALL_COMMUNICATION"
    assert rel.is_observed is True
    assert rel.source_raw_id == "+919811002233"
    assert rel.target_raw_id == "+919899887766"
    assert rel.source_normalized_id == "+919811002233"
    assert rel.target_normalized_id == "+919899887766"
    assert rel.evidence_basis == ["ev-cdr-001"]


def test_missing_required_headers():
    """Verify rejection when mandatory headers are missing."""
    csv_text = "id,source,dest\n1,+919811002233,+919899887766\n"
    result = parse_cdr_csv(csv_text, source_evidence_id="ev-test-01")

    assert len(result.valid_records) == 0
    assert len(result.rejected_records) == 1
    issue = result.rejected_records[0]
    assert issue.issue_type == "MISSING_REQUIRED_HEADERS"
    assert "caller_phone" in issue.message or "duration_seconds" in issue.message


def test_empty_csv_file():
    """Verify handling of completely empty CSV content."""
    result = parse_cdr_csv("", source_evidence_id="ev-test-empty")
    assert result.total_rows_processed == 0
    assert len(result.rejected_records) == 1
    assert result.rejected_records[0].issue_type == "EMPTY_FILE"


def test_row_level_validation_and_rejected_records():
    """Verify non-destructive row-level rejection with explicit issues."""
    csv_content = (
        "record_id,caller_phone,receiver_phone,call_timestamp,duration_seconds,cell_tower_id\n"
        "rec-01,+919811002233,+919899887766,2026-11-10T12:00:00Z,60,TOWER-1\n"
        ",+919811002233,+919899887766,2026-11-10T12:05:00Z,30,TOWER-1\n"  # missing record_id
        "rec-03,,+919899887766,2026-11-10T12:10:00Z,45,TOWER-1\n"  # missing caller
        "rec-04,+919811002233,+919899887766,2026-11-10T12:15:00Z,-15,TOWER-1\n"  # negative duration
        "rec-05,+919811002233,+919899887766,,120,TOWER-1\n"  # missing timestamp
        "rec-06,+919811002233,+919899887766,2026-11-10T12:25:00Z,invalid_dur,TOWER-1\n"  # invalid duration
    )

    result = parse_cdr_csv(csv_content, source_evidence_id="ev-cdr-err")
    assert result.total_rows_processed == 6
    assert len(result.valid_records) == 1
    assert result.valid_records[0].record_id == "rec-01"

    assert len(result.rejected_records) == 5
    field_names = [iss.field_name for iss in result.rejected_records]
    assert "record_id" in field_names
    assert "caller_phone" in field_names
    assert "duration_seconds" in field_names
    assert "call_timestamp" in field_names


def test_evidence_file_immutability():
    """Verify that parsing a file never mutates its content or SHA-256 hash."""
    sha_before = hashlib.sha256(SAMPLE_CDR_PATH.read_bytes()).hexdigest()
    _ = parse_cdr_csv(SAMPLE_CDR_PATH, source_evidence_id="ev-cdr-001")
    sha_after = hashlib.sha256(SAMPLE_CDR_PATH.read_bytes()).hexdigest()
    assert sha_before == sha_after, "Forensic violation: source evidence was mutated!"
