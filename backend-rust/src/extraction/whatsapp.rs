//! WhatsApp plain-text chat export parser (`Export chat` → `.txt`).
//!
//! WhatsApp exports are the most common chat evidence seized in India. Two layouts exist:
//!
//! * Android: `12/03/2024, 9:45 pm - Rahul: message` (also 24-hour and `\u{202f}` before am/pm)
//! * iOS:     `[12/03/24, 9:45:12 PM] Rahul: message`
//!
//! Lines that do not start a message continue the previous one (multi-line messages). Lines
//! without a `sender: ` part are system notices (encryption banner, "X added Y") and are counted,
//! not treated as messages.
//!
//! Forensic rules:
//! * The exhibit is never modified. Each message keeps its 1-based source line number as its
//!   record ID (`wa-line-<n>`) and the timestamp text exactly as written
//!   (`metadata.original_timestamp_text`).
//! * Timestamps carry no timezone. They are emitted as local naive ISO values; the exhibit's
//!   declared timezone (or the labelled default) is applied later by reconstruction.
//! * Day/month order is decided from the data: a first field above 12 proves DD/MM, a second
//!   field above 12 proves MM/DD. When every value is 12 or below the order cannot be proven and
//!   DD/MM (the Indian locale default) is used and labelled as an assumption in the metadata.
//! * Exports without seconds are `MINUTE` precision, not exact seconds.
//! * Recipients are only filled for one-to-one chats (exactly two senders). In group chats the
//!   export does not say who received a message, so no recipient is fabricated.

use std::collections::BTreeSet;
use std::sync::OnceLock;

use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use regex::Regex;
use serde_json::{json, Map, Value};

use super::chat::parse_chat_json;
use super::model::{ChatExtractionResult, ExtractionValidationIssue};

/// Format marker written into the intermediate chat document.
pub const WHATSAPP_TXT_FORMAT: &str = "whatsapp_txt_export";

fn android_header() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"^\u{200e}?(\d{1,2})[/.\-](\d{1,2})[/.\-](\d{2,4}),?\s+(\d{1,2})[:.](\d{2})(?:[:.](\d{2}))?(?:[\s\u{202f}\u{00a0}]*([AaPp])\.?\s?[Mm]\.?)?\s+-\s+(.*)$",
        )
        .expect("valid android regex")
    })
}

fn ios_header() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"^\u{200e}?\[(\d{1,2})[/.\-](\d{1,2})[/.\-](\d{2,4}),?\s+(\d{1,2})[:.](\d{2})(?:[:.](\d{2}))?(?:[\s\u{202f}\u{00a0}]*([AaPp])\.?\s?[Mm]\.?)?\]\s+(.*)$",
        )
        .expect("valid ios regex")
    })
}

/// One message header line, as captured.
#[derive(Debug, Clone)]
struct Header {
    line: usize,
    a: u32,
    b: u32,
    year: i32,
    hour: u32,
    minute: u32,
    second: Option<u32>,
    meridiem: Option<char>,
    rest: String,
    original_timestamp_text: String,
}

fn parse_header(line_no: usize, line: &str) -> Option<Header> {
    let caps = android_header()
        .captures(line)
        .or_else(|| ios_header().captures(line))?;
    let num = |i: usize| caps.get(i).and_then(|m| m.as_str().parse::<u32>().ok());
    let year_raw = caps.get(3)?.as_str();
    let year: i32 = year_raw.parse().ok()?;
    let year = if year_raw.len() == 2 {
        2000 + year
    } else {
        year
    };
    let rest = caps.get(8)?.as_str().to_string();
    let full = caps.get(0)?.as_str();
    let original_timestamp_text = full[..full.len() - rest.len()]
        .trim_start_matches('\u{200e}')
        .trim()
        .trim_start_matches('[')
        .trim_end_matches('-')
        .trim()
        .trim_end_matches(']')
        .trim()
        .to_string();
    Some(Header {
        line: line_no,
        a: num(1)?,
        b: num(2)?,
        year,
        hour: num(4)?,
        minute: num(5)?,
        second: num(6),
        meridiem: caps
            .get(7)
            .and_then(|m| m.as_str().chars().next())
            .map(|c| c.to_ascii_lowercase()),
        rest,
        original_timestamp_text,
    })
}

/// True when the text looks like a WhatsApp `.txt` export (used by ingestion classification).
pub fn looks_like_whatsapp_txt(text: &str) -> bool {
    let candidates: Vec<&str> = text
        .lines()
        .map(str::trim_end)
        .filter(|l| !l.trim().is_empty())
        .take(10)
        .collect();
    let hits = candidates
        .iter()
        .enumerate()
        .filter(|(i, l)| parse_header(i + 1, l).is_some())
        .count();
    hits >= 1 && hits * 2 >= candidates.len().min(4)
}

/// Day/month order decided from the data, and why.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateOrder {
    DayFirstProven,
    MonthFirstProven,
    DayFirstAssumed,
}

impl DateOrder {
    pub fn basis(self) -> &'static str {
        match self {
            Self::DayFirstProven => "DD/MM proven: a first date field above 12 appears in the export",
            Self::MonthFirstProven => "MM/DD proven: a second date field above 12 appears in the export",
            Self::DayFirstAssumed => "DD/MM ASSUMED: every date field is 12 or below, so the order cannot be proven from the data; DD/MM is the Indian locale default and must be confirmed against the source device's locale",
        }
    }
}

fn decide_order(headers: &[Header]) -> DateOrder {
    if headers.iter().any(|h| h.a > 12) {
        DateOrder::DayFirstProven
    } else if headers.iter().any(|h| h.b > 12) {
        DateOrder::MonthFirstProven
    } else {
        DateOrder::DayFirstAssumed
    }
}

fn to_local(h: &Header, order: DateOrder) -> Result<NaiveDateTime, String> {
    let (day, month) = match order {
        DateOrder::MonthFirstProven => (h.b, h.a),
        _ => (h.a, h.b),
    };
    let hour = match h.meridiem {
        None => h.hour,
        Some(_) if h.hour == 0 || h.hour > 12 => {
            return Err(format!("hour {} is not valid with am/pm", h.hour))
        }
        Some('a') => h.hour % 12,
        Some(_) => h.hour % 12 + 12,
    };
    let date = NaiveDate::from_ymd_opt(h.year, month, day)
        .ok_or_else(|| format!("{day:02}/{month:02}/{} is not a calendar date", h.year))?;
    let time = NaiveTime::from_hms_opt(hour, h.minute, h.second.unwrap_or(0))
        .ok_or_else(|| format!("{hour:02}:{:02} is not a valid time", h.minute))?;
    Ok(NaiveDateTime::new(date, time))
}

/// Parses a WhatsApp `.txt` export into the same result shape as the JSON chat parser.
pub fn parse_whatsapp_txt(
    raw_text: &str,
    source_evidence_id: &str,
    default_country_prefix: Option<&str>,
) -> ChatExtractionResult {
    // 1. Split into messages (header line + continuation lines) and system notices.
    struct Draft {
        header: Header,
        sender: String,
        content: String,
    }
    let mut drafts: Vec<Draft> = Vec::new();
    let mut system_notices = 0usize;
    let mut in_message = false;
    for (idx, raw_line) in raw_text.lines().enumerate() {
        let line_no = idx + 1;
        let line = raw_line.trim_end_matches('\r');
        if let Some(h) = parse_header(line_no, line) {
            match h.rest.split_once(": ") {
                Some((sender, content)) if !sender.trim().is_empty() => {
                    drafts.push(Draft {
                        sender: sender.trim().trim_start_matches('\u{200e}').to_string(),
                        content: content.to_string(),
                        header: h,
                    });
                    in_message = true;
                }
                _ => {
                    system_notices += 1;
                    in_message = false;
                }
            }
        } else if in_message {
            if let Some(last) = drafts.last_mut() {
                last.content.push('\n');
                last.content.push_str(line);
            }
        }
    }

    // 2. Decide the date order from all headers at once.
    let headers: Vec<Header> = drafts.iter().map(|d| d.header.clone()).collect();
    let order = decide_order(&headers);
    let senders: BTreeSet<&str> = drafts.iter().map(|d| d.sender.as_str()).collect();
    let one_to_one = senders.len() == 2;

    // 3. Build the intermediate chat document and remember per-message facts.
    let mut messages = Vec::new();
    let mut extra: Vec<(String, Map<String, Value>)> = Vec::new();
    let mut timestamp_issues = Vec::new();
    for d in &drafts {
        let record_id = format!("wa-line-{}", d.header.line);
        let local = match to_local(&d.header, order) {
            Ok(t) => t,
            Err(reason) => {
                timestamp_issues.push(ExtractionValidationIssue {
                    record_id: Some(record_id),
                    field_name: "timestamp".to_string(),
                    issue_type: "INVALID_TIMESTAMP".to_string(),
                    raw_value: Some(d.header.original_timestamp_text.clone()),
                    message: format!("Line {}: {reason}.", d.header.line),
                    row_index: Some(d.header.line),
                });
                continue;
            }
        };
        let recipients: Vec<&str> = if one_to_one {
            senders.iter().copied().filter(|s| *s != d.sender).collect()
        } else {
            Vec::new()
        };
        messages.push(json!({
            "message_id": record_id,
            "sender_id": d.sender,
            "sender_name": d.sender,
            "recipients": recipients,
            "timestamp": local.format("%Y-%m-%dT%H:%M:%S").to_string(),
            "content": d.content,
        }));
        let mut meta = Map::new();
        meta.insert("export_format".into(), json!(WHATSAPP_TXT_FORMAT));
        meta.insert("source_line".into(), json!(d.header.line));
        meta.insert(
            "original_timestamp_text".into(),
            json!(d.header.original_timestamp_text),
        );
        meta.insert("date_order_basis".into(), json!(order.basis()));
        meta.insert(
            "date_order_assumed".into(),
            json!(order == DateOrder::DayFirstAssumed),
        );
        meta.insert(
            "recipients_basis".into(),
            json!(if one_to_one {
                "one-to-one chat: the other participant"
            } else {
                "group or single-sender export: recipients are not recorded by WhatsApp and are not inferred"
            }),
        );
        extra.push((record_id, meta));
    }
    let doc = json!({
        "export_format": WHATSAPP_TXT_FORMAT,
        "chat_id": format!("whatsapp-{source_evidence_id}"),
        "messages": messages,
    });

    // 4. Reuse the JSON chat parser, then restore WhatsApp facts the JSON shape cannot carry.
    let mut result = parse_chat_json(&doc.to_string(), source_evidence_id, default_country_prefix);
    let minute_only: BTreeSet<String> = drafts
        .iter()
        .filter(|d| d.header.second.is_none())
        .map(|d| format!("wa-line-{}", d.header.line))
        .collect();
    for candidate in &mut result.event_candidates {
        if minute_only.contains(&candidate.source_record_id) {
            candidate.timestamp_precision = Some("MINUTE".to_string());
        }
        if let Some((_, meta)) = extra
            .iter()
            .find(|(id, _)| *id == candidate.source_record_id)
        {
            if let Value::Object(existing) = &mut candidate.metadata {
                for (k, v) in meta {
                    existing.insert(k.clone(), v.clone());
                }
            }
        }
    }
    result.total_messages_processed = drafts.len();
    result.rejected_messages.extend(timestamp_issues);
    if system_notices > 0 && drafts.is_empty() {
        result.rejected_messages.push(ExtractionValidationIssue {
            record_id: None,
            field_name: "messages".to_string(),
            issue_type: "NO_MESSAGES".to_string(),
            raw_value: None,
            message: format!(
                "The export contains {system_notices} system notice(s) but no sender messages."
            ),
            row_index: None,
        });
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    const ANDROID: &str = "\
12/03/2024, 9:41 pm - Messages and calls are end-to-end encrypted. No one outside of this chat can read them.
12/03/2024, 9:45 pm - Rahul Sharma: Consignment left the South warehouse
13/03/2024, 12:05 am - +91 98110 02233: Call me on 09811002233
second line of the same message
13/03/2024, 12:07 am - Rahul Sharma: <Media omitted>
";

    const IOS: &str = "\
[03/12/24, 9:45:12 PM] Rahul: First
[03/12/24, 9:46:00 PM] Priya: Second
";

    #[test]
    fn android_export_is_parsed_with_minute_precision_and_proven_day_first() {
        assert!(looks_like_whatsapp_txt(ANDROID));
        let r = parse_whatsapp_txt(ANDROID, "ev-1", Some("91"));
        assert_eq!(
            r.total_messages_processed, 3,
            "system notice is not a message"
        );
        assert_eq!(r.valid_messages.len(), 3);
        assert!(r.rejected_messages.is_empty(), "{:?}", r.rejected_messages);
        let first = &r.event_candidates[0];
        assert_eq!(first.source_record_id, "wa-line-2");
        assert_eq!(first.raw_timestamp, "2024-03-12T21:45:00");
        assert_eq!(first.timestamp_precision.as_deref(), Some("MINUTE"));
        assert_eq!(
            first.metadata["original_timestamp_text"],
            "12/03/2024, 9:45 pm"
        );
        assert_eq!(first.metadata["date_order_assumed"], false);
        // 12:05 am is just after midnight on the 13th.
        assert_eq!(r.event_candidates[1].raw_timestamp, "2024-03-13T00:05:00");
        // Multi-line message kept whole.
        assert!(r.valid_messages[1]
            .content_raw
            .contains("second line of the same message"));
        // One-to-one: the other sender is the recipient.
        assert_eq!(
            r.valid_messages[0].recipients_raw,
            vec!["+91 98110 02233".to_string()]
        );
    }

    #[test]
    fn ambiguous_dates_are_labelled_as_assumed_day_first() {
        let r = parse_whatsapp_txt(IOS, "ev-2", None);
        assert_eq!(r.valid_messages.len(), 2);
        let c = &r.event_candidates[0];
        // 03/12/24 read as 3 December 2024 (DD/MM assumed), seconds present -> exact.
        assert_eq!(c.raw_timestamp, "2024-12-03T21:45:12");
        assert_eq!(c.timestamp_precision.as_deref(), Some("EXACT_SECOND"));
        assert_eq!(c.metadata["date_order_assumed"], true);
        assert!(c.metadata["date_order_basis"]
            .as_str()
            .unwrap()
            .contains("ASSUMED"));
    }

    #[test]
    fn month_first_is_detected_and_invalid_dates_are_rejected_not_guessed() {
        let us = "1/25/24, 10:00 AM - A: x\n1/26/24, 10:01 AM - B: y\n";
        let r = parse_whatsapp_txt(us, "ev-3", None);
        assert_eq!(r.event_candidates[0].raw_timestamp, "2024-01-25T10:00:00");
        let bad = "31/02/2024, 10:00 - A: x\n13/02/2024, 10:00 - B: y\n";
        let r = parse_whatsapp_txt(bad, "ev-4", None);
        assert_eq!(r.valid_messages.len(), 1);
        assert_eq!(r.rejected_messages.len(), 1);
        assert_eq!(r.rejected_messages[0].issue_type, "INVALID_TIMESTAMP");
        assert_eq!(r.rejected_messages[0].row_index, Some(1));
    }

    #[test]
    fn group_chats_do_not_invent_recipients() {
        let group =
            "13/03/2024, 10:00 - A: x\n13/03/2024, 10:01 - B: y\n13/03/2024, 10:02 - C: z\n";
        let r = parse_whatsapp_txt(group, "ev-5", None);
        assert!(r.valid_messages.iter().all(|m| m.recipients_raw.is_empty()));
    }

    #[test]
    fn csv_is_not_mistaken_for_whatsapp() {
        assert!(!looks_like_whatsapp_txt(
            "record_id,caller_phone,receiver_phone\ncdr-1,+91,+91\n"
        ));
    }
}
