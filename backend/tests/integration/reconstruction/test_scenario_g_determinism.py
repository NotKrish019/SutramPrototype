"""Scenario G: determinism (same input three times gives identical output)."""

import random
from copy import deepcopy
from datetime import UTC, datetime

from fastapi.testclient import TestClient

from app.modules.reporting.forensic_report import build_forensic_report
from app.modules.reporting.report_render import render_html, render_markdown
from tests.integration.reconstruction.support import (
    BASE,
    FIXED_NOW,
    SyntheticCase,
    make_service,
    post_run,
    run_pipeline,
    use_service,
)


def test_pipeline_three_runs_are_byte_identical(burglary_case: SyntheticCase) -> None:
    results = [run_pipeline(burglary_case) for _ in range(3)]
    assert len({r.run_id for r in results}) == 1
    assert len({r.result_sha256 for r in results}) == 1
    assert len({r.to_snapshot_json() for r in results}) == 1
    assert results[0].result_sha256 == results[0].compute_result_sha256()


def test_input_order_does_not_change_output(burglary_case: SyntheticCase) -> None:
    baseline = run_pipeline(burglary_case)
    rng = random.Random(20261119)
    for _ in range(3):
        evidence, candidates, entities = (deepcopy(x) for x in (burglary_case.evidence, burglary_case.candidates, burglary_case.entities))
        for items in (evidence, candidates, entities):
            rng.shuffle(items)
        shuffled = run_pipeline(burglary_case, evidence=evidence, candidates=candidates, entities=entities)
        assert shuffled.run_id == baseline.run_id
        assert shuffled.to_snapshot_json() == baseline.to_snapshot_json()


def test_failing_inputs_are_deterministic_too(burglary_case: SyntheticCase) -> None:
    candidates = deepcopy(burglary_case.candidates)
    candidates[0]["timestamp_precision"] = "FORTNIGHT"
    candidates.append(deepcopy(candidates[1]))
    snapshots = {run_pipeline(burglary_case, candidates=candidates).to_snapshot_json() for _ in range(3)}
    assert len(snapshots) == 1


def test_api_three_posts_give_identical_results(client: TestClient, burglary_case: SyntheticCase) -> None:
    use_service(make_service(burglary_case))
    summaries = [post_run(client, burglary_case) for _ in range(3)]
    assert all(s == summaries[0] for s in summaries)
    run_id = summaries[0]["run_id"]
    bodies = {client.get(f"{BASE}/runs/{run_id}").text for _ in range(3)}
    assert len(bodies) == 1


def test_report_three_times_is_identical_in_every_format(burglary_case: SyntheticCase) -> None:
    result = run_pipeline(burglary_case)
    reports = [build_forensic_report(run_pipeline(burglary_case), generated_at=FIXED_NOW) for _ in range(3)]
    assert len({r.to_json() for r in reports}) == 1
    assert len({render_markdown(r) for r in reports}) == 1
    assert len({render_html(r) for r in reports}) == 1
    later = build_forensic_report(result, generated_at=datetime(2030, 1, 1, tzinfo=UTC))
    assert later.content_sha256 == reports[0].content_sha256  # generation time is excluded
    assert later.metadata.generated_at_utc == "2030-01-01T00:00:00Z"


def test_report_endpoint_is_deterministic_apart_from_generation_time(
    client: TestClient, burglary_case: SyntheticCase
) -> None:
    use_service(make_service(burglary_case, clock=lambda: datetime.now(UTC)))
    run_id = post_run(client, burglary_case)["run_id"]
    hashes = {client.get(f"{BASE}/runs/{run_id}/report").json()["content_sha256"] for _ in range(3)}
    assert len(hashes) == 1
