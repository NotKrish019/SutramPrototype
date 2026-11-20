"""Reconstruction run storage boundary (Member 4 — Phase 3G).

``ReconstructionRunStore`` is the small interface the reconstruction API depends on. This
module ships only ``InMemoryRunStore``, a process-local, bounded store for development and
tests. Runs are lost on restart.

TODO(Member 2): implement ``ReconstructionRunStore`` on PostgreSQL. Store the snapshot
(``ReconstructionResult.to_snapshot()``) keyed by ``(case_id, run_id)``, together with
``result_sha256``, ``algorithm_version`` and ``configuration_version``. Keep ``weight`` nullable.
Then return it from ``app.modules.reconstruction.service.get_reconstruction_service``. No
database code lives in the Member 4 module.
"""

from collections import OrderedDict
from threading import Lock
from typing import Protocol

from app.modules.reconstruction.pipeline import ReconstructionResult


class RunNotFoundError(LookupError):
    """No run with this id exists for this case."""

    def __init__(self, case_id: str, run_id: str) -> None:
        super().__init__(f"No reconstruction run '{run_id}' exists for case '{case_id}'.")
        self.case_id = case_id
        self.run_id = run_id


class ReconstructionRunStore(Protocol):
    """Persistence boundary for reconstruction runs."""

    def save(self, result: ReconstructionResult) -> None:
        """Store (or replace) the run identified by ``(result.case_id, result.run_id)``."""
        ...

    def get(self, case_id: str, run_id: str) -> ReconstructionResult:
        """Return the run, or raise ``RunNotFoundError`` (also when it belongs to another case)."""
        ...


class InMemoryRunStore:
    """Thread-safe, bounded in-memory store. Evicts the oldest run beyond ``max_runs``."""

    def __init__(self, max_runs: int = 256) -> None:
        if max_runs < 1:
            raise ValueError("max_runs must be >= 1.")
        self._max_runs = max_runs
        self._runs: OrderedDict[tuple[str, str], ReconstructionResult] = OrderedDict()
        self._lock = Lock()

    def save(self, result: ReconstructionResult) -> None:
        key = (result.case_id, result.run_id)
        with self._lock:
            self._runs.pop(key, None)
            self._runs[key] = result
            while len(self._runs) > self._max_runs:
                self._runs.popitem(last=False)

    def get(self, case_id: str, run_id: str) -> ReconstructionResult:
        with self._lock:
            result = self._runs.get((case_id, run_id))
        if result is None:
            raise RunNotFoundError(case_id, run_id)
        return result

    def __len__(self) -> int:
        with self._lock:
            return len(self._runs)
