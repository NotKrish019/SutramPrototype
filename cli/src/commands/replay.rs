//! `yukti replay`: the reconstructed timeline played back event by event over a terminal map.
//!
//! Reads `GET …/reconstruction` (timeline + leads) and `GET …/robustness`. Each frame shows the
//! event, its source, the map of every location visited so far (current one highlighted) and,
//! when the step into this event is a contradiction lead, the lead and its robustness class.
//! Locations are plotted on a simple longitude/latitude grid; tower centroids are marked as
//! inferred. Nothing is interpolated between records: the map shows only recorded positions.

use std::io::Write;

use serde_json::Value;

use super::require_case;
use crate::cli::ReplayArgs;
use crate::client::{
    case_evidence_path, case_reconstruction_path, case_robustness_path, ApiClient,
};
use crate::error::exit;
use crate::output::{render_error_with_notes, render_warnings, to_json, Rendered};
use crate::ui::{pad, truncate, Glyph, Tone, Ui, MARGIN};

pub const REPLAY_SCHEMA: &str = "yukti.cli.replay/v1";

/// One recorded position on the map.
#[derive(Debug, Clone, PartialEq)]
struct Point {
    lat: f64,
    lon: f64,
    inferred: bool,
}

/// Everything needed to draw the frames.
pub struct Replay {
    entries: Vec<Value>,
    files: Vec<(String, String)>,
    leads: Vec<Value>,
}

fn point(e: &Value) -> Option<Point> {
    let c = e["coordinates"].as_object()?;
    Some(Point {
        lat: c.get("lat")?.as_f64()?,
        lon: c.get("lon")?.as_f64()?,
        inferred: e["assumptions"].as_array().is_some_and(|a| {
            a.iter()
                .any(|x| x["kind"] == "COORDINATES_INFERRED_FROM_TOWER_ID")
        }),
    })
}

impl Replay {
    pub fn new(reconstruction: &Value, evidence: &Value, robustness: Option<&Value>) -> Self {
        let leads = match robustness.and_then(|r| r["leads"].as_array()) {
            Some(l) => l.clone(),
            None => reconstruction["contradictions"]["leads"]
                .as_array()
                .cloned()
                .unwrap_or_default(),
        };
        Self {
            entries: reconstruction["timeline"]["entries"]
                .as_array()
                .cloned()
                .unwrap_or_default(),
            files: evidence
                .as_array()
                .into_iter()
                .flatten()
                .map(|e| {
                    (
                        e["evidence_id"].as_str().unwrap_or("").to_string(),
                        e["original_filename"].as_str().unwrap_or("").to_string(),
                    )
                })
                .collect(),
            leads,
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn file_of(&self, e: &Value) -> String {
        let id = e["source_evidence_id"].as_str().unwrap_or("");
        self.files
            .iter()
            .find(|(i, _)| i == id)
            .map(|(_, f)| f.clone())
            .unwrap_or_else(|| id.to_string())
    }

    fn leads_into(&self, event_id: &str) -> Vec<&Value> {
        self.leads
            .iter()
            .filter(|l| l["event_b_id"].as_str() == Some(event_id))
            .collect()
    }

    fn bounds(&self) -> Option<(f64, f64, f64, f64)> {
        let pts: Vec<Point> = self.entries.iter().filter_map(point).collect();
        if pts.is_empty() {
            return None;
        }
        let min_lat = pts.iter().map(|p| p.lat).fold(f64::MAX, f64::min);
        let max_lat = pts.iter().map(|p| p.lat).fold(f64::MIN, f64::max);
        let min_lon = pts.iter().map(|p| p.lon).fold(f64::MAX, f64::min);
        let max_lon = pts.iter().map(|p| p.lon).fold(f64::MIN, f64::max);
        Some((min_lat, max_lat, min_lon, max_lon))
    }

    fn map(&self, upto: usize, ui: &Ui) -> String {
        let Some((min_lat, max_lat, min_lon, max_lon)) = self.bounds() else {
            return format!(
                "{MARGIN}{}\n",
                ui.paint(Tone::Subtle, "No located events: nothing to map.")
            );
        };
        let w = ui.width.saturating_sub(10).clamp(30, 72);
        let h = 12usize;
        let cell = |p: &Point| -> (usize, usize) {
            let x = if (max_lon - min_lon).abs() < 1e-9 {
                w / 2
            } else {
                (((p.lon - min_lon) / (max_lon - min_lon)) * (w - 1) as f64).round() as usize
            };
            let y = if (max_lat - min_lat).abs() < 1e-9 {
                h / 2
            } else {
                (((max_lat - p.lat) / (max_lat - min_lat)) * (h - 1) as f64).round() as usize
            };
            (x.min(w - 1), y.min(h - 1))
        };
        let mut grid: Vec<Vec<Option<(Glyph, Tone)>>> = vec![vec![None; w]; h];
        for (i, e) in self.entries.iter().enumerate().take(upto + 1) {
            if let Some(p) = point(e) {
                let (x, y) = cell(&p);
                let mark = if i == upto {
                    (Glyph::Observed, Tone::Brand)
                } else if p.inferred {
                    (Glyph::Inferred, Tone::Inferred)
                } else {
                    (Glyph::Observed, Tone::Muted)
                };
                if grid[y][x].is_none_or(|(_, t)| t != Tone::Brand) {
                    grid[y][x] = Some(mark);
                }
            }
        }
        let line = ui.g(Glyph::Rule).repeat(w);
        let mut out = format!("{MARGIN}{}\n", ui.paint(Tone::Line, &format!(" {line} ")));
        for row in grid {
            out.push_str(MARGIN);
            out.push_str(&ui.paint(Tone::Line, ui.g(Glyph::Rail)));
            for c in row {
                match c {
                    Some((g, t)) => out.push_str(&ui.mark(g, t)),
                    None => out.push_str(&ui.paint(Tone::Line, if ui.unicode { "·" } else { "." })),
                }
            }
            out.push_str(&ui.paint(Tone::Line, ui.g(Glyph::Rail)));
            out.push('\n');
        }
        out.push_str(&format!(
            "{MARGIN}{}\n",
            ui.paint(Tone::Line, &format!(" {line} "))
        ));
        out.push_str(&format!(
            "{MARGIN}{}\n",
            ui.paint(
                Tone::Subtle,
                &format!("lat {min_lat:.3}..{max_lat:.3}  lon {min_lon:.3}..{max_lon:.3}  (not to scale for distance)")
            )
        ));
        out
    }

    /// One frame for event `i`.
    pub fn frame(&self, i: usize, case_id: &str, ui: &Ui) -> String {
        let e = &self.entries[i];
        let mut out = ui.header(&["replay"], case_id);
        out.push('\n');
        let raw = e["raw_timestamp"].as_str().unwrap_or("");
        let local = raw
            .split_once('T')
            .map(|(d, t)| format!("{d} {}", t.get(0..5).unwrap_or(t)))
            .unwrap_or_default();
        out.push_str(&format!(
            "{MARGIN}{}  {}  {}\n",
            ui.bold(Tone::Strong, &local),
            ui.paint(Tone::Subtle, &format!("event {} of {}", i + 1, self.len())),
            ui.paint(Tone::Muted, &self.file_of(e))
        ));
        let desc = e["raw_description"].as_str().unwrap_or("(no description)");
        out.push_str(&ui.para(
            MARGIN.len(),
            Tone::Text,
            &truncate(&desc.replace('\n', " "), 200, ui.g(Glyph::Ellipsis)),
        ));
        let written = e["metadata"]["original_timestamp_text"]
            .as_str()
            .unwrap_or(raw);
        let place = match point(e) {
            Some(p) if p.inferred => {
                format!("{:.4}, {:.4} (tower centroid: inferred)", p.lat, p.lon)
            }
            Some(p) => format!("{:.4}, {:.4}", p.lat, p.lon),
            None => "no location in this record".into(),
        };
        out.push_str(&ui.para(
            MARGIN.len(),
            Tone::Subtle,
            &format!(
                "time as written \"{written}\"  {}  {place}",
                ui.g(Glyph::Dot)
            ),
        ));
        out.push('\n');
        out.push_str(&self.map(i, ui));
        for l in self.leads_into(e["event_id"].as_str().unwrap_or("")) {
            let class = l["robustness_class"].as_str().unwrap_or("LEAD");
            let tone = match class {
                "ROBUST" => Tone::Ok,
                "SENSITIVE" => Tone::Warn,
                _ => Tone::Flag,
            };
            let msg = l["verdict"]
                .as_str()
                .or_else(|| l["neutral_explanation"].as_str())
                .unwrap_or("");
            out.push_str(&ui.hang(
                &format!(
                    "{MARGIN}{} {} ",
                    ui.mark(Glyph::Flag, Tone::Flag),
                    ui.bold(tone, &pad(class, 9))
                ),
                &" ".repeat(MARGIN.len() + 12),
                MARGIN.len() + 12,
                Tone::Text,
                &truncate(msg, 220, ui.g(Glyph::Ellipsis)),
            ));
        }
        out.push('\n');
        out.push_str(&format!(
            "{MARGIN}{}  {}\n",
            ui.meter((i + 1) as f64 / self.len().max(1) as f64, 30, Tone::Brand),
            ui.paint(Tone::Subtle, &format!("{}/{}", i + 1, self.len()))
        ));
        out
    }

    /// The whole replay as text (no animation): the last frame plus the event list.
    pub fn summary(&self, case_id: &str, ui: &Ui) -> String {
        if self.is_empty() {
            let mut out = ui.header(&["replay"], case_id);
            out.push_str(&format!(
                "\n{MARGIN}{}\n",
                ui.paint(Tone::Muted, "The timeline has no events to replay.")
            ));
            return out;
        }
        let mut out = self.frame(self.len() - 1, case_id, ui);
        out.push_str(&ui.section("All events", &format!("{}", self.len())));
        for (i, e) in self.entries.iter().enumerate() {
            let raw = e["raw_timestamp"].as_str().unwrap_or("");
            let hm = raw
                .split_once('T')
                .and_then(|(_, t)| t.get(0..5))
                .unwrap_or("");
            let has_lead = !self
                .leads_into(e["event_id"].as_str().unwrap_or(""))
                .is_empty();
            out.push_str(&format!(
                "{MARGIN}{} {}  {}  {}\n",
                if has_lead {
                    ui.mark(Glyph::Flag, Tone::Flag)
                } else if point(e).is_some() {
                    ui.mark(Glyph::Observed, Tone::Text)
                } else {
                    ui.mark(Glyph::Inferred, Tone::Subtle)
                },
                ui.paint(Tone::Strong, &format!("{:>2}. {hm}", i + 1)),
                ui.paint(
                    Tone::Muted,
                    &pad(&truncate(&self.file_of(e), 26, ui.g(Glyph::Ellipsis)), 26)
                ),
                ui.paint(
                    Tone::Text,
                    &truncate(
                        &e["raw_description"]
                            .as_str()
                            .unwrap_or("")
                            .replace('\n', " "),
                        60,
                        ui.g(Glyph::Ellipsis)
                    )
                )
            ));
        }
        out
    }
}

async fn load(
    args: &ReplayArgs,
    client: &ApiClient,
    json: bool,
) -> Result<(String, Value, Replay, Vec<String>), Rendered> {
    let (case_id, case) = require_case(&args.case_id, client, json, "replayed").await?;
    let path_0 = case_reconstruction_path(&case_id);
    let path_1 = case_evidence_path(&case_id);
    let path_2 = case_robustness_path(&case_id);
    let (rec, ev, rob) = tokio::join!(
        client.get_json(&path_0),
        client.get_json(&path_1),
        client.get_json(&path_2),
    );
    let rec = rec.map_err(|e| render_error_with_notes(&e, json, &[]))?;
    let ev = ev.unwrap_or(Value::Array(vec![]));
    let replay = Replay::new(&rec, &ev, rob.as_ref().ok());
    let warnings = super::placeholder_warning(&case_id, &case)
        .into_iter()
        .collect();
    Ok((case_id.as_str().to_string(), rec, replay, warnings))
}

/// Non-animated replay (`--no-animate`, pipes, `--json`).
pub async fn run(args: &ReplayArgs, client: &ApiClient, json: bool, ui: &Ui) -> Rendered {
    let (case_id, rec, replay, warnings) = match load(args, client, json).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    if json {
        let doc = serde_json::json!({
            "schema": REPLAY_SCHEMA,
            "api_url": client.base_url(),
            "case_id": case_id,
            "events": rec["timeline"]["entries"],
            "leads": replay.leads,
        });
        return Rendered::stdout(to_json(&doc), exit::SUCCESS);
    }
    Rendered {
        stdout: replay.summary(&case_id, ui),
        stderr: render_warnings(&warnings),
        exit_code: exit::SUCCESS,
    }
}

/// Animated replay on a terminal: one frame per event, `delay` apart.
pub async fn animate(args: &ReplayArgs, client: &ApiClient, ui: &Ui) -> u8 {
    let (case_id, _rec, replay, warnings) = match load(args, client, false).await {
        Ok(v) => v,
        Err(r) => {
            let _ = std::io::stderr()
                .write_all(ui.for_stderr().style_diagnostics(&r.stderr).as_bytes());
            return r.exit_code;
        }
    };
    let mut stdout = std::io::stdout();
    if replay.is_empty() {
        let _ = stdout.write_all(replay.summary(&case_id, ui).as_bytes());
        return exit::SUCCESS;
    }
    for i in 0..replay.len() {
        let _ = write!(stdout, "\x1b[2J\x1b[H{}", replay.frame(i, &case_id, ui));
        let _ = stdout.flush();
        tokio::time::sleep(std::time::Duration::from_millis(args.delay)).await;
    }
    let _ = writeln!(stdout);
    let _ = stdout.write_all(
        ui.hint("replay finished; `yukti robustness` explains every flagged step")
            .as_bytes(),
    );
    if !warnings.is_empty() {
        let _ = std::io::stderr().write_all(
            ui.for_stderr()
                .style_diagnostics(&render_warnings(&warnings))
                .as_bytes(),
        );
    }
    exit::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn data() -> (Value, Value, Value) {
        let rec = json!({"timeline": {"entries": [
            {"event_id": "a", "source_evidence_id": "ev", "raw_timestamp": "2026-11-10T21:00:00+05:30", "raw_description": "Call at Saket",
             "coordinates": {"lat": 28.52, "lon": 77.20}, "assumptions": [{"kind": "COORDINATES_INFERRED_FROM_TOWER_ID"}], "metadata": {}},
            {"event_id": "b", "source_evidence_id": "ev", "raw_timestamp": "2026-11-10T21:30:00+05:30", "raw_description": "Call at BKC",
             "coordinates": {"lat": 19.06, "lon": 72.86}, "assumptions": [], "metadata": {}},
            {"event_id": "c", "source_evidence_id": "wa", "raw_timestamp": "2026-11-10T23:15:00", "raw_description": "Done for today.\nTalk tomorrow.",
             "metadata": {"original_timestamp_text": "10/11/2026, 11:15 pm"}}
        ]}});
        let ev = json!([{"evidence_id": "ev", "original_filename": "carrier_a_cdr.csv"}]);
        let rob = json!({"leads": [{"event_a_id": "a", "event_b_id": "b", "robustness_class": "ROBUST", "verdict": "ROBUST: survives"}]});
        (rec, ev, rob)
    }

    #[test]
    fn frames_show_the_event_its_source_and_the_lead_into_it() {
        let (rec, ev, rob) = data();
        let r = Replay::new(&rec, &ev, Some(&rob));
        assert_eq!(r.len(), 3);
        let f = r.frame(1, "case", &Ui::plain());
        assert!(f.contains("event 2 of 3"));
        assert!(f.contains("carrier_a_cdr.csv"));
        assert!(f.contains("ROBUST"));
        assert!(f.contains("19.0600, 72.8600"));
        let first = r.frame(0, "case", &Ui::plain());
        assert!(first.contains("tower centroid: inferred"));
        assert!(!first.contains("ROBUST"), "no lead into the first event");
        let last = r.frame(2, "case", &Ui::plain());
        assert!(last.contains("no location in this record"));
        assert!(last.contains("10/11/2026, 11:15 pm"));
    }

    #[test]
    fn summary_lists_every_event_and_flags_leads() {
        let (rec, ev, rob) = data();
        let s = Replay::new(&rec, &ev, Some(&rob)).summary("case", &Ui::plain());
        assert!(s.contains("ALL EVENTS"));
        assert!(s.contains(" 2. 21:30"));
        assert!(s.contains("▲  2. 21:30"));
    }
}
