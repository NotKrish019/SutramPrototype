"""Phase 3E — Evidence Storage Adapter tests (canonical keys, URNs, integrity, DEC-006)."""

import json
import os
from copy import deepcopy
from pathlib import Path
from typing import Any

import pytest
from tests.reconstruction.adapters.synthetic_burglary import (
    CASE_ID,
    SyntheticCase,
    evidence_path,
    snapshot_tree,
    storage_key,
)

from app.modules.reconstruction.adapters.contracts import (
    PROPOSED_CONTRACT_DECISIONS,
    ContractDecision,
)
from app.modules.reconstruction.adapters.errors import (
    ClientStoragePathRejectedError,
    EvidenceIntegrityError,
    EvidencePathSecurityError,
    EvidenceRecordError,
    EvidenceStorageError,
    RemoteStorageNotSupportedError,
)
from app.modules.reconstruction.adapters.evidence_adapter import (
    ClockCalibrationStatus,
    EvidenceRecordView,
    evidence_to_video_extraction_options,
    parse_evidence_record,
    resolve_evidence_storage_path,
    storage_key_for,
    storage_reference_for,
    verify_evidence_file,
)
from app.schemas.reconstruction import VideoExtractionOptions


def _view(
    case: SyntheticCase,
    evidence_id: str,
    approved: Any = PROPOSED_CONTRACT_DECISIONS,
    **overrides: object,
) -> EvidenceRecordView:
    record = deepcopy(case.evidence_by_id(evidence_id))
    record.update(overrides)
    return parse_evidence_record(record, case_id=CASE_ID, approved_decisions=approved)


# --- Canonical storage keys and references ------------------------------------------------


def test_storage_key_and_reference_format() -> None:
    assert storage_key_for("case-1", "ev-9") == "data/evidence/raw/case-1/ev-9/ev-9.raw"
    assert storage_reference_for("ev-9") == "urn:yukti:evidence:ev-9:storage"


@pytest.mark.parametrize("evidence_id", ["ev-cdr-tower", "ev-cctv-dock"])
def test_canonical_key_and_urn_both_resolve(burglary_case: SyntheticCase, evidence_id: str) -> None:
    view = _view(burglary_case, evidence_id)
    assert view.storage_key == storage_key(evidence_id)
    assert view.storage_reference == f"urn:yukti:evidence:{evidence_id}:storage"
    path = resolve_evidence_storage_path(view, burglary_case.storage_root)
    assert path == Path(os.path.realpath(evidence_path(burglary_case.storage_root, evidence_id)))


@pytest.mark.parametrize(
    "uri",
    [
        "../outside.csv",
        "cdr/tower_records.csv",
        "/etc/passwd",
        "file:///etc/passwd",
        "file://localhost/etc/passwd",
        "C:\\evidence\\cdr.csv",
        "urn:yukti:evidence:ev-chat-export:storage",  # another evidence item's URN
        storage_key("ev-cdr-tower", case_id="case-other"),  # canonical shape, wrong case
        storage_key("ev-chat-export"),  # canonical key of another evidence item
    ],
)
def test_client_supplied_paths_rejected(burglary_case: SyntheticCase, uri: str) -> None:
    view = _view(burglary_case, "ev-cdr-tower", storage_uri=uri)
    with pytest.raises(ClientStoragePathRejectedError) as excinfo:
        resolve_evidence_storage_path(view, burglary_case.storage_root)
    message = str(excinfo.value)
    assert "Client-supplied storage paths are not accepted" in message
    assert "/etc/passwd" not in message  # supplied value is never echoed
    assert str(burglary_case.storage_root) not in message


@pytest.mark.parametrize(
    "uri",
    [
        "s3://evidence-bucket/case/cdr.csv",
        "https://storage.example.invalid/cdr.csv",
        "gs://bucket/cdr.csv",
        "file://evidence-server/share/cdr.csv",
        "custom://host/cdr.csv",
    ],
)
def test_remote_uris_rejected_staging_not_implemented(burglary_case: SyntheticCase, uri: str) -> None:
    view = _view(burglary_case, "ev-cdr-tower", storage_uri=uri)
    with pytest.raises(RemoteStorageNotSupportedError, match="staging is not implemented"):
        resolve_evidence_storage_path(view, burglary_case.storage_root)


@pytest.mark.parametrize("evidence_id", ["../ev", "ev/sub", ".hidden", "ev 1", "", "x" * 129])
def test_unsafe_evidence_ids_rejected(burglary_case: SyntheticCase, evidence_id: str) -> None:
    record = deepcopy(burglary_case.evidence_by_id("ev-cdr-tower"))
    record["evidence_id"] = evidence_id
    with pytest.raises(EvidenceRecordError):
        parse_evidence_record(record, case_id=CASE_ID)


def test_unsafe_case_id_rejected(burglary_case: SyntheticCase) -> None:
    record = deepcopy(burglary_case.evidence_by_id("ev-cdr-tower"))
    record["case_id"] = "../case"
    with pytest.raises(EvidenceRecordError, match="safe storage path"):
        parse_evidence_record(record, case_id="../case")


def test_symlinked_file_rejected(burglary_case: SyntheticCase, tmp_path: Path) -> None:
    target = evidence_path(burglary_case.storage_root, "ev-cdr-tower")
    outside = tmp_path / "outside.csv"
    outside.write_bytes(target.read_bytes())
    target.unlink()
    target.symlink_to(outside)
    with pytest.raises(EvidencePathSecurityError, match="symlink"):
        resolve_evidence_storage_path(_view(burglary_case, "ev-cdr-tower"), burglary_case.storage_root)


def test_symlinked_directory_component_rejected(burglary_case: SyntheticCase, tmp_path: Path) -> None:
    case_dir = burglary_case.storage_root / "data" / "evidence" / "raw" / CASE_ID
    moved = tmp_path / "moved_case_dir"
    case_dir.rename(moved)
    case_dir.symlink_to(moved)
    with pytest.raises(EvidencePathSecurityError, match="symlink"):
        resolve_evidence_storage_path(_view(burglary_case, "ev-cdr-tower"), burglary_case.storage_root)


def test_missing_file_and_directory_rejected(burglary_case: SyntheticCase) -> None:
    target = evidence_path(burglary_case.storage_root, "ev-cdr-tower")
    target.unlink()
    view = _view(burglary_case, "ev-cdr-tower")
    with pytest.raises(EvidenceStorageError, match="not found"):
        resolve_evidence_storage_path(view, burglary_case.storage_root)
    target.mkdir()
    with pytest.raises(EvidenceStorageError, match="not a regular file"):
        resolve_evidence_storage_path(view, burglary_case.storage_root)


def test_missing_storage_root_message_has_no_host_path(burglary_case: SyntheticCase, tmp_path: Path) -> None:
    missing_root = tmp_path / "no-such-root"
    with pytest.raises(EvidenceStorageError) as excinfo:
        resolve_evidence_storage_path(_view(burglary_case, "ev-cdr-tower"), missing_root)
    assert str(tmp_path) not in str(excinfo.value)


# --- Integrity ---------------------------------------------------------------------------


def test_sha256_verified_and_serialization_has_no_host_path(burglary_case: SyntheticCase) -> None:
    view = _view(burglary_case, "ev-cdr-tower")
    verified = verify_evidence_file(view, burglary_case.storage_root)
    assert verified.sha256_verified == view.sha256_hash
    assert verified.storage_key == storage_key("ev-cdr-tower")
    assert verified.storage_reference == "urn:yukti:evidence:ev-cdr-tower:storage"
    dumped = verified.model_dump(mode="json")
    assert "resolved_path" not in dumped
    assert str(burglary_case.storage_root) not in json.dumps(dumped)
    assert os.path.realpath(burglary_case.storage_root) not in json.dumps(dumped)


def test_sha256_mismatch_rejected(burglary_case: SyntheticCase) -> None:
    view = _view(burglary_case, "ev-cdr-tower", sha256_hash="0" * 64)
    with pytest.raises(EvidenceIntegrityError, match="SHA-256 mismatch"):
        verify_evidence_file(view, burglary_case.storage_root)


def test_tampered_bytes_detected(burglary_case: SyntheticCase) -> None:
    view = _view(burglary_case, "ev-chat-export")
    target = evidence_path(burglary_case.storage_root, "ev-chat-export")
    data = bytearray(target.read_bytes())
    data[0] ^= 0xFF
    target.write_bytes(bytes(data))
    with pytest.raises(EvidenceIntegrityError, match="SHA-256 mismatch"):
        verify_evidence_file(view, burglary_case.storage_root)


def test_size_mismatch_rejected(burglary_case: SyntheticCase) -> None:
    size = burglary_case.evidence_by_id("ev-cdr-tower")["file_size_bytes"]
    view = _view(burglary_case, "ev-cdr-tower", file_size_bytes=size + 1)
    with pytest.raises(EvidenceIntegrityError, match="size mismatch"):
        verify_evidence_file(view, burglary_case.storage_root)


def test_verification_never_modifies_evidence(burglary_case: SyntheticCase) -> None:
    before = snapshot_tree(burglary_case.storage_root)
    for record in burglary_case.evidence:
        verify_evidence_file(parse_evidence_record(record, case_id=CASE_ID), burglary_case.storage_root)
    assert snapshot_tree(burglary_case.storage_root) == before


# --- Record validation -------------------------------------------------------------------


@pytest.mark.parametrize(
    ("field", "value", "match"),
    [
        ("sha256_hash", "abc123", "sha256_hash"),
        ("file_size_bytes", -1, "file_size_bytes"),
        ("file_size_bytes", True, "file_size_bytes"),
        ("source_type", "GPS_TRACK", "source_type"),
        ("case_id", "case-other", "belongs to case"),
        ("storage_uri", "", "storage_uri"),
        ("metadata", ["not", "a", "mapping"], "metadata"),
    ],
)
def test_invalid_evidence_records_rejected(
    burglary_case: SyntheticCase, field: str, value: object, match: str
) -> None:
    record = deepcopy(burglary_case.evidence_by_id("ev-cdr-tower"))
    record[field] = value
    with pytest.raises(EvidenceRecordError, match=match):
        parse_evidence_record(record, case_id=CASE_ID)


def test_parse_does_not_mutate_input(burglary_case: SyntheticCase) -> None:
    record = burglary_case.evidence_by_id("ev-cctv-dock")
    original = deepcopy(record)
    view = parse_evidence_record(record, case_id=CASE_ID)
    view.metadata["clock_offset_seconds"] = 999.0
    assert record == original


# --- DEC-006 metadata ----------------------------------------------------------------------


def test_dec006_metadata_applied(burglary_case: SyntheticCase) -> None:
    view = _view(burglary_case, "ev-cctv-dock")
    settings = view.time_settings
    assert settings.applied_from_metadata is True
    assert settings.source_timezone == "Asia/Kolkata"
    assert settings.clock_offset_seconds == -120.0
    assert settings.recording_start_timestamp == "2026-11-10T21:30:00"
    assert settings.clock_calibration == ClockCalibrationStatus.DECLARED_NOT_INDEPENDENTLY_VERIFIED
    assert view.device_coordinates is not None and view.device_coordinates.lat == 28.5355


def test_undeclared_offset_and_timezone_stay_null(burglary_case: SyntheticCase) -> None:
    settings = _view(burglary_case, "ev-cdr-tower").time_settings
    assert settings.source_timezone is None
    assert settings.clock_offset_seconds is None
    assert settings.clock_calibration == ClockCalibrationStatus.NOT_DECLARED_UNCALIBRATED
    assert settings.as_temporal_evidence_record() == {"metadata": {"clock_offset_seconds": 0.0}}


def test_withheld_decisions_ignore_metadata(burglary_case: SyntheticCase) -> None:
    view = _view(burglary_case, "ev-cctv-dock", approved={ContractDecision.EVIDENCE_METADATA_KEYS})
    assert view.time_settings.applied_from_metadata is False
    assert view.time_settings.clock_calibration == ClockCalibrationStatus.NOT_APPLIED_DECISION_WITHHELD
    assert view.time_settings.ignored_metadata_keys == [
        "clock_offset_seconds",
        "recording_start_timestamp",
        "source_timezone",
    ]


@pytest.mark.parametrize(
    ("key", "value"),
    [
        ("source_timezone", "Mars/Olympus_Mons"),
        ("source_timezone", 530),
        ("clock_offset_seconds", "fast"),
        ("clock_offset_seconds", float("nan")),
        ("clock_offset_seconds", True),
        ("recording_start_timestamp", "not-a-time"),
    ],
)
def test_invalid_time_metadata_rejected(burglary_case: SyntheticCase, key: str, value: object) -> None:
    record = deepcopy(burglary_case.evidence_by_id("ev-cctv-dock"))
    record["metadata"][key] = value
    with pytest.raises(EvidenceRecordError, match="time metadata"):
        parse_evidence_record(
            record, case_id=CASE_ID, approved_decisions=PROPOSED_CONTRACT_DECISIONS
        )


@pytest.mark.parametrize("coords", [{"lat": 128.0, "lon": 77.0}, {"latitude": 28.5, "longitude": 77.2}])
def test_invalid_device_coordinates_rejected(
    burglary_case: SyntheticCase, coords: dict[str, float]
) -> None:
    record = deepcopy(burglary_case.evidence_by_id("ev-cctv-dock"))
    record["metadata"]["device_coordinates"] = coords
    with pytest.raises(EvidenceRecordError, match="device_coordinates"):
        parse_evidence_record(record, case_id=CASE_ID)


# --- Video options -----------------------------------------------------------------------


def test_video_options_anchor_once_from_evidence(burglary_case: SyntheticCase) -> None:
    base = VideoExtractionOptions(interval_seconds=0.5, max_frames=7, create_unique_run_subdir=True)
    options = evidence_to_video_extraction_options(_view(burglary_case, "ev-cctv-dock"), base_options=base)
    assert options.interval_seconds == 0.5 and options.max_frames == 7
    assert options.source_timezone == "Asia/Kolkata"
    assert options.clock_offset_seconds == -120.0
    assert options.recording_start_timestamp == "2026-11-10T21:30:00"


def test_naive_recording_start_without_timezone_not_anchored(burglary_case: SyntheticCase) -> None:
    record = deepcopy(burglary_case.evidence_by_id("ev-cctv-dock"))
    del record["metadata"]["source_timezone"]
    view = parse_evidence_record(record, case_id=CASE_ID)
    assert view.time_settings.recording_start_anchorable is False
    options = evidence_to_video_extraction_options(view)
    assert options.recording_start_timestamp is None
    assert options.clock_offset_seconds == 0.0


def test_base_options_cannot_inject_anchoring(burglary_case: SyntheticCase) -> None:
    view = _view(burglary_case, "ev-cctv-dock", approved=set())
    base = VideoExtractionOptions(recording_start_timestamp="2026-11-10T00:00:00Z", clock_offset_seconds=30.0)
    options = evidence_to_video_extraction_options(view, base_options=base)
    assert options.recording_start_timestamp is None
    assert options.clock_offset_seconds == 0.0


def test_non_video_evidence_rejected_for_video_options(burglary_case: SyntheticCase) -> None:
    with pytest.raises(EvidenceRecordError, match="not supported video evidence"):
        evidence_to_video_extraction_options(_view(burglary_case, "ev-cdr-tower"))


# --- Phase 3D.2: Member 2 undeclared ingestion defaults are unknown, not calibration ---------


@pytest.mark.parametrize(
    ("metadata", "tz", "offset", "ignored"),
    [
        # Member 2 marker + both defaults → both unknown.
        ({"content_detection_reason": "x", "source_timezone": "UTC", "clock_offset_seconds": 0.0},
         None, None, ["clock_offset_seconds", "source_timezone"]),
        # Member 2 marker + declared timezone, default offset → only the offset is unknown.
        ({"content_detection_reason": "x", "source_timezone": "Asia/Kolkata", "clock_offset_seconds": 0},
         "Asia/Kolkata", None, ["clock_offset_seconds"]),
        # Member 2 marker + non-default values → kept as declared.
        ({"content_detection_reason": "x", "source_timezone": "Asia/Kolkata", "clock_offset_seconds": -300},
         "Asia/Kolkata", -300.0, []),
        # No Member 2 marker → a literal UTC / 0.0 declaration is kept.
        ({"source_timezone": "UTC", "clock_offset_seconds": 0.0}, "UTC", 0.0, []),
    ],
)
def test_member2_ingestion_defaults_treated_as_unknown(
    burglary_case: SyntheticCase,
    metadata: dict[str, object],
    tz: str | None,
    offset: float | None,
    ignored: list[str],
) -> None:
    view = _view(burglary_case, "ev-chat-export", metadata=metadata)
    settings = view.time_settings
    assert (settings.source_timezone, settings.clock_offset_seconds) == (tz, offset)
    assert settings.ingestion_default_keys_ignored == ignored
    if offset is None:
        assert settings.clock_calibration == ClockCalibrationStatus.NOT_DECLARED_UNCALIBRATED
