"""Unit tests for the Unified Evidence Extraction Dispatcher (Member 1).

Covers all 12 mandatory Phase 1E test scenarios:
1. Valid CDR dispatch.
2. Valid chat JSON dispatch.
3. Unsupported source type rejection.
4. Generic CSV rejected as invalid CDR.
5. Missing evidence file handling.
6. SHA-256 mismatch rejection.
7. Malformed CDR handling.
8. Malformed JSON handling.
9. Partial extraction with structured validation issues.
10. Parser exception handling and process crash prevention.
11. Source evidence and record provenance preservation.
12. Raw evidence file immutability verification.
"""

from __future__ import annotations

import hashlib
from pathlib import Path

import pytest

from app.modules.extraction.service import compute_sha256, extract_evidence
from app.schemas.extraction import EvidenceExtractionRequest, ExtractionStatus

REPO_ROOT = Path(__file__).resolve().parents[3]
SYNTHETIC_DIR = REPO_ROOT / "data" / "synthetic"

SAMPLE_CDR_PATH = SYNTHETIC_DIR / "sample_cdr.csv"
SAMPLE_CDR_HASH = "c874a7c8b0e05da52db5f29d5bb0f398439237ad69a757cb05bfe9ec76803a8d"

SAMPLE_CHAT_PATH = SYNTHETIC_DIR / "sample_chat_export.json"
SAMPLE_CHAT_HASH = "78eaf79b73ec12a15b15f3d26386c3f2456020c914d7c496aa9efeca20fec40e"


def _sha256_of_str(content: str) -> str:
    return hashlib.sha256(content.encode("utf-8")).hexdigest()


# 1. Valid CDR dispatch
def test_valid_cdr_dispatch():
    """Verify successful dispatch and end-to-end extraction from a valid CDR CSV file."""
    assert SAMPLE_CDR_PATH.exists(), f"Synthetic fixture missing: {SAMPLE_CDR_PATH}"

    req = EvidenceExtractionRequest(
        evidence_id="ev-cdr-001",
        case_id="case-delhi-2026-001",
        source_type="CALL_DETAIL_RECORD",
        file_path=SAMPLE_CDR_PATH,
        expected_sha256=SAMPLE_CDR_HASH,
        default_country_prefix="91",
        metadata={"carrier": "AIRTEL_SYNTHETIC"},
    )

    result = extract_evidence(req)

    assert result.status == ExtractionStatus.SUCCESS
    assert result.evidence_id == "ev-cdr-001"
    assert result.case_id == "case-delhi-2026-001"
    assert result.valid_records_count == 5
    assert result.rejected_records_count == 0
    assert result.total_records_processed == 5

    # Check extracted components
    assert len(result.identifiers) > 0
    assert len(result.event_candidates) == 5
    assert len(result.relationship_candidates) == 5
    assert len(result.resolved_entities) > 0
    assert len(result.validation_issues) == 0

    # Check metrics and metadata
    assert result.processing_time_ms is not None
    assert result.processing_time_ms >= 0.0
    assert result.extraction_metadata["parser_name"] == "cdr_parser"
    assert result.extraction_metadata["verified_sha256"] == SAMPLE_CDR_HASH
    assert result.extraction_metadata["carrier"] == "AIRTEL_SYNTHETIC"


# 2. Valid chat JSON dispatch
def test_valid_chat_json_dispatch():
    """Verify successful dispatch and end-to-end extraction from a valid Chat JSON file."""
    assert SAMPLE_CHAT_PATH.exists(), f"Synthetic fixture missing: {SAMPLE_CHAT_PATH}"

    req = EvidenceExtractionRequest(
        evidence_id="ev-chat-001",
        case_id="case-delhi-2026-001",
        source_type="CHAT_EXPORT",
        file_path=SAMPLE_CHAT_PATH,
        expected_sha256=SAMPLE_CHAT_HASH,
        default_country_prefix="91",
        metadata={"platform": "WHATSAPP_EXPORT"},
    )

    result = extract_evidence(req)

    assert result.status == ExtractionStatus.SUCCESS
    assert result.evidence_id == "ev-chat-001"
    assert result.case_id == "case-delhi-2026-001"
    assert result.valid_records_count == 3
    assert result.rejected_records_count == 0
    assert result.total_records_processed == 3

    # Verify candidate generation
    assert len(result.identifiers) > 0
    assert len(result.event_candidates) == 3
    assert len(result.relationship_candidates) == 4
    assert len(result.resolved_entities) > 0
    assert len(result.validation_issues) == 0

    assert result.extraction_metadata["parser_name"] == "chat_parser"
    assert result.extraction_metadata["verified_sha256"] == SAMPLE_CHAT_HASH


# 3. Unsupported source type
def test_unsupported_source_type_rejected(tmp_path):
    """Verify rejection of unhandled evidence source types with structured validation issues."""
    dummy_file = tmp_path / "document.pdf"
    content = b"%PDF-1.4 dummy content"
    dummy_file.write_bytes(content)
    digest = hashlib.sha256(content).hexdigest()

    req = EvidenceExtractionRequest(
        evidence_id="ev-doc-999",
        case_id="case-001",
        source_type="DOCUMENT_PDF",
        file_path=dummy_file,
        expected_sha256=digest,
    )

    result = extract_evidence(req)

    assert result.status == ExtractionStatus.FAILED
    assert result.valid_records_count == 0
    assert result.rejected_records_count == 1
    assert len(result.validation_issues) == 1
    assert result.validation_issues[0].issue_type == "UNSUPPORTED_SOURCE_TYPE"
    assert "DOCUMENT_PDF" in result.validation_issues[0].message


# 4. Generic CSV rejected as invalid CDR
def test_generic_csv_rejected_as_invalid_cdr(tmp_path):
    """Verify defense against misclassifying generic CSVs lacking required CDR columns."""
    generic_csv = tmp_path / "generic_table.csv"
    text = "student_id,name,score\n1,Aman,95\n2,Priya,98\n"
    generic_csv.write_text(text, encoding="utf-8")
    digest = compute_sha256(generic_csv)

    req = EvidenceExtractionRequest(
        evidence_id="ev-csv-001",
        case_id="case-001",
        source_type="CALL_DETAIL_RECORD",
        file_path=generic_csv,
        expected_sha256=digest,
    )

    result = extract_evidence(req)

    assert result.status == ExtractionStatus.FAILED
    assert result.valid_records_count == 0
    assert result.rejected_records_count > 0
    assert any(
        issue.issue_type == "MISSING_REQUIRED_HEADERS"
        for issue in result.validation_issues
    )
    assert len(result.identifiers) == 0
    assert len(result.resolved_entities) == 0


# 5. Missing evidence file
def test_missing_evidence_file_handling(tmp_path):
    """Verify structured failure handling when the target evidence file does not exist."""
    missing_file = tmp_path / "non_existent_file.csv"

    req = EvidenceExtractionRequest(
        evidence_id="ev-missing-001",
        case_id="case-001",
        source_type="CALL_DETAIL_RECORD",
        file_path=missing_file,
        expected_sha256="a" * 64,
    )

    result = extract_evidence(req)

    assert result.status == ExtractionStatus.FAILED
    assert result.valid_records_count == 0
    assert len(result.validation_issues) == 1
    assert result.validation_issues[0].issue_type == "FILE_NOT_FOUND"


# 6. SHA-256 mismatch
def test_sha256_mismatch_rejected():
    """Verify strict cryptographic rejection when expected and computed SHA-256 differ."""
    tampered_digest = "0" * 64

    req = EvidenceExtractionRequest(
        evidence_id="ev-cdr-tampered",
        case_id="case-001",
        source_type="CALL_DETAIL_RECORD",
        file_path=SAMPLE_CDR_PATH,
        expected_sha256=tampered_digest,
    )

    result = extract_evidence(req)

    assert result.status == ExtractionStatus.FAILED
    assert result.valid_records_count == 0
    assert len(result.validation_issues) == 1
    assert result.validation_issues[0].issue_type == "HASH_MISMATCH"
    assert result.validation_issues[0].raw_value == SAMPLE_CDR_HASH


# 7. Malformed CDR
def test_malformed_cdr_fails_extraction(tmp_path):
    """Verify that a CDR with required headers but all malformed data rows evaluates to FAILED."""
    bad_cdr = tmp_path / "all_bad.csv"
    text = (
        "record_id,caller_phone,receiver_phone,call_timestamp,duration_seconds\n"
        "rec-01,,+919811002233,2026-03-01 10:00:00,60\n"  # missing caller phone
        "rec-02,+919811002233,+919899887766,,60\n"  # missing call timestamp
        "rec-03,+919811002233,+919899887766,2026-03-01 10:00:00,-50\n"  # negative duration
    )
    bad_cdr.write_text(text, encoding="utf-8")
    digest = compute_sha256(bad_cdr)

    req = EvidenceExtractionRequest(
        evidence_id="ev-bad-cdr",
        case_id="case-001",
        source_type="CALL_DETAIL_RECORD",
        file_path=bad_cdr,
        expected_sha256=digest,
    )

    result = extract_evidence(req)

    assert result.status == ExtractionStatus.FAILED
    assert result.valid_records_count == 0
    assert result.rejected_records_count == 3
    assert len(result.validation_issues) == 3


# 8. Malformed JSON
def test_malformed_json_fails_extraction(tmp_path):
    """Verify that malformed/corrupted JSON export files are trapped and reported as FAILED."""
    corrupt_json = tmp_path / "corrupt.json"
    text = "{\"chat_id\": \"ch-01\", \"messages\": [{\"id\": 1, \"sender\": "
    corrupt_json.write_text(text, encoding="utf-8")
    digest = compute_sha256(corrupt_json)

    req = EvidenceExtractionRequest(
        evidence_id="ev-corrupt-json",
        case_id="case-001",
        source_type="CHAT_EXPORT",
        file_path=corrupt_json,
        expected_sha256=digest,
    )

    result = extract_evidence(req)

    assert result.status == ExtractionStatus.FAILED
    assert result.valid_records_count == 0
    assert result.rejected_records_count >= 1
    assert any("JSON" in issue.issue_type for issue in result.validation_issues)


# 9. Partial extraction with validation issues
def test_partial_extraction_with_validation_issues(tmp_path):
    """Verify that a file containing both valid and invalid records produces PARTIAL_SUCCESS."""
    mixed_cdr = tmp_path / "mixed.csv"
    text = (
        "record_id,caller_phone,receiver_phone,call_timestamp,duration_seconds\n"
        "cdr-01,+919811002233,+919899887766,2026-03-01 10:00:00,120\n"
        "cdr-02,,+919899887766,2026-03-01 10:05:00,45\n"  # missing caller phone
        "cdr-03,+919811002233,+919899887766,2026-03-01 10:15:00,60\n"
    )
    mixed_cdr.write_text(text, encoding="utf-8")
    digest = compute_sha256(mixed_cdr)

    req = EvidenceExtractionRequest(
        evidence_id="ev-mixed-cdr",
        case_id="case-001",
        source_type="CALL_DETAIL_RECORD",
        file_path=mixed_cdr,
        expected_sha256=digest,
        default_country_prefix="91",
    )

    result = extract_evidence(req)

    assert result.status == ExtractionStatus.PARTIAL_SUCCESS
    assert result.valid_records_count == 2
    assert result.rejected_records_count == 1
    assert len(result.validation_issues) == 1
    assert result.validation_issues[0].record_id == "cdr-02"

    # Valid records must still yield candidates
    assert len(result.event_candidates) == 2
    assert len(result.relationship_candidates) == 2
    assert len(result.resolved_entities) > 0


# 10. Parser exception handling
def test_parser_exception_handling_graceful(monkeypatch):
    """Verify that unexpected parser exceptions do not crash the process and yield FAILED."""
    def crash_parser(*args, **kwargs):
        raise RuntimeError("Simulated unexpected internal crash during parsing")

    monkeypatch.setattr("app.modules.extraction.service.parse_cdr_csv", crash_parser)

    req = EvidenceExtractionRequest(
        evidence_id="ev-crash-test",
        case_id="case-001",
        source_type="CALL_DETAIL_RECORD",
        file_path=SAMPLE_CDR_PATH,
        expected_sha256=SAMPLE_CDR_HASH,
    )

    result = extract_evidence(req)

    assert result.status == ExtractionStatus.FAILED
    assert len(result.validation_issues) == 1
    assert result.validation_issues[0].issue_type == "PARSER_EXCEPTION"
    assert "Simulated unexpected internal crash" in result.validation_issues[0].message
    assert result.extraction_metadata["error"] == "PARSER_EXCEPTION"


# 11. Source evidence and record provenance preservation
def test_provenance_preservation_across_candidates():
    """Verify that evidence ID and source record references are strictly preserved."""
    req = EvidenceExtractionRequest(
        evidence_id="ev-prov-100",
        case_id="case-delhi-2026-001",
        source_type="CALL_DETAIL_RECORD",
        file_path=SAMPLE_CDR_PATH,
        expected_sha256=SAMPLE_CDR_HASH,
        default_country_prefix="91",
    )

    result = extract_evidence(req)

    assert result.status == ExtractionStatus.SUCCESS
    assert result.evidence_id == "ev-prov-100"

    # Every identifier must carry source_evidence_id matching the request
    for ident in result.identifiers:
        assert ident.source_evidence_id == "ev-prov-100"
        assert ident.source_record_id.startswith("cdr-")

    # Every event candidate must carry source_evidence_id
    for ev in result.event_candidates:
        assert ev.source_evidence_id == "ev-prov-100"
        assert ev.source_record_id.startswith("cdr-")

    # Every relationship candidate must cite the evidence basis
    for rel in result.relationship_candidates:
        assert rel.source_evidence_id == "ev-prov-100"
        assert "ev-prov-100" in rel.evidence_basis

    # Every resolved entity candidate must list source_evidence_ids
    for ent in result.resolved_entities:
        assert "ev-prov-100" in ent.source_evidence_ids


# 12. Raw evidence remains unchanged after extraction
def test_raw_evidence_remains_unchanged(tmp_path):
    """Verify forensic immutability: evidence file is not modified in place during extraction."""
    test_file = tmp_path / "immutable_test.csv"
    original_bytes = SAMPLE_CDR_PATH.read_bytes()
    test_file.write_bytes(original_bytes)

    hash_before = hashlib.sha256(test_file.read_bytes()).hexdigest()
    mtime_before = test_file.stat().st_mtime_ns

    req = EvidenceExtractionRequest(
        evidence_id="ev-immut-001",
        case_id="case-001",
        source_type="CALL_DETAIL_RECORD",
        file_path=test_file,
        expected_sha256=hash_before,
        default_country_prefix="91",
    )

    result = extract_evidence(req)
    assert result.status == ExtractionStatus.SUCCESS

    hash_after = hashlib.sha256(test_file.read_bytes()).hexdigest()
    mtime_after = test_file.stat().st_mtime_ns

    assert hash_before == hash_after
    assert mtime_before == mtime_after
