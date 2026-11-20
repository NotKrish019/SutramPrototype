"""Scenario C: evidence integrity (tampered, missing, remote URI, path traversal).

Each problem must produce a clear diagnostic and an exclusion, and the run must continue.
"""

from copy import deepcopy
from typing import Any

import pytest

from app.modules.reconstruction.pipeline import PipelineStatus, ReconstructionResult
from app.modules.reporting.forensic_report import IntegrityStatus, build_forensic_report
from tests.integration.reconstruction.support import (
    FIXED_NOW,
    SyntheticCase,
    evidence_path,
    host_variants,
    run_pipeline,
)


def _evidence_with(case: SyntheticCase, target_id: str, **changes: Any) -> list[dict[str, Any]]:
    evidence = deepcopy(case.evidence)
    next(e for e in evidence if e["evidence_id"] == target_id).update(changes)
    return evidence


def _assert_run_continued(result: ReconstructionResult, excluded_evidence: str, code: str) -> None:
    assert result.status == PipelineStatus.COMPLETED_WITH_ERRORS
    errors = [d for d in result.diagnostics if d.severity.value == "ERROR"]
    assert [(d.code, d.subject_id) for d in errors if d.subject_type == "EVIDENCE"] == [(code, excluded_evidence)]
    assert (("EVIDENCE", excluded_evidence, code)) in {
        (x.subject_type, x.subject_id, x.reason_code) for x in result.excluded_inputs
    }
    assert any(g.gap_type.value == "EVIDENCE_EXCLUDED" and g.subject_id == excluded_evidence for g in result.evidence_gaps)
    # The rest of the case was still reconstructed.
    assert len(result.timeline_events) >= 3
    assert all(e.source_evidence_id != excluded_evidence for e in result.timeline_events)


def test_tampered_file_sha256_mismatch(burglary_case: SyntheticCase) -> None:
    target = evidence_path(burglary_case.storage_root, "ev-cdr-tower")
    data = bytearray(target.read_bytes())
    data[0] ^= 0x01  # same size, one flipped bit
    target.write_bytes(bytes(data))
    result = run_pipeline(burglary_case)
    _assert_run_continued(result, "ev-cdr-tower", "EVIDENCE_INTEGRITY_MISMATCH")
    dependent = {x.subject_id: x.reason_code for x in result.excluded_inputs if x.subject_type == "EVENT_CANDIDATE"}
    assert dependent == {"evt-02-outbound-call": "SOURCE_EVIDENCE_EXCLUDED"}
    item = {i.evidence_id: i for i in build_forensic_report(result, generated_at=FIXED_NOW).evidence_inventory}
    assert item["ev-cdr-tower"].integrity_status == IntegrityStatus.SHA256_MISMATCH
    assert item["ev-cctv-dock"].integrity_status == IntegrityStatus.VERIFIED


def test_recorded_hash_mismatch(burglary_case: SyntheticCase) -> None:
    result = run_pipeline(burglary_case, evidence=_evidence_with(burglary_case, "ev-cdr-tower", sha256_hash="0" * 64))
    _assert_run_continued(result, "ev-cdr-tower", "EVIDENCE_INTEGRITY_MISMATCH")


def test_missing_file(burglary_case: SyntheticCase) -> None:
    evidence_path(burglary_case.storage_root, "ev-cdr-tower").unlink()
    result = run_pipeline(burglary_case)
    _assert_run_continued(result, "ev-cdr-tower", "EVIDENCE_STORAGE_UNAVAILABLE")
    item = {i.evidence_id: i for i in build_forensic_report(result, generated_at=FIXED_NOW).evidence_inventory}
    assert item["ev-cdr-tower"].integrity_status == IntegrityStatus.FILE_UNAVAILABLE


@pytest.mark.parametrize(
    "uri",
    ["https://storage.example.invalid/ev.raw", "s3://bucket/ev-cdr-tower.raw", "file://remote-host/share/ev.raw"],
)
def test_remote_uri_is_rejected_without_fetching(burglary_case: SyntheticCase, uri: str) -> None:
    result = run_pipeline(burglary_case, evidence=_evidence_with(burglary_case, "ev-cdr-tower", storage_uri=uri))
    _assert_run_continued(result, "ev-cdr-tower", "REMOTE_STORAGE_URI_REJECTED")
    item = {i.evidence_id: i for i in build_forensic_report(result, generated_at=FIXED_NOW).evidence_inventory}
    assert item["ev-cdr-tower"].integrity_status == IntegrityStatus.STORAGE_REFERENCE_REJECTED


@pytest.mark.parametrize(
    "uri",
    [
        "../../../etc/passwd",
        "data/evidence/raw/case-wh-burglary-001/ev-cdr-tower/../../../../../etc/passwd",
        "/etc/passwd",
        "file:///etc/passwd",
    ],
)
def test_path_traversal_storage_uri_is_rejected(burglary_case: SyntheticCase, uri: str) -> None:
    result = run_pipeline(burglary_case, evidence=_evidence_with(burglary_case, "ev-cdr-tower", storage_uri=uri))
    _assert_run_continued(result, "ev-cdr-tower", "CLIENT_STORAGE_PATH_REJECTED")


def test_path_traversal_evidence_id_is_rejected(burglary_case: SyntheticCase) -> None:
    result = run_pipeline(burglary_case, evidence=_evidence_with(burglary_case, "ev-cdr-tower", evidence_id="../../etc"))
    assert result.status == PipelineStatus.COMPLETED_WITH_ERRORS
    assert ("EVIDENCE", "../../etc", "INVALID_EVIDENCE_RECORD") in {
        (x.subject_type, x.subject_id, x.reason_code) for x in result.excluded_inputs
    }
    assert len(result.timeline_events) == 4


def test_symlinked_evidence_is_rejected(burglary_case: SyntheticCase) -> None:
    target = evidence_path(burglary_case.storage_root, "ev-cdr-tower")
    real = target.with_name("real.bin")
    target.rename(real)
    target.symlink_to(real)
    result = run_pipeline(burglary_case)
    assert result.status == PipelineStatus.COMPLETED_WITH_ERRORS
    assert "ev-cdr-tower" in {x.subject_id for x in result.excluded_inputs}


def test_integrity_failures_never_leak_host_paths(burglary_case: SyntheticCase) -> None:
    evidence_path(burglary_case.storage_root, "ev-cdr-tower").unlink()
    evidence = _evidence_with(burglary_case, "ev-chat-export", storage_uri="/etc/passwd")
    result = run_pipeline(burglary_case, evidence=evidence)
    text = result.to_snapshot_json() + build_forensic_report(result, generated_at=FIXED_NOW).to_json()
    for variant in host_variants(burglary_case.storage_root):
        assert variant not in text


def test_all_evidence_bad_still_returns_a_result(burglary_case: SyntheticCase) -> None:
    evidence = deepcopy(burglary_case.evidence)
    for item in evidence:
        item["sha256_hash"] = "f" * 64
    result = run_pipeline(burglary_case, evidence=evidence)
    assert result.status == PipelineStatus.COMPLETED_WITH_ERRORS
    assert result.timeline_events == []
    assert {lim.code for lim in result.limitations} >= {"NO_ACCEPTED_EVENTS", "EXCLUDED_INPUTS"}
