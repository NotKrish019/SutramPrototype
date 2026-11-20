"""Scenario I: API contract (every endpoint, error envelope, no host paths, null weight)."""

import re
from typing import Any

import pytest
from fastapi.testclient import TestClient

from app.modules.reconstruction.run_store import InMemoryRunStore
from app.modules.reconstruction.service import ReconstructionService
from tests.integration.reconstruction.support import (
    BASE,
    CASE_ID,
    SyntheticCase,
    assert_envelope,
    host_variants,
    make_config,
    make_service,
    post_run,
    request_body,
    use_service,
)

GET_VIEWS = ("", "/timeline", "/spatial", "/contradictions", "/relationships", "/diagnostics", "/report")
REPORT_FORMATS = ("json", "markdown", "html")


def test_every_endpoint_answers_for_a_stored_run(client: TestClient, burglary_case: SyntheticCase) -> None:
    use_service(make_service(burglary_case))
    summary = post_run(client, burglary_case)
    run_id = summary["run_id"]
    assert set(summary["links"].values()) == {f"{BASE}/runs/{run_id}{v}" for v in GET_VIEWS}
    for view in GET_VIEWS:
        response = client.get(f"{BASE}/runs/{run_id}{view}")
        assert response.status_code == 200, view
        body = response.json()
        meta = body["metadata"] if view == "/report" else body
        assert (meta["run_id"], meta["case_id"]) == (run_id, CASE_ID)
        status_key = "run_status" if view == "/report" else "status"
        assert meta[status_key] == summary["status"]
        assert meta["result_sha256"] == summary["result_sha256"]
    for fmt in REPORT_FORMATS:
        assert client.get(f"{BASE}/runs/{run_id}/report", params={"format": fmt}).status_code == 200


@pytest.mark.parametrize("view", GET_VIEWS)
def test_unknown_run_is_404_envelope(client: TestClient, burglary_case: SyntheticCase, view: str) -> None:
    use_service(make_service(burglary_case))
    body = assert_envelope(client.get(f"{BASE}/runs/run-unknown{view}"), 404, "RUN_NOT_FOUND")
    assert body["error"]["details"] is None


@pytest.mark.parametrize("view", GET_VIEWS)
def test_run_of_another_case_is_404(client: TestClient, burglary_case: SyntheticCase, view: str) -> None:
    use_service(make_service(burglary_case))
    run_id = post_run(client, burglary_case)["run_id"]
    assert_envelope(client.get(f"/cases/case-other/reconstruction/runs/{run_id}{view}"), 404, "RUN_NOT_FOUND")


@pytest.mark.parametrize(
    "path",
    [
        "/cases/bad%20case/reconstruction/runs/run-1",
        "/cases/.hidden/reconstruction/runs/run-1/timeline",
        f"{BASE}/runs/{'r' * 200}",
        f"{BASE}/runs/run-1/report?format=pdf",
        f"{BASE}/runs/run-1/report?format=",
    ],
)
def test_invalid_path_or_query_is_422_envelope(client: TestClient, burglary_case: SyntheticCase, path: str) -> None:
    use_service(make_service(burglary_case))
    body = assert_envelope(client.get(path), 422, "REQUEST_VALIDATION_FAILED")
    for item in body["error"]["details"]:
        assert set(item) == {"loc", "msg", "type"}


@pytest.mark.parametrize(
    ("payload", "raw"),
    [
        ("{not json", True),
        ({"evidence": []}, False),
        ({"evidence": [{}], "event_candidates": [], "unexpected": 1}, False),
        ({"event_candidates": []}, False),
    ],
)
def test_bad_post_body_is_422_envelope(client: TestClient, burglary_case: SyntheticCase, payload: Any, raw: bool) -> None:
    use_service(make_service(burglary_case))
    if raw:
        response = client.post(f"{BASE}/run", content=payload, headers={"content-type": "application/json"})
    else:
        response = client.post(f"{BASE}/run", json=payload)
    assert_envelope(response, 422, "REQUEST_VALIDATION_FAILED")


def test_validation_errors_never_echo_submitted_values(client: TestClient, burglary_case: SyntheticCase) -> None:
    use_service(make_service(burglary_case))
    secret_path = "/Users/investigator/private/case-files/evidence.raw"
    body = request_body(burglary_case, unexpected={"path": secret_path})
    response = client.post(f"{BASE}/run", json=body)
    assert_envelope(response, 422, "REQUEST_VALIDATION_FAILED")
    assert secret_path not in response.text


def test_rejected_inputs_are_422_input_rejected(client: TestClient, burglary_case: SyntheticCase) -> None:
    class RejectingService(ReconstructionService):
        def run(self, case_id: str, **kwargs: Any) -> Any:
            raise ValueError("case_id must be a non-empty string.")

    use_service(RejectingService(make_config(burglary_case), InMemoryRunStore()))
    response = client.post(f"{BASE}/run", json=request_body(burglary_case))
    body = assert_envelope(response, 422, "RECONSTRUCTION_INPUT_REJECTED")
    assert body["error"]["details"] is None


def test_unexpected_failure_is_500_envelope_without_details(client: TestClient, burglary_case: SyntheticCase) -> None:
    class BrokenService(ReconstructionService):
        def get(self, case_id: str, run_id: str) -> Any:
            raise RuntimeError(f"disk failure at {burglary_case.storage_root}")

    use_service(BrokenService(make_config(burglary_case), InMemoryRunStore()))
    for view in GET_VIEWS:
        response = client.get(f"{BASE}/runs/run-x{view}")
        body = assert_envelope(response, 500, "INTERNAL_SERVER_ERROR")
        assert body["error"]["details"] is None
        assert str(burglary_case.storage_root) not in response.text


def test_no_absolute_host_paths_in_any_response(client: TestClient, burglary_case: SyntheticCase) -> None:
    use_service(make_service(burglary_case))
    post = client.post(f"{BASE}/run", json=request_body(burglary_case))
    run_id = post.json()["run_id"]
    texts = [post.text]
    texts += [client.get(f"{BASE}/runs/{run_id}{v}").text for v in GET_VIEWS]
    texts += [client.get(f"{BASE}/runs/{run_id}/report", params={"format": f}).text for f in REPORT_FORMATS]
    for variant in host_variants(burglary_case.storage_root, burglary_case.artifact_root, burglary_case.storage_root.parent):
        assert all(variant not in text for text in texts), variant


def test_null_weight_stays_null_everywhere(client: TestClient, burglary_case: SyntheticCase) -> None:
    use_service(make_service(burglary_case))
    run_id = post_run(client, burglary_case)["run_id"]
    for view in ("", "/relationships"):
        (rel,) = client.get(f"{BASE}/runs/{run_id}{view}").json()["relationships"]
        assert "weight" in rel and rel["weight"] is None
    report = client.get(f"{BASE}/runs/{run_id}/report").json()
    (entry,) = report["relationships"]
    assert entry["weight"] is None
    assert entry["weight_display"] == "Not provided"
    markdown = client.get(f"{BASE}/runs/{run_id}/report", params={"format": "markdown"}).text
    (row,) = [line for line in markdown.splitlines() if line.startswith("| R1 |")]
    assert row.split(" | ")[4] == "Not provided"
    html = client.get(f"{BASE}/runs/{run_id}/report", params={"format": "html"}).text
    assert "<td>Not provided</td>" in html
    zero_percent = re.compile(r"(?<![\d.])0(\.0+)?\s?%")
    assert not zero_percent.search(markdown) and not zero_percent.search(html)


def test_openapi_documents_report_endpoint(client: TestClient) -> None:
    schema = client.get("/openapi.json").json()
    operation = schema["paths"]["/cases/{case_id}/reconstruction/runs/{run_id}/report"]["get"]
    (param,) = [p for p in operation["parameters"] if p["in"] == "query"]
    assert param["name"] == "format"
    content = operation["responses"]["200"]["content"]
    assert {"application/json", "text/markdown", "text/html"} <= set(content)
    assert operation["responses"]["404"]["content"]["application/json"]["schema"]["$ref"].endswith("/ErrorEnvelope")
    weight = schema["components"]["schemas"]["RelationshipEntry"]["properties"]["weight"]
    assert {"type": "null"} in weight["anyOf"]
