//! `yukti challenge`: the case as defence counsel would attack it.
//!
//! Reads what the API already reports (reconstruction assumptions, robustness margins, evidence
//! gaps, custody audit, export safeguard) and lists each weakness with the question counsel would
//! ask and the corroboration that would close it, most serious first. The ranking rules are fixed
//! and listed in [`Severity`]; nothing is scored statistically and nothing new is inferred.

use serde::Serialize;
use serde_json::Value;

use super::require_case;
use crate::cli::CaseArgs;
use crate::client::{
    case_custody_path, case_reconstruction_path, case_robustness_path, case_view_path, ApiClient,
};
use crate::error::exit;
use crate::output::{render_warnings, to_json, Rendered};
use crate::ui::{Glyph, Tone, Ui, MARGIN};

pub const CHALLENGE_SCHEMA: &str = "yukti.cli.challenge/v1";

/// Fixed ranking of a weakness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Severity {
    /// Integrity failure: the evidence itself is in question.
    Critical,
    /// A lead an ordinary error would dissolve, or a high-severity gap.
    High,
    /// A lead sensitive to plausible errors, an assumption under many events, a medium gap.
    Medium,
    /// Context worth disclosing.
    Low,
}

/// One weakness.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Weakness {
    pub severity: Severity,
    pub area: &'static str,
    /// The question defence counsel would put.
    pub attack: String,
    /// What the record shows.
    pub basis: String,
    /// What would close it.
    pub close_it: String,
}

fn kind_text(kind: &str) -> (&'static str, &'static str, &'static str) {
    match kind {
        "TIMEZONE_NOT_DECLARED_ON_EVENT" => (
            "How do you know which timezone these timestamps are in?",
            "the event times were converted with an assumed timezone",
            "Obtain the source device or operator's timezone declaration and reprocess with --source-timezone.",
        ),
        "CLOCK_OFFSET_NOT_DECLARED_ON_EVENT" => (
            "Was the device clock ever checked against a reference?",
            "a zero clock offset was assumed, not measured",
            "Record the device's clock against an NTP or network reference at seizure and reprocess with --clock-offset-seconds.",
        ),
        "COORDINATES_INFERRED_FROM_TOWER_ID" => (
            "Is that the phone's location, or just the tower's?",
            "locations are cell-tower centroids looked up from tower IDs, not handset positions",
            "Obtain operator sector/azimuth data, a timing-advance record, or independent location evidence (CCTV, GPS).",
        ),
        "PRECISION_DEFAULTED" => (
            "How precise is that time, really?",
            "the timestamp precision was not supplied and a default was used",
            "Confirm the source's timestamp resolution and reprocess.",
        ),
        _ => (
            "What is this value based on?",
            "a value was assumed rather than observed",
            "Replace the assumed value with an observed one.",
        ),
    }
}

/// Builds the ranked weaknesses from API documents (pure; unit-tested).
pub fn weaknesses(
    reconstruction: Option<&Value>,
    robustness: Option<&Value>,
    gaps: Option<&Value>,
    custody: Option<&Value>,
) -> Vec<Weakness> {
    let mut out = Vec::new();

    // Custody.
    if let Some(c) = custody {
        for e in c["exhibits"].as_array().map(Vec::as_slice).unwrap_or(&[]) {
            if e["verified"].as_bool() == Some(false) {
                out.push(Weakness {
                    severity: Severity::Critical,
                    area: "custody",
                    attack: format!(
                        "Is exhibit {} the file that was seized?",
                        e["original_filename"].as_str().unwrap_or("?")
                    ),
                    basis: format!(
                        "re-hashing the stored file gave {}",
                        e["stored_file"].as_str().unwrap_or("?")
                    ),
                    close_it: "Re-acquire the exhibit from the original media and compare its SHA-256 with the seizure record.".into(),
                });
            }
        }
        if c["ledger_mode"].as_str() == Some("FILESYSTEM_NO_LEDGER")
            && c["exhibit_count"].as_u64().unwrap_or(0) > 0
        {
            out.push(Weakness {
                severity: Severity::Low,
                area: "custody",
                attack: "Who handled the evidence between seizure and analysis, and when?".into(),
                basis: "the system ran without its database custody ledger; integrity rests on file hashes only".into(),
                close_it: "Run YUKTI with PostgreSQL so every access is written to the hash-chained custody ledger, and keep the seizure memo.".into(),
            });
        }
    }

    // Leads and their robustness.
    if let Some(r) = robustness {
        for l in r["leads"].as_array().map(Vec::as_slice).unwrap_or(&[]) {
            let class = l["robustness_class"].as_str().unwrap_or("");
            let events = format!(
                "{} -> {}",
                l["event_a_id"].as_str().unwrap_or("?"),
                l["event_b_id"].as_str().unwrap_or("?")
            );
            let severity = match class {
                "FRAGILE" => Severity::High,
                "SENSITIVE" => Severity::Medium,
                _ => Severity::Low,
            };
            if class != "ROBUST" {
                out.push(Weakness {
                    severity,
                    area: "lead",
                    attack: format!("Couldn't an ordinary clock or location error explain the lead {events}?"),
                    basis: l["verdict"].as_str().unwrap_or("").to_string(),
                    close_it: if l["same_clock"].as_bool() == Some(true) {
                        "Corroborate the locations with an independent source (CCTV, GPS, operator sector data).".into()
                    } else {
                        "Establish both sources' clock offsets (operator NTP logs or seizure-time clock readings) and reprocess.".into()
                    },
                });
            } else if l["same_clock"].as_bool() == Some(true) {
                out.push(Weakness {
                    severity: Severity::Low,
                    area: "lead",
                    attack: format!("The lead {events} rests on one record. Couldn't that record simply be wrong?"),
                    basis: "both events come from a single exhibit; the lead is robust to errors but not independently corroborated".into(),
                    close_it: "Corroborate one of the two events with a second, independent source.".into(),
                });
            }
        }
    }

    // Assumptions under many events.
    if let Some(rec) = reconstruction {
        let entries = rec["timeline"]["entries"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let total = entries.len();
        let mut kinds: Vec<(String, usize)> = Vec::new();
        for e in entries {
            let mut seen: Vec<&str> = e["assumptions"]
                .as_array()
                .map(|a| a.iter().filter_map(|x| x["kind"].as_str()).collect())
                .unwrap_or_default();
            seen.dedup();
            for k in seen {
                match kinds.iter_mut().find(|(n, _)| n == k) {
                    Some((_, c)) => *c += 1,
                    None => kinds.push((k.to_string(), 1)),
                }
            }
        }
        for (kind, count) in kinds {
            let (attack, basis, close) = kind_text(&kind);
            out.push(Weakness {
                severity: if total > 0 && count * 2 >= total {
                    Severity::Medium
                } else {
                    Severity::Low
                },
                area: "assumption",
                attack: attack.to_string(),
                basis: format!("{count} of {total} event(s): {basis} ({kind})"),
                close_it: close.to_string(),
            });
        }
        if rec["export_safeguard"]["status"].as_str() == Some("BLOCKED") {
            out.push(Weakness {
                severity: Severity::Low,
                area: "export",
                attack: "Your own system says the reconstruction is not ready for court. Why rely on it?".into(),
                basis: "YUKTI's judicial export safeguard is BLOCKED until a person reviews the assumptions and leads above".into(),
                close_it: "Resolve or explicitly accept each assumption and lead, then re-run the reconstruction.".into(),
            });
        }
    }

    // Gaps, grouped by type.
    if let Some(g) = gaps {
        let mut groups: Vec<(String, String, usize, String, String)> = Vec::new();
        for gap in g["gaps"].as_array().map(Vec::as_slice).unwrap_or(&[]) {
            let t = gap["gap_type"].as_str().unwrap_or("?").to_string();
            let sev = gap["severity"].as_str().unwrap_or("LOW").to_string();
            match groups
                .iter_mut()
                .find(|(gt, gs, ..)| *gt == t && *gs == sev)
            {
                Some((.., c, _, _)) => *c += 1,
                None => groups.push((
                    t,
                    sev,
                    1,
                    gap["title"].as_str().unwrap_or("").to_string(),
                    gap["suggested_corroborating_evidence"]
                        .as_str()
                        .unwrap_or("")
                        .to_string(),
                )),
            }
        }
        for (t, sev, count, title, close) in groups {
            out.push(Weakness {
                severity: match sev.as_str() {
                    "CRITICAL" => Severity::Critical,
                    "HIGH" => Severity::High,
                    "MEDIUM" => Severity::Medium,
                    _ => Severity::Low,
                },
                area: "gap",
                attack: "What evidence did you not collect?".into(),
                basis: if count > 1 {
                    format!("{count} gaps of type {t}, e.g. {title}")
                } else {
                    format!("{t}: {title}")
                },
                close_it: close,
            });
        }
    }
    out.sort_by_key(|w| w.severity);
    out
}

#[derive(Serialize)]
struct ChallengeDocument<'a> {
    schema: &'static str,
    api_url: &'a str,
    case_id: &'a str,
    weaknesses: &'a [Weakness],
    unavailable: &'a [String],
}

pub async fn run(args: &CaseArgs, client: &ApiClient, json: bool, ui: &Ui) -> Rendered {
    let (case_id, case) = match require_case(&args.case_id, client, json, "challenged").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let path_0 = case_reconstruction_path(&case_id);
    let path_1 = case_robustness_path(&case_id);
    let path_2 = case_view_path(&case_id, "gaps");
    let path_3 = case_custody_path(&case_id);
    let (rec, rob, gaps, cust) = tokio::join!(
        client.get_json(&path_0),
        client.get_json(&path_1),
        client.get_json(&path_2),
        client.get_json(&path_3),
    );
    let mut unavailable = Vec::new();
    for (name, r) in [
        ("reconstruction", &rec),
        ("robustness", &rob),
        ("gaps", &gaps),
        ("custody audit", &cust),
    ] {
        if let Err(e) = r {
            unavailable.push(format!("{name}: {e}"));
        }
    }
    let list = weaknesses(
        rec.as_ref().ok(),
        rob.as_ref().ok(),
        gaps.as_ref().ok(),
        cust.as_ref().ok(),
    );
    let mut warnings: Vec<String> = super::placeholder_warning(&case_id, &case)
        .into_iter()
        .collect();
    warnings.extend(
        unavailable
            .iter()
            .map(|u| format!("Not considered (unavailable) - {u}")),
    );
    if json {
        let doc = ChallengeDocument {
            schema: CHALLENGE_SCHEMA,
            api_url: client.base_url(),
            case_id: case_id.as_str(),
            weaknesses: &list,
            unavailable: &unavailable,
        };
        return Rendered::stdout(to_json(&doc), exit::SUCCESS);
    }
    let count = |s: Severity| list.iter().filter(|w| w.severity == s).count();
    let mut out = ui.header(&["challenge"], case_id.as_str());
    out.push('\n');
    out.push_str(&ui.para(
        MARGIN.len(),
        Tone::Muted,
        "The case as defence counsel would attack it: the weakest points first, each with what would close it.",
    ));
    out.push('\n');
    out.push_str(&ui.stats(&[
        (list.len().to_string(), "points", Tone::Strong),
        (
            count(Severity::Critical).to_string(),
            "critical",
            if count(Severity::Critical) > 0 {
                Tone::Flag
            } else {
                Tone::Muted
            },
        ),
        (
            count(Severity::High).to_string(),
            "high",
            if count(Severity::High) > 0 {
                Tone::Flag
            } else {
                Tone::Muted
            },
        ),
        (
            count(Severity::Medium).to_string(),
            "medium",
            if count(Severity::Medium) > 0 {
                Tone::Warn
            } else {
                Tone::Muted
            },
        ),
        (count(Severity::Low).to_string(), "low", Tone::Muted),
    ]));
    out.push_str(&ui.section("Cross-examination", "most serious first"));
    if list.is_empty() {
        out.push_str(&format!(
            "{MARGIN}{}\n",
            ui.paint(Tone::Ok, "No weakness found in what the API reports.")
        ));
    }
    for (i, w) in list.iter().enumerate() {
        let (glyph, tone, label) = match w.severity {
            Severity::Critical => (Glyph::Cross, Tone::Flag, "CRITICAL"),
            Severity::High => (Glyph::Flag, Tone::Flag, "HIGH"),
            Severity::Medium => (Glyph::Assumed, Tone::Warn, "MEDIUM"),
            Severity::Low => (Glyph::Inferred, Tone::Muted, "LOW"),
        };
        out.push_str(&format!(
            "{MARGIN}{} {}  {}  {}\n",
            ui.mark(glyph, tone),
            ui.bold(tone, &crate::ui::pad(label, 8)),
            ui.paint(Tone::Subtle, &format!("#{} {}", i + 1, w.area)),
            ""
        ));
        let rail = ui.paint(tone, ui.g(Glyph::Rail));
        let cont = format!("{MARGIN}{rail}   ");
        let w_cont = MARGIN.len() + 4;
        out.push_str(&ui.hang(
            &format!("{cont}{} ", ui.paint(Tone::Strong, "Q")),
            &format!("{cont}  "),
            w_cont + 2,
            Tone::Strong,
            &format!("\"{}\"", w.attack),
        ));
        out.push_str(&ui.hang(
            &format!("{cont}{} ", ui.paint(Tone::Subtle, "·")),
            &format!("{cont}  "),
            w_cont + 2,
            Tone::Muted,
            &w.basis,
        ));
        out.push_str(&ui.hang(
            &format!("{cont}{} ", ui.mark(Glyph::Prompt, Tone::Ok)),
            &format!("{cont}  "),
            w_cont + 2,
            Tone::Supplied,
            &w.close_it,
        ));
        out.push_str(&format!(
            "{MARGIN}{}\n",
            ui.paint(tone, ui.g(Glyph::RailEnd))
        ));
    }
    out.push_str(&ui.footnote("Fixed ranking: integrity failures, then leads an ordinary error would dissolve and high gaps, then sensitive leads and widespread assumptions, then context worth disclosing."));
    Rendered {
        stdout: out,
        stderr: render_warnings(&warnings),
        exit_code: exit::SUCCESS,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn integrity_failures_come_first_and_robust_single_source_leads_are_disclosed() {
        let custody = json!({"ledger_mode": "POSTGRES_LEDGER", "exhibit_count": 1, "exhibits": [{"original_filename": "cdr.csv", "verified": false, "stored_file": "HASH_MISMATCH"}]});
        let rob = json!({"leads": [
            {"robustness_class": "ROBUST", "same_clock": true, "event_a_id": "a", "event_b_id": "b", "verdict": "ROBUST: ..."},
            {"robustness_class": "FRAGILE", "same_clock": false, "event_a_id": "c", "event_b_id": "d", "verdict": "FRAGILE: ..."}
        ]});
        let rec = json!({"timeline": {"entries": [
            {"assumptions": [{"kind": "COORDINATES_INFERRED_FROM_TOWER_ID"}]},
            {"assumptions": [{"kind": "COORDINATES_INFERRED_FROM_TOWER_ID"}]}
        ]}, "export_safeguard": {"status": "BLOCKED"}});
        let gaps = json!({"gaps": [
            {"gap_type": "EVENT_WITHOUT_COORDINATES", "severity": "MEDIUM", "title": "t1", "suggested_corroborating_evidence": "x"},
            {"gap_type": "EVENT_WITHOUT_COORDINATES", "severity": "MEDIUM", "title": "t2", "suggested_corroborating_evidence": "x"}
        ]});
        let w = weaknesses(Some(&rec), Some(&rob), Some(&gaps), Some(&custody));
        assert_eq!(w[0].severity, Severity::Critical);
        assert!(w[0].basis.contains("HASH_MISMATCH"));
        assert_eq!(w[1].severity, Severity::High);
        assert!(w[1].close_it.contains("clock"));
        assert!(w.iter().any(|x| x.attack.contains("rests on one record")));
        let tower = w
            .iter()
            .find(|x| x.basis.contains("COORDINATES_INFERRED_FROM_TOWER_ID"))
            .unwrap();
        assert_eq!(
            tower.severity,
            Severity::Medium,
            "assumption under every event"
        );
        assert!(w.iter().any(|x| x
            .basis
            .starts_with("2 gaps of type EVENT_WITHOUT_COORDINATES")));
        let sorted: Vec<Severity> = w.iter().map(|x| x.severity).collect();
        let mut expected = sorted.clone();
        expected.sort();
        assert_eq!(sorted, expected);
    }

    #[test]
    fn nothing_reported_means_nothing_invented() {
        assert!(weaknesses(None, None, None, None).is_empty());
    }
}
