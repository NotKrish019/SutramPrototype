"""Markdown and print-ready HTML renderings of a ``ForensicReport`` (Member 4 — Phase 3I).

Both renderers read only the structured ``ForensicReport``, so the JSON, Markdown and HTML
formats always carry the same content. Every value that comes from evidence or extraction
(descriptions, ids, diagnostic messages) is escaped:

- HTML: ``html.escape`` on every interpolated value. The page has no scripts and declares a
  Content-Security-Policy that blocks scripts and remote resources. Browsers can print it to PDF.
- Markdown: identifiers and codes go into code spans; free text is escaped so it cannot inject
  HTML, links, tables or headings.
"""

import html
import re
from collections.abc import Callable, Iterable, Sequence

from app.modules.reporting.forensic_report import (
    NOT_PROVIDED,
    Citation,
    ForensicReport,
)
from app.schemas.reconstruction import GeoPoint

SECTION_TITLES = (
    "1. Case summary and run metadata",
    "2. Evidence inventory",
    "3. Timeline of events",
    "4. Movement and spatial analysis",
    "5. Contradictions and inconsistencies (analytical leads)",
    "6. Relationships (observed vs inferred)",
    "7. Limitations, evidence gaps and pending items",
    "Appendix A. Chain of custody and provenance",
)

# --- Shared formatting ---------------------------------------------------------------------


def _short_sha(value: str | None) -> str:
    if not value:
        return NOT_PROVIDED
    return f"{value[:12]}…{value[-4:]}" if len(value) > 20 else value


def _num(value: float | None, fmt: str = "{:.2f}", unit: str = "") -> str:
    if value is None:
        return NOT_PROVIDED
    return fmt.format(value) + unit


def _opt(value: object | None) -> str:
    if value is None or value == "" or value == []:
        return NOT_PROVIDED
    return str(value)


def _coords(point: GeoPoint | None) -> str:
    return NOT_PROVIDED if point is None else f"{point.lat:.5f}, {point.lon:.5f}"


def _window(start: str | None, end: str | None) -> str:
    if start is None or end is None:
        return NOT_PROVIDED
    return f"[{start}, {end})"


# --- Markdown ------------------------------------------------------------------------------

_MD_SPECIAL = re.compile(r"([\\`*_\[\]#|~])")


def md_text(value: object) -> str:
    """Escape free text for Markdown: no HTML, links, emphasis, tables or headings."""
    text = " ".join(str(value).split())
    text = text.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")
    return _MD_SPECIAL.sub(r"\\\1", text)


def md_code(value: object) -> str:
    """Identifier or code in a code span (contents are never interpreted)."""
    text = " ".join(str(value).split()).replace("`", "'")
    return f"`{text}`" if text else "`-`"


_UNESCAPED_PIPE = re.compile(r"(?<!\\)\|")


def _md_cell(cell: str) -> str:
    """GFM splits table cells on '|' even inside code spans, so every pipe in a cell is escaped."""
    return _UNESCAPED_PIPE.sub(r"\\|", cell)


def _md_table(headers: Sequence[str], rows: Iterable[Sequence[str]]) -> list[str]:
    lines = ["| " + " | ".join(headers) + " |", "|" + "|".join(" --- " for _ in headers) + "|"]
    lines.extend("| " + " | ".join(_md_cell(cell) for cell in row) + " |" for row in rows)
    return lines


def _md_citations(citations: Sequence[Citation]) -> str:
    if not citations:
        return "**No citation available**"
    parts = []
    for c in citations:
        piece = f"{md_code(c.evidence_id)} (SHA-256 {md_code(_short_sha(c.evidence_sha256))}"
        if c.source_record_ids:
            piece += "; records " + ", ".join(md_code(r) for r in c.source_record_ids)
        if c.frame_artifact_ids:
            piece += "; frames captured during window " + ", ".join(md_code(f) for f in c.frame_artifact_ids)
        parts.append(piece + ")")
    return "; ".join(parts)


def render_markdown(report: ForensicReport) -> str:
    m = report.metadata
    out: list[str] = [f"# {md_text(report.title)}", ""]
    out += [f"> **Disclaimer.** {md_text(report.disclaimer)}", ""]

    out += [f"## {SECTION_TITLES[0]}", ""]
    out += _md_table(
        ["Field", "Value"],
        [
            ["Case id", md_code(m.case_id)],
            ["Run id", md_code(m.run_id)],
            ["Run status", f"{md_code(m.run_status.value)}: {md_text(m.run_status_explanation)}"],
            ["Algorithm version", md_code(m.algorithm_version)],
            ["Configuration version", md_code(m.configuration_version)],
            ["Result SHA-256", md_code(m.result_sha256)],
            ["Report generated (UTC)", md_code(m.generated_at_utc)],
            ["Report generator", md_code(m.report_generator_version)],
            ["Relationship export", md_code(m.relationship_export_status.value)],
            ["Contract decisions enabled for this run", ", ".join(md_code(d) for d in m.approved_contract_decisions) or NOT_PROVIDED],
            ["Enabled without verified team sign-off (proposal only)", ", ".join(md_code(d) for d in m.unverified_contract_decisions) or "None"],
            ["Withheld contract decisions", ", ".join(md_code(d) for d in m.withheld_contract_decisions) or "None"],
            ["Reference speed threshold", _num(m.configured_speed_threshold_kmh, "{:g}", " km/h")],
            ["Report content SHA-256", md_code(report.content_sha256)],
        ],
    )
    s = report.summary
    out += ["", "**Counts.** " + ", ".join(f"{md_text(k.replace('_', ' '))}: {v}" for k, v in s.counts.items()), ""]
    out += [f"**Timeline span (UTC).** {md_code(_window(s.timeline_span_start_utc, s.timeline_span_end_utc))}", ""]
    if s.key_leads:
        out += ["**Analytical leads for review (inferred, not findings of fact):**", ""]
        for lead in s.key_leads:
            out.append(f"- **{lead.ref}** {md_text(lead.statement)} Sources: {_md_citations(lead.citations)}")
        out.append("")
    else:
        out += ["No contradiction leads were produced in this run.", ""]
    out += [f"*{md_text(s.interpretation_notice)}*", ""]

    out += [f"## {SECTION_TITLES[1]}", ""]
    out += _md_table(
        ["Evidence", "Type", "SHA-256", "Integrity", "Used", "Clock calibration", "Events"],
        [
            [
                md_code(i.evidence_id),
                md_text(_opt(i.source_type)),
                md_code(i.sha256 or NOT_PROVIDED),
                f"{md_code(i.integrity_status.value)} {md_text(i.integrity_note)}",
                "Yes" if i.usable else f"No ({md_code(i.exclusion_reason_code or '-')})",
                md_code(i.clock_calibration.value) if i.clock_calibration else NOT_PROVIDED,
                str(i.timeline_events_cited),
            ]
            for i in report.evidence_inventory
        ],
    )
    out.append("")

    out += [f"## {SECTION_TITLES[2]}", "", f"*{md_text(report.frame_notice)}*", ""]
    if not report.timeline:
        out += ["No events were accepted into the timeline.", ""]
    for e in report.timeline:
        out += [f"### {e.ref}. {md_text(e.event_type)}: {md_text(e.description)}", ""]
        out += [
            f"- Basis: **{e.basis.value}**. {md_text(e.basis_note)}",
            f"- Event id: {md_code(e.event_id)}",
            f"- Raw timestamp: {md_code(e.raw_timestamp)} (precision {md_code(e.raw_precision)})",
            f"- Corrected UTC: {md_code(e.timestamp_utc)} (precision {md_code(e.precision)})",
            f"- Uncertainty window: {md_code(_window(e.uncertainty_window_start_utc, e.uncertainty_window_end_utc))}"
            + (" (estimated window assumption)" if e.is_estimated_window_assumption else ""),
            (f"- Time basis: {md_code(e.timezone_basis)}; source timezone {md_code(_opt(e.source_timezone))}; "
            f"clock offset {md_code(_num(e.clock_offset_seconds, '{:+g}', ' s'))}; calibration {md_code(e.clock_calibration.value)}"),
            f"- Ordering: {md_code(e.ordering_status)}"
            + (f" (concurrent with {', '.join(md_code(c) for c in e.concurrent_event_ids)})" if e.concurrent_event_ids else ""),
            f"- Entity references: {', '.join(md_code(x) for x in e.involved_entity_ids) or 'None resolved'}"
            + (f"; unresolved {', '.join(md_code(x) for x in e.unresolved_entity_references)}" if e.unresolved_entity_references else ""),
            f"- Coordinates: {md_code(_coords(e.coordinates))}",
            f"- Frames captured during this window: {', '.join(md_code(f) for f in e.frame_artifact_ids) or 'None'}",
            f"- Sources: {_md_citations(e.citations)}",
            "",
        ]

    out += [f"## {SECTION_TITLES[3]}", ""]
    out += ["### Spatial observations (observed)", ""]
    if report.spatial_observations:
        out += _md_table(
            ["Ref", "Entity reference", "Event", "Coordinates", "Window (UTC)", "Sources"],
            [
                [
                    o.ref,
                    md_code(o.entity_id),
                    md_code(o.event_id),
                    md_code(_coords(o.coordinates)),
                    md_code(_window(o.uncertainty_window_start_utc, o.uncertainty_window_end_utc)),
                    _md_citations(o.citations),
                ]
                for o in report.spatial_observations
            ],
        )
    else:
        out.append("No located observations. No locations were inferred.")
    out += ["", "### Movement segments and travel feasibility (inferred)", ""]
    if not report.movement_segments:
        out += ["No movement segments were built.", ""]
    for seg in report.movement_segments:
        out += [
            (f"#### {seg.ref}. {md_code(seg.origin_event_id)} → {md_code(seg.destination_event_id)} "
            f"(entity reference {md_code(seg.entity_id)})"),
            "",
            f"- Basis: **{seg.basis.value}**",
            f"- Straight-line distance: {_num(seg.distance_km, '{:.2f}', ' km')}",
            (f"- Elapsed time: nominal {_num(seg.nominal_elapsed_seconds, '{:g}', ' s')}, possible range "
            f"{_num(seg.min_possible_elapsed_seconds, '{:g}', ' s')} to {_num(seg.max_possible_elapsed_seconds, '{:g}', ' s')}"),
            (f"- Minimum required speed: {_num(seg.min_required_speed_kmh, '{:.1f}', ' km/h')} "
            f"(reference {seg.configured_speed_threshold_kmh:g} km/h) → {md_code(seg.threshold_evaluation.value)}"),
            f"- Feasibility: {md_text(seg.feasibility_statement)}",
            f"- Assumption: {md_text(seg.threshold_assumption_note)}",
        ]
        if seg.continuity_note:
            out.append(f"- Continuity: {md_text(seg.continuity_note)}")
        out += [f"- Sources: {_md_citations(seg.citations)}", ""]

    out += [f"## {SECTION_TITLES[4]}", "", f"> {md_text(report.contradiction_notice)}", ""]
    if not report.contradictions:
        out += ["No contradiction leads were produced in this run.", ""]
    for c in report.contradictions:
        out += [
            f"### {c.ref}. ANALYTICAL LEAD (inferred, not proven): {md_code(c.lead_type)}",
            "",
            f"- Lead id: {md_code(c.lead_id)}",
            f"- Entity reference: {md_code(c.entity_id)}",
            (f"- Events: {md_code(c.origin_event_id)} {md_code(' '.join(c.origin_window_utc))} → "
            f"{md_code(c.destination_event_id)} {md_code(' '.join(c.destination_window_utc))}"),
            (f"- Derived: distance {_num(c.distance_km, '{:.2f}', ' km')}, longest possible elapsed "
            f"{_num(c.max_possible_elapsed_seconds, '{:g}', ' s')}, minimum required speed "
            f"{_num(c.min_required_speed_kmh, '{:.1f}', ' km/h')} vs reference {c.configured_speed_threshold_kmh:g} km/h "
            f"({md_code(_opt(c.threshold_evaluation))})"),
            f"- Explanation: {md_text(c.explanation)}",
            f"- Exported relationship: {md_code(_opt(c.relationship_id))}",
        ]
        out += [f"- Lead limitation: {md_text(lim)}" for lim in c.lead_limitations]
        out += [f"- Sources: {_md_citations(c.citations)}", ""]
    if report.contradiction_pairs_not_evaluated:
        out += ["**Pairs not evaluated (order unknown):**", ""]
        out += [
            f"- {md_code(p.entity_id)}: {md_code(p.event_a_id)} / {md_code(p.event_b_id)}. {md_text(p.note)}"
            for p in report.contradiction_pairs_not_evaluated
        ]
        out.append("")

    out += [f"## {SECTION_TITLES[5]}", ""]
    if report.relationships:
        out += _md_table(
            ["Ref", "Type", "Source → target", "Basis", "Weight", "Sources"],
            [
                [
                    r.ref,
                    md_code(r.relation_type),
                    f"{md_code(r.source_id)} → {md_code(r.target_id)}",
                    f"**{r.basis.value}** (is_observed {str(r.is_observed).lower()})",
                    md_text(r.weight_display),
                    _md_citations(r.citations),
                ]
                for r in report.relationships
            ],
        )
    else:
        out.append(f"No relationships were exported (status {md_code(m.relationship_export_status.value)}).")
    out.append("")

    lim = report.limitations
    out += [f"## {SECTION_TITLES[6]}", "", "### Limitations", ""]
    out += [
        f"- {md_code(x.code)} {md_text(x.message)}"
        + (f" Applies to: {', '.join(md_code(s) for s in x.subject_ids)}." if x.subject_ids else "")
        for x in lim.limitations
    ]
    out += ["", "### Evidence gaps", ""]
    out += [
        f"- {md_code(g.gap_type)} {md_code(g.subject_id)}: {md_text(g.description)}"
        + (f" ({md_code(g.reason_code)})" if g.reason_code else "")
        for g in lim.evidence_gaps
    ] or ["- None recorded."]
    out += ["", "### Clock calibrations (none independently verified)", ""]
    out += [
        f"- {md_code(c.evidence_id)}: {md_code(c.clock_calibration.value)}, offset "
        f"{md_code(_num(c.clock_offset_seconds, '{:+g}', ' s'))}, timezone {md_code(_opt(c.source_timezone))}. {md_text(c.note)}"
        for c in lim.unverified_clock_calibrations
    ] or ["- No usable evidence."]
    out += ["", "### Pending items", ""]
    out += [
        f"- {md_code(p.code)}" + (f" {md_code(p.subject_id)}" if p.subject_id else "") + f": {md_text(p.description)}"
        for p in lim.pending_items
    ] or ["- None."]
    out += ["", "### Excluded inputs", ""]
    out += [
        f"- {md_code(x['subject_type'])} {md_code(x['subject_id'])}: {md_code(x['reason_code'])}"
        for x in lim.excluded_inputs
    ] or ["- None."]
    out += ["", "### Errors and warnings", ""]
    out += [
        f"- {md_code(d.severity)} {md_code(d.code)}" + (f" {md_code(d.subject_id)}" if d.subject_id else "")
        + f": {md_text(d.message)}"
        for d in lim.diagnostics
    ] or ["- None."]
    out.append("")

    cu = report.chain_of_custody
    out += [f"## {SECTION_TITLES[7]}", "", md_text(cu.statement), "", "### Evidence", ""]
    out += _md_table(
        ["Evidence", "SHA-256", "Storage reference", "Integrity", "Video before / after processing"],
        [
            [
                md_code(e.evidence_id),
                md_code(e.sha256 or NOT_PROVIDED),
                md_code(_opt(e.storage_reference)),
                md_code(e.integrity_status.value),
                (
                    f"{md_code(_short_sha(e.video_sha256_before_processing))} / "
                    f"{md_code(_short_sha(e.video_sha256_after_processing))}"
                    + (" (unchanged)" if e.video_source_unchanged else "")
                )
                if e.video_sha256_before_processing
                else "n/a",
            ]
            for e in cu.evidence
        ],
    )
    out += ["", "### Video processing", ""]
    out += [
        f"- {md_code(v.evidence_id)}: {md_code(v.status)}, {md_code(v.processing_kind)}, handoff "
        f"{md_code(_opt(v.frame_handoff_status))}, {v.frames_extracted} frame(s). {md_text(v.processing_note)}"
        for v in cu.video_processing
    ] or ["- No video evidence processed."]
    out += ["", "### Frame artifacts", ""]
    if cu.frame_artifacts:
        out += _md_table(
            ["Artifact", "Source", "Source video SHA-256", "Index", "PTS (s)", "UTC", "Frame SHA-256"],
            [
                [
                    md_code(f.artifact_reference),
                    md_code(f.source_evidence_id),
                    md_code(_short_sha(f.source_video_sha256)),
                    str(f.frame_index),
                    f"{f.pts_offset_seconds:g}",
                    md_code(_opt(f.timestamp_utc)),
                    md_code(_short_sha(f.sha256)),
                ]
                for f in cu.frame_artifacts
            ],
        )
    else:
        out.append("- None.")
    out += ["", "### Finding provenance", ""]
    out += _md_table(
        ["Finding", "Type", "Observed", "Evidence (SHA-256)", "Source records"],
        [
            [
                md_code(p.finding_id),
                md_code(p.finding_type.value),
                "observed" if p.is_observed else "inferred",
                ", ".join(f"{md_code(k)} {md_code(_short_sha(v))}" for k, v in sorted(p.evidence_sha256.items())) or NOT_PROVIDED,
                ", ".join(md_code(r) for r in p.source_record_ids) or NOT_PROVIDED,
            ]
            for p in cu.finding_provenance
        ],
    )
    out += ["", f"*{md_text(cu.run_interpretation_notice)}*", ""]
    out += ["---", "", f"> {md_text(report.disclaimer)}", ""]
    return "\n".join(out)


# --- HTML ----------------------------------------------------------------------------------


def h(value: object) -> str:
    """HTML-escape any value for text or attribute context."""
    return html.escape(str(value), quote=True)


def _code(value: object) -> str:
    return f"<code>{h(value)}</code>"


_CSS = """
:root { --ink:#1b1f24; --muted:#5b6470; --line:#d0d5dc; --soft:#f4f6f8; --lead:#8a1c1c; --lead-bg:#fbeeee;
  --inf:#6b4e00; --inf-bg:#fff6dc; --obs:#0f5132; --obs-bg:#e7f4ec; }
* { box-sizing: border-box; }
body { margin: 0; background: #fff; color: var(--ink); font: 10.5pt/1.45 "Segoe UI", Helvetica, Arial, sans-serif; }
main { max-width: 1100px; margin: 0 auto; padding: 24px 16px 48px; }
h1 { font-size: 19pt; margin: 0 0 4px; } h2 { font-size: 14pt; margin: 28px 0 8px; border-bottom: 2px solid var(--ink); padding-bottom: 3px; }
h3 { font-size: 11.5pt; margin: 18px 0 6px; } h4 { font-size: 10.5pt; margin: 12px 0 4px; }
.disclaimer { border: 2px solid var(--ink); padding: 10px 12px; margin: 12px 0 16px; background: var(--soft); font-weight: 600; }
.notice { border-left: 4px solid var(--lead); background: var(--lead-bg); padding: 8px 10px; margin: 8px 0; }
.muted { color: var(--muted); }
table { width: 100%; border-collapse: collapse; margin: 6px 0 12px; table-layout: auto; }
th, td { border: 1px solid var(--line); padding: 4px 6px; vertical-align: top; text-align: left; overflow-wrap: anywhere; }
th { background: var(--soft); font-weight: 600; overflow-wrap: normal; word-break: keep-all; hyphens: none; }
code { font: 9pt/1.3 ui-monospace, Menlo, Consolas, monospace; overflow-wrap: anywhere; }
.badge { display: inline-block; padding: 0 6px; border-radius: 3px; font-size: 8.5pt; font-weight: 700; letter-spacing: .02em; }
.OBSERVED { background: var(--obs-bg); color: var(--obs); } .INFERRED { background: var(--inf-bg); color: var(--inf); }
.LEAD { background: var(--lead-bg); color: var(--lead); }
.finding { border: 1px solid var(--line); padding: 8px 10px; margin: 8px 0; break-inside: avoid; page-break-inside: avoid; }
.finding.lead { border: 1px dashed var(--lead); }
.cite { font-size: 9pt; color: var(--muted); margin-top: 4px; }
dl { display: grid; grid-template-columns: max-content 1fr; gap: 2px 12px; margin: 4px 0; }
dt { color: var(--muted); } dd { margin: 0; min-width: 0; overflow-wrap: anywhere; }
h1, p { overflow-wrap: anywhere; }
footer { margin-top: 32px; border-top: 1px solid var(--line); padding-top: 8px; font-size: 9pt; color: var(--muted); }
@page { size: A4; margin: 16mm 14mm; }
@media print {
  body { font-size: 9.5pt; } main { max-width: none; padding: 0; }
  h2 { break-before: page; page-break-before: always; } h2.first { break-before: auto; page-break-before: auto; }
  tr, .finding { break-inside: avoid; page-break-inside: avoid; } thead { display: table-header-group; }
  .badge, .notice, .disclaimer, th { -webkit-print-color-adjust: exact; print-color-adjust: exact; }
}
@media (max-width: 640px) { dl { grid-template-columns: 1fr; } table { font-size: 9pt; } }
"""


def _badge(label: str, cls: str | None = None) -> str:
    return f'<span class="badge {h(cls or label)}">{h(label)}</span>'


def _html_table(headers: Sequence[str], rows: Iterable[Sequence[str]]) -> str:
    head = "".join(f"<th>{h(x)}</th>" for x in headers)
    body = "".join("<tr>" + "".join(f"<td>{cell}</td>" for cell in row) + "</tr>" for row in rows)
    return f"<table><thead><tr>{head}</tr></thead><tbody>{body}</tbody></table>"


def _html_citations(citations: Sequence[Citation]) -> str:
    if not citations:
        return '<div class="cite"><strong>No citation available</strong></div>'
    parts = []
    for c in citations:
        piece = f"{_code(c.evidence_id)} SHA-256 {_code(_short_sha(c.evidence_sha256))}"
        if c.source_record_ids:
            piece += "; records " + ", ".join(_code(r) for r in c.source_record_ids)
        if c.frame_artifact_ids:
            piece += "; frames captured during window " + ", ".join(_code(f) for f in c.frame_artifact_ids)
        parts.append(piece)
    return '<div class="cite">Sources: ' + " · ".join(parts) + "</div>"


def _dl(items: Sequence[tuple[str, str]]) -> str:
    return "<dl>" + "".join(f"<dt>{h(k)}</dt><dd>{v}</dd>" for k, v in items) + "</dl>"


def _list(items: Sequence[str], empty: str) -> str:
    if not items:
        return f"<p class=\"muted\">{h(empty)}</p>"
    return "<ul>" + "".join(f"<li>{i}</li>" for i in items) + "</ul>"


def render_html(report: ForensicReport) -> str:
    m, s, lim, cu = report.metadata, report.summary, report.limitations, report.chain_of_custody
    parts: list[str] = []
    add: Callable[[str], None] = parts.append

    add(f"<h1>{h(report.title)}</h1>")
    add(f'<p class="muted">Run {_code(m.run_id)} · generated {_code(m.generated_at_utc)} · {_code(m.report_schema_version)}</p>')
    add(f'<div class="disclaimer" role="note">{h(report.disclaimer)}</div>')

    add(f'<h2 class="first">{h(SECTION_TITLES[0])}</h2>')
    add(
        _dl(
            [
                ("Case id", _code(m.case_id)),
                ("Run id", _code(m.run_id)),
                ("Run status", f"{_code(m.run_status.value)} {h(m.run_status_explanation)}"),
                ("Algorithm version", _code(m.algorithm_version)),
                ("Configuration version", _code(m.configuration_version)),
                ("Result SHA-256", _code(m.result_sha256)),
                ("Report generated (UTC)", _code(m.generated_at_utc)),
                ("Report generator", _code(m.report_generator_version)),
                ("Relationship export", _code(m.relationship_export_status.value)),
                ("Contract decisions enabled for this run", ", ".join(_code(d) for d in m.approved_contract_decisions) or h(NOT_PROVIDED)),
                (
                    "Enabled without verified team sign-off (proposal only)",
                    ", ".join(_code(d) for d in m.unverified_contract_decisions) or "None",
                ),
                ("Withheld contract decisions", ", ".join(_code(d) for d in m.withheld_contract_decisions) or "None"),
                ("Reference speed threshold", h(_num(m.configured_speed_threshold_kmh, "{:g}", " km/h"))),
                ("Timeline span (UTC)", _code(_window(s.timeline_span_start_utc, s.timeline_span_end_utc))),
                ("Report content SHA-256", _code(report.content_sha256)),
            ]
        )
    )
    add(_html_table(["Count", "Value"], [[h(k.replace("_", " ")), h(v)] for k, v in s.counts.items()]))
    if s.key_leads:
        add("<h3>Analytical leads for review</h3>")
        for lead in s.key_leads:
            add(
                f'<div class="finding lead">{_badge("ANALYTICAL LEAD", "LEAD")} {_badge(lead.basis.value)} '
                f"<strong>{h(lead.ref)}</strong> {h(lead.statement)}{_html_citations(lead.citations)}</div>"
            )
    else:
        add('<p class="muted">No contradiction leads were produced in this run.</p>')
    add(f'<p class="muted"><em>{h(s.interpretation_notice)}</em></p>')

    add(f"<h2>{h(SECTION_TITLES[1])}</h2>")
    add(
        _html_table(
            ["Evidence", "Type", "SHA-256", "Integrity", "Used", "Clock calibration", "Events"],
            [
                [
                    _code(i.evidence_id),
                    h(_opt(i.source_type)),
                    _code(i.sha256 or NOT_PROVIDED),
                    f"{_code(i.integrity_status.value)}<br>{h(i.integrity_note)}",
                    "Yes" if i.usable else f"No {_code(i.exclusion_reason_code or '-')}",
                    _code(i.clock_calibration.value) if i.clock_calibration else h(NOT_PROVIDED),
                    h(i.timeline_events_cited),
                ]
                for i in report.evidence_inventory
            ],
        )
    )

    add(f"<h2>{h(SECTION_TITLES[2])}</h2>")
    add(f'<p class="muted">{h(report.frame_notice)}</p>')
    if not report.timeline:
        add('<p class="muted">No events were accepted into the timeline.</p>')
    for e in report.timeline:
        rows = [
            ("Event id", _code(e.event_id)),
            ("Raw timestamp", f"{_code(e.raw_timestamp)} precision {_code(e.raw_precision)}"),
            ("Corrected UTC", f"{_code(e.timestamp_utc)} precision {_code(e.precision)}"),
            (
                "Uncertainty window",
                _code(_window(e.uncertainty_window_start_utc, e.uncertainty_window_end_utc))
                + (" estimated window assumption" if e.is_estimated_window_assumption else ""),
            ),
            (
                "Time basis",
                (f"{_code(e.timezone_basis)} · timezone {_code(_opt(e.source_timezone))} · offset "
                f"{_code(_num(e.clock_offset_seconds, '{:+g}', ' s'))} · {_code(e.clock_calibration.value)}"),
            ),
            (
                "Ordering",
                _code(e.ordering_status)
                + (" concurrent with " + ", ".join(_code(c) for c in e.concurrent_event_ids) if e.concurrent_event_ids else ""),
            ),
            (
                "Entity references",
                (", ".join(_code(x) for x in e.involved_entity_ids) or "None resolved")
                + ("; unresolved " + ", ".join(_code(x) for x in e.unresolved_entity_references) if e.unresolved_entity_references else ""),
            ),
            ("Coordinates", _code(_coords(e.coordinates))),
            ("Frames captured during this window", ", ".join(_code(f) for f in e.frame_artifact_ids) or "None"),
        ]
        add(
            f'<div class="finding">{_badge(e.basis.value)} <strong>{h(e.ref)}. {h(e.event_type)}</strong>: '
            f'{h(e.description)}<div class="muted">{h(e.basis_note)}</div>{_dl(rows)}{_html_citations(e.citations)}</div>'
        )

    add(f"<h2>{h(SECTION_TITLES[3])}</h2>")
    add(f"<h3>Spatial observations {_badge('OBSERVED')}</h3>")
    if report.spatial_observations:
        add(
            _html_table(
                ["Ref", "Entity reference", "Event", "Coordinates", "Window (UTC)", "Sources"],
                [
                    [
                        h(o.ref),
                        _code(o.entity_id),
                        _code(o.event_id),
                        _code(_coords(o.coordinates)),
                        _code(_window(o.uncertainty_window_start_utc, o.uncertainty_window_end_utc)),
                        _html_citations(o.citations),
                    ]
                    for o in report.spatial_observations
                ],
            )
        )
    else:
        add('<p class="muted">No located observations. No locations were inferred.</p>')
    add(f"<h3>Movement segments and travel feasibility {_badge('INFERRED')}</h3>")
    if not report.movement_segments:
        add('<p class="muted">No movement segments were built.</p>')
    for seg in report.movement_segments:
        rows = [
            ("Entity reference", _code(seg.entity_id)),
            ("Straight-line distance", h(_num(seg.distance_km, "{:.2f}", " km"))),
            (
                "Elapsed time",
                h(
                    f"nominal {_num(seg.nominal_elapsed_seconds, '{:g}', ' s')}, possible range "
                    f"{_num(seg.min_possible_elapsed_seconds, '{:g}', ' s')} to "
                    f"{_num(seg.max_possible_elapsed_seconds, '{:g}', ' s')}"
                ),
            ),
            (
                "Minimum required speed",
                (f"{h(_num(seg.min_required_speed_kmh, '{:.1f}', ' km/h'))} vs reference "
                f"{h(f'{seg.configured_speed_threshold_kmh:g}')} km/h {_code(seg.threshold_evaluation.value)}"),
            ),
            ("Feasibility", h(seg.feasibility_statement)),
            ("Assumption", h(seg.threshold_assumption_note)),
        ]
        if seg.continuity_note:
            rows.append(("Continuity", h(seg.continuity_note)))
        add(
            f'<div class="finding">{_badge(seg.basis.value)} <strong>{h(seg.ref)}.</strong> '
            f"{_code(seg.origin_event_id)} → {_code(seg.destination_event_id)}{_dl(rows)}{_html_citations(seg.citations)}</div>"
        )

    add(f"<h2>{h(SECTION_TITLES[4])}</h2>")
    add(f'<div class="notice">{h(report.contradiction_notice)}</div>')
    if not report.contradictions:
        add('<p class="muted">No contradiction leads were produced in this run.</p>')
    for c in report.contradictions:
        rows = [
            ("Lead id", _code(c.lead_id)),
            ("Classification", _code(c.classification)),
            ("Entity reference", _code(c.entity_id)),
            ("Origin event", f"{_code(c.origin_event_id)} {_code(_window(*c.origin_window_utc))}"),
            ("Destination event", f"{_code(c.destination_event_id)} {_code(_window(*c.destination_window_utc))}"),
            (
                "Derived values",
                h(
                    f"distance {_num(c.distance_km, '{:.2f}', ' km')}; longest possible elapsed "
                    f"{_num(c.max_possible_elapsed_seconds, '{:g}', ' s')}; minimum required speed "
                    f"{_num(c.min_required_speed_kmh, '{:.1f}', ' km/h')} vs reference "
                    f"{c.configured_speed_threshold_kmh:g} km/h"
                )
                + f" {_code(_opt(c.threshold_evaluation))}",
            ),
            ("Explanation", h(c.explanation)),
            ("Exported relationship", _code(_opt(c.relationship_id))),
        ]
        rows += [("Lead limitation", h(x)) for x in c.lead_limitations]
        add(
            f'<div class="finding lead">{_badge("ANALYTICAL LEAD", "LEAD")} {_badge(c.basis.value)} '
            f"<strong>{h(c.ref)}. {h(c.lead_type)}</strong> (not proven){_dl(rows)}{_html_citations(c.citations)}</div>"
        )
    if report.contradiction_pairs_not_evaluated:
        add("<h3>Pairs not evaluated (order unknown)</h3>")
        add(
            _list(
                [
                    f"{_code(p.entity_id)}: {_code(p.event_a_id)} / {_code(p.event_b_id)}. {h(p.note)}"
                    for p in report.contradiction_pairs_not_evaluated
                ],
                "None.",
            )
        )

    add(f"<h2>{h(SECTION_TITLES[5])}</h2>")
    if report.relationships:
        add(
            _html_table(
                ["Ref", "Type", "Source → target", "Basis", "Weight", "Sources"],
                [
                    [
                        h(r.ref),
                        _code(r.relation_type),
                        f"{_code(r.source_id)} → {_code(r.target_id)}",
                        f"{_badge(r.basis.value)} is_observed {_code(str(r.is_observed).lower())}",
                        h(r.weight_display),
                        _html_citations(r.citations),
                    ]
                    for r in report.relationships
                ],
            )
        )
    else:
        add(f'<p class="muted">No relationships were exported (status {_code(m.relationship_export_status.value)}).</p>')

    add(f"<h2>{h(SECTION_TITLES[6])}</h2>")
    add("<h3>Limitations</h3>")
    add(
        _list(
            [
                f"{_code(x.code)} {h(x.message)}"
                + (" Applies to: " + ", ".join(_code(sid) for sid in x.subject_ids) + "." if x.subject_ids else "")
                for x in lim.limitations
            ],
            "None.",
        )
    )
    add("<h3>Evidence gaps</h3>")
    add(
        _list(
            [
                f"{_code(g.gap_type)} {_code(g.subject_id)}: {h(g.description)}"
                + (f" {_code(g.reason_code)}" if g.reason_code else "")
                for g in lim.evidence_gaps
            ],
            "None recorded.",
        )
    )
    add("<h3>Clock calibrations (none independently verified)</h3>")
    add(
        _list(
            [
                f"{_code(c.evidence_id)}: {_code(c.clock_calibration.value)}, offset "
                f"{_code(_num(c.clock_offset_seconds, '{:+g}', ' s'))}, timezone {_code(_opt(c.source_timezone))}. {h(c.note)}"
                for c in lim.unverified_clock_calibrations
            ],
            "No usable evidence.",
        )
    )
    add("<h3>Pending items</h3>")
    add(
        _list(
            [
                _code(p.code) + (f" {_code(p.subject_id)}" if p.subject_id else "") + f": {h(p.description)}"
                for p in lim.pending_items
            ],
            "None.",
        )
    )
    add("<h3>Excluded inputs</h3>")
    add(
        _list(
            [
                f"{_code(x['subject_type'])} {_code(x['subject_id'])}: {_code(x['reason_code'])}"
                for x in lim.excluded_inputs
            ],
            "None.",
        )
    )
    add("<h3>Errors and warnings</h3>")
    add(
        _list(
            [
                f"{_code(d.severity)} {_code(d.code)}" + (f" {_code(d.subject_id)}" if d.subject_id else "") + f": {h(d.message)}"
                for d in lim.diagnostics
            ],
            "None.",
        )
    )

    add(f"<h2>{h(SECTION_TITLES[7])}</h2>")
    add(f"<p>{h(cu.statement)}</p>")
    add("<h3>Evidence</h3>")
    add(
        _html_table(
            ["Evidence", "SHA-256", "Storage reference", "Integrity", "Video before / after processing"],
            [
                [
                    _code(e.evidence_id),
                    _code(e.sha256 or NOT_PROVIDED),
                    _code(_opt(e.storage_reference)),
                    _code(e.integrity_status.value),
                    (
                        f"{_code(_short_sha(e.video_sha256_before_processing))} / "
                        f"{_code(_short_sha(e.video_sha256_after_processing))}"
                        + (" unchanged" if e.video_source_unchanged else "")
                    )
                    if e.video_sha256_before_processing
                    else "n/a",
                ]
                for e in cu.evidence
            ],
        )
    )
    add("<h3>Video processing</h3>")
    add(
        _list(
            [
                f"{_code(v.evidence_id)}: {_code(v.status)}, {_code(v.processing_kind)}, handoff "
                f"{_code(_opt(v.frame_handoff_status))}, {h(v.frames_extracted)} frame(s). {h(v.processing_note)}"
                for v in cu.video_processing
            ],
            "No video evidence processed.",
        )
    )
    add("<h3>Frame artifacts</h3>")
    if cu.frame_artifacts:
        add(
            _html_table(
                ["Artifact", "Source", "Source video SHA-256", "Index", "PTS (s)", "UTC", "Frame SHA-256"],
                [
                    [
                        _code(f.artifact_reference),
                        _code(f.source_evidence_id),
                        _code(_short_sha(f.source_video_sha256)),
                        h(f.frame_index),
                        h(f"{f.pts_offset_seconds:g}"),
                        _code(_opt(f.timestamp_utc)),
                        _code(_short_sha(f.sha256)),
                    ]
                    for f in cu.frame_artifacts
                ],
            )
        )
    else:
        add('<p class="muted">None.</p>')
    add("<h3>Finding provenance</h3>")
    add(
        _html_table(
            ["Finding", "Type", "Basis", "Evidence (SHA-256)", "Source records"],
            [
                [
                    _code(p.finding_id),
                    _code(p.finding_type.value),
                    _badge("OBSERVED" if p.is_observed else "INFERRED"),
                    "<br>".join(f"{_code(k)} {_code(_short_sha(v))}" for k, v in sorted(p.evidence_sha256.items()))
                    or h(NOT_PROVIDED),
                    ", ".join(_code(r) for r in p.source_record_ids) or h(NOT_PROVIDED),
                ]
                for p in cu.finding_provenance
            ],
        )
    )
    add(f'<p class="muted"><em>{h(cu.run_interpretation_notice)}</em></p>')
    add(f"<footer>{h(report.disclaimer)}</footer>")

    return (
        "<!doctype html>\n"
        '<html lang="en">\n<head>\n<meta charset="utf-8">\n'
        '<meta name="viewport" content="width=device-width, initial-scale=1">\n'
        "<meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'unsafe-inline'\">\n"
        '<meta name="referrer" content="no-referrer">\n'
        f"<title>{h(report.title)}</title>\n<style>{_CSS}</style>\n</head>\n<body>\n<main>\n"
        + "\n".join(parts)
        + "\n</main>\n</body>\n</html>\n"
    )
