"""Performance check: 5,000-event synthetic case through the pipeline, the API and the report.

Timings are printed (run with ``-s`` to see them) and checked against generous budgets, so the
test catches regressions of an order of magnitude without being flaky on slow CI machines.
Override the size with ``YUKTI_PERF_EVENTS``.
"""

import json
import math
import os
import time
from collections.abc import Callable
from typing import Any

from fastapi.testclient import TestClient

from app.modules.reporting.forensic_report import build_forensic_report
from app.modules.reporting.report_render import render_html, render_markdown
from tests.integration.reconstruction.support import (
    BASE,
    FIXED_NOW,
    SyntheticCase,
    make_service,
    request_body,
    run_pipeline,
    synthetic_candidate,
    use_service,
)

EVENTS = int(os.environ.get("YUKTI_PERF_EVENTS", "5000"))
ENTITIES = ("ent-person-ravi", "ent-phone-01", "ent-vehicle-van")
SOURCES = (("ev-cdr-tower", "Z"), ("ev-chat-export", "+05:30"), ("ev-cctv-dock", ""))
JUMP_EVERY = 250  # one implausible jump per 250 events gives a realistic number of leads

# Budgets in seconds (about 10x the timings measured on a 2024 laptop).
BUDGETS = {
    "pipeline": 30.0,
    "report_build": 15.0,
    "render_markdown": 15.0,
    "render_html": 15.0,
    "api_post": 45.0,
    "api_get_report_html": 45.0,
}


def perf_candidates(n: int) -> list[dict[str, Any]]:
    """Entities walk slowly (~6 km/h) with an occasional jump; times are 12 s apart overall."""
    out: list[dict[str, Any]] = []
    for i in range(n):
        evidence_id, suffix = SOURCES[i % 3]
        entity = ENTITIES[i % 3]
        step = i // 3
        lat = 28.50 + 0.0001 * step + (0.3 if (step // JUMP_EVERY) % 2 and step % JUMP_EVERY == 0 else 0.0)
        lon = 77.20 + 0.05 * math.sin(step / 500) + 0.01 * (i % 3)
        utc_seconds = 10 * 3600 + 12 * i
        local_seconds = utc_seconds + (19800 if suffix != "Z" else 0) + (120 if evidence_id == "ev-cctv-dock" else 0)
        day, rem = divmod(local_seconds, 86400)
        stamp = f"2026-11-{10 + day:02d}T{rem // 3600:02d}:{rem // 60 % 60:02d}:{rem % 60:02d}{suffix}"
        out.append(
            synthetic_candidate(
                f"perf-{i:05d}",
                evidence_id,
                stamp,
                entity_ids=[entity],
                coordinates={"lat": lat, "lon": lon},
                description=f"Synthetic performance event {i}",
            )
        )
    return out


def _timed[T](timings: dict[str, float], name: str, fn: Callable[[], T]) -> T:
    start = time.perf_counter()
    value = fn()
    timings[name] = time.perf_counter() - start
    return value


def test_large_case_performance(client: TestClient, burglary_case: SyntheticCase) -> None:
    candidates = perf_candidates(EVENTS)
    timings: dict[str, float] = {}

    result = _timed(timings, "pipeline", lambda: run_pipeline(burglary_case, candidates=candidates))
    assert len(result.timeline_events) == EVENTS
    assert result.excluded_inputs == []
    snapshot = _timed(timings, "snapshot_json", result.to_snapshot_json)
    report = _timed(timings, "report_build", lambda: build_forensic_report(result, generated_at=FIXED_NOW))
    report_json = _timed(timings, "report_json", report.to_json)
    markdown = _timed(timings, "render_markdown", lambda: render_markdown(report))
    html = _timed(timings, "render_html", lambda: render_html(report))
    assert len(report.timeline) == EVENTS
    assert all(entry.citations for entry in report.timeline)

    use_service(make_service(burglary_case))
    body = request_body(burglary_case, event_candidates=candidates)
    post = _timed(timings, "api_post", lambda: client.post(f"{BASE}/run", json=body))
    assert post.status_code == 201
    run_id = post.json()["run_id"]
    full = _timed(timings, "api_get_full_result", lambda: client.get(f"{BASE}/runs/{run_id}"))
    assert full.status_code == 200
    page = _timed(
        timings, "api_get_report_html", lambda: client.get(f"{BASE}/runs/{run_id}/report", params={"format": "html"})
    )
    assert page.status_code == 200

    sizes_kib = {
        "snapshot_json": len(snapshot) // 1024,
        "report_json": len(report_json) // 1024,
        "report_markdown": len(markdown) // 1024,
        "report_html": len(html) // 1024,
        "api_full_result": len(full.content) // 1024,
    }
    counts = {
        "events": len(result.timeline_events),
        "movement_segments": len(result.movement_segments),
        "contradiction_leads": len(result.contradiction_leads),
        "relationships": len(result.relationships),
    }
    print("\nPERF " + json.dumps({"timings_s": {k: round(v, 3) for k, v in timings.items()}, "sizes_kib": sizes_kib, "counts": counts}))

    over = {k: round(timings[k], 2) for k, budget in BUDGETS.items() if timings[k] > budget}
    assert not over, f"over budget: {over}"
