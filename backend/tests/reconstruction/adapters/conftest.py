"""Pytest fixtures for Phase 3D adapter tests (synthetic warehouse-burglary case)."""

from pathlib import Path

import pytest
from tests.reconstruction.adapters.synthetic_burglary import SyntheticCase, build_case


@pytest.fixture
def burglary_case(tmp_path: Path) -> SyntheticCase:
    return build_case(tmp_path)
