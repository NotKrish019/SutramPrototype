"""Synthetic warehouse-burglary case fixtures for the Member 4 adapter and pipeline tests.

All people, numbers, places and files are fabricated for testing. No real case data is used.

Case ``case-wh-burglary-001``. Event candidates follow the Member 1 contract and keep
``raw_timestamp`` exactly as each source shows it (DEC-007):

| candidate                | evidence        | raw_timestamp               | corrected UTC (once) |
| ------------------------ | --------------- | --------------------------- | -------------------- |
| evt-01-shutter-forced    | CCTV (IST, -120)| 2026-11-10T21:42:00 (naive) | 16:10:00Z            |
| evt-02-outbound-call     | CDR (no offset) | 2026-11-10T16:12:00Z        | 16:12:00Z            |
| evt-03-location-shared   | chat (IST, 0)   | 2026-11-10T21:44:00+05:30   | 16:14:00Z            |
| evt-04-vehicle-message   | chat (IST, 0)   | 2026-11-10T21:50:00 (naive) | 16:20:00Z            |
| evt-05-van-departs       | CCTV (IST, -120)| 2026-11-10T21:57:00 (naive) | 16:25:00Z            |

The DVR runs 120 s fast, so DEC-001 gives ``clock_offset_seconds = -120``. The 16:10 → 16:14
sightings of ent-person-ravi (warehouse → Gurugram, ~25 km) produce exactly one conservative
SPEED_THRESHOLD_EXCEEDANCE_LEAD.
"""

import hashlib
import json
import os
import shutil
import subprocess
from copy import deepcopy
from dataclasses import dataclass
from pathlib import Path
from typing import Any

CASE_ID = "case-wh-burglary-001"

FFMPEG_AVAILABLE = shutil.which("ffmpeg") is not None and shutil.which("ffprobe") is not None

WAREHOUSE = {"lat": 28.5355, "lon": 77.2710}
TOWER_NEAR_WAREHOUSE = {"lat": 28.5360, "lon": 77.2700}
GURUGRAM = {"lat": 28.4595, "lon": 77.0266}

CCTV_TIME_METADATA: dict[str, Any] = {
    "source_timezone": "Asia/Kolkata",
    "clock_offset_seconds": -120.0,
    "recording_start_timestamp": "2026-11-10T21:30:00",
    "device_coordinates": dict(WAREHOUSE),
}
CHAT_TIME_METADATA: dict[str, Any] = {
    "source_timezone": "Asia/Kolkata",
    "clock_offset_seconds": 0.0,
}

EXPECTED_UTC = {
    "evt-01-shutter-forced": "2026-11-10T16:10:00Z",
    "evt-02-outbound-call": "2026-11-10T16:12:00Z",
    "evt-03-location-shared": "2026-11-10T16:14:00Z",
    "evt-04-vehicle-message": "2026-11-10T16:20:00Z",
    "evt-05-van-departs": "2026-11-10T16:25:00Z",
}


@dataclass
class SyntheticCase:
    storage_root: Path
    artifact_root: Path
    evidence: list[dict[str, Any]]
    candidates: list[dict[str, Any]]
    entities: list[dict[str, Any]]

    def evidence_by_id(self, evidence_id: str) -> dict[str, Any]:
        return next(e for e in self.evidence if e["evidence_id"] == evidence_id)

    def candidate_by_id(self, candidate_id: str) -> dict[str, Any]:
        return next(c for c in self.candidates if c["candidate_id"] == candidate_id)

    def unified_extraction_json(self) -> str:
        """Candidates serialized like Rust ``UnifiedExtractionResult``."""
        return json.dumps({"case_id": CASE_ID, "event_candidates": self.candidates})


def storage_key(evidence_id: str, case_id: str = CASE_ID) -> str:
    return f"data/evidence/raw/{case_id}/{evidence_id}/{evidence_id}.raw"


def evidence_path(root: Path, evidence_id: str, case_id: str = CASE_ID) -> Path:
    return root / storage_key(evidence_id, case_id)


def sha256_of(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write_evidence_bytes(root: Path, evidence_id: str, data: bytes) -> tuple[int, str]:
    target = evidence_path(root, evidence_id)
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_bytes(data)
    return len(data), hashlib.sha256(data).hexdigest()


def make_synthetic_mp4(path: Path, *, duration_seconds: float = 2.0) -> None:
    """Generate a tiny synthetic test-pattern video with FFmpeg (MP4 bytes, .raw name)."""
    path.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run(
        [
            "ffmpeg",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            f"testsrc=duration={duration_seconds}:size=64x48:rate=5",
            "-pix_fmt",
            "yuv420p",
            "-f",
            "mp4",
            str(path),
        ],
        check=True,
        capture_output=True,
        timeout=60,
    )


def snapshot_tree(root: Path) -> dict[str, tuple[str, int, int, int]]:
    """(sha256, size, mtime_ns, mode) for every regular file under root."""
    snap: dict[str, tuple[str, int, int, int]] = {}
    for path in sorted(root.rglob("*")):
        if path.is_file() and not path.is_symlink():
            st = os.stat(path)
            snap[path.relative_to(root).as_posix()] = (
                sha256_of(path),
                st.st_size,
                st.st_mtime_ns,
                st.st_mode,
            )
    return snap


def _evidence_record(
    evidence_id: str,
    source_type: str,
    filename: str,
    size_and_hash: tuple[int, str],
    *,
    storage_uri: str,
    metadata: dict[str, Any] | None = None,
) -> dict[str, Any]:
    size, digest = size_and_hash
    return {
        "evidence_id": evidence_id,
        "case_id": CASE_ID,
        "source_type": source_type,
        "original_filename": filename,
        "file_size_bytes": size,
        "sha256_hash": digest,
        "storage_uri": storage_uri,
        "ingested_at": "2026-11-11T09:00:00Z",
        "metadata": metadata or {},
    }


def build_candidates() -> list[dict[str, Any]]:
    def candidate(
        cid: str,
        evidence_id: str,
        record_id: str,
        raw: str,
        precision: str,
        event_type: str,
        description: str,
        refs: list[str],
        coords: dict[str, float] | None,
        duration: float | None = None,
        confidence: float | None = 0.9,
    ) -> dict[str, Any]:
        return {
            "candidate_id": cid,
            "source_evidence_id": evidence_id,
            "source_record_id": record_id,
            "raw_timestamp": raw,
            "timestamp_precision": precision,
            "event_type": event_type,
            "description": description,
            "involved_entity_raw_ids": refs,
            "coordinates": dict(coords) if coords is not None else None,
            "duration_seconds": duration,
            "extraction_confidence": confidence,
        }

    return [
        candidate(
            "evt-01-shutter-forced", "ev-cctv-dock", "osd:2026-11-10T21:42:00",
            "2026-11-10T21:42:00", "EXACT_SECOND", "FORCED_ENTRY",
            "Rear shutter forced open at loading dock", ["ent-person-ravi"], WAREHOUSE,
            duration=30.0,
        ),
        candidate(
            "evt-02-outbound-call", "ev-cdr-tower", "row:2",
            "2026-11-10T16:12:00Z", "MINUTE", "PHONE_CALL",
            "Outbound call from handset near warehouse", ["ent-phone-01"], TOWER_NEAR_WAREHOUSE,
        ),
        candidate(
            "evt-03-location-shared", "ev-chat-export", "line:1",
            "2026-11-10T21:44:00+05:30", "MINUTE", "LOCATION_SHARE",
            "Live location shared in chat", ["ent-person-ravi"], GURUGRAM,
        ),
        candidate(
            "evt-04-vehicle-message", "ev-chat-export", "line:2",
            "2026-11-10T21:50:00", "MINUTE", "MESSAGE",
            "Chat message mentioning pickup vehicle", ["ent-person-ravi", "ent-unknown-77"], None,
            confidence=None,
        ),
        candidate(
            "evt-05-van-departs", "ev-cctv-dock", "osd:2026-11-10T21:57:00",
            "2026-11-10T21:57:00", "EXACT_SECOND", "VEHICLE_DEPARTURE",
            "Van leaves warehouse premises", ["ent-vehicle-van"], WAREHOUSE,
        ),
    ]


def engine_level_events() -> list[dict[str, Any]]:
    """Engine-API events with already-corrected UTC timestamps, for direct engine calls only.

    These bypass the adapters on purpose. The adapters never accept such pre-corrected values
    as candidate input.
    """
    by_id = {c["candidate_id"]: c for c in build_candidates()}
    return [
        {
            "event_id": cid,
            "case_id": CASE_ID,
            "title": by_id[cid]["description"],
            "timestamp": utc,
            "timestamp_precision": by_id[cid]["timestamp_precision"],
            "coordinates": by_id[cid]["coordinates"],
            "involved_entity_ids": [
                r for r in by_id[cid]["involved_entity_raw_ids"] if r != "ent-unknown-77"
            ],
            "source_evidence_id": by_id[cid]["source_evidence_id"],
        }
        for cid, utc in EXPECTED_UTC.items()
    ]


def build_entities() -> list[dict[str, Any]]:
    def entity(eid: str, etype: str, name: str, aliases: list[str], sources: list[str]) -> dict[str, Any]:
        return {
            "entity_id": eid,
            "case_id": CASE_ID,
            "entity_type": etype,
            "canonical_name": name,
            "aliases": aliases,
            "source_evidence_ids": sources,
            "confidence": 1.0,
        }

    return [
        entity("ent-person-ravi", "PERSON", "Ravi Kumar", ["R. Kumar"], ["ev-cctv-dock", "ev-chat-export"]),
        entity("ent-person-ravi2", "PERSON", "Ravi Kumaar", [], ["ev-chat-export"]),
        entity("ent-phone-01", "PHONE_NUMBER", "+910000000001", ["Handset A"], ["ev-cdr-tower"]),
        entity("ent-vehicle-van", "VEHICLE", "DL00XX0000", [], ["ev-cctv-dock"]),
    ]


def build_case(tmp_path: Path, *, real_video: bool = False) -> SyntheticCase:
    storage_root = tmp_path / "storage"
    artifact_root = tmp_path / "derived"
    storage_root.mkdir()
    artifact_root.mkdir()

    if real_video:
        target = evidence_path(storage_root, "ev-cctv-dock")
        make_synthetic_mp4(target)
        data = target.read_bytes()
        cctv = (len(data), hashlib.sha256(data).hexdigest())
    else:
        cctv = write_evidence_bytes(storage_root, "ev-cctv-dock", b"SYNTHETIC-CCTV-PLACEHOLDER-BYTES")

    cdr = write_evidence_bytes(
        storage_root,
        "ev-cdr-tower",
        b"msisdn,peer,start_utc,cell\n+910000000001,+910000000002,2026-11-10T16:12:04Z,OKH-7B\n",
    )
    chat = write_evidence_bytes(
        storage_root,
        "ev-chat-export",
        b"[synthetic] 21:44 location shared\n[synthetic] 21:50 pickup vehicle\n",
    )

    evidence = [
        _evidence_record(
            "ev-cctv-dock", "CCTV", "dock_cam_01.mp4", cctv,
            storage_uri="urn:yukti:evidence:ev-cctv-dock:storage",
            metadata=deepcopy(CCTV_TIME_METADATA),
        ),
        _evidence_record(
            "ev-cdr-tower", "CDR", "tower_records.csv", cdr,
            storage_uri=storage_key("ev-cdr-tower"),
        ),
        _evidence_record(
            "ev-chat-export", "CHAT_EXPORT", "export.txt", chat,
            storage_uri=storage_key("ev-chat-export"),
            metadata=deepcopy(CHAT_TIME_METADATA),
        ),
    ]
    return SyntheticCase(
        storage_root=storage_root,
        artifact_root=artifact_root,
        evidence=evidence,
        candidates=build_candidates(),
        entities=build_entities(),
    )
