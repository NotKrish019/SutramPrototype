"""Phase 3I report content: citations, limitations, null weight, escaping, labels, disclaimer."""

import re
from collections.abc import Iterator
from copy import deepcopy
from html.parser import HTMLParser
from typing import Any

import pytest

from app.modules.reconstruction.pipeline import ReconstructionResult
from app.modules.reporting.forensic_report import (
    DISCLAIMER,
    NOT_PROVIDED,
    FindingBasis,
    ForensicReport,
    build_forensic_report,
    format_weight,
)
from app.modules.reporting.report_render import (
    SECTION_TITLES,
    render_html,
    render_markdown,
)
from tests.integration.reconstruction.support import (
    FIXED_NOW,
    SyntheticCase,
    evidence_path,
    run_pipeline,
)

SHA256 = re.compile(r"^[0-9a-f]{64}$")
ACCUSATORY = re.compile(
    r"\b(guilty|perpetrator|culprit|suspect|offender|proves|proven guilty|identified as|confirmed (identity|match))\b",
    re.IGNORECASE,
)

HOSTILE_DESCRIPTION = (
    '<script>alert("x")</script><img src=x onerror=alert(1)> | cell | [link](https://evil.invalid) '
    "# heading `code` **bold** & ampersand"
)


def _report(result: ReconstructionResult) -> ForensicReport:
    return build_forensic_report(result, generated_at=FIXED_NOW)


def _findings(report: ForensicReport) -> Iterator[tuple[str, Any]]:
    for section in ("timeline", "spatial_observations", "movement_segments", "contradictions", "relationships"):
        for entry in getattr(report, section):
            yield section, entry
    for lead in report.summary.key_leads:
        yield "summary.key_leads", lead


def _scenario_results(case: SyntheticCase) -> dict[str, ReconstructionResult]:
    tampered = deepcopy(case.evidence)
    tampered[1]["sha256_hash"] = "0" * 64
    return {
        "happy": run_pipeline(case),
        "partial": run_pipeline(case, evidence=tampered),
        "empty": run_pipeline(case, candidates=[], entities=[]),
        "failed": run_pipeline(case, candidates="{not json"),
    }


# --- Citations ------------------------------------------------------------------------------


def test_every_finding_cites_hashed_evidence(burglary_case: SyntheticCase) -> None:
    report = _report(run_pipeline(burglary_case))
    inventory = {i.evidence_id: i.sha256 for i in report.evidence_inventory}
    seen = 0
    for section, entry in _findings(report):
        assert entry.citations, f"{section} {entry.ref} has no citation"
        for citation in entry.citations:
            assert citation.evidence_sha256 is not None and SHA256.match(citation.evidence_sha256)
            assert citation.evidence_sha256 == inventory[citation.evidence_id]
            assert citation.event_ids and citation.source_record_ids
        seen += 1
    assert seen == 5 + 4 + 1 + 1 + 1 + 1


def test_lead_citations_cover_both_events(burglary_case: SyntheticCase) -> None:
    report = _report(run_pipeline(burglary_case))
    (lead,) = report.contradictions
    assert {c.evidence_id for c in lead.citations} == {"ev-cctv-dock", "ev-chat-export"}
    assert {e for c in lead.citations for e in c.event_ids} == {lead.origin_event_id, lead.destination_event_id}


def test_rendered_formats_show_sources_for_every_finding(burglary_case: SyntheticCase) -> None:
    report = _report(run_pipeline(burglary_case))
    findings = sum(1 for _ in _findings(report))
    markdown, html = render_markdown(report), render_html(report)
    assert markdown.count("Sources: ") + markdown.count("(SHA-256 ") >= findings
    assert html.count('<div class="cite">Sources: ') == findings
    assert "No citation available" not in markdown + html


# --- Limitations ----------------------------------------------------------------------------


@pytest.mark.parametrize("scenario", ["happy", "partial", "empty", "failed"])
def test_limitations_are_always_present(burglary_case: SyntheticCase, scenario: str) -> None:
    report = _report(_scenario_results(burglary_case)[scenario])
    codes = [lim.code for lim in report.limitations.limitations]
    assert codes[0] == "REPORT_SCOPE"
    assert len(codes) >= 2, codes  # scope plus at least one run-specific limitation
    for text in (render_markdown(report), render_html(report)):
        assert SECTION_TITLES[6] in text
        assert "REPORT_SCOPE" in text


def test_unverified_clocks_and_pending_items_are_listed(burglary_case: SyntheticCase) -> None:
    report = _report(run_pipeline(burglary_case))
    calibrations = {c.evidence_id: c.clock_calibration.value for c in report.limitations.unverified_clock_calibrations}
    assert calibrations == {
        "ev-cctv-dock": "DECLARED_NOT_INDEPENDENTLY_VERIFIED",
        "ev-cdr-tower": "NOT_DECLARED_UNCALIBRATED",
        "ev-chat-export": "DECLARED_NOT_INDEPENDENTLY_VERIFIED",
    }
    assert ("VIDEO_FRAMES_NOT_EXTRACTED", "ev-cctv-dock") in {
        (p.code, p.subject_id) for p in report.limitations.pending_items
    }
    assert {g.gap_type for g in report.limitations.evidence_gaps} >= {"EVENT_WITHOUT_COORDINATES"}


# --- Weight, labels, claims ----------------------------------------------------------------


def test_null_weight_is_not_provided_never_zero() -> None:
    assert format_weight(None) == NOT_PROVIDED == "Not provided"
    assert format_weight(0.0) == "0.00"
    assert format_weight(0.75) == "0.75"


def test_null_weight_in_report(burglary_case: SyntheticCase) -> None:
    report = _report(run_pipeline(burglary_case))
    (rel,) = report.relationships
    assert rel.weight is None and rel.weight_display == "Not provided"
    assert '"weight": null' in report.to_json()


def test_inferred_items_are_labelled(burglary_case: SyntheticCase) -> None:
    report = _report(run_pipeline(burglary_case))
    assert all(t.basis == FindingBasis.OBSERVED for t in report.timeline)
    assert all(s.basis == FindingBasis.OBSERVED for s in report.spatial_observations)
    assert all(m.basis == FindingBasis.INFERRED for m in report.movement_segments)
    assert all(r.basis == FindingBasis.INFERRED and not r.is_observed for r in report.relationships)
    for lead in report.contradictions:
        assert lead.basis == FindingBasis.INFERRED
        assert lead.classification == "ANALYTICAL_LEAD_NOT_A_FINDING_OF_FACT"
    html, markdown = render_html(report), render_markdown(report)
    assert html.count('<span class="badge LEAD">ANALYTICAL LEAD</span>') == 2  # summary + section 5
    assert "ANALYTICAL LEAD (inferred, not proven)" in markdown
    assert "not a proven fact" in report.contradiction_notice


def test_no_claim_of_guilt_or_identity(burglary_case: SyntheticCase) -> None:
    report = _report(run_pipeline(burglary_case))
    names = {e["canonical_name"] for e in burglary_case.entities} | {
        a for e in burglary_case.entities for a in e["aliases"]
    }
    for text in (report.to_json(), render_markdown(report), render_html(report)):
        assert not ACCUSATORY.search(text), ACCUSATORY.search(text)
        for name in names:
            assert name not in text, name


def test_disclaimer_is_prominent_in_every_format(burglary_case: SyntheticCase) -> None:
    report = _report(run_pipeline(burglary_case))
    assert report.disclaimer == DISCLAIMER
    assert DISCLAIMER.startswith("DECISION SUPPORT FOR INVESTIGATORS.")
    markdown, html = render_markdown(report), render_html(report)
    assert markdown.splitlines()[2].startswith("> **Disclaimer.** DECISION SUPPORT FOR INVESTIGATORS.")
    assert markdown.rstrip().endswith("relied upon.")
    assert html.count("DECISION SUPPORT FOR INVESTIGATORS.") == 2  # banner and footer


def test_run_metadata(burglary_case: SyntheticCase) -> None:
    result = run_pipeline(burglary_case)
    meta = _report(result).metadata
    assert (meta.case_id, meta.run_id, meta.result_sha256) == (result.case_id, result.run_id, result.result_sha256)
    assert meta.algorithm_version == result.algorithm_version
    assert meta.configuration_version == result.configuration_version
    assert meta.generated_at_utc == "2026-11-19T12:00:00Z"
    with pytest.raises(ValueError, match="timezone-aware"):
        build_forensic_report(result, generated_at=FIXED_NOW.replace(tzinfo=None))


def test_evidence_inventory_and_custody(burglary_case: SyntheticCase) -> None:
    evidence_path(burglary_case.storage_root, "ev-cdr-tower").write_bytes(b"changed")
    report = _report(run_pipeline(burglary_case))
    status = {i.evidence_id: i.integrity_status.value for i in report.evidence_inventory}
    assert status == {"ev-cctv-dock": "VERIFIED", "ev-cdr-tower": "SHA256_MISMATCH", "ev-chat-export": "VERIFIED"}
    custody = {e.evidence_id: e for e in report.chain_of_custody.evidence}
    assert custody["ev-cdr-tower"].sha256 == burglary_case.evidence[1]["sha256_hash"]
    assert custody["ev-cctv-dock"].storage_reference == "urn:yukti:evidence:ev-cctv-dock:storage"
    assert "artifact_relative_path" not in report.to_json()
    assert report.chain_of_custody.finding_provenance


# --- Escaping -------------------------------------------------------------------------------


class _TagCollector(HTMLParser):
    def __init__(self) -> None:
        super().__init__()
        self.tags: list[str] = []

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        self.tags.append(tag)


def _hostile_report(case: SyntheticCase) -> ForensicReport:
    candidates = deepcopy(case.candidates)
    candidates[0]["description"] = HOSTILE_DESCRIPTION
    candidates[0]["event_type"] = "<b>FORCED</b>"
    candidates[0]["source_record_id"] = "osd:<i>21:42</i>|cam"
    candidates[3]["involved_entity_raw_ids"] = ["ent-person-ravi", "<svg/onload=alert(1)>"]
    return _report(run_pipeline(case, candidates=candidates))


def test_html_escapes_evidence_text(burglary_case: SyntheticCase) -> None:
    html = render_html(_hostile_report(burglary_case))
    collector = _TagCollector()
    collector.feed(html)
    assert not {"script", "img", "svg", "b", "i", "a", "iframe"} & set(collector.tags)
    assert "&lt;script&gt;alert(&quot;x&quot;)&lt;/script&gt;" in html
    assert "&lt;svg/onload=alert(1)&gt;" in html
    assert "&lt;b&gt;FORCED&lt;/b&gt;" in html
    assert "default-src 'none'" in html


def test_markdown_escapes_evidence_text(burglary_case: SyntheticCase) -> None:
    markdown = render_markdown(_hostile_report(burglary_case))
    # Code spans are rendered literally by CommonMark; everything outside them must be escaped.
    outside_code = re.sub(r"`[^`]*`", "", markdown)
    assert not re.search(r"<[A-Za-z/]", outside_code)
    assert "`<svg/onload=alert(1)>`" in markdown and "`osd:<i>21:42</i>|cam`" in markdown
    assert "&lt;script&gt;" in markdown
    assert r"\| cell \|" in markdown
    assert r"\[link\](https://evil.invalid)" in markdown
    assert not any(line.startswith("# heading") for line in markdown.splitlines())
    # Tables keep their shape even when a cited record id contains a pipe.
    rows = [line for line in markdown.splitlines() if line.startswith(("| S1 |", "| R1 |"))]
    assert len(rows) == 2
    for line in rows:
        assert len(re.findall(r"(?<!\\)\|", line)) == 7, line


def test_json_report_keeps_raw_text_unmodified(burglary_case: SyntheticCase) -> None:
    report = _hostile_report(burglary_case)
    event = next(t for t in report.timeline if t.event_id == "evt-01-shutter-forced")
    assert event.description == HOSTILE_DESCRIPTION  # JSON is data; escaping is the renderer's job
