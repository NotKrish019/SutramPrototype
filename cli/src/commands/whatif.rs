//! `yukti whatif`: stateless hypothetical scenario simulation.
//!
//! API contract (`backend-rust/src/reconstruction/scenario.rs`):
//! `POST /api/v1/cases/:case_id/what-if` with a `ScenarioSimulationRequest`
//! `{ scenario_name?, assumptions: ScenarioAssumptions, raw_events? }` returns a
//! `ScenarioSimulationResponse` comparing the scenario with the baseline reconstruction.
//!
//! Forensic rules: the simulation is hypothetical and the API does not store it. The CLI sends
//! only the alternative assumptions you give (validated as finite numbers), shows the result as
//! HYPOTHETICAL, and never presents a simulated finding as an observed one.

use serde::Serialize;
use serde_json::{Map, Value};

use super::{require_case, RequestDoc};
use crate::cli::WhatIfArgs;
use crate::client::{case_whatif_path, ApiClient};
use crate::dto::ReconstructionResult;
use crate::error::{exit, CliError};
use crate::output::{render_error_with_notes, render_warnings, to_json, Rendered};
use crate::ui::{short_id, Badge, Glyph, Tone, Ui, MARGIN};

pub const WHATIF_SCHEMA: &str = "yukti.cli.whatif/v1";

fn finite(raw: &str, option: &str) -> Result<f64, CliError> {
    raw.trim()
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .ok_or_else(|| {
            CliError::InvalidArgument(format!("{option} must be a finite number, got {raw:?}"))
        })
}

fn number(v: f64) -> Value {
    serde_json::Number::from_f64(v).map_or(Value::Null, Value::Number)
}

/// Builds the request body from `--scenario-file` (optional) and the flags. Flags override the
/// file's assumptions. No network access.
pub fn build_request(args: &WhatIfArgs) -> Result<Value, CliError> {
    let mut body: Map<String, Value> =
        match &args.scenario_file {
            None => Map::new(),
            Some(path) => {
                let text = std::fs::read_to_string(path).map_err(|e| {
                    CliError::InvalidArgument(format!(
                        "cannot read --scenario-file {}: {e}",
                        path.display()
                    ))
                })?;
                match serde_json::from_str::<Value>(&text) {
                    Ok(Value::Object(map)) => map,
                    Ok(_) => return Err(CliError::InvalidArgument(
                        "--scenario-file must contain a JSON object (ScenarioSimulationRequest)"
                            .to_string(),
                    )),
                    Err(e) => {
                        return Err(CliError::InvalidArgument(format!(
                            "--scenario-file is not valid JSON ({e})"
                        )))
                    }
                }
            }
        };
    let mut assumptions = match body.remove("assumptions") {
        None | Some(Value::Null) => Map::new(),
        Some(Value::Object(m)) => m,
        Some(_) => {
            return Err(CliError::InvalidArgument(
                "--scenario-file: `assumptions` must be a JSON object".to_string(),
            ))
        }
    };

    if let Some(raw) = &args.clock_offset {
        assumptions.insert(
            "default_clock_offset_seconds".into(),
            number(finite(raw, "--clock-offset")?),
        );
    }
    if !args.exhibit_offset.is_empty() {
        let mut map = match assumptions.remove("exhibit_clock_offsets") {
            Some(Value::Object(m)) => m,
            _ => Map::new(),
        };
        for pair in &args.exhibit_offset {
            let (id, secs) = pair.split_once('=').ok_or_else(|| {
                CliError::InvalidArgument(format!(
                    "--exhibit-offset must be EVIDENCE_ID=SECONDS, got {pair:?}"
                ))
            })?;
            let id = id.trim();
            if id.is_empty() {
                return Err(CliError::InvalidArgument(format!(
                    "--exhibit-offset has an empty evidence ID: {pair:?}"
                )));
            }
            map.insert(id.to_string(), number(finite(secs, "--exhibit-offset")?));
        }
        assumptions.insert("exhibit_clock_offsets".into(), Value::Object(map));
    }
    if let Some(tz) = &args.timezone {
        if tz.trim().is_empty() {
            return Err(CliError::InvalidArgument(
                "--timezone must not be blank".to_string(),
            ));
        }
        assumptions.insert(
            "default_timezone".into(),
            Value::String(tz.trim().to_string()),
        );
    }
    if let Some(raw) = &args.speed_threshold {
        let v = finite(raw, "--speed-threshold")?;
        if v <= 0.0 {
            return Err(CliError::InvalidArgument(
                "--speed-threshold must be greater than 0 km/h".to_string(),
            ));
        }
        assumptions.insert("speed_threshold_kmh".into(), number(v));
    }
    if let Some(raw) = &args.half_width {
        let v = finite(raw, "--half-width")?;
        if v < 0.0 {
            return Err(CliError::InvalidArgument(
                "--half-width must not be negative".to_string(),
            ));
        }
        assumptions.insert("estimated_half_width_seconds".into(), number(v));
    }
    for (key, values, option) in [
        (
            "excluded_evidence_ids",
            &args.exclude_evidence,
            "--exclude-evidence",
        ),
        ("excluded_event_ids", &args.exclude_event, "--exclude-event"),
    ] {
        if values.is_empty() {
            continue;
        }
        let mut list = match assumptions.remove(key) {
            Some(Value::Array(a)) => a,
            _ => Vec::new(),
        };
        for v in values {
            if v.trim().is_empty() {
                return Err(CliError::InvalidArgument(format!(
                    "{option} must not be blank"
                )));
            }
            let item = Value::String(v.trim().to_string());
            if !list.contains(&item) {
                list.push(item);
            }
        }
        assumptions.insert(key.into(), Value::Array(list));
    }

    let has_events = body
        .get("raw_events")
        .and_then(Value::as_array)
        .is_some_and(|a| !a.is_empty());
    if assumptions.is_empty() && !has_events {
        return Err(CliError::InvalidArgument(
            "give at least one alternative assumption (e.g. --clock-offset, --timezone, --speed-threshold, --exclude-evidence) or a --scenario-file; otherwise the scenario equals the baseline".to_string(),
        ));
    }
    if let Some(name) = &args.name {
        if name.trim().is_empty() {
            return Err(CliError::InvalidArgument(
                "--name must not be blank".to_string(),
            ));
        }
        body.insert(
            "scenario_name".into(),
            Value::String(name.trim().to_string()),
        );
    }
    body.insert("assumptions".into(), Value::Object(assumptions));
    Ok(Value::Object(body))
}

#[derive(Serialize)]
struct WhatIfDocument<'a> {
    schema: &'static str,
    api_url: &'a str,
    case_id: &'a str,
    request: RequestDoc<'a>,
    /// Always true for this command: the API does not store simulations.
    hypothetical: bool,
    response: &'a Value,
    warnings: &'a [String],
}

pub async fn run(args: &WhatIfArgs, client: &ApiClient, json: bool, ui: &Ui) -> Rendered {
    let body = match build_request(args) {
        Ok(b) => b,
        Err(err) => return render_error_with_notes(&err, json, &[]),
    };
    let (case_id, case) = match require_case(&args.case_id, client, json, "simulated").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let path = case_whatif_path(&case_id);
    let raw = match client.post_json(&path, &body).await {
        Ok(v) => v,
        Err(err) => {
            let note = "Simulations are stateless: nothing was stored and the baseline reconstruction is unchanged.".to_string();
            return render_error_with_notes(&err, json, &[note]);
        }
    };
    let mut warnings: Vec<String> = super::placeholder_warning(&case_id, &case)
        .into_iter()
        .collect();
    if raw["is_hypothetical"].as_bool() != Some(true) {
        warnings.push(
            "The API did not mark this result as hypothetical (is_hypothetical != true); it is a simulation all the same and must not be read as observed.".to_string(),
        );
    }
    if json {
        let doc = WhatIfDocument {
            schema: WHATIF_SCHEMA,
            api_url: client.base_url(),
            case_id: case_id.as_str(),
            request: RequestDoc {
                method: "POST",
                path: &path,
                body: Some(&body),
            },
            hypothetical: true,
            response: &raw,
            warnings: &warnings,
        };
        return Rendered::stdout(to_json(&doc), exit::SUCCESS);
    }
    let mut out = ui.header(&["whatif"], case_id.as_str());
    out.push_str(&render_simulation(&raw, ui));
    Rendered {
        stdout: out,
        stderr: render_warnings(&warnings),
        exit_code: exit::SUCCESS,
    }
}

fn items(v: &Value) -> &[Value] {
    v.as_array().map(Vec::as_slice).unwrap_or(&[])
}

fn finding_line(f: &Value, glyph: Glyph, tone: Tone, sign: &str, ui: &Ui) -> String {
    let s = |k: &str| f[k].as_str().unwrap_or("").to_string();
    let events: Vec<&str> = items(&f["event_ids"])
        .iter()
        .filter_map(Value::as_str)
        .collect();
    let mut out = format!(
        "{MARGIN}{} {} {}  {}\n",
        ui.mark(glyph, tone),
        ui.bold(tone, sign),
        ui.bold(Tone::Strong, &s("finding_type")),
        ui.paint(Tone::Subtle, &s("finding_id"))
    );
    let rail = ui.paint(Tone::Line, ui.g(Glyph::Rail));
    let cont = format!("{MARGIN}{rail}   ");
    out.push_str(&format!(
        "{cont}{}\n",
        ui.paint(
            Tone::Muted,
            &format!(
                "entity {}  {}  events {}",
                short_id(&s("entity_id"), ui),
                ui.g(Glyph::Dot),
                events.join(", ")
            )
        )
    ));
    let summary = s("summary");
    if !summary.is_empty() {
        out.push_str(&ui.hang(&cont, &cont, MARGIN.len() + 4, Tone::Text, &summary));
    }
    out
}

/// Themed `ScenarioSimulationResponse`.
pub fn render_simulation(raw: &Value, ui: &Ui) -> String {
    let s = |k: &str| raw[k].as_str().unwrap_or("").to_string();
    let c = &raw["comparison"];
    let appearing = items(&c["findings_newly_appearing"]);
    let disappearing = items(&c["findings_disappearing"]);
    let changed = items(&c["findings_robustness_changed"]);
    let unchanged = items(&c["findings_unchanged"]);

    let mut out = String::from("\n");
    out.push_str(&format!(
        "{MARGIN}{}  {}\n",
        ui.badge(Badge::Warn, "hypothetical"),
        ui.bold(
            Tone::Strong,
            &format!("Scenario \"{}\"", s("scenario_name"))
        )
    ));
    out.push_str(&ui.para(
        MARGIN.len(),
        Tone::Subtle,
        &format!(
            "baseline {} {} scenario {} {} simulated {} {} not stored",
            s("baseline_run_id"),
            ui.g(Glyph::Dot),
            s("scenario_run_id"),
            ui.g(Glyph::Dot),
            s("simulated_at"),
            ui.g(Glyph::Dot)
        ),
    ));

    out.push_str(&ui.section("Assumptions applied", "alternatives to the baseline"));
    let applied = items(&c["applied_assumptions_summary"]);
    if applied.is_empty() {
        out.push_str(&format!(
            "{MARGIN}{}\n",
            ui.paint(Tone::Muted, "none listed by the API")
        ));
    }
    for a in applied.iter().filter_map(Value::as_str) {
        let pre = format!("{MARGIN}{} ", ui.mark(Glyph::Assumed, Tone::Warn));
        out.push_str(&ui.hang(
            &pre,
            &" ".repeat(MARGIN.len() + 2),
            MARGIN.len() + 2,
            Tone::Warn,
            a,
        ));
    }

    out.push_str(&ui.section("Delta against baseline", ""));
    out.push_str(&ui.stats(&[
        (
            format!("+{}", appearing.len()),
            "appearing",
            if appearing.is_empty() {
                Tone::Muted
            } else {
                Tone::Flag
            },
        ),
        (
            format!("-{}", disappearing.len()),
            "disappearing",
            if disappearing.is_empty() {
                Tone::Muted
            } else {
                Tone::Ok
            },
        ),
        (
            format!("~{}", changed.len()),
            "changed",
            if changed.is_empty() {
                Tone::Muted
            } else {
                Tone::Warn
            },
        ),
        (format!("={}", unchanged.len()), "unchanged", Tone::Muted),
    ]));
    let robust = unchanged.len() + changed.len();
    let baseline_total = robust + disappearing.len();
    if baseline_total > 0 {
        out.push_str(&ui.kv_painted(
            "Persistence",
            &format!(
                "{}  {}",
                ui.meter(robust as f64 / baseline_total as f64, 20, Tone::Inferred),
                ui.paint(
                    Tone::Muted,
                    &format!("{robust} of {baseline_total} baseline finding(s) persist under this scenario")
                )
            ),
        ));
    }

    if !appearing.is_empty() {
        out.push_str(&ui.section("Newly appearing", "emerge only under the scenario"));
        for f in appearing {
            out.push_str(&finding_line(f, Glyph::Flag, Tone::Flag, "+", ui));
        }
    }
    if !disappearing.is_empty() {
        out.push_str(&ui.section("Disappearing", "baseline findings that resolve"));
        for f in disappearing {
            out.push_str(&finding_line(f, Glyph::Inferred, Tone::Ok, "-", ui));
        }
    }
    if !changed.is_empty() {
        out.push_str(&ui.section("Robustness changed", "metric baseline → scenario"));
        for d in changed {
            let base = d["baseline_value"].as_f64().unwrap_or(f64::NAN);
            let scen = d["scenario_value"].as_f64().unwrap_or(f64::NAN);
            let delta = scen - base;
            out.push_str(&format!(
                "{MARGIN}{} {}  {}  {}\n",
                ui.mark(Glyph::Assumed, Tone::Warn),
                ui.bold(Tone::Strong, d["finding_type"].as_str().unwrap_or("")),
                ui.paint(Tone::Subtle, d["finding_id"].as_str().unwrap_or("")),
                ui.paint(
                    Tone::Warn,
                    &format!(
                        "{} {base:.3} {} {scen:.3} ({}{delta:.3})",
                        d["metric_name"].as_str().unwrap_or("metric"),
                        ui.g(Glyph::Arrow),
                        if delta >= 0.0 { "+" } else { "" }
                    )
                )
            ));
            if let Some(e) = d["explanation"].as_str() {
                let cont = format!("{MARGIN}{}   ", ui.paint(Tone::Line, ui.g(Glyph::Rail)));
                out.push_str(&ui.hang(&cont, &cont, MARGIN.len() + 4, Tone::Muted, e));
            }
        }
    }
    let explanations = items(&c["explanations"]);
    if !explanations.is_empty() {
        out.push_str(&ui.section("Explanations", ""));
        for e in explanations.iter().filter_map(Value::as_str) {
            let pre = format!("{MARGIN}{} ", ui.mark(Glyph::Supplied, Tone::Supplied));
            out.push_str(&ui.hang(
                &pre,
                &" ".repeat(MARGIN.len() + 2),
                MARGIN.len() + 2,
                Tone::Muted,
                e,
            ));
        }
    }
    if let Ok(sim) =
        serde_json::from_value::<ReconstructionResult>(raw["simulated_reconstruction"].clone())
    {
        let sum = super::reconstruction::ReconstructionSummary::of(&sim);
        out.push_str(&ui.section("Simulated reconstruction", "hypothetical"));
        out.push_str(&ui.stats(&[
            (
                sum.timeline_total_events.to_string(),
                "events",
                Tone::Strong,
            ),
            (
                sum.spatial_movement_segments.to_string(),
                "movements",
                Tone::Muted,
            ),
            (
                sum.contradiction_leads.to_string(),
                "leads",
                if sum.contradiction_leads > 0 {
                    Tone::Flag
                } else {
                    Tone::Muted
                },
            ),
            (sum.assumptions.to_string(), "assumptions", Tone::Warn),
        ]));
    }
    out.push_str(&ui.footnote(
        "Simulated findings are hypothetical: they show how conclusions depend on assumptions, not what happened.",
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::WhatIfArgs;
    use serde_json::json;

    fn args() -> WhatIfArgs {
        WhatIfArgs {
            case_id: "c".into(),
            name: None,
            clock_offset: None,
            exhibit_offset: vec![],
            timezone: None,
            speed_threshold: None,
            half_width: None,
            exclude_evidence: vec![],
            exclude_event: vec![],
            scenario_file: None,
        }
    }

    #[test]
    fn builds_only_the_assumptions_given() {
        let mut a = args();
        a.clock_offset = Some("-120".into());
        a.exhibit_offset = vec!["ev1=30".into()];
        a.exclude_event = vec!["evt-9".into(), "evt-9".into()];
        a.name = Some("Clock skew".into());
        let body = build_request(&a).unwrap();
        assert_eq!(
            body,
            json!({
                "scenario_name": "Clock skew",
                "assumptions": {
                    "default_clock_offset_seconds": -120.0,
                    "exhibit_clock_offsets": {"ev1": 30.0},
                    "excluded_event_ids": ["evt-9"]
                }
            })
        );
    }

    #[test]
    fn rejects_empty_and_invalid_scenarios() {
        let err = build_request(&args()).unwrap_err();
        assert!(err
            .to_string()
            .contains("at least one alternative assumption"));
        for (field, value, expected) in [
            ("clock", "NaN", "--clock-offset"),
            ("speed", "0", "greater than 0"),
            ("speed", "fast", "--speed-threshold"),
            ("half", "-1", "must not be negative"),
            ("pair", "ev1:30", "EVIDENCE_ID=SECONDS"),
        ] {
            let mut a = args();
            match field {
                "clock" => a.clock_offset = Some(value.into()),
                "speed" => a.speed_threshold = Some(value.into()),
                "half" => a.half_width = Some(value.into()),
                _ => a.exhibit_offset = vec![value.into()],
            }
            let err = build_request(&a).unwrap_err();
            assert_eq!(err.exit_code(), 2);
            assert!(err.to_string().contains(expected), "{value}: {err}");
        }
    }

    #[test]
    fn flags_override_the_scenario_file() {
        let dir = std::env::temp_dir().join(format!("yukti-cli-whatif-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("s.json");
        std::fs::write(
            &file,
            r#"{"scenario_name":"file","assumptions":{"speed_threshold_kmh":80,"default_timezone":"UTC"}}"#,
        )
        .unwrap();
        let mut a = args();
        a.scenario_file = Some(file);
        a.timezone = Some("Asia/Kolkata".into());
        let body = build_request(&a).unwrap();
        assert_eq!(body["scenario_name"], "file");
        assert_eq!(body["assumptions"]["default_timezone"], "Asia/Kolkata");
        assert_eq!(body["assumptions"]["speed_threshold_kmh"], 80);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn simulations_are_always_labelled_hypothetical() {
        let text = render_simulation(
            &json!({"scenario_name":"s","comparison":{"findings_newly_appearing":[{"finding_id":"f1","finding_type":"SPATIAL_TRANSIT_IMPOSSIBLE","entity_id":"e","event_ids":["a","b"],"summary":"x"}]}}),
            &Ui::plain(),
        );
        assert!(text.contains("HYPOTHETICAL"), "{text}");
        assert!(text.contains("+1 appearing"), "{text}");
        assert!(text.contains("SPATIAL_TRANSIT_IMPOSSIBLE"), "{text}");
    }
}
