"""Generate the sample forensic report of the synthetic burglary case into ``docs/demo/``.

Usage (from ``backend/``)::

    uv run python -m tests.integration.reconstruction.demo_report

The case is fully synthetic. The demo uses placeholder evidence bytes, so frame extraction is
skipped and the run is reproducible on any machine. ``test_report_demo.py`` fails when the
committed files no longer match the code.
"""

import json
import tempfile
from datetime import UTC, datetime
from pathlib import Path

from app.modules.reconstruction.adapters.contracts import PROPOSED_CONTRACT_DECISIONS
from app.modules.reconstruction.pipeline import (
    ReconstructionPipeline,
    ReconstructionPipelineConfig,
    ReconstructionResult,
)
from app.modules.reporting.forensic_report import ForensicReport, build_forensic_report
from app.modules.reporting.report_render import render_html, render_markdown
from tests.integration.reconstruction.support import CASE_ID, build_case

DEMO_DIR = Path(__file__).resolve().parents[4] / "docs" / "demo"
DEMO_STEM = "forensic-report-warehouse-burglary"


def demo_paths() -> dict[str, Path]:
    return {ext: DEMO_DIR / f"{DEMO_STEM}.{ext}" for ext in ("json", "md", "html")}


def build_demo_result() -> ReconstructionResult:
    """Burglary run with the PROPOSED decisions enabled explicitly.

    No decision is team-approved, so the default configuration would reconstruct nothing. The
    report lists every decision as "enabled without verified team sign-off (proposal only)".
    """
    with tempfile.TemporaryDirectory() as tmp:
        case = build_case(Path(tmp))
        pipeline = ReconstructionPipeline(
            ReconstructionPipelineConfig(
                storage_root=case.storage_root,
                approved_contract_decisions=PROPOSED_CONTRACT_DECISIONS,
            )
        )
        return pipeline.run(
            case_id=CASE_ID, evidence=case.evidence, event_candidates=case.candidates, entities=case.entities
        )


def render_demo(generated_at: datetime) -> dict[str, str]:
    report: ForensicReport = build_forensic_report(build_demo_result(), generated_at=generated_at)
    return {"json": report.to_json() + "\n", "md": render_markdown(report), "html": render_html(report)}


def committed_generated_at() -> datetime:
    data = json.loads(demo_paths()["json"].read_text(encoding="utf-8"))
    return datetime.fromisoformat(data["metadata"]["generated_at_utc"])


def main() -> None:
    DEMO_DIR.mkdir(parents=True, exist_ok=True)
    outputs = render_demo(datetime.now(UTC).replace(microsecond=0))
    for ext, path in demo_paths().items():
        path.write_text(outputs[ext], encoding="utf-8")
        print(f"wrote {path.relative_to(DEMO_DIR.parents[1])}")


if __name__ == "__main__":
    main()
