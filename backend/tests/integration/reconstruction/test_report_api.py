"""Phase 3I report endpoint: formats, headers, parity with the renderers, demo file freshness."""

import json

import pytest
from fastapi.testclient import TestClient

from app.modules.reporting.forensic_report import build_forensic_report
from app.modules.reporting.report_render import render_html, render_markdown
from tests.integration.reconstruction.demo_report import (
    committed_generated_at,
    demo_paths,
    render_demo,
)
from tests.integration.reconstruction.support import (
    BASE,
    FIXED_NOW,
    SyntheticCase,
    make_service,
    post_run,
    use_service,
)


@pytest.mark.parametrize(
    ("fmt", "media_type", "extension"),
    [
        (None, "application/json", "json"),
        ("json", "application/json", "json"),
        ("markdown", "text/markdown; charset=utf-8", "md"),
        ("html", "text/html; charset=utf-8", "html"),
    ],
)
def test_report_formats_and_headers(
    client: TestClient, burglary_case: SyntheticCase, fmt: str | None, media_type: str, extension: str
) -> None:
    use_service(make_service(burglary_case))
    run_id = post_run(client, burglary_case)["run_id"]
    params = {"format": fmt} if fmt else None
    response = client.get(f"{BASE}/runs/{run_id}/report", params=params)
    assert response.status_code == 200
    assert response.headers["content-type"] == media_type
    assert response.headers["content-disposition"] == f'inline; filename="report-{run_id}.{extension}"'
    assert response.headers["cache-control"] == "no-store"
    assert response.headers["x-content-type-options"] == "nosniff"
    assert len(response.headers["x-report-content-sha256"]) == 64
    if fmt == "html":
        assert response.headers["content-security-policy"] == "default-src 'none'; style-src 'unsafe-inline'"


def test_report_matches_renderers_for_the_stored_run(client: TestClient, burglary_case: SyntheticCase) -> None:
    service = make_service(burglary_case)
    use_service(service)
    run_id = post_run(client, burglary_case)["run_id"]
    expected = build_forensic_report(service.get("case-wh-burglary-001", run_id), generated_at=FIXED_NOW)
    body = client.get(f"{BASE}/runs/{run_id}/report").json()
    assert body == json.loads(expected.to_json())
    assert body["content_sha256"] == expected.content_sha256
    assert client.get(f"{BASE}/runs/{run_id}/report", params={"format": "markdown"}).text == render_markdown(expected)
    assert client.get(f"{BASE}/runs/{run_id}/report", params={"format": "html"}).text == render_html(expected)


def test_report_header_hash_matches_body(client: TestClient, burglary_case: SyntheticCase) -> None:
    use_service(make_service(burglary_case))
    run_id = post_run(client, burglary_case)["run_id"]
    response = client.get(f"{BASE}/runs/{run_id}/report")
    assert response.headers["x-report-content-sha256"] == response.json()["content_sha256"]


def test_report_for_failed_run_is_served(client: TestClient, burglary_case: SyntheticCase) -> None:
    use_service(make_service(burglary_case))
    response = client.post(f"{BASE}/run", json={"evidence": burglary_case.evidence, "event_candidates": [], "entities": [{"bad": 1}]})
    assert response.status_code == 201
    summary = response.json()
    assert summary["status"] == "FAILED"
    report = client.get(f"{BASE}/runs/{summary['run_id']}/report").json()
    assert report["metadata"]["run_status"] == "FAILED"
    assert "RUN_FAILED_NO_RECONSTRUCTION" in {lim["code"] for lim in report["limitations"]["limitations"]}


def test_committed_demo_report_is_current() -> None:
    paths = demo_paths()
    assert all(p.is_file() for p in paths.values()), "run: uv run python -m tests.integration.reconstruction.demo_report"
    expected = render_demo(committed_generated_at())
    for ext, path in paths.items():
        assert path.read_text(encoding="utf-8") == expected[ext], (
            f"{path.name} is stale; regenerate with: uv run python -m tests.integration.reconstruction.demo_report"
        )
