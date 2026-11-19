"""Unit and integration tests for Evidence Ingestion & Idempotent Run Tracking (Member 2)."""

from io import BytesIO
import json
from pathlib import Path
import pytest
from fastapi.testclient import TestClient

from app.main import app
from app.core.config import settings
from app.api.routes.evidence import ingestion_service
from app.modules.ingestion.service import default_registry, EvidenceIngestionService
from app.modules.ingestion.storage import (
    get_safe_storage_path,
    sanitize_filename,
    verify_evidence_integrity,
)
from app.schemas.ingestion import ExtractionRunStatus, IngestionStatus


@pytest.fixture(autouse=True)
def clean_registry_and_tmp_dir(tmp_path, monkeypatch):
    """Set temporary storage directory and clean registry between tests."""
    test_storage = tmp_path / "evidence_storage"
    test_storage.mkdir(parents=True, exist_ok=True)
    monkeypatch.setattr(settings, "EVIDENCE_STORAGE_PATH", str(test_storage))
    monkeypatch.setattr(ingestion_service, "storage_base_dir", test_storage)
    default_registry.clear()
    yield test_storage
    default_registry.clear()


@pytest.fixture
def client():
    """FastAPI TestClient instance."""
    return TestClient(app)


SAMPLE_CDR_CSV = (
    "call_id,caller_phone,receiver_phone,call_timestamp,duration_seconds,cell_tower_id,call_type,cell_tower_lat,cell_tower_lon\n"
    "cdr-001,+919811002233,+919822003344,2026-11-10T21:45:00Z,184,TOW-DL-01,VOICE_CALL,28.6139,77.2090\n"
    "cdr-002,+919811002233,+919833004455,2026-11-10T21:50:00Z,42,TOW-DL-02,VOICE_CALL,28.6145,77.2105\n"
)

SAMPLE_CHAT_JSON = json.dumps({
    "chat_id": "chat-sample-01",
    "platform": "Signal",
    "export_timestamp": "2026-11-11T10:00:00Z",
    "messages": [
        {
            "message_id": "msg-001",
            "sender_id": "+919811002233",
            "sender_name": "Target",
            "recipients": ["+919822003344"],
            "timestamp": "2026-11-10T21:40:00Z",
            "content": "Meeting at the rendezvous. Handset IMEI: 352099001761481.",
        }
    ]
}, indent=2)


def test_valid_evidence_upload(client):
    """Verify standard evidence upload without immediate extraction."""
    files = {"file": ("call_logs.csv", BytesIO(SAMPLE_CDR_CSV.encode("utf-8")), "text/csv")}
    data = {
        "case_id": "case-alpha-001",
        "source_timezone": "Asia/Kolkata",
        "clock_offset_seconds": "-120.0",
        "metadata": json.dumps({"seizure_location": "Dwarka Sector 9", "device_model": "Vivo V29"}),
        "trigger_extraction": "false",
    }
    response = client.post("/api/v1/evidence/upload", files=files, data=data)
    assert response.status_code == 201

    payload = response.json()
    assert payload["case_id"] == "case-alpha-001"
    assert payload["original_filename"] == "call_logs.csv"
    assert payload["source_type"] == "CALL_DETAIL_RECORD"
    assert payload["status"] == "INGESTED"
    assert len(payload["sha256_hash"]) == 64
    assert payload["file_size_bytes"] == len(SAMPLE_CDR_CSV.encode("utf-8"))
    assert payload["metadata"]["source_timezone"] == "Asia/Kolkata"
    assert payload["metadata"]["clock_offset_seconds"] == -120.0

    # Verify physical file was written and integrity matches
    storage_path = Path(payload["storage_uri"])
    assert storage_path.is_file()
    assert verify_evidence_integrity(storage_path, payload["sha256_hash"]) is True


def test_valid_evidence_upload_with_extraction(client):
    """Verify evidence upload with immediate extraction invocation."""
    files = {"file": ("export.json", BytesIO(SAMPLE_CHAT_JSON.encode("utf-8")), "application/json")}
    data = {
        "case_id": "case-beta-002",
        "trigger_extraction": "true",
    }
    response = client.post("/api/v1/evidence/upload", files=files, data=data)
    assert response.status_code == 201

    payload = response.json()
    assert payload["status"] == "EXTRACTED"
    assert payload["active_extraction_run_id"] is not None
    assert payload["extraction_summary"]["status"] == "SUCCESS"
    assert payload["extraction_summary"]["records_processed"] == 1
    assert payload["extraction_summary"]["valid_records_count"] == 1


def test_unsupported_file_type_rejected(client):
    """Verify that unknown/unsupported binary payloads are rejected by content validation."""
    garbage_bytes = b"\x00\x01\x02\x03\x04\x05\x06\x07\x08\x09\xaa\xbb\xcc\xdd\xee\xff"
    files = {"file": ("malicious.bin", BytesIO(garbage_bytes), "application/octet-stream")}
    data = {"case_id": "case-gamma-003"}

    response = client.post("/api/v1/evidence/upload", files=files, data=data)
    assert response.status_code == 422
    assert "Content validation rejected evidence" in response.json()["detail"]


def test_oversized_upload_rejected(client, monkeypatch):
    """Verify that uploads exceeding MAX_UPLOAD_SIZE_BYTES are halted and cleaned up."""
    # Temporarily set max size to 50 bytes
    monkeypatch.setattr(settings, "MAX_UPLOAD_SIZE_BYTES", 50)

    oversized_data = b"A" * 128
    files = {"file": ("huge_file.csv", BytesIO(oversized_data), "text/csv")}
    data = {"case_id": "case-oversize-004"}

    response = client.post("/api/v1/evidence/upload", files=files, data=data)
    assert response.status_code == 413
    assert "maximum allowable evidence size" in response.json()["detail"]


def test_malformed_json_content_sniffing(client):
    """Verify that a file declared as JSON but containing malformed syntax is caught."""
    bad_json = b"{\ninvalid: json without quotes,\n"
    files = {"file": ("corrupted.json", BytesIO(bad_json), "application/json")}
    data = {"case_id": "case-malformed-005"}

    response = client.post("/api/v1/evidence/upload", files=files, data=data)
    assert response.status_code == 422
    assert "Malformed JSON content" in response.json()["detail"]


def test_safe_storage_paths_and_path_traversal(client, tmp_path):
    """Verify path traversal prevention across filename and case_id inputs."""
    # 1. Sanitize filename checks
    assert sanitize_filename("../../etc/passwd") == "passwd"
    assert sanitize_filename("..\\..\\windows\\system32\\cmd.exe") == "cmd.exe"
    assert sanitize_filename("CON.csv") == "safe_CON.csv"
    assert sanitize_filename("") == "unnamed_evidence.bin"

    # 2. get_safe_storage_path traversal checks
    base_dir = tmp_path / "storage"
    base_dir.mkdir(parents=True, exist_ok=True)

    with pytest.raises(ValueError, match="Invalid case_id"):
        get_safe_storage_path(base_dir, case_id="../evil", evidence_id="ev-01", filename="test.csv")

    with pytest.raises(ValueError, match="Invalid evidence_id"):
        get_safe_storage_path(base_dir, case_id="case-01", evidence_id="../../ev", filename="test.csv")

    safe_dest = get_safe_storage_path(base_dir, case_id="case-01", evidence_id="ev-01", filename="../../report.csv")
    assert safe_dest.name == "report.csv"
    assert str(safe_dest).startswith(str(base_dir.resolve()))

    # 3. Via API upload
    files = {"file": ("../../trojan.csv", BytesIO(SAMPLE_CDR_CSV.encode("utf-8")), "text/csv")}
    data = {"case_id": "case-traversal-006"}
    res = client.post("/api/v1/evidence/upload", files=files, data=data)
    assert res.status_code == 201
    assert res.json()["original_filename"] == "trojan.csv"


def test_duplicate_uploads_within_same_case(client):
    """Verify that uploading the exact same evidence twice to the same case is prevented."""
    files1 = {"file": ("cdr_one.csv", BytesIO(SAMPLE_CDR_CSV.encode("utf-8")), "text/csv")}
    data = {"case_id": "case-dup-007"}
    res1 = client.post("/api/v1/evidence/upload", files=files1, data=data)
    assert res1.status_code == 201

    # Second upload of identical bytes to same case
    files2 = {"file": ("cdr_duplicate.csv", BytesIO(SAMPLE_CDR_CSV.encode("utf-8")), "text/csv")}
    res2 = client.post("/api/v1/evidence/upload", files=files2, data=data)
    assert res2.status_code == 409
    assert "Duplicate evidence detected" in res2.json()["detail"]

    # Uploading identical bytes to a DIFFERENT case is allowed (multi-case segregation)
    data_diff = {"case_id": "case-other-008"}
    files3 = {"file": ("cdr_other_case.csv", BytesIO(SAMPLE_CDR_CSV.encode("utf-8")), "text/csv")}
    res3 = client.post("/api/v1/evidence/upload", files=files3, data=data_diff)
    assert res3.status_code == 201


def test_storage_failure_handling(client, monkeypatch):
    """Verify clean 500 error handling when disk write or filesystem fails."""
    def broken_save(*args, **kwargs):
        raise IOError("Disk write I/O error: disk full (simulated)")

    monkeypatch.setattr("app.modules.ingestion.service.save_evidence_file", broken_save)

    files = {"file": ("records.csv", BytesIO(SAMPLE_CDR_CSV.encode("utf-8")), "text/csv")}
    data = {"case_id": "case-io-009"}

    response = client.post("/api/v1/evidence/upload", files=files, data=data)
    assert response.status_code == 500
    assert "Disk write I/O error" in response.json()["detail"]


def test_extraction_invocation_failure_handling(client):
    """Verify that an unparseable modality marks extraction FAILED but preserves the evidence."""
    # A CSV file with valid text, but missing CDR headers required by CDR extractor
    incomplete_csv = "just,random,headers\n1,2,3\n"
    files = {"file": ("non_cdr.csv", BytesIO(incomplete_csv.encode("utf-8")), "text/csv")}
    data = {
        "case_id": "case-fail-010",
        "source_type": "CALL_DETAIL_RECORD",
        "trigger_extraction": "true",
    }
    response = client.post("/api/v1/evidence/upload", files=files, data=data)
    assert response.status_code == 201

    payload = response.json()
    ev_id = payload["evidence_id"]
    # Evidence status records that extraction failed
    assert payload["status"] == "EXTRACTION_FAILED"

    # Query extraction runs
    runs_res = client.get(f"/api/v1/evidence/{ev_id}/runs")
    assert runs_res.status_code == 200
    runs = runs_res.json()
    assert len(runs) == 1
    assert runs[0]["status"] == "FAILED"
    assert runs[0]["is_active"] is False


def test_reprocessing_behavior_and_versioned_runs(client):
    """Verify non-destructive re-extraction run versioning and status superseding."""
    # 1. Initial upload with successful extraction
    files = {"file": ("chat.json", BytesIO(SAMPLE_CHAT_JSON.encode("utf-8")), "application/json")}
    data = {"case_id": "case-reprocess-011", "trigger_extraction": "true"}
    res1 = client.post("/api/v1/evidence/upload", files=files, data=data)
    assert res1.status_code == 201
    ev_id = res1.json()["evidence_id"]
    run1_id = res1.json()["active_extraction_run_id"]

    # 2. Re-process with different clock offset
    reprocess_res = client.post(
        f"/api/v1/evidence/{ev_id}/reprocess",
        data={"source_timezone": "UTC", "clock_offset_seconds": "+45.0"},
    )
    assert reprocess_res.status_code == 200
    run2_dto = reprocess_res.json()
    run2_id = run2_dto["extraction_run_id"]
    assert run2_id != run1_id
    assert run2_dto["status"] == "SUCCESS"
    assert run2_dto["is_active"] is True

    # 3. Audit full run history
    history_res = client.get(f"/api/v1/evidence/{ev_id}/runs")
    assert history_res.status_code == 200
    runs = history_res.json()
    assert len(runs) == 2

    run_map = {r["extraction_run_id"]: r for r in runs}
    # Run 1 is now superseded and inactive
    assert run_map[run1_id]["status"] == "SUPERSEDED"
    assert run_map[run1_id]["is_active"] is False

    # Run 2 is active
    assert run_map[run2_id]["status"] == "SUCCESS"
    assert run_map[run2_id]["is_active"] is True
