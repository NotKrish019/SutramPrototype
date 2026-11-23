//! `yukti ask "<question>"`: plain-English questions answered only from the case's evidence.
//!
//! This is deliberately not a language model. A small fixed parser recognises a handful of
//! question shapes (where was X, who did X contact, what happened between T1 and T2, any
//! contradictions, what is missing, how robust) and answers from the timeline, leads, gaps and
//! robustness the API already returned. Every fact cites its exhibit file, record and the
//! timestamp exactly as written in the source. When nothing in the evidence answers the
//! question, it says so instead of guessing.
//!
//! Phone numbers are matched on their last ten digits, so `+91 98110 02233`, `+919811002233` and
//! `09811002233` are the same number. Times in questions are local wall-clock times, compared with
//! the time written in each exhibit.

use serde::Serialize;
use serde_json::Value;

use super::require_case;
use crate::cli::AskArgs;
use crate::client::{
    case_evidence_path, case_reconstruction_path, case_robustness_path, case_view_path, ApiClient,
};
use crate::error::exit;
use crate::output::{render_warnings, to_json, Rendered};
use crate::ui::{Glyph, Tone, Ui, MARGIN};

pub const ASK_SCHEMA: &str = "yukti.cli.ask/v1";

/// What the question asks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Intent {
    Where {
        number: Option<String>,
        minute_of_day: Option<u32>,
    },
    Contacts {
        number: Option<String>,
    },
    Between {
        from: Option<u32>,
        to: Option<u32>,
    },
    Leads,
    Gaps,
    Robustness,
    Search {
        words: Vec<String>,
    },
}

/// Last ten digits of every phone-like token (at least ten digits).
pub fn numbers_in(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut digits = String::new();
    let flush = |digits: &mut String, out: &mut Vec<String>| {
        if digits.len() >= 10 {
            out.push(digits[digits.len() - 10..].to_string());
        }
        digits.clear();
    };
    let chars: Vec<char> = text.chars().collect();
    for (i, c) in chars.iter().enumerate() {
        if c.is_ascii_digit() {
            digits.push(*c);
        } else if (*c == ' ' || *c == '-')
            && !digits.is_empty()
            && chars.get(i + 1).is_some_and(char::is_ascii_digit)
        {
            // "+91 98110 02233" keeps going across single separators.
        } else if *c == '+' && digits.is_empty() {
        } else {
            flush(&mut digits, &mut out);
        }
    }
    flush(&mut digits, &mut out);
    out
}

/// Every wall-clock time in the text as minutes after midnight ("9 pm", "21:30", "9:15pm").
pub fn times_in(text: &str) -> Vec<u32> {
    let lower = text.to_lowercase().replace('.', "");
    let words: Vec<&str> = lower.split_whitespace().collect();
    let mut out = Vec::new();
    for (i, w) in words.iter().enumerate() {
        let (body, mut meridiem) = if let Some(b) = w.strip_suffix("pm") {
            (b, Some(true))
        } else if let Some(b) = w.strip_suffix("am") {
            (b, Some(false))
        } else {
            (*w, None)
        };
        if meridiem.is_none() {
            meridiem = match words.get(i + 1) {
                Some(&"pm") => Some(true),
                Some(&"am") => Some(false),
                _ => None,
            };
        }
        let (h, m) = match body.split_once(':') {
            Some((h, m)) => (h.parse::<u32>().ok(), m.parse::<u32>().ok()),
            None if meridiem.is_some() => (body.parse::<u32>().ok(), Some(0)),
            None => (None, None),
        };
        let (Some(mut h), Some(m)) = (h, m) else {
            continue;
        };
        if m > 59 || h > 23 || body.len() > 5 {
            continue;
        }
        match meridiem {
            Some(true) if h < 12 => h += 12,
            Some(false) if h == 12 => h = 0,
            _ => {}
        }
        out.push(h * 60 + m);
    }
    out
}

const STOP: [&str; 24] = [
    "what", "when", "where", "which", "about", "there", "their", "with", "from", "that", "this",
    "were", "have", "does", "show", "tell", "case", "evidence", "anything", "said", "say", "says",
    "message", "messages",
];

/// Classifies a question.
pub fn parse_question(q: &str) -> Intent {
    let lower = q.to_lowercase();
    let number = numbers_in(q).into_iter().next();
    let times = times_in(q);
    let has = |w: &[&str]| w.iter().any(|x| lower.contains(x));
    if has(&["contradiction", "impossible", "lead", "inconsisten"]) {
        Intent::Leads
    } else if has(&[
        "how sure",
        "robust",
        "confiden",
        "breaking",
        "how strong",
        "fragile",
    ]) {
        Intent::Robustness
    } else if has(&["missing", "gap", "not collected", "what else"]) {
        Intent::Gaps
    } else if has(&["where"]) {
        Intent::Where {
            number,
            minute_of_day: times.first().copied(),
        }
    } else if has(&["who", "contact", "talk", "spoke", "call", "in touch"]) && !has(&["between"]) {
        Intent::Contacts { number }
    } else if has(&["between", "what happened", "timeline", "events"]) || times.len() >= 2 {
        Intent::Between {
            from: times.first().copied(),
            to: times.get(1).copied(),
        }
    } else {
        Intent::Search {
            words: lower
                .split(|c: char| !c.is_alphanumeric())
                .filter(|w| w.len() >= 4 && !STOP.contains(w))
                .map(str::to_string)
                .collect(),
        }
    }
}

/// One cited fact.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Fact {
    /// `observed` or `inferred`.
    pub basis: &'static str,
    pub text: String,
    pub citation: String,
}

/// The answer.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Answer {
    pub intent: Intent,
    pub answered: bool,
    pub headline: String,
    pub facts: Vec<Fact>,
    pub caveats: Vec<String>,
}

fn local_hm(raw: &str) -> Option<(String, u32)> {
    let t = raw.split_once('T')?.1;
    let h: u32 = t.get(0..2)?.parse().ok()?;
    let m: u32 = t.get(3..5)?.parse().ok()?;
    Some((format!("{h:02}:{m:02}"), h * 60 + m))
}

fn involves(e: &Value, number: &str) -> bool {
    e["entity_ids"]
        .as_array()
        .map(|ids| {
            ids.iter()
                .filter_map(Value::as_str)
                .any(|id| numbers_in(id).iter().any(|n| n == number))
        })
        .unwrap_or(false)
}

fn citation(e: &Value, files: &[(String, String)]) -> String {
    let ev = e["source_evidence_id"].as_str().unwrap_or("");
    let file = files
        .iter()
        .find(|(id, _)| id == ev)
        .map(|(_, f)| f.clone())
        .unwrap_or_else(|| ev.to_string());
    let m = &e["metadata"];
    let record = m["source_line"]
        .as_u64()
        .map(|l| format!("line {l}"))
        .or_else(|| {
            m["source_record_id"]
                .as_str()
                .map(|r| format!("record {r}"))
        })
        .unwrap_or_else(|| e["event_id"].as_str().unwrap_or("").to_string());
    let written = m["original_timestamp_text"]
        .as_str()
        .or_else(|| e["raw_timestamp"].as_str())
        .unwrap_or("");
    format!("{file}, {record}, time as written \"{written}\"")
}

fn location_fact(e: &Value, files: &[(String, String)]) -> Option<Fact> {
    let c = e["coordinates"].as_object()?;
    let (lat, lon) = (c.get("lat")?.as_f64()?, c.get("lon")?.as_f64()?);
    let inferred = e["assumptions"].as_array().is_some_and(|a| {
        a.iter()
            .any(|x| x["kind"] == "COORDINATES_INFERRED_FROM_TOWER_ID")
    });
    let when = local_hm(e["raw_timestamp"].as_str().unwrap_or(""))
        .map(|(t, _)| t)
        .unwrap_or_default();
    let desc = e["raw_description"].as_str().unwrap_or("");
    Some(Fact {
        basis: if inferred { "inferred" } else { "observed" },
        text: format!(
            "{when}  {lat:.4}, {lon:.4}{}  {desc}",
            if inferred {
                " (cell-tower centroid, not the handset position)"
            } else {
                ""
            }
        ),
        citation: citation(e, files),
    })
}

/// Answers from API documents (pure; unit-tested).
pub fn answer(
    intent: Intent,
    reconstruction: &Value,
    files: &[(String, String)],
    robustness: Option<&Value>,
    gaps: Option<&Value>,
) -> Answer {
    let entries: Vec<&Value> = reconstruction["timeline"]["entries"]
        .as_array()
        .map(|a| a.iter().collect())
        .unwrap_or_default();
    let mut facts = Vec::new();
    let mut caveats = Vec::new();
    let headline;
    match &intent {
        Intent::Where { number: None, .. } | Intent::Contacts { number: None } => {
            return Answer {
                intent,
                answered: false,
                headline: "Which number? Include a phone number, e.g. yukti ask \"where was 9811002233 at 9 pm\".".into(),
                facts,
                caveats,
            }
        }
        Intent::Where { number: Some(n), minute_of_day } => {
            let located: Vec<&Value> = entries.iter().copied().filter(|e| involves(e, n) && e["coordinates"].is_object()).collect();
            match minute_of_day {
                None => {
                    for e in &located {
                        facts.extend(location_fact(e, files));
                    }
                    headline = format!("Every located record for ...{n}: {} location(s).", located.len());
                }
                Some(t) => {
                    let at = |e: &&Value| local_hm(e["raw_timestamp"].as_str().unwrap_or("")).map(|(_, m)| m);
                    let before = located.iter().filter(|e| at(e).is_some_and(|m| m <= *t)).max_by_key(|e| at(e));
                    let after = located.iter().filter(|e| at(e).is_some_and(|m| m > *t)).min_by_key(|e| at(e));
                    let hm = format!("{:02}:{:02}", t / 60, t % 60);
                    match (before, after) {
                        (Some(b), _) if at(b) == Some(*t) => {
                            headline = format!("At {hm} ...{n} is recorded at one location.");
                            facts.extend(location_fact(b, files));
                        }
                        (b, a) => {
                            headline = format!(
                                "No record places ...{n} exactly at {hm}. The nearest located records are:"
                            );
                            if let Some(b) = b {
                                facts.extend(location_fact(b, files));
                            }
                            if let Some(a) = a {
                                facts.extend(location_fact(a, files));
                            }
                            caveats.push("Where the number was between two records is not established by the evidence.".into());
                        }
                    }
                }
            }
            if located.iter().any(|e| e["assumptions"].as_array().is_some_and(|a| a.iter().any(|x| x["kind"] == "COORDINATES_INFERRED_FROM_TOWER_ID"))) {
                caveats.push("Tower centroids locate the serving tower, not the handset.".into());
            }
        }
        Intent::Contacts { number: Some(n) } => {
            let mut counts: Vec<(String, usize, Vec<&Value>)> = Vec::new();
            for e in entries.iter().filter(|e| involves(e, n)) {
                for id in e["entity_ids"].as_array().into_iter().flatten().filter_map(Value::as_str) {
                    if numbers_in(id).iter().any(|x| x == n) {
                        continue;
                    }
                    match counts.iter_mut().find(|(k, ..)| k == id) {
                        Some((_, c, es)) => {
                            *c += 1;
                            es.push(e);
                        }
                        None => counts.push((id.to_string(), 1, vec![e])),
                    }
                }
            }
            counts.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
            headline = format!("...{n} appears with {} other party(ies) in the evidence.", counts.len());
            for (party, c, es) in &counts {
                facts.push(Fact {
                    basis: "observed",
                    text: format!("{party}: {c} event(s)"),
                    citation: citation(es[0], files),
                });
            }
            caveats.push("Appearing in the same call or chat is a communication record, not proof of who held the phone.".into());
        }
        Intent::Between { from, to } => {
            let (lo, hi) = match (from, to) {
                (Some(a), Some(b)) => (*a.min(b), *a.max(b)),
                (Some(a), None) => (*a, 24 * 60),
                _ => (0, 24 * 60),
            };
            for e in &entries {
                let Some((hm, m)) = local_hm(e["raw_timestamp"].as_str().unwrap_or("")) else { continue };
                if m >= lo && m <= hi {
                    facts.push(Fact {
                        basis: "observed",
                        text: format!("{hm}  {}", e["raw_description"].as_str().unwrap_or("(no description)")),
                        citation: citation(e, files),
                    });
                }
            }
            headline = format!(
                "{} event(s) between {:02}:{:02} and {:02}:{:02} (local times as written in each exhibit).",
                facts.len(), lo / 60, lo % 60, (hi / 60).min(23), if hi >= 24 * 60 { 59 } else { hi % 60 }
            );
        }
        Intent::Leads | Intent::Robustness => {
            let leads = robustness.and_then(|r| r["leads"].as_array()).map(Vec::as_slice).unwrap_or(&[]);
            for l in leads {
                facts.push(Fact {
                    basis: "inferred",
                    text: format!(
                        "{} {} -> {}: {}",
                        l["lead_type"].as_str().unwrap_or(""),
                        l["event_a_id"].as_str().unwrap_or(""),
                        l["event_b_id"].as_str().unwrap_or(""),
                        l["verdict"].as_str().unwrap_or("")
                    ),
                    citation: "yukti robustness".into(),
                });
            }
            headline = if leads.is_empty() {
                "The reconstruction raises no contradiction leads.".into()
            } else {
                format!("{} contradiction lead(s); each is an investigative lead, not proof.", leads.len())
            };
        }
        Intent::Gaps => {
            for g in gaps.and_then(|g| g["gaps"].as_array()).map(Vec::as_slice).unwrap_or(&[]).iter().take(10) {
                facts.push(Fact {
                    basis: "inferred",
                    text: format!(
                        "{}: {} -> {}",
                        g["severity"].as_str().unwrap_or(""),
                        g["title"].as_str().unwrap_or(""),
                        g["suggested_corroborating_evidence"].as_str().unwrap_or("")
                    ),
                    citation: "yukti gaps".into(),
                });
            }
            headline = format!("{} evidence gap(s) limit the reconstruction.", gaps.and_then(|g| g["total_gaps_count"].as_u64()).unwrap_or(0));
        }
        Intent::Search { words } => {
            for e in &entries {
                let desc = e["raw_description"].as_str().unwrap_or("").to_lowercase();
                if !words.is_empty() && words.iter().any(|w| desc.contains(w.as_str())) {
                    let hm = local_hm(e["raw_timestamp"].as_str().unwrap_or("")).map(|(t, _)| t).unwrap_or_default();
                    facts.push(Fact {
                        basis: "observed",
                        text: format!("{hm}  {}", e["raw_description"].as_str().unwrap_or("")),
                        citation: citation(e, files),
                    });
                }
            }
            headline = format!("{} record(s) mention {}.", facts.len(), words.join(", "));
        }
    }
    let answered = !facts.is_empty();
    Answer {
        headline: if answered {
            headline
        } else {
            "No ingested evidence answers this question. YUKTI answers only from the case's evidence and does not guess.".into()
        },
        intent,
        answered,
        facts,
        caveats,
    }
}

#[derive(Serialize)]
struct AskDocument<'a> {
    schema: &'static str,
    api_url: &'a str,
    case_id: &'a str,
    question: &'a str,
    answer: &'a Answer,
}

pub async fn run(args: &AskArgs, client: &ApiClient, json: bool, ui: &Ui) -> Rendered {
    let question = args.question.join(" ");
    let (case_id, case) = match require_case(&args.case_id, client, json, "asked").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let path_0 = case_reconstruction_path(&case_id);
    let path_1 = case_evidence_path(&case_id);
    let path_2 = case_robustness_path(&case_id);
    let path_3 = case_view_path(&case_id, "gaps");
    let (rec, evidence, rob, gaps) = tokio::join!(
        client.get_json(&path_0),
        client.get_json(&path_1),
        client.get_json(&path_2),
        client.get_json(&path_3),
    );
    let rec = match rec {
        Ok(v) => v,
        Err(err) => return crate::output::render_error_with_notes(&err, json, &[]),
    };
    let files: Vec<(String, String)> = evidence
        .ok()
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default()
        .iter()
        .map(|e| {
            (
                e["evidence_id"].as_str().unwrap_or("").to_string(),
                e["original_filename"].as_str().unwrap_or("").to_string(),
            )
        })
        .collect();
    let ans = answer(
        parse_question(&question),
        &rec,
        &files,
        rob.as_ref().ok(),
        gaps.as_ref().ok(),
    );
    let warnings: Vec<String> = super::placeholder_warning(&case_id, &case)
        .into_iter()
        .collect();
    if json {
        let doc = AskDocument {
            schema: ASK_SCHEMA,
            api_url: client.base_url(),
            case_id: case_id.as_str(),
            question: &question,
            answer: &ans,
        };
        return Rendered::stdout(to_json(&doc), exit::SUCCESS);
    }
    let mut out = ui.header(&["ask"], case_id.as_str());
    out.push('\n');
    out.push_str(&format!(
        "{MARGIN}{} {}\n\n",
        ui.paint(Tone::Brand, ui.g(Glyph::Prompt)),
        ui.bold(Tone::Strong, &question)
    ));
    out.push_str(&ui.para(
        MARGIN.len(),
        if ans.answered { Tone::Text } else { Tone::Warn },
        &ans.headline,
    ));
    for f in &ans.facts {
        let (g, t) = if f.basis == "observed" {
            (Glyph::Observed, Tone::Text)
        } else {
            (Glyph::Inferred, Tone::Inferred)
        };
        let pre = format!("{MARGIN}{} ", ui.mark(g, t));
        out.push('\n');
        out.push_str(&ui.hang(&pre, &format!("{MARGIN}  "), MARGIN.len() + 2, t, &f.text));
        out.push_str(&ui.hang(
            &format!("{MARGIN}  "),
            &format!("{MARGIN}  "),
            MARGIN.len() + 2,
            Tone::Subtle,
            &format!("source: {}", f.citation),
        ));
    }
    for c in &ans.caveats {
        out.push('\n');
        out.push_str(&ui.hang(
            &format!("{MARGIN}{} ", ui.mark(Glyph::Assumed, Tone::Warn)),
            &format!("{MARGIN}  "),
            MARGIN.len() + 2,
            Tone::Warn,
            c,
        ));
    }
    if !ans.answered {
        out.push('\n');
        for (label, example) in [
            ("location", "where was 9811002233 at 9 pm"),
            ("contacts", "who did 9811002233 contact"),
            ("window", "what happened between 8:30 pm and 10 pm"),
            ("leads", "are there any contradictions"),
            ("gaps", "what evidence is missing"),
        ] {
            out.push_str(&ui.next(label, &format!("yukti ask \"{example}\"")));
        }
    }
    out.push_str(&ui.footnote(
        "Answered by fixed rules from the case's records only; no language model is used.",
    ));
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
    fn numbers_and_times_are_recognised() {
        assert_eq!(
            numbers_in("where was +91 98110 02233 at 9pm"),
            vec!["9811002233"]
        );
        assert_eq!(
            numbers_in("call 09811002233 or +919811002233"),
            vec!["9811002233", "9811002233"]
        );
        assert!(numbers_in("at 21:00 on 10/11").is_empty());
        assert_eq!(times_in("at 9 pm"), vec![21 * 60]);
        assert_eq!(
            times_in("between 8:30pm and 10 p.m."),
            vec![20 * 60 + 30, 22 * 60]
        );
        assert_eq!(times_in("21:15"), vec![21 * 60 + 15]);
        assert_eq!(times_in("12 am"), vec![0]);
    }

    #[test]
    fn questions_are_classified() {
        assert!(matches!(
            parse_question("where was 9811002233 at 9 pm"),
            Intent::Where {
                number: Some(_),
                minute_of_day: Some(1260)
            }
        ));
        assert!(matches!(
            parse_question("who did 9811002233 talk to"),
            Intent::Contacts { .. }
        ));
        assert!(matches!(
            parse_question("what happened between 8:30 pm and 10 pm"),
            Intent::Between {
                from: Some(1230),
                to: Some(1320)
            }
        ));
        assert_eq!(parse_question("any contradictions?"), Intent::Leads);
        assert_eq!(parse_question("what evidence is missing"), Intent::Gaps);
        assert_eq!(parse_question("how sure are we"), Intent::Robustness);
        assert!(matches!(
            parse_question("package gate"),
            Intent::Search { .. }
        ));
    }

    fn rec() -> Value {
        json!({"timeline": {"entries": [
            {"event_id": "e1", "source_evidence_id": "ev-a", "raw_timestamp": "2026-11-10T21:00:00+05:30",
             "raw_description": "Call via TOWER-DELHI-SOUTH", "entity_ids": ["+919811002233", "+919800000101"],
             "coordinates": {"lat": 28.5244, "lon": 77.2066}, "metadata": {"source_record_id": "a-001"},
             "assumptions": [{"kind": "COORDINATES_INFERRED_FROM_TOWER_ID"}]},
            {"event_id": "e2", "source_evidence_id": "ev-a", "raw_timestamp": "2026-11-10T21:30:00+05:30",
             "raw_description": "Call via TOWER-MUMBAI-BKC", "entity_ids": ["+919811002233", "+919800000102"],
             "coordinates": {"lat": 19.06, "lon": 72.86}, "metadata": {"source_record_id": "a-002"}, "assumptions": []},
            {"event_id": "e3", "source_evidence_id": "ev-w", "raw_timestamp": "2026-11-10T20:52:00",
             "raw_description": "Message from +91 98110 02233: Keep the package ready.", "entity_ids": ["+91 98110 02233", "Priya"],
             "metadata": {"source_line": 3, "original_timestamp_text": "10/11/2026, 8:52 pm"}, "assumptions": []}
        ]}})
    }

    #[test]
    fn where_answers_cite_sources_and_admit_gaps_between_records() {
        let files = vec![("ev-a".to_string(), "carrier_a_cdr.csv".to_string())];
        let a = answer(
            parse_question("where was 9811002233 at 9:15 pm"),
            &rec(),
            &files,
            None,
            None,
        );
        assert!(a.answered);
        assert!(a.headline.contains("No record places"));
        assert_eq!(a.facts.len(), 2);
        assert!(a.facts[0]
            .citation
            .contains("carrier_a_cdr.csv, record a-001"));
        assert_eq!(a.facts[0].basis, "inferred", "tower centroid");
        assert!(a.caveats.iter().any(|c| c.contains("not established")));
        let exact = answer(
            parse_question("where was 9811002233 at 9 pm"),
            &rec(),
            &files,
            None,
            None,
        );
        assert!(exact.headline.starts_with("At 21:00"));
    }

    #[test]
    fn contacts_merge_number_formats_and_search_cites_lines() {
        let a = answer(
            parse_question("who did +91 98110 02233 contact"),
            &rec(),
            &[],
            None,
            None,
        );
        assert_eq!(a.facts.len(), 3, "{:?}", a.facts);
        let s = answer(parse_question("package"), &rec(), &[], None, None);
        assert_eq!(s.facts.len(), 1);
        assert!(s.facts[0].citation.contains("line 3"));
        assert!(s.facts[0].citation.contains("10/11/2026, 8:52 pm"));
    }

    #[test]
    fn unanswerable_questions_are_refused() {
        let a = answer(
            parse_question("what is the suspect's motive"),
            &rec(),
            &[],
            None,
            None,
        );
        assert!(!a.answered);
        assert!(a.headline.contains("does not guess"));
        let no_number = answer(parse_question("where was he"), &rec(), &[], None, None);
        assert!(!no_number.answered);
    }
}
