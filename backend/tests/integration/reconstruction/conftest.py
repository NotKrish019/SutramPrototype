"""Fixtures for the reconstruction integration scenarios (synthetic data only)."""

from collections.abc import Iterator
from pathlib import Path

import pytest
from fastapi.testclient import TestClient

from app.main import app
from tests.integration.reconstruction.support import SyntheticCase, build_case


@pytest.fixture
def burglary_case(tmp_path: Path) -> SyntheticCase:
    return build_case(tmp_path)


@pytest.fixture
def client() -> Iterator[TestClient]:
    with TestClient(app, raise_server_exceptions=False) as test_client:
        yield test_client
    app.dependency_overrides.clear()
