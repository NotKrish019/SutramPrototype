"""Phase 3D.2: real producer paths into the Member 4 pipeline.

Unlike the other scenarios, these tests do not hand-build producer output. They call the actual
upstream code:

- Member 1: ``app.modules.extraction.service.extract_evidence`` returning the real
  ``UnifiedExtractionResult`` / ``ExtractedEventCandidate`` models;
- Member 2: ``app.modules.ingestion.service.EvidenceIngestionService.ingest_evidence`` returning
  the real ``EvidenceRecord``.

The tests document today's status at each boundary: blocked, with an explicit diagnostic. They
are not proof of end-to-end integration. All data is synthetic.
"""

import asyncio
import io
from dataclasses import asdict
from pathlib import Path
from typing import Any

from app.modules.extraction.service import compute_sha256, extract_evidence
from app.modules.ingestion.service import EvidenceIngestionService, EvidenceRecord, IngestionRepository
from app.modules.reconstruction.adapters.contracts import PROPOSED_CONTRACT_DECISIONS
from app.modules.reconstruction.adapters.evidence_adapter import ClockCalibrationStatus
from app.modules.reconstruction.pipeline import (
    PipelineStatus,
    ReconstructionPipeline,
    ReconstructionPipelineConfig,
    ReconstructionResult,
)
from app.schemas.extraction import EvidenceExtractionRequest, ExtractedEventCandidate, UnifiedExtractionResult
from tests.reconstruction.adapters.synthetic_burglary import storage_key

CASE = "case-real-producers-001"
EVIDENCE_ID = "ev-cdr-synthetic"

# Synthetic CDR. Row 1 has a naive device timestamp (the device runs on IST), row 2 has an
# explicit offset.
CDR_CSV = (
    "record_id,caller_phone,receiver_phone,call_timestamp,duration_seconds,cell_tower_id,call_type\n"
    "r-001,+919800000001,+919800000002,2026-11-10T22:00:00,60,TOWER-SYN-01,VOICE_OUTGOING\n"
    "r-002,+919800000002,+919800000001,2026-11-10T22:10:00+05:30,30,TOWER-SYN-02,VOICE_INCOMING\n"
)


def _pipeline(storage_root: Path) -> ReconstructionPipeline:
    # Behaviour conditional on the PROPOSED decisions; none is team-approved.
    return ReconstructionPipeline(
        ReconstructionPipelineConfig(
            storage_root=storage_root, approved_contract_decisions=PROPOSED_CONTRACT_DECISIONS
        )
    )


def _stage_canonical(storage_root: Path, data: bytes, evidence_id: str = EVIDENCE_ID) -> Path:
    target = storage_root / storage_key(evidence_id, CASE)
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_bytes(data)
    return target


def _shared_evidence(path: Path, metadata: dict[str, Any], evidence_id: str = EVIDENCE_ID) -> dict[str, Any]:
    return {
        "evidence_id": evidence_id,
        "case_id": CASE,
        "source_type": "CDR",
        "original_filename": "calls.csv",
        "file_size_bytes": path.stat().st_size,
        "sha256_hash": compute_sha256(path),
        "storage_uri": storage_key(evidence_id, CASE),
        "ingested_at": "2026-11-11T09:00:00Z",
        "metadata": metadata,
    }


def _member1_extract(path: Path) -> UnifiedExtractionResult:
    return extract_evidence(
        EvidenceExtractionRequest(
            evidence_id=EVIDENCE_ID,
            case_id=CASE,
            source_type="CALL_DETAIL_RECORD",
            file_path=path,
            expected_sha256=compute_sha256(path),
        )
    )


def _member2_ingest(tmp_path: Path, **kwargs: Any) -> EvidenceRecord:
    service = EvidenceIngestionService(storage_base_dir=tmp_path / "m2-store", registry=IngestionRepository())
    response = asyncio.run(
        service.ingest_evidence(CASE, io.BytesIO(CDR_CSV.encode("utf-8")), "calls.csv", **kwargs)
    )
    record = service.get_evidence(response.evidence_id)
    assert record is not None
    return record


def _run(storage_root: Path, evidence: list[dict[str, Any]], candidates: Any) -> ReconstructionResult:
    return _pipeline(storage_root).run(case_id=CASE, evidence=evidence, event_candidates=candidates, entities=[])


# --- Member 1: real ExtractedEventCandidate -------------------------------------------------


def test_member1_real_extraction_output_is_blocked_as_contract_gap(tmp_path: Path) -> None:
    path = _stage_canonical(tmp_path, CDR_CSV.encode("utf-8"))
    extraction = _member1_extract(path)
    assert extraction.status.value == "SUCCESS"
    assert len(extraction.event_candidates) == 2
    assert all(isinstance(c, ExtractedEventCandidate) for c in extraction.event_candidates)

    evidence = [_shared_evidence(path, {"source_timezone": "Asia/Kolkata"})]
    result = _run(tmp_path, evidence, extraction)  # the real model object, not a dict

    assert result.status == PipelineStatus.COMPLETED_WITH_ERRORS
    assert result.timeline_events == []
    excluded = {(x.subject_id, x.reason_code) for x in result.excluded_inputs}
    assert excluded == {(c.candidate_id, "PRODUCER_CONTRACT_GAP") for c in extraction.event_candidates}
    for diag in (d for d in result.diagnostics if d.code == "PRODUCER_CONTRACT_GAP"):
        assert "timestamp_precision" in diag.message and "description" in diag.message
        assert "location_name" in diag.message and "metadata" in diag.message  # reported, not dropped


def test_member1_candidate_once_contract_fields_are_supplied_keeps_source_fields(tmp_path: Path) -> None:
    """What happens once Member 1 supplies the two missing fields.

    The two values are added here by the test to show that preservation works. They are NOT a
    proposed mapping: Member 4 never fills them in itself.
    """
    path = _stage_canonical(tmp_path, CDR_CSV.encode("utf-8"))
    extraction = _member1_extract(path)
    candidates = []
    for model in extraction.event_candidates:
        item = model.model_dump(mode="json")
        item.update(timestamp_precision="EXACT_SECOND", description="supplied by producer (test)")
        candidates.append(item)
    evidence = [_shared_evidence(path, {"source_timezone": "Asia/Kolkata"})]
    result = _run(tmp_path, evidence, candidates)

    by_id = {e.candidate_id: e for e in result.timeline_events}
    assert set(by_id) == {c.candidate_id for c in extraction.event_candidates}
    for model in extraction.event_candidates:
        event = by_id[model.candidate_id]
        assert event.raw_timestamp == model.raw_timestamp
        assert event.source_fields_preserved == {
            "location_name": model.location_name,
            "metadata": model.metadata,
        }
    assert by_id["evt-cdr-ev-cdr-synthetic-r-001"].timestamp_utc.isoformat() == "2026-11-10T16:30:00+00:00"
    assert by_id["evt-cdr-ev-cdr-synthetic-r-002"].timestamp_utc.isoformat() == "2026-11-10T16:40:00+00:00"
    # The snapshot round trip keeps the preserved fields.
    round_trip = {c["candidate_id"]: c for c in result.to_event_candidates()}
    assert round_trip["evt-cdr-ev-cdr-synthetic-r-001"]["location_name"] == "TOWER-SYN-01"


# --- Member 2: real EvidenceRecord ------------------------------------------------------------


def test_member2_real_evidence_record_is_blocked_by_storage_contract(tmp_path: Path) -> None:
    record = _member2_ingest(tmp_path)
    raw = asdict(record)
    raw["storage_path"] = str(raw["storage_path"])
    raw["status"] = str(raw["status"])

    def first_error(record_dict: dict[str, Any]) -> tuple[str, str]:
        result = _run(tmp_path, [record_dict], [])
        assert [(x.subject_type, x.subject_id) for x in result.excluded_inputs] == [
            ("EVIDENCE", record.evidence_id)
        ]
        (diag,) = [d for d in result.diagnostics if d.subject_id == record.evidence_id and d.severity == "ERROR"]
        return diag.code, diag.message

    # Gap 1: source_type vocabulary (Member 2 'CALL_DETAIL_RECORD' vs shared EvidenceType 'CDR').
    code, message = first_error(raw)
    assert (code, "'CALL_DETAIL_RECORD'" in message) == ("INVALID_EVIDENCE_RECORD", True)
    # Gap 2: no storage_uri. Member 2 has created_at, not ingested_at, and an absolute storage_path.
    code, message = first_error({**raw, "source_type": "CDR"})
    assert code == "INVALID_EVIDENCE_RECORD" and "storage_uri" in message
    # Gap 3: Member 2's absolute host path is not a canonical storage key, so it is refused.
    code, _ = first_error({**raw, "source_type": "CDR", "storage_uri": raw["storage_path"]})
    assert code == "CLIENT_STORAGE_PATH_REJECTED"
    # Member 2 stores bytes at {base}/{case}/{evidence}/{filename}, not at the canonical key.
    assert record.storage_path.name == "calls.csv"
    assert "ingested_at" not in raw and "created_at" in raw


# --- Member 2 clock metadata: undeclared defaults must stay unknown -------------------------


def _with_member2_metadata(tmp_path: Path, **ingest_kwargs: Any) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    """Shared Evidence built from a real Member 2 record. Only the storage location is bridged
    (bytes copied to the canonical key); the metadata is Member 2's, unchanged."""
    record = _member2_ingest(tmp_path, **ingest_kwargs)
    path = _stage_canonical(tmp_path, record.storage_path.read_bytes(), record.evidence_id)
    return [_shared_evidence(path, dict(record.metadata), record.evidence_id)], dict(record.metadata)


def _ist_candidates(evidence_id: str) -> list[dict[str, Any]]:
    def cand(cid: str, raw: str) -> dict[str, Any]:
        return {
            "candidate_id": cid,
            "source_evidence_id": evidence_id,
            "source_record_id": cid,
            "raw_timestamp": raw,
            "timestamp_precision": "EXACT_SECOND",
            "event_type": "SYNTHETIC",
            "description": "synthetic",
            "involved_entity_raw_ids": [],
            "coordinates": None,
            "duration_seconds": None,
            "extraction_confidence": None,
        }

    return [cand("c-naive-ist", "2026-11-10T22:00:00"), cand("c-explicit", "2026-11-10T22:00:00+05:30")]


def test_undeclared_member2_defaults_do_not_normalize_ist_device_time_as_utc(tmp_path: Path) -> None:
    evidence, metadata = _with_member2_metadata(tmp_path)
    assert metadata["source_timezone"] == "UTC" and metadata["clock_offset_seconds"] == 0.0  # M2 defaults
    eid = evidence[0]["evidence_id"]

    result = _run(tmp_path, evidence, _ist_candidates(eid))

    utc = {e.candidate_id: e.timestamp_utc.isoformat() for e in result.timeline_events}
    assert "c-naive-ist" not in utc  # never 2026-11-10T22:00:00Z
    assert ("EVENT_CANDIDATE", "c-naive-ist", "SOURCE_TIMEZONE_UNKNOWN") in {
        (x.subject_type, x.subject_id, x.reason_code) for x in result.excluded_inputs
    }
    assert utc["c-explicit"] == "2026-11-10T16:30:00+00:00"  # explicit offset still parsed once
    (event,) = [e for e in result.timeline_events if e.candidate_id == "c-explicit"]
    assert event.clock_calibration == ClockCalibrationStatus.NOT_DECLARED_UNCALIBRATED
    assert event.clock_offset_seconds is None and event.source_timezone is None
    codes = {d.code for d in result.diagnostics if d.subject_id == eid}
    assert "CLOCK_METADATA_INGESTION_DEFAULT_IGNORED" in codes
    (view,) = [v for v in result.evidence if v.evidence_id == eid]
    assert view.model_dump()["clock_calibration"] != ClockCalibrationStatus.DECLARED_NOT_INDEPENDENTLY_VERIFIED


def test_explicit_member2_timezone_and_offset_are_preserved_and_applied_once(tmp_path: Path) -> None:
    evidence, metadata = _with_member2_metadata(
        tmp_path, source_timezone="Asia/Kolkata", clock_offset_seconds=-300.0
    )
    eid = evidence[0]["evidence_id"]
    result = _run(tmp_path, evidence, _ist_candidates(eid))

    utc = {e.candidate_id: e.timestamp_utc.isoformat() for e in result.timeline_events}
    assert utc == {
        "c-naive-ist": "2026-11-10T16:25:00+00:00",
        "c-explicit": "2026-11-10T16:25:00+00:00",
    }
    for event in result.timeline_events:
        assert event.source_timezone == "Asia/Kolkata"
        assert event.clock_offset_seconds == -300.0
        assert event.clock_calibration == ClockCalibrationStatus.DECLARED_NOT_INDEPENDENTLY_VERIFIED
    assert "CLOCK_METADATA_INGESTION_DEFAULT_IGNORED" not in {d.code for d in result.diagnostics}
