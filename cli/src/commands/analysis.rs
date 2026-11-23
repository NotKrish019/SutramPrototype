//! Read-only reconstruction views: `yukti timeline | spatial | contradictions | gaps |
//! provenance | report`.
//!
//! API contracts (`backend-rust/src/api/routes/reconstruction.rs`):
//! * `GET /api/v1/cases/:case_id/timeline` returns `TimelineReconstructionResult`.
//! * `GET /api/v1/cases/:case_id/spatial` returns `SpatialReconstructionResult`.
//! * `GET /api/v1/cases/:case_id/contradictions` returns `ContradictionAnalysisResult`.
//! * `GET /api/v1/cases/:case_id/gaps` returns `EvidenceGapAnalysisReport`.
//! * `GET /api/v1/cases/:case_id/reconstruction/provenance` returns `CaseProvenanceReport`.
//! * `GET /api/v1/cases/:case_id/findings/:finding_id/provenance` returns `FindingProvenanceTrace`.
//! * `GET /api/v1/cases/:case_id/report` returns `CaseForensicReport`.
//!
//! Each view uses the latest stored reconstruction, or one the API computes on the fly. Like
//! every command, the case is checked first, and nothing is computed by the CLI beyond counting
//! and filtering what the API returned.

use std::io::Write as _;
use std::path::Path;

use serde::Serialize;
use serde_json::Value;

use super::{require_case, typed, CaseViewDocument, RequestDoc};
use crate::cli::{
    CaseArgs, GapsArgs, ProvenanceArgs, ReportArgs, ReportFormat, Severity, TimelineArgs,
};
use crate::client::{case_report_path, case_view_path, finding_provenance_path, ApiClient};
use crate::dto::{CaseReportRecord, ContradictionsDto, SpatialDto, TimelineDto, TimelineEntryDto};
use crate::error::{exit, CliError};
use crate::output::{render_error_with_notes, render_warnings, to_json, Rendered};
use crate::ui::{pad, short_id, status_badge, Badge, Glyph, Tone, Ui, MARGIN};

pub const TIMELINE_SCHEMA: &str = "yukti.cli.timeline/v1";
pub const SPATIAL_SCHEMA: &str = "yukti.cli.spatial/v1";
pub const CONTRADICTIONS_SCHEMA: &str = "yukti.cli.contradictions/v1";
pub const GAPS_SCHEMA: &str = "yukti.cli.gaps/v1";
pub const PROVENANCE_SCHEMA: &str = "yukti.cli.provenance/v1";
pub const REPORT_SCHEMA: &str = "yukti.cli.report/v1";

/// Fetches one case view after the case check. Errors are already rendered.
async fn fetch(
    raw_case_id: &str,
    suffix: &str,
    client: &ApiClient,
    json: bool,
) -> Result<(crate::dto::CaseId, Vec<String>, String, Value), Rendered> {
    let (case_id, case) = require_case(raw_case_id, client, json, "requested").await?;
    let path = case_view_path(&case_id, suffix);
    let value = client
        .get_json(&path)
        .await
        .map_err(|e| render_error_with_notes(&e, json, &[]))?;
    let warnings = super::placeholder_warning(&case_id, &case)
        .into_iter()
        .collect();
    Ok((case_id, warnings, path, value))
}

fn view_json(
    schema: &'static str,
    client: &ApiClient,
    case_id: &str,
    path: &str,
    response: &Value,
    warnings: &[String],
) -> Rendered {
    let doc = CaseViewDocument {
        schema,
        api_url: client.base_url(),
        case_id,
        request: RequestDoc {
            method: "GET",
            path,
            body: None,
        },
        response,
        warnings,
    };
    Rendered::stdout(to_json(&doc), exit::SUCCESS)
}

fn done(stdout: String, warnings: &[String]) -> Rendered {
    Rendered {
        stdout,
        stderr: render_warnings(warnings),
        exit_code: exit::SUCCESS,
    }
}

/* ------------------------------------------------------------------------------ timeline */

#[derive(Serialize)]
struct TimelineFilters {
    limit: Option<usize>,
    assumed_only: bool,
    uncertain_only: bool,
}

#[derive(Serialize)]
struct TimelineDocument<'a> {
    schema: &'static str,
    api_url: &'a str,
    case_id: &'a str,
    request: RequestDoc<'a>,
    filters: TimelineFilters,
    /// Entries left after the filters, exactly as returned, in the API's order.
    shown_count: usize,
    entries: Vec<&'a Value>,
    /// The complete response body, exactly as returned.
    response: &'a Value,
    warnings: &'a [String],
}

/// Indexes of entries that pass the filters, in order.
pub fn select_entries(entries: &[TimelineEntryDto], args: &TimelineArgs) -> Vec<usize> {
    entries
        .iter()
        .enumerate()
        .filter(|(_, e)| !args.assumed_only || !e.assumptions.is_empty())
        .filter(|(_, e)| !args.uncertain_only || e.ordering_status != "STRICTLY_ORDERED")
        .map(|(i, _)| i)
        .take(args.limit.unwrap_or(usize::MAX))
        .collect()
}

pub async fn timeline(args: &TimelineArgs, client: &ApiClient, json: bool, ui: &Ui) -> Rendered {
    if args.limit == Some(0) {
        let err = CliError::InvalidArgument("--limit must be at least 1".to_string());
        return render_error_with_notes(&err, json, &[]);
    }
    let (case_id, mut warnings, path, raw) =
        match fetch(&args.case_id, "timeline", client, json).await {
            Ok(v) => v,
            Err(r) => return r,
        };
    let url = client.url_for(&path);
    let dto: TimelineDto = match typed(&url, &raw, "TimelineReconstructionResult") {
        Ok(d) => d,
        Err(e) => return render_error_with_notes(&e, json, &[]),
    };
    let picked = select_entries(&dto.entries, args);
    if dto.total_events == 0 {
        warnings.push("The timeline contains no events.".to_string());
    }
    if json {
        let raw_entries = raw["entries"].as_array();
        let doc = TimelineDocument {
            schema: TIMELINE_SCHEMA,
            api_url: client.base_url(),
            case_id: case_id.as_str(),
            request: RequestDoc {
                method: "GET",
                path: &path,
                body: None,
            },
            filters: TimelineFilters {
                limit: args.limit,
                assumed_only: args.assumed_only,
                uncertain_only: args.uncertain_only,
            },
            shown_count: picked.len(),
            entries: picked
                .iter()
                .filter_map(|&i| raw_entries.and_then(|a| a.get(i)))
                .collect(),
            response: &raw,
            warnings: &warnings,
        };
        return Rendered::stdout(to_json(&doc), exit::SUCCESS);
    }

    let mut out = ui.header(&["timeline"], case_id.as_str());
    out.push('\n');
    let assumed = dto
        .entries
        .iter()
        .filter(|e| !e.assumptions.is_empty())
        .count();
    out.push_str(&ui.stats(&[
        (dto.total_events.to_string(), "events", Tone::Strong),
        (
            dto.strictly_ordered_count.to_string(),
            "strictly ordered",
            Tone::Muted,
        ),
        (
            dto.concurrent_uncertain_count.to_string(),
            "concurrent/uncertain",
            if dto.concurrent_uncertain_count > 0 {
                Tone::Inferred
            } else {
                Tone::Muted
            },
        ),
        (
            assumed.to_string(),
            "with assumptions",
            if assumed > 0 { Tone::Warn } else { Tone::Muted },
        ),
        (
            dto.excluded_events.len().to_string(),
            "excluded",
            Tone::Muted,
        ),
    ]));
    if let (Some(first), Some(last)) = (dto.entries.first(), dto.entries.last()) {
        out.push_str(&ui.kv(
            "Span",
            Tone::Muted,
            &format!(
                "{} {} {} (nominal UTC, first and last in the API's order)",
                first.time_window.nominal_utc,
                ui.g(Glyph::Arrow),
                last.time_window.nominal_utc
            ),
        ));
    }
    let filtered = args.limit.is_some() || args.assumed_only || args.uncertain_only;
    out.push_str(&ui.section(
        "Events",
        &if filtered {
            format!("{} of {} shown", picked.len(), dto.entries.len())
        } else {
            format!("{} entries, in the API's order", dto.entries.len())
        },
    ));
    if picked.is_empty() {
        out.push_str(&format!(
            "{MARGIN}{}\n",
            ui.paint(Tone::Muted, "No entries to show.")
        ));
    } else {
        let entries: Vec<&TimelineEntryDto> = picked.iter().map(|&i| &dto.entries[i]).collect();
        out.push_str(&super::reconstruction::render_timeline_rail(&entries, ui));
    }
    if !dto.overlaps.is_empty() {
        out.push_str(&ui.footnote(&format!(
            "{} pair(s) of uncertainty windows overlap: their relative order is not established.",
            dto.overlaps.len()
        )));
    }
    if !dto.excluded_events.is_empty() {
        out.push_str(&ui.section("Excluded events", "not placed on the timeline"));
        for e in &dto.excluded_events {
            out.push_str(&format!(
                "{MARGIN}{} {}  {}\n",
                ui.mark(Glyph::Cross, Tone::Subtle),
                ui.paint(Tone::Text, &e.event_id),
                ui.paint(Tone::Muted, &e.reason)
            ));
        }
    }
    out.push('\n');
    out.push_str(&ui.legend());
    done(out, &warnings)
}

/* ------------------------------------------------------------------------------- spatial */

pub async fn spatial(args: &CaseArgs, client: &ApiClient, json: bool, ui: &Ui) -> Rendered {
    let (case_id, warnings, path, raw) = match fetch(&args.case_id, "spatial", client, json).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let dto: SpatialDto = match typed(&client.url_for(&path), &raw, "SpatialReconstructionResult") {
        Ok(d) => d,
        Err(e) => return render_error_with_notes(&e, json, &[]),
    };
    if json {
        return view_json(
            SPATIAL_SCHEMA,
            client,
            case_id.as_str(),
            &path,
            &raw,
            &warnings,
        );
    }
    let segments: usize = dto.trajectories.iter().map(|t| t.segments.len()).sum();
    let flagged = dto
        .trajectories
        .iter()
        .flat_map(|t| &t.segments)
        .filter(|s| s.transit_metrics.exceeds_configured_speed_threshold)
        .count();
    let distance: f64 = dto.trajectories.iter().map(|t| t.total_distance_km).sum();
    let mut out = ui.header(&["spatial"], case_id.as_str());
    out.push('\n');
    out.push_str(&ui.stats(&[
        (
            dto.trajectories.len().to_string(),
            "trajectories",
            Tone::Strong,
        ),
        (segments.to_string(), "segments", Tone::Muted),
        (format!("{distance:.2}"), "km", Tone::Muted),
        (
            flagged.to_string(),
            "over threshold",
            if flagged > 0 { Tone::Flag } else { Tone::Muted },
        ),
        (dto.overlaps.len().to_string(), "overlaps", Tone::Inferred),
    ]));
    out.push_str(&super::reconstruction::render_spatial(&dto, ui));
    out.push_str(&ui.footnote(
        "Distances and speeds are inferred from located events; a segment over the threshold is a lead to review, not proof of impossibility.",
    ));
    done(out, &warnings)
}

/* ------------------------------------------------------------------------ contradictions */

pub async fn contradictions(args: &CaseArgs, client: &ApiClient, json: bool, ui: &Ui) -> Rendered {
    let (case_id, warnings, path, raw) =
        match fetch(&args.case_id, "contradictions", client, json).await {
            Ok(v) => v,
            Err(r) => return r,
        };
    let dto: ContradictionsDto =
        match typed(&client.url_for(&path), &raw, "ContradictionAnalysisResult") {
            Ok(d) => d,
            Err(e) => return render_error_with_notes(&e, json, &[]),
        };
    if json {
        return view_json(
            CONTRADICTIONS_SCHEMA,
            client,
            case_id.as_str(),
            &path,
            &raw,
            &warnings,
        );
    }
    let mut out = ui.header(&["contradictions"], case_id.as_str());
    out.push('\n');
    out.push_str(&ui.stats(&[
        (
            dto.leads.len().to_string(),
            "leads",
            if dto.leads.is_empty() {
                Tone::Muted
            } else {
                Tone::Flag
            },
        ),
        (
            dto.total_evaluated_segments.to_string(),
            "segments evaluated",
            Tone::Muted,
        ),
        (
            dto.flagged_segments_count.to_string(),
            "flagged",
            Tone::Muted,
        ),
    ]));
    // Lead types, in first-seen order.
    let mut by_type: Vec<(&str, usize)> = Vec::new();
    for lead in &dto.leads {
        match by_type.iter_mut().find(|(t, _)| *t == lead.lead_type) {
            Some((_, n)) => *n += 1,
            None => by_type.push((&lead.lead_type, 1)),
        }
    }
    for (t, n) in &by_type {
        out.push_str(&ui.kv(t, Tone::Flag, &format!("{n} lead(s)")));
    }
    out.push_str(&super::reconstruction::render_leads(&dto, true, ui));
    if let Some(first) = dto.leads.first() {
        out.push('\n');
        out.push_str(&ui.next(
            "trace a lead",
            &format!(
                "yukti provenance --case {} --finding {}",
                case_id.as_str(),
                first.lead_id
            ),
        ));
        out.push_str(&ui.next(
            "test it",
            &format!(
                "yukti whatif --case {} --speed-threshold <kmh>",
                case_id.as_str()
            ),
        ));
    }
    done(out, &warnings)
}

/* ---------------------------------------------------------------------------------- gaps */

#[derive(Serialize)]
struct GapsDocument<'a> {
    schema: &'static str,
    api_url: &'a str,
    case_id: &'a str,
    request: RequestDoc<'a>,
    min_severity: Option<Severity>,
    shown_count: usize,
    /// Gaps at or above `min_severity`, exactly as returned, in the API's order.
    gaps: Vec<&'a Value>,
    response: &'a Value,
    warnings: &'a [String],
}

impl Serialize for Severity {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(match self {
            Severity::Low => "LOW",
            Severity::Medium => "MEDIUM",
            Severity::High => "HIGH",
            Severity::Critical => "CRITICAL",
        })
    }
}

fn severity_tone(severity: &str) -> Tone {
    match Severity::rank(severity) {
        2 | 3 => Tone::Flag,
        1 => Tone::Warn,
        _ => Tone::Muted,
    }
}

pub async fn gaps(args: &GapsArgs, client: &ApiClient, json: bool, ui: &Ui) -> Rendered {
    let (case_id, warnings, path, raw) = match fetch(&args.case_id, "gaps", client, json).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Some(all) = raw["gaps"].as_array() else {
        let err = CliError::BadResponse {
            url: client.url_for(&path),
            reason: "response does not match EvidenceGapAnalysisReport (no `gaps` array)"
                .to_string(),
        };
        return render_error_with_notes(&err, json, &[]);
    };
    let min = args.min_severity.map_or(0, Severity::as_rank);
    let shown: Vec<&Value> = all
        .iter()
        .filter(|g| Severity::rank(g["severity"].as_str().unwrap_or("")) >= min)
        .collect();
    if json {
        let doc = GapsDocument {
            schema: GAPS_SCHEMA,
            api_url: client.base_url(),
            case_id: case_id.as_str(),
            request: RequestDoc {
                method: "GET",
                path: &path,
                body: None,
            },
            min_severity: args.min_severity,
            shown_count: shown.len(),
            gaps: shown,
            response: &raw,
            warnings: &warnings,
        };
        return Rendered::stdout(to_json(&doc), exit::SUCCESS);
    }

    let sev = |k: &str| raw["gaps_by_severity"][k].as_u64().unwrap_or(0);
    let mut out = ui.header(&["gaps"], case_id.as_str());
    out.push('\n');
    out.push_str(&ui.stats(&[
        (all.len().to_string(), "gaps", Tone::Strong),
        (
            sev("CRITICAL").to_string(),
            "critical",
            if sev("CRITICAL") > 0 {
                Tone::Flag
            } else {
                Tone::Muted
            },
        ),
        (
            sev("HIGH").to_string(),
            "high",
            if sev("HIGH") > 0 {
                Tone::Flag
            } else {
                Tone::Muted
            },
        ),
        (
            sev("MEDIUM").to_string(),
            "medium",
            if sev("MEDIUM") > 0 {
                Tone::Warn
            } else {
                Tone::Muted
            },
        ),
        (sev("LOW").to_string(), "low", Tone::Muted),
    ]));
    if let Some(run) = raw["reconstruction_run_id"].as_str() {
        out.push_str(&ui.kv("Reconstruction", Tone::Subtle, run));
    }
    if let Some(types) = raw["gaps_by_type"].as_object() {
        for (t, n) in types {
            out.push_str(&ui.kv(t, Tone::Muted, &n.to_string()));
        }
    }

    // Most severe first; stable, so the API's order holds within a severity.
    let mut sorted = shown.clone();
    sorted.sort_by_key(|g| std::cmp::Reverse(Severity::rank(g["severity"].as_str().unwrap_or(""))));
    out.push_str(&ui.section(
        "Gaps",
        &match args.min_severity {
            Some(_) => format!(
                "{} of {} at or above the minimum severity",
                shown.len(),
                all.len()
            ),
            None => "most severe first".to_string(),
        },
    ));
    if sorted.is_empty() {
        out.push_str(&format!(
            "{MARGIN}{}\n",
            ui.paint(Tone::Muted, "No gaps to show.")
        ));
    }
    let s = |g: &Value, k: &str| g[k].as_str().unwrap_or("").to_string();
    for g in sorted {
        let severity = s(g, "severity");
        let tone = severity_tone(&severity);
        let rail = ui.paint(tone, ui.g(Glyph::Rail));
        let cont = format!("{MARGIN}{rail}   ");
        let cont_w = MARGIN.len() + 4;
        out.push_str(&format!(
            "{MARGIN}{} {}  {}  {}\n",
            ui.mark(
                if Severity::rank(&severity) >= 2 {
                    Glyph::Flag
                } else {
                    Glyph::Assumed
                },
                tone
            ),
            ui.bold(tone, &pad(&severity, 8)),
            ui.paint(Tone::Muted, &s(g, "gap_type")),
            ui.paint(Tone::Subtle, &s(g, "gap_id"))
        ));
        out.push_str(&ui.hang(&cont, &cont, cont_w, Tone::Strong, &s(g, "title")));
        out.push_str(&ui.hang(&cont, &cont, cont_w, Tone::Muted, &s(g, "explanation")));
        const LABEL: usize = 13;
        let label = |l: &str| format!("{cont}{}", ui.paint(Tone::Subtle, &pad(l, LABEL)));
        let deep = format!("{cont}{}", " ".repeat(LABEL));
        out.push_str(&ui.hang(
            &label("impact"),
            &deep,
            cont_w + LABEL,
            Tone::Text,
            &s(g, "impact_on_reconstruction"),
        ));
        out.push_str(&ui.hang(
            &label("corroborate"),
            &deep,
            cont_w + LABEL,
            Tone::Supplied,
            &s(g, "suggested_corroborating_evidence"),
        ));
        let confirmed = g["is_confirmed_missing"].as_bool().unwrap_or(false);
        out.push_str(&ui.hang(
            &cont,
            &cont,
            cont_w,
            Tone::Subtle,
            &format!(
                "{} {} {} rule {} {} {}",
                s(g, "subject_type"),
                s(g, "subject_id"),
                ui.g(Glyph::Dot),
                s(g, "rule_id"),
                ui.g(Glyph::Dot),
                if confirmed {
                    "confirmed missing"
                } else {
                    "not yet supplied"
                }
            ),
        ));
        out.push_str(&format!(
            "{MARGIN}{}\n",
            ui.paint(tone, ui.g(Glyph::RailEnd))
        ));
    }
    out.push_str(&ui.footnote(
        "Gaps describe what the reconstruction cannot establish; \"not yet supplied\" means it may exist but has not been ingested.",
    ));
    done(out, &warnings)
}

/* ---------------------------------------------------------------------------- provenance */

fn classification_mark(class: &str) -> (Glyph, Tone) {
    match class {
        "OBSERVED" => (Glyph::Observed, Tone::Text),
        "EXTRACTED" => (Glyph::Supplied, Tone::Supplied),
        "SIMULATED" => (Glyph::Assumed, Tone::Warn),
        _ => (Glyph::Inferred, Tone::Inferred),
    }
}

fn integrity_badge(status: &str) -> Badge {
    match status {
        "VERIFIED_MATCH" => Badge::Ok,
        "UNKNOWN_EVIDENCE" => Badge::Warn,
        _ => Badge::Flag,
    }
}

/// One `FindingProvenanceTrace`: claim, classification and source anchors.
pub fn render_trace(f: &Value, ui: &Ui) -> String {
    let s = |v: &Value, k: &str| v[k].as_str().unwrap_or("").to_string();
    let class = s(f, "epistemic_classification");
    let (glyph, tone) = classification_mark(&class);
    let missing: Vec<&str> = f["missing_provenance_fields"]
        .as_array()
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let anchors = f["source_anchors"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let intact = missing.is_empty()
        && anchors
            .iter()
            .all(|a| a["integrity_status"].as_str() == Some("VERIFIED_MATCH"));
    let mut out = format!(
        "{MARGIN}{} {}  {}  {}{}\n",
        ui.mark(glyph, tone),
        ui.bold(Tone::Strong, &s(f, "finding_id")),
        ui.paint(Tone::Muted, &s(f, "finding_type")),
        ui.paint(tone, &class),
        if f["is_hypothetical"].as_bool() == Some(true) {
            ui.paint(Tone::Warn, "  HYPOTHETICAL")
        } else {
            String::new()
        }
    );
    let rail = ui.paint(Tone::Line, ui.g(Glyph::Rail));
    let cont = format!("{MARGIN}{rail}   ");
    let cont_w = MARGIN.len() + 4;
    out.push_str(&ui.hang(&cont, &cont, cont_w, Tone::Text, &s(f, "claim_summary")));
    out.push_str(&format!(
        "{cont}{}\n",
        if intact {
            ui.badge(Badge::Ok, "provenance intact")
        } else {
            ui.badge(Badge::Flag, "provenance flagged")
        }
    ));
    for a in anchors {
        let integrity = s(a, "integrity_status");
        out.push_str(&format!(
            "{MARGIN}{rail}   {} {}  {}  {}\n",
            ui.paint(Tone::Line, ui.g(Glyph::Branch)),
            ui.paint(Tone::Text, &format!("event {}", s(a, "event_id"))),
            ui.paint(
                Tone::Muted,
                &format!(
                    "evidence {}{}",
                    short_id(&s(a, "evidence_id"), ui),
                    a["original_filename"]
                        .as_str()
                        .map_or(String::new(), |n| format!(" ({n})"))
                )
            ),
            ui.badge(integrity_badge(&integrity), &integrity)
        ));
        let deep = format!("{MARGIN}{rail}      ");
        let mut facts = vec![format!("raw {:?}", s(a, "original_source_timestamp"))];
        if let Some(n) = a["normalized_timestamp_utc"].as_str() {
            facts.push(format!("utc {n}"));
        }
        if let Some(r) = a["source_record_id"].as_str() {
            facts.push(format!("record {r}"));
        }
        if let Some(h) = a["evidence_sha256"].as_str() {
            facts.push(format!(
                "sha256 {}{}",
                h.get(..16).unwrap_or(h),
                ui.g(Glyph::Ellipsis)
            ));
        }
        out.push_str(&ui.hang(
            &deep,
            &deep,
            cont_w + 3,
            Tone::Subtle,
            &facts.join(&format!(" {} ", ui.g(Glyph::Dot))),
        ));
    }
    if !missing.is_empty() {
        out.push_str(&format!(
            "{cont}{} {}\n",
            ui.mark(Glyph::Assumed, Tone::Warn),
            ui.paint(
                Tone::Warn,
                &format!("missing provenance: {}", missing.join(", "))
            )
        ));
    }
    if let Some(obj) = f["derived_inferences"]
        .as_object()
        .filter(|o| !o.is_empty())
    {
        let text = obj
            .iter()
            .map(|(k, v)| {
                format!(
                    "{k} {}",
                    v.as_str().map_or_else(|| v.to_string(), str::to_string)
                )
            })
            .collect::<Vec<_>>()
            .join(&format!(" {} ", ui.g(Glyph::Dot)));
        let pre = format!("{cont}{} ", ui.mark(Glyph::Inferred, Tone::Inferred));
        out.push_str(&ui.hang(
            &pre,
            &format!("{cont}  "),
            cont_w + 2,
            Tone::Inferred,
            &text,
        ));
    }
    out.push_str(&format!(
        "{MARGIN}{}\n",
        ui.paint(Tone::Line, ui.g(Glyph::RailEnd))
    ));
    out
}

pub async fn provenance(
    args: &ProvenanceArgs,
    client: &ApiClient,
    json: bool,
    ui: &Ui,
) -> Rendered {
    let (case_id, case) = match require_case(&args.case_id, client, json, "requested").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let warnings: Vec<String> = super::placeholder_warning(&case_id, &case)
        .into_iter()
        .collect();
    let path = match &args.finding {
        Some(f) if f.trim().is_empty() => {
            let err = CliError::InvalidArgument("--finding must not be blank".to_string());
            return render_error_with_notes(&err, json, &[]);
        }
        Some(f) => finding_provenance_path(&case_id, f.trim()),
        None => case_view_path(&case_id, "reconstruction/provenance"),
    };
    let raw = match client.get_json(&path).await {
        Ok(v) => v,
        Err(err) => return render_error_with_notes(&err, json, &[]),
    };
    if json {
        return view_json(
            PROVENANCE_SCHEMA,
            client,
            case_id.as_str(),
            &path,
            &raw,
            &warnings,
        );
    }
    let mut out = ui.header(&["provenance"], case_id.as_str());
    out.push('\n');
    if args.finding.is_some() {
        out.push_str(&ui.section("Finding trace", raw["audited_at"].as_str().unwrap_or("")));
        out.push_str(&render_trace(&raw, ui));
    } else {
        let n = |k: &str| raw[k].as_u64().unwrap_or(0);
        let total = n("total_findings_count");
        out.push_str(&ui.stats(&[
            (total.to_string(), "findings", Tone::Strong),
            (n("intact_provenance_count").to_string(), "intact", Tone::Ok),
            (
                n("flagged_provenance_count").to_string(),
                "flagged",
                if n("flagged_provenance_count") > 0 {
                    Tone::Flag
                } else {
                    Tone::Muted
                },
            ),
        ]));
        out.push_str(&ui.kv_painted(
            "Integrity",
            &format!(
                "{}  {}",
                ui.meter(
                    n("intact_provenance_count") as f64 / total.max(1) as f64,
                    24,
                    Tone::Ok
                ),
                ui.paint(Tone::Muted, "share of findings with intact provenance")
            ),
        ));
        if let Some(run) = raw["reconstruction_run_id"].as_str() {
            out.push_str(&ui.kv("Reconstruction", Tone::Subtle, run));
        }
        out.push_str(&ui.kv(
            "Audited",
            Tone::Subtle,
            raw["audited_at"].as_str().unwrap_or(""),
        ));
        let findings = raw["findings"].as_array().map(Vec::as_slice).unwrap_or(&[]);
        out.push_str(&ui.section("Findings", &format!("{}", findings.len())));
        for f in findings {
            out.push_str(&render_trace(f, ui));
        }
    }
    out.push('\n');
    out.push_str(&ui.legend());
    done(out, &warnings)
}

/* -------------------------------------------------------------------------------- report */

#[derive(Serialize)]
struct WrittenFile {
    path: String,
    format: &'static str,
    size_bytes: usize,
    /// SHA-256 of the bytes written, so the export can be verified later.
    sha256: String,
}

#[derive(Serialize)]
struct ReportDocument<'a> {
    schema: &'static str,
    api_url: &'a str,
    case_id: &'a str,
    request: RequestDoc<'a>,
    response: &'a Value,
    /// The file written with --out, or `null`.
    written: Option<WrittenFile>,
    warnings: &'a [String],
}

/// Writes `bytes` to `path`. Without `force` an existing file is never replaced.
pub fn write_export(path: &Path, bytes: &[u8], force: bool) -> Result<(), CliError> {
    let shown = path.display();
    let mut options = std::fs::OpenOptions::new();
    options.write(true);
    if force {
        options.create(true).truncate(true);
    } else {
        options.create_new(true);
    }
    let mut file = options.open(path).map_err(|e| {
        CliError::InvalidArgument(match e.kind() {
            std::io::ErrorKind::AlreadyExists => {
                format!("{shown} already exists; pass --force to replace it")
            }
            _ => format!("cannot write {shown}: {e}"),
        })
    })?;
    file.write_all(bytes)
        .and_then(|()| file.flush())
        .map_err(|e| CliError::InvalidArgument(format!("cannot write {shown}: {e}")))
}

pub async fn report(args: &ReportArgs, client: &ApiClient, json: bool, ui: &Ui) -> Rendered {
    let (case_id, case) = match require_case(&args.case_id, client, json, "exported").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let mut warnings: Vec<String> = super::placeholder_warning(&case_id, &case)
        .into_iter()
        .collect();
    let path = case_report_path(&case_id);
    let record: CaseReportRecord = match client.get_case_report(&case_id).await {
        Ok(r) => r,
        Err(err) => return render_error_with_notes(&err, json, &[]),
    };
    let r = &record.dto;
    if !r.case_id.eq_ignore_ascii_case(case_id.as_str()) {
        warnings.push(format!(
            "The report names case_id {:?}, not the requested case {}.",
            r.case_id,
            case_id.as_str()
        ));
    }

    let written = match &args.out {
        None => None,
        Some(out_path) => {
            let (bytes, format) = match args.format {
                ReportFormat::Markdown => (r.markdown_dossier.clone().into_bytes(), "markdown"),
                ReportFormat::Json => (to_json(&record.raw).into_bytes(), "json"),
            };
            if let Err(err) = write_export(out_path, &bytes, args.force) {
                return render_error_with_notes(&err, json, &[]);
            }
            Some(WrittenFile {
                path: out_path.display().to_string(),
                format,
                size_bytes: bytes.len(),
                sha256: super::evidence::sha256_hex(&bytes),
            })
        }
    };

    if json {
        let doc = ReportDocument {
            schema: REPORT_SCHEMA,
            api_url: client.base_url(),
            case_id: case_id.as_str(),
            request: RequestDoc {
                method: "GET",
                path: &path,
                body: None,
            },
            response: &record.raw,
            written,
            warnings: &warnings,
        };
        return Rendered::stdout(to_json(&doc), exit::SUCCESS);
    }

    let mut out = ui.header(&["report"], case_id.as_str());
    out.push('\n');
    out.push_str(&format!(
        "{MARGIN}{}  {}\n",
        ui.bold(Tone::Strong, &r.title),
        ui.badge(
            status_badge(&r.judicial_export_status),
            &r.judicial_export_status
        )
    ));
    out.push_str(&ui.para(MARGIN.len(), Tone::Muted, &r.judicial_export_reason));
    out.push('\n');
    out.push_str(&ui.stats(&[
        (
            r.summary.evidence_count.to_string(),
            "exhibits",
            Tone::Strong,
        ),
        (
            r.summary.total_timeline_events.to_string(),
            "events",
            Tone::Muted,
        ),
        (
            r.summary.excluded_events_count.to_string(),
            "excluded",
            Tone::Muted,
        ),
        (
            r.summary.open_contradictions_count.to_string(),
            "open leads",
            if r.summary.open_contradictions_count > 0 {
                Tone::Flag
            } else {
                Tone::Muted
            },
        ),
    ]));
    out.push_str(&ui.section("Report", ""));
    out.push_str(&ui.kv("Report ID", Tone::Text, &r.report_id));
    out.push_str(&ui.kv("Generated", Tone::Muted, &r.generated_at));
    out.push_str(&ui.kv("Disclaimer", Tone::Subtle, &r.forensic_disclaimer));
    match &written {
        Some(w) => {
            out.push_str(&ui.section("Exported", w.format));
            out.push_str(&ui.kv("File", Tone::Strong, &w.path));
            out.push_str(&ui.kv(
                "Size",
                Tone::Muted,
                &crate::ui::human_bytes(w.size_bytes as u64),
            ));
            out.push_str(&ui.kv_painted(
                "SHA-256",
                &format!(
                    "{} {}",
                    ui.mark(Glyph::Check, Tone::Ok),
                    ui.paint(Tone::Text, &w.sha256)
                ),
            ));
            out.push_str(&ui.footnote("Record this hash with the exported dossier to show it has not changed since export."));
        }
        None => {
            out.push_str(&super::reconstruction::render_dossier(
                &r.markdown_dossier,
                ui,
            ));
            out.push('\n');
            out.push_str(&ui.next(
                "export",
                &format!("yukti report --case {} --out dossier.md", case_id.as_str()),
            ));
        }
    }
    done(out, &warnings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exports_never_replace_files_without_force() {
        let dir = std::env::temp_dir().join(format!("yukti-cli-export-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("dossier.md");
        let _ = std::fs::remove_file(&path);
        write_export(&path, b"one", false).unwrap();
        let err = write_export(&path, b"two", false).unwrap_err();
        assert!(err.to_string().contains("--force"), "{err}");
        assert_eq!(std::fs::read(&path).unwrap(), b"one");
        write_export(&path, b"two", true).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"two");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn classification_glyphs_follow_the_evidence_semantics() {
        assert_eq!(classification_mark("OBSERVED").0, Glyph::Observed);
        assert_eq!(classification_mark("INFERRED").0, Glyph::Inferred);
        assert_eq!(classification_mark("SIMULATED").1, Tone::Warn);
        assert_eq!(integrity_badge("HASH_MISMATCH"), Badge::Flag);
        assert_eq!(integrity_badge("VERIFIED_MATCH"), Badge::Ok);
    }
}
