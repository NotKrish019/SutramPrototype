"""Reconstruction service: runs the pipeline and stores results (Member 4 — Phase 3G).

API route handlers stay thin and delegate to ``ReconstructionService``. The storage root,
artifact root and run store are configured here, not in the routes.

Environment (all optional):

- ``YUKTI_RECONSTRUCTION_STORAGE_ROOT``: root against which canonical evidence keys
  ``data/evidence/raw/...`` are resolved. Default: the repository root.
- ``YUKTI_RECONSTRUCTION_ARTIFACT_ROOT``: derived-frame directory. Unset means frame extraction
  is skipped (``VIDEO_STAGE_SKIPPED``).
- ``YUKTI_RECONSTRUCTION_MAX_STORED_RUNS``: capacity of the in-memory run store.
"""

from collections.abc import Callable, Mapping, Sequence
from datetime import UTC, datetime
from functools import lru_cache
from pathlib import Path
from typing import Any

from pydantic_settings import BaseSettings, SettingsConfigDict

from app.modules.reconstruction.pipeline import (
    ReconstructionPipeline,
    ReconstructionPipelineConfig,
    ReconstructionResult,
)
from app.modules.reconstruction.run_store import (
    InMemoryRunStore,
    ReconstructionRunStore,
)
from app.modules.reporting.forensic_report import ForensicReport, build_forensic_report

_REPOSITORY_ROOT = Path(__file__).resolve().parents[4]


def _utc_now() -> datetime:
    return datetime.now(UTC)


class ReconstructionSettings(BaseSettings):
    model_config = SettingsConfigDict(env_prefix="YUKTI_RECONSTRUCTION_", extra="ignore")

    STORAGE_ROOT: Path = _REPOSITORY_ROOT
    ARTIFACT_ROOT: Path | None = None
    MAX_STORED_RUNS: int = 256


class ReconstructionService:
    """Runs reconstructions for a case and serves stored results."""

    def __init__(
        self,
        config: ReconstructionPipelineConfig,
        store: ReconstructionRunStore,
        *,
        clock: Callable[[], datetime] = _utc_now,
    ) -> None:
        self._pipeline = ReconstructionPipeline(config)
        self._store = store
        self._clock = clock

    def run(
        self,
        case_id: str,
        *,
        evidence: Sequence[Mapping[str, Any]],
        event_candidates: Sequence[Mapping[str, Any]] | Mapping[str, Any],
        entities: Sequence[Mapping[str, Any]],
        frame_artifacts: Sequence[Mapping[str, Any]],
    ) -> ReconstructionResult:
        result = self._pipeline.run(
            case_id=case_id,
            evidence=evidence,
            event_candidates=event_candidates,
            entities=entities,
            frame_artifacts=frame_artifacts,
        )
        self._store.save(result)
        return result

    def get(self, case_id: str, run_id: str) -> ReconstructionResult:
        return self._store.get(case_id, run_id)

    def report(self, case_id: str, run_id: str) -> ForensicReport:
        """Evidence-linked report of a stored run, generated now (UTC)."""
        return build_forensic_report(self.get(case_id, run_id), generated_at=self._clock())


@lru_cache(maxsize=1)
def get_reconstruction_service() -> ReconstructionService:
    """Process-wide service. Tests override this dependency in FastAPI."""
    settings = ReconstructionSettings()
    config = ReconstructionPipelineConfig(
        storage_root=settings.STORAGE_ROOT,
        artifact_root=settings.ARTIFACT_ROOT,
    )
    # TODO(Member 2): replace InMemoryRunStore with the PostgreSQL-backed ReconstructionRunStore.
    return ReconstructionService(config, InMemoryRunStore(settings.MAX_STORED_RUNS))
