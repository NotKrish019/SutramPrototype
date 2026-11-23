//! `yukti robustness` (alias `breaking-point`): how wrong each assumption must be before a
//! contradiction lead dissolves.
//!
//! API contract: `GET /api/v1/cases/:case_id/robustness` returns `CaseRobustnessReport`
//! (`backend-rust/src/reconstruction/robustness.rs`). The backend computes the margins and
//! verifies each boundary by recomputation; the CLI only orders and displays them.

use serde_json::Value;

use super::{require_case, CaseViewDocument, RequestDoc};
use crate::cli::CaseArgs;
use crate::client::{case_robustness_path, ApiClient};
use crate::error::exit;
use crate::output::{render_error_with_notes, render_warnings, to_json, Rendered};
use crate::ui::{pad, short_id, Badge, Glyph, Tone, Ui, MARGIN};

pub const ROBUSTNESS_SCHEMA: &str = "yukti.cli.robustness/v1";

/// Badge and tone for a robustness class.
pub fn class_style(class: &str) -> (Badge, Tone) {
    match class {
        "ROBUST" => (Badge::Ok, Tone::Ok),
        "SENSITIVE" => (Badge::Warn, Tone::Warn),
        _ => (Badge::Flag, Tone::Flag),
    }
}

/// Sort key: fragile first.
pub fn class_rank(class: &str) -> u8 {
    match class {
        "FRAGILE" => 0,
        "SENSITIVE" => 1,
        _ => 2,
    }
}

fn human_seconds(s: f64) -> String {
    if s < 90.0 {
        format!("{s:.0} s")
    } else if s < 5400.0 {
        format!("{:.0} min {:02.0} s", (s / 60.0).floor(), s % 60.0)
    } else {
        format!("{:.1} h", s / 3600.0)
    }
}

/// One breaking-point line: label, meter against the ROBUST band, value, explanation.
fn render_point(p: &Value, bands: &Value, ui: &Ui) -> String {
    let dim = p["dimension"].as_str().unwrap_or("");
    let applicable = p["applicable"].as_bool().unwrap_or(false);
    let margin = p["margin"].as_f64();
    let (label, value, scale, classified) = match dim {
        "CLOCK_OFFSET" => (
            "Clock",
            margin.map(human_seconds),
            bands["clock_robust_from_seconds"].as_f64().unwrap_or(900.0),
            true,
        ),
        "LOCATION_ERROR" => (
            "Location",
            margin.map(|m| format!("{m:.2} km")),
            bands["location_robust_from_km"].as_f64().unwrap_or(10.0),
            true,
        ),
        _ => (
            "Threshold",
            margin.map(|m| format!("{m:.0} km/h")),
            1.0,
            false,
        ),
    };
    let mut out = String::new();
    let rail = ui.paint(Tone::Line, ui.g(Glyph::Rail));
    let lead = format!(
        "{MARGIN}{rail}   {}",
        ui.paint(Tone::Muted, &pad(label, 11))
    );
    match (applicable, value) {
        (true, Some(v)) if classified => {
            let ratio = margin.unwrap_or(0.0) / scale;
            let tone = if ratio >= 1.0 {
                Tone::Ok
            } else if ratio >= 0.13 {
                Tone::Warn
            } else {
                Tone::Flag
            };
            out.push_str(&format!(
                "{lead}{}  {}\n",
                ui.meter(ratio, 14, tone),
                ui.bold(tone, &v)
            ));
        }
        (true, Some(v)) => out.push_str(&format!(
            "{lead}{}  {}\n",
            ui.paint(Tone::Subtle, &" ".repeat(14)),
            ui.paint(Tone::Muted, &format!("{v} (reference benchmark)"))
        )),
        _ => out.push_str(&format!(
            "{lead}{}  {}\n",
            ui.paint(Tone::Subtle, &" ".repeat(14)),
            ui.paint(Tone::Subtle, "not applicable")
        )),
    }
    let cont = format!("{MARGIN}{rail}   {}", " ".repeat(11));
    out.push_str(&ui.hang(
        &cont,
        &cont,
        MARGIN.len() + 15,
        Tone::Subtle,
        p["explanation"].as_str().unwrap_or(""),
    ));
    out
}

/// One lead card.
pub fn render_lead(l: &Value, bands: &Value, ui: &Ui) -> String {
    let s = |k: &str| l[k].as_str().unwrap_or("").to_string();
    let class = s("robustness_class");
    let (badge, tone) = class_style(&class);
    let mut out = format!(
        "{MARGIN}{}  {}  {}\n",
        ui.badge(badge, &class),
        ui.bold(Tone::Strong, &s("lead_type")),
        ui.paint(Tone::Subtle, &short_id(&s("lead_id"), ui))
    );
    let rail = ui.paint(Tone::Line, ui.g(Glyph::Rail));
    let mut facts = vec![format!(
        "{} {} {}",
        s("event_a_id"),
        ui.g(Glyph::Arrow),
        s("event_b_id")
    )];
    if let Some(d) = l["distance_km"].as_f64() {
        facts.push(format!("{d:.2} km"));
    }
    facts.push(format!(
        "at most {} available",
        human_seconds(l["max_possible_elapsed_seconds"].as_f64().unwrap_or(0.0))
    ));
    facts.push(if l["same_clock"].as_bool() == Some(true) {
        "one exhibit, one clock".to_string()
    } else {
        "two exhibits, two clocks".to_string()
    });
    let cont = format!("{MARGIN}{rail}   ");
    out.push_str(&ui.hang(
        &cont,
        &cont,
        MARGIN.len() + 4,
        Tone::Muted,
        &facts.join(&format!(" {} ", ui.g(Glyph::Dot))),
    ));
    for p in l["breaking_points"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[])
    {
        out.push_str(&render_point(p, bands, ui));
    }
    out.push_str(&ui.hang(&cont, &cont, MARGIN.len() + 4, tone, &s("verdict")));
    let deps: Vec<&str> = l["assumption_dependencies"]
        .as_array()
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    if !deps.is_empty() {
        out.push_str(&format!(
            "{cont}{} {}\n",
            ui.mark(Glyph::Assumed, Tone::Warn),
            ui.paint(Tone::Warn, &format!("rests on: {}", deps.join(", ")))
        ));
    }
    out.push_str(&format!(
        "{cont}{}\n",
        if l["boundary_verified"].as_bool() == Some(true) {
            format!(
                "{} {}",
                ui.mark(Glyph::Check, Tone::Ok),
                ui.paint(
                    Tone::Subtle,
                    "boundary verified: recomputed just inside and just outside each margin"
                )
            )
        } else {
            format!(
                "{} {}",
                ui.mark(Glyph::Assumed, Tone::Warn),
                ui.paint(
                    Tone::Warn,
                    "boundary could not be confirmed by recomputation"
                )
            )
        }
    ));
    out.push_str(&format!(
        "{MARGIN}{}\n",
        ui.paint(Tone::Line, ui.g(Glyph::RailEnd))
    ));
    out
}

pub async fn run(args: &CaseArgs, client: &ApiClient, json: bool, ui: &Ui) -> Rendered {
    let (case_id, case) = match require_case(&args.case_id, client, json, "requested").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let warnings: Vec<String> = super::placeholder_warning(&case_id, &case)
        .into_iter()
        .collect();
    let path = case_robustness_path(&case_id);
    let raw = match client.get_json(&path).await {
        Ok(v) => v,
        Err(err) => {
            let notes = if matches!(err, crate::error::CliError::Http { status: 404, .. }) {
                vec![
                    "This API version has no robustness endpoint; update the YUKTI server."
                        .to_string(),
                ]
            } else {
                Vec::new()
            };
            return render_error_with_notes(&err, json, &notes);
        }
    };
    if json {
        let doc = CaseViewDocument {
            schema: ROBUSTNESS_SCHEMA,
            api_url: client.base_url(),
            case_id: case_id.as_str(),
            request: RequestDoc {
                method: "GET",
                path: &path,
                body: None,
            },
            response: &raw,
            warnings: &warnings,
        };
        return Rendered::stdout(to_json(&doc), exit::SUCCESS);
    }
    let n = |k: &str| raw[k].as_u64().unwrap_or(0);
    let mut out = ui.header(&["robustness"], case_id.as_str());
    out.push('\n');
    out.push_str(&ui.para(
        MARGIN.len(),
        Tone::Muted,
        "For each contradiction lead: how wrong would an assumption have to be for the lead to disappear?",
    ));
    out.push('\n');
    out.push_str(&ui.stats(&[
        (n("total_leads").to_string(), "leads", Tone::Strong),
        (n("robust_count").to_string(), "robust", Tone::Ok),
        (
            n("sensitive_count").to_string(),
            "sensitive",
            if n("sensitive_count") > 0 {
                Tone::Warn
            } else {
                Tone::Muted
            },
        ),
        (
            n("fragile_count").to_string(),
            "fragile",
            if n("fragile_count") > 0 {
                Tone::Flag
            } else {
                Tone::Muted
            },
        ),
    ]));
    let bands = &raw["bands"];
    out.push_str(&ui.kv(
        "Bands",
        Tone::Subtle,
        &format!(
            "FRAGILE below {} clock or {} km location; ROBUST from {} clock and {} km",
            human_seconds(bands["clock_fragile_below_seconds"].as_f64().unwrap_or(120.0)),
            bands["location_fragile_below_km"].as_f64().unwrap_or(2.0),
            human_seconds(bands["clock_robust_from_seconds"].as_f64().unwrap_or(900.0)),
            bands["location_robust_from_km"].as_f64().unwrap_or(10.0),
        ),
    ));
    let mut leads: Vec<&Value> = raw["leads"]
        .as_array()
        .map(|a| a.iter().collect())
        .unwrap_or_default();
    leads.sort_by_key(|l| class_rank(l["robustness_class"].as_str().unwrap_or("")));
    out.push_str(&ui.section("Leads", "weakest first"));
    if leads.is_empty() {
        out.push_str(&format!(
            "{MARGIN}{}\n",
            ui.paint(Tone::Muted, "No contradiction leads to analyse.")
        ));
    }
    for l in leads {
        out.push_str(&render_lead(l, bands, ui));
    }
    out.push_str(&ui.footnote(raw["disclaimer"].as_str().unwrap_or("")));
    out.push_str(&ui.next(
        "defence view",
        &format!("yukti challenge --case {}", case_id.as_str()),
    ));
    Rendered {
        stdout: out,
        stderr: render_warnings(&warnings),
        exit_code: exit::SUCCESS,
    }
}
