"""Shared helpers for the Phase 3H integration scenarios and Phase 3I report tests.

Every scenario starts from the synthetic warehouse-burglary case
(``tests/reconstruction/adapters/synthetic_burglary.py``). All data is fabricated.
"""

import os
from collections.abc import Callable
from copy import deepcopy
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

from fastapi.testclient import TestClient

from app.main import app
from app.modules.reconstruction.adapters.contracts import PROPOSED_CONTRACT_DECISIONS
from app.modules.reconstruction.adapters.frame_handoff import ExtractedFrameConsumer
from app.modules.reconstruction.pipeline import (
    ReconstructionPipeline,
    ReconstructionPipelineConfig,
    ReconstructionResult,
)
from app.modules.reconstruction.run_store import InMemoryRunStore
from app.modules.reconstruction.service import (
    ReconstructionService,
    get_reconstruction_service,
)
from app.schemas.reconstruction import VideoExtractionOptions
from tests.reconstruction.adapters.synthetic_burglary import (
    CASE_ID,
    EXPECTED_UTC,
    FFMPEG_AVAILABLE,
    GURUGRAM,
    WAREHOUSE,
    SyntheticCase,
    build_case,
    evidence_path,
    storage_key,
    write_evidence_bytes,
)

__all__ = [
    "BASE",
    "CASE_ID",
    "EXPECTED_UTC",
    "FFMPEG_AVAILABLE",
    "FIXED_NOW",
    "GURUGRAM",
    "WAREHOUSE",
    "SyntheticCase",
    "assert_envelope",
    "build_case",
    "evidence_path",
    "fixed_clock",
    "get_view",
    "host_variants",
    "make_config",
    "make_service",
    "post_run",
    "request_body",
    "run_pipeline",
    "storage_key",
    "synthetic_candidate",
    "use_service",
    "write_evidence_bytes",
]

BASE = f"/cases/{CASE_ID}/reconstruction"
FIXED_NOW = datetime(2026, 11, 19, 12, 0, 0, tzinfo=UTC)


def fixed_clock() -> datetime:
    return FIXED_NOW


def make_config(case: SyntheticCase, *, with_video: bool = False, **overrides: Any) -> ReconstructionPipelineConfig:
    """Scenario config. Enables the PROPOSED decisions explicitly: none is team-approved, so the
    scenarios exercise behaviour conditional on approval (results list them as unverified)."""
    kwargs: dict[str, Any] = {
        "approved_contract_decisions": PROPOSED_CONTRACT_DECISIONS,
        "storage_root": case.storage_root,
        "artifact_root": case.artifact_root if with_video else None,
        "video_extraction_options": VideoExtractionOptions(
            interval_seconds=1.0, max_frames=5, create_unique_run_subdir=True
        ),
        **overrides,
    }
    return ReconstructionPipelineConfig(**kwargs)


def run_pipeline(
    case: SyntheticCase,
    *,
    evidence: Any = None,
    candidates: Any = None,
    entities: Any = None,
    frames: Any = None,
    consumer: ExtractedFrameConsumer | None = None,
    with_video: bool = False,
    **config: Any,
) -> ReconstructionResult:
    pipeline = ReconstructionPipeline(make_config(case, with_video=with_video, **config), frame_consumer=consumer)
    return pipeline.run(
        case_id=CASE_ID,
        evidence=case.evidence if evidence is None else evidence,
        event_candidates=case.candidates if candidates is None else candidates,
        entities=case.entities if entities is None else entities,
        frame_artifacts=frames,
    )


def make_service(
    case: SyntheticCase,
    *,
    with_video: bool = False,
    clock: Callable[[], datetime] = fixed_clock,
    **config: Any,
) -> ReconstructionService:
    return ReconstructionService(make_config(case, with_video=with_video, **config), InMemoryRunStore(), clock=clock)


def use_service(service: ReconstructionService) -> None:
    app.dependency_overrides[get_reconstruction_service] = lambda: service


def request_body(case: SyntheticCase, **overrides: Any) -> dict[str, Any]:
    body: dict[str, Any] = {
        "evidence": deepcopy(case.evidence),
        "event_candidates": deepcopy(case.candidates),
        "entities": deepcopy(case.entities),
        "frame_artifacts": [],
    }
    body.update(overrides)
    return body


def post_run(client: TestClient, case: SyntheticCase, **overrides: Any) -> dict[str, Any]:
    response = client.post(f"{BASE}/run", json=request_body(case, **overrides))
    assert response.status_code == 201, response.text
    data: dict[str, Any] = response.json()
    return data


def get_view(client: TestClient, run_id: str, view: str = "") -> dict[str, Any]:
    response = client.get(f"{BASE}/runs/{run_id}{view}")
    assert response.status_code == 200, response.text
    data: dict[str, Any] = response.json()
    return data


def assert_envelope(response: Any, status_code: int, code: str) -> dict[str, Any]:
    assert response.status_code == status_code, response.text
    body: dict[str, Any] = response.json()
    assert set(body) == {"error"}
    assert set(body["error"]) == {"code", "message", "details"}
    assert body["error"]["code"] == code
    return body


def host_variants(*paths: Path) -> set[str]:
    out: set[str] = set()
    for path in paths:
        out |= {str(path), os.path.realpath(path), os.path.abspath(path)}
    return out


def synthetic_candidate(
    candidate_id: str,
    evidence_id: str,
    raw_timestamp: str,
    *,
    precision: str = "EXACT_SECOND",
    entity_ids: list[str] | None = None,
    coordinates: dict[str, float] | None = None,
    description: str = "Synthetic event",
) -> dict[str, Any]:
    """A Member 1 contract candidate with every field present."""
    return {
        "candidate_id": candidate_id,
        "source_evidence_id": evidence_id,
        "source_record_id": f"rec:{candidate_id}",
        "raw_timestamp": raw_timestamp,
        "timestamp_precision": precision,
        "event_type": "SYNTHETIC",
        "description": description,
        "involved_entity_raw_ids": list(entity_ids or []),
        "coordinates": dict(coordinates) if coordinates is not None else None,
        "duration_seconds": None,
        "extraction_confidence": 0.5,
    }
