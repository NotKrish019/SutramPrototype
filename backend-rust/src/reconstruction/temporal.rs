//! Temporal Reconstruction Engine (Member 4).
//!
//! Provides deterministic ISO 8601 timestamp normalization, IANA timezone localization,
//! device/DVR clock drift offset correction (DEC-001), precision-based and explicit half-open
//! uncertainty interval derivation [start, end) (DEC-002), and overlap-aware timeline reconstruction.

use chrono::{DateTime, Duration, NaiveDateTime, TimeZone, Timelike, Utc};
use chrono_tz::Tz;
use std::str::FromStr;

use super::model::{
    AssumptionKind, CanonicalTimePrecision, NormalizedTimeWindow, ReconstructionAssumption,
    TemporalOrderingStatus, TimelineEntry, TimelineOverlapPair, TimelineReconstructionResult,
};
use crate::error::AppError;

/// Configurable default half-width for CanonicalTimePrecision::Estimated.
pub const DEFAULT_ESTIMATED_HALF_WIDTH_SECONDS: f64 = 900.0;

/// Resolves an IANA timezone identifier into a `chrono_tz::Tz`.
pub fn validate_timezone(source_timezone: &str) -> Result<Tz, AppError> {
    let trimmed = source_timezone.trim();
    if trimmed.is_empty() {
        return Err(AppError::BadRequest(
            "Invalid source_timezone: timezone name cannot be empty.".to_string(),
        ));
    }

    if trimmed.eq_ignore_ascii_case("utc") {
        return Ok(chrono_tz::UTC);
    }

    Tz::from_str(trimmed).map_err(|_| {
        AppError::BadRequest(format!(
            "Unrecognized IANA timezone '{}'. Expected a valid IANA timezone name (e.g. 'Asia/Kolkata', 'America/New_York').",
            trimmed
        ))
    })
}

/// Normalizes a precision string or legacy alias into `CanonicalTimePrecision`.
pub fn normalize_time_precision(raw: &str) -> Result<CanonicalTimePrecision, AppError> {
    let trimmed = raw.trim().to_ascii_uppercase();
    match trimmed.as_str() {
        "EXACT_SECOND" | "EXACT" | "SECOND" => Ok(CanonicalTimePrecision::ExactSecond),
        "MINUTE" => Ok(CanonicalTimePrecision::Minute),
        "HOUR" => Ok(CanonicalTimePrecision::Hour),
        "DAY" => Ok(CanonicalTimePrecision::Day),
        "ESTIMATED" | "APPROXIMATE" => Ok(CanonicalTimePrecision::Estimated),
        _ => Err(AppError::BadRequest(format!(
            "Unsupported timestamp_precision '{}'. Canonical values: EXACT_SECOND, MINUTE, HOUR, DAY, ESTIMATED.",
            raw
        ))),
    }
}

/// Parses an ISO 8601 string into a UTC `DateTime<Utc>`.
///
/// If naive, localizes it in `source_timezone` first.
/// Applies additive clock drift offset (DEC-001):
/// True UTC = Device Time + Offset
pub fn parse_and_normalize_timestamp(
    raw_timestamp: &str,
    source_timezone: &str,
    clock_offset_seconds: f64,
) -> Result<DateTime<Utc>, AppError> {
    if !clock_offset_seconds.is_finite() {
        return Err(AppError::BadRequest(
            "Invalid clock_offset_seconds: must be a finite number.".to_string(),
        ));
    }

    let cleaned = raw_timestamp.trim();
    if cleaned.is_empty() {
        return Err(AppError::BadRequest(
            "Timestamp string cannot be empty.".to_string(),
        ));
    }

    let tz = validate_timezone(source_timezone)?;

    // Try parsing as timezone-aware ISO 8601 first
    let parsed_utc = if let Ok(dt) = DateTime::parse_from_rfc3339(cleaned) {
        dt.with_timezone(&Utc)
    } else if let Ok(dt) = DateTime::parse_from_str(cleaned, "%Y-%m-%d %H:%M:%S%z") {
        dt.with_timezone(&Utc)
    } else {
        // Try parsing as naive datetime with time component
        let naive = if let Ok(ndt) = NaiveDateTime::parse_from_str(cleaned, "%Y-%m-%dT%H:%M:%S%.f")
        {
            ndt
        } else if let Ok(ndt) = NaiveDateTime::parse_from_str(cleaned, "%Y-%m-%dT%H:%M:%S") {
            ndt
        } else if let Ok(ndt) = NaiveDateTime::parse_from_str(cleaned, "%Y-%m-%d %H:%M:%S%.f") {
            ndt
        } else if let Ok(ndt) = NaiveDateTime::parse_from_str(cleaned, "%Y-%m-%d %H:%M:%S") {
            ndt
        } else {
            return Err(AppError::BadRequest(format!(
                "Malformed ISO 8601 timestamp '{}': must include date and time components.",
                raw_timestamp
            )));
        };

        // Localize naive timestamp in source_timezone
        match tz.from_local_datetime(&naive) {
            chrono::LocalResult::Single(local_dt) => local_dt.with_timezone(&Utc),
            chrono::LocalResult::Ambiguous(_earliest, _latest) => {
                // Ambiguous local time during DST fall-back transition
                return Err(AppError::BadRequest(format!(
                    "Ambiguous naive local timestamp '{}' in timezone '{}' (falls within DST fall-back transition). Provide explicit ISO 8601 offset.",
                    raw_timestamp, source_timezone
                )));
            }
            chrono::LocalResult::None => {
                // Nonexistent local time during DST spring-forward transition
                return Err(AppError::BadRequest(format!(
                    "Nonexistent naive local timestamp '{}' in timezone '{}' (falls within DST spring-forward gap). Provide explicit ISO 8601 offset.",
                    raw_timestamp, source_timezone
                )));
            }
        }
    };

    let offset_duration = Duration::milliseconds((clock_offset_seconds * 1000.0).round() as i64);
    Ok(parsed_utc + offset_duration)
}

/// Type alias for nominal time and uncertainty bounds (nominal, start, end).
pub type UncertaintyInterval = (DateTime<Utc>, DateTime<Utc>, DateTime<Utc>);

/// Derives nominal UTC timestamp and half-open uncertainty interval [window_start_utc, window_end_utc).
///
/// Forensic guarantees (DEC-002):
/// 1. MINUTE, HOUR, and DAY boundaries are floored and advanced in the local `source_timezone`
///    before converting both boundaries to UTC.
/// 2. `clock_offset_seconds` is applied additively to all three bounds.
/// 3. All returned intervals satisfy start < end.
pub fn compute_uncertainty_window(
    raw_timestamp: &str,
    precision: CanonicalTimePrecision,
    source_timezone: &str,
    clock_offset_seconds: f64,
    estimated_half_width_seconds: f64,
    explicit_start: Option<&str>,
    explicit_end: Option<&str>,
) -> Result<UncertaintyInterval, AppError> {
    if !clock_offset_seconds.is_finite() {
        return Err(AppError::BadRequest(
            "clock_offset_seconds must be a finite number.".to_string(),
        ));
    }
    if !estimated_half_width_seconds.is_finite() || estimated_half_width_seconds <= 0.0 {
        return Err(AppError::BadRequest(
            "estimated_half_width_seconds must be a finite positive number.".to_string(),
        ));
    }

    let offset_duration = Duration::milliseconds((clock_offset_seconds * 1000.0).round() as i64);
    let nominal_utc =
        parse_and_normalize_timestamp(raw_timestamp, source_timezone, clock_offset_seconds)?;

    // Handle explicit uncertainty bounds if supplied
    if explicit_start.is_some() || explicit_end.is_some() {
        let (s_str, e_str) = match (explicit_start, explicit_end) {
            (Some(s), Some(e)) => (s, e),
            _ => {
                return Err(AppError::BadRequest(
                    "Both explicit_start and explicit_end must be provided when specifying explicit uncertainty bounds."
                        .to_string(),
                ));
            }
        };

        let start_utc =
            parse_and_normalize_timestamp(s_str, source_timezone, clock_offset_seconds)?;
        let end_utc = parse_and_normalize_timestamp(e_str, source_timezone, clock_offset_seconds)?;

        if start_utc >= end_utc {
            return Err(AppError::BadRequest(format!(
                "Invalid explicit uncertainty bounds: start ({}) must be strictly earlier than end ({}).",
                start_utc.to_rfc3339(),
                end_utc.to_rfc3339()
            )));
        }

        if nominal_utc < start_utc || nominal_utc >= end_utc {
            return Err(AppError::BadRequest(format!(
                "Nominal UTC timestamp ({}) falls outside explicit uncertainty window [{}, {}).",
                nominal_utc.to_rfc3339(),
                start_utc.to_rfc3339(),
                end_utc.to_rfc3339()
            )));
        }

        return Ok((nominal_utc, start_utc, end_utc));
    }

    let tz = validate_timezone(source_timezone)?;
    // Nominal time in local source_timezone before offset
    let unshifted_nominal = nominal_utc - offset_duration;
    let local_dt = unshifted_nominal.with_timezone(&tz);

    let (start_utc, end_utc) = match precision {
        CanonicalTimePrecision::ExactSecond => {
            let local_start = local_dt.with_nanosecond(0).unwrap_or(local_dt);
            let s_utc = local_start.with_timezone(&Utc) + offset_duration;
            let e_utc = s_utc + Duration::seconds(1);
            (s_utc, e_utc)
        }
        CanonicalTimePrecision::Minute => {
            let local_start = local_dt
                .with_second(0)
                .and_then(|dt| dt.with_nanosecond(0))
                .unwrap_or(local_dt);
            let s_utc = local_start.with_timezone(&Utc) + offset_duration;
            let e_utc = s_utc + Duration::minutes(1);
            (s_utc, e_utc)
        }
        CanonicalTimePrecision::Hour => {
            let local_start = local_dt
                .with_minute(0)
                .and_then(|dt| dt.with_second(0))
                .and_then(|dt| dt.with_nanosecond(0))
                .unwrap_or(local_dt);
            let s_utc = local_start.with_timezone(&Utc) + offset_duration;
            let e_utc = s_utc + Duration::hours(1);
            (s_utc, e_utc)
        }
        CanonicalTimePrecision::Day => {
            // Midnight start in local calendar
            let local_start_naive = local_dt.date_naive().and_hms_opt(0, 0, 0).unwrap();
            let local_end_naive = local_dt
                .date_naive()
                .succ_opt()
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap();

            let s_utc = match tz.from_local_datetime(&local_start_naive) {
                chrono::LocalResult::Single(dt) => dt.with_timezone(&Utc),
                chrono::LocalResult::Ambiguous(earliest, _) => earliest.with_timezone(&Utc),
                chrono::LocalResult::None => local_dt.with_timezone(&Utc),
            } + offset_duration;

            let e_utc = match tz.from_local_datetime(&local_end_naive) {
                chrono::LocalResult::Single(dt) => dt.with_timezone(&Utc),
                chrono::LocalResult::Ambiguous(earliest, _) => earliest.with_timezone(&Utc),
                chrono::LocalResult::None => s_utc + Duration::days(1),
            } + offset_duration;

            (s_utc, e_utc)
        }
        CanonicalTimePrecision::Estimated => {
            let half_duration =
                Duration::milliseconds((estimated_half_width_seconds * 1000.0).round() as i64);
            (nominal_utc - half_duration, nominal_utc + half_duration)
        }
    };

    Ok((nominal_utc, start_utc, end_utc))
}

/// Returns the half-open intersection [overlap_start, overlap_end) if two intervals intersect.
///
/// Because intervals are half-open [start, end), adjacent intervals [t0, t1) and [t1, t2)
/// have max(start_a, start_b) == min(end_a, end_b) == t1 and do NOT overlap.
pub fn intervals_overlap(
    start_a: DateTime<Utc>,
    end_a: DateTime<Utc>,
    start_b: DateTime<Utc>,
    end_b: DateTime<Utc>,
) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
    let overlap_start = start_a.max(start_b);
    let overlap_end = end_a.min(end_b);
    if overlap_start < overlap_end {
        Some((overlap_start, overlap_end))
    } else {
        None
    }
}

/// Raw input event for timeline reconstruction.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RawTimelineEvent {
    pub event_id: String,
    pub source_evidence_id: String,
    pub timestamp: String,
    pub timestamp_precision: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uncertainty_window_start: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uncertainty_window_end: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coordinates: Option<super::model::GeoPoint>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_description: Option<String>,
    #[serde(default)]
    pub entity_ids: Vec<String>,
    #[serde(default)]
    pub metadata: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_timezone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clock_offset_seconds: Option<f64>,
    /// Values already substituted before this stage (e.g. by the extraction adapter).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assumptions: Vec<ReconstructionAssumption>,
}

/// Reconstructs a deterministic timeline from input events.
pub fn reconstruct_timeline(
    events: &[RawTimelineEvent],
    default_timezone: &str,
    default_clock_offset: f64,
    estimated_half_width_seconds: f64,
    case_id: Option<String>,
) -> Result<TimelineReconstructionResult, AppError> {
    let mut entries: Vec<TimelineEntry> = Vec::with_capacity(events.len());
    let mut excluded_events: Vec<super::model::ExcludedTimelineEvent> = Vec::new();

    for ev in events {
        let tz = ev.source_timezone.as_deref().unwrap_or(default_timezone);
        let offset = ev.clock_offset_seconds.unwrap_or(default_clock_offset);
        let precision = match normalize_time_precision(&ev.timestamp_precision) {
            Ok(p) => p,
            Err(e) => {
                excluded_events.push(super::model::ExcludedTimelineEvent {
                    event_id: ev.event_id.clone(),
                    source_evidence_id: ev.source_evidence_id.clone(),
                    reason: format!(
                        "Unsupported timestamp precision '{}': {}",
                        ev.timestamp_precision, e
                    ),
                });
                continue;
            }
        };
        let mut assumptions = ev.assumptions.clone();
        if ev.source_timezone.is_none() {
            assumptions.push(ReconstructionAssumption {
                kind: AssumptionKind::TimezoneNotDeclaredOnEvent,
                field: "source_timezone".to_string(),
                assumed_value: default_timezone.to_string(),
                basis: "Reconstruction default; the event declared no timezone.".to_string(),
            });
        }
        if ev.clock_offset_seconds.is_none() {
            assumptions.push(ReconstructionAssumption {
                kind: AssumptionKind::ClockOffsetNotDeclaredOnEvent,
                field: "clock_offset_seconds".to_string(),
                assumed_value: default_clock_offset.to_string(),
                basis: "Reconstruction default; the event declared no clock offset, so the \
                        source clock is uncalibrated."
                    .to_string(),
            });
        }
        assumptions.sort();
        assumptions.dedup();

        let has_explicit =
            ev.uncertainty_window_start.is_some() || ev.uncertainty_window_end.is_some();
        let (nominal_utc, start_utc, end_utc) = match compute_uncertainty_window(
            &ev.timestamp,
            precision,
            tz,
            offset,
            estimated_half_width_seconds,
            ev.uncertainty_window_start.as_deref(),
            ev.uncertainty_window_end.as_deref(),
        ) {
            Ok(window) => window,
            Err(e) => {
                excluded_events.push(super::model::ExcludedTimelineEvent {
                    event_id: ev.event_id.clone(),
                    source_evidence_id: ev.source_evidence_id.clone(),
                    reason: format!("Invalid timestamp or timezone: {}", e),
                });
                continue;
            }
        };

        let span_seconds = (end_utc - start_utc).num_milliseconds() as f64 / 1000.0;
        let is_estimated_assumption =
            precision == CanonicalTimePrecision::Estimated && !has_explicit;

        let time_window = NormalizedTimeWindow {
            nominal_utc,
            precision,
            raw_precision_input: ev.timestamp_precision.clone(),
            window_start_utc: start_utc,
            window_end_utc: end_utc,
            uncertainty_span_seconds: span_seconds,
            has_explicit_bounds: has_explicit,
            is_estimated_window_assumption: is_estimated_assumption,
            estimated_half_width_seconds: if is_estimated_assumption {
                Some(estimated_half_width_seconds)
            } else {
                None
            },
            duration_seconds: ev.duration_seconds,
            source_timezone: tz.to_string(),
            clock_offset_seconds: offset,
        };

        entries.push(TimelineEntry {
            event_id: ev.event_id.clone(),
            source_evidence_id: ev.source_evidence_id.clone(),
            raw_timestamp: ev.timestamp.clone(),
            time_window,
            coordinates: ev.coordinates,
            raw_description: ev.raw_description.clone(),
            entity_ids: ev.entity_ids.clone(),
            metadata: ev.metadata.clone(),
            ordering_status: TemporalOrderingStatus::StrictlyOrdered,
            concurrent_event_ids: Vec::new(),
            assumptions,
        });
    }

    // Sort entries deterministically: window_start_utc, nominal_utc, event_id
    entries.sort_by(|a, b| {
        a.time_window
            .window_start_utc
            .cmp(&b.time_window.window_start_utc)
            .then_with(|| a.time_window.nominal_utc.cmp(&b.time_window.nominal_utc))
            .then_with(|| a.event_id.cmp(&b.event_id))
    });

    // Detect pairwise overlapping half-open intervals
    let mut overlaps: Vec<TimelineOverlapPair> = Vec::new();
    let n = entries.len();

    for i in 0..n {
        for j in (i + 1)..n {
            let (start_a, end_a) = (
                entries[i].time_window.window_start_utc,
                entries[i].time_window.window_end_utc,
            );
            let (start_b, end_b) = (
                entries[j].time_window.window_start_utc,
                entries[j].time_window.window_end_utc,
            );

            if let Some((o_start, o_end)) = intervals_overlap(start_a, end_a, start_b, end_b) {
                let span = (o_end - o_start).num_milliseconds() as f64 / 1000.0;
                let id_a = entries[i].event_id.clone();
                let id_b = entries[j].event_id.clone();

                overlaps.push(TimelineOverlapPair {
                    event_a_id: id_a.clone(),
                    event_b_id: id_b.clone(),
                    overlap_start_utc: o_start,
                    overlap_end_utc: o_end,
                    overlap_span_seconds: span,
                });

                // Mark both entries as ConcurrentUncertain
                entries[i].ordering_status = TemporalOrderingStatus::ConcurrentUncertain;
                entries[j].ordering_status = TemporalOrderingStatus::ConcurrentUncertain;
                if !entries[i].concurrent_event_ids.contains(&id_b) {
                    entries[i].concurrent_event_ids.push(id_b.clone());
                }
                if !entries[j].concurrent_event_ids.contains(&id_a) {
                    entries[j].concurrent_event_ids.push(id_a);
                }
            }
        }
    }

    let concurrent_cnt = entries
        .iter()
        .filter(|e| e.ordering_status == TemporalOrderingStatus::ConcurrentUncertain)
        .count();
    let strictly_ordered_cnt = entries.len() - concurrent_cnt;

    Ok(TimelineReconstructionResult {
        case_id,
        total_events: entries.len(),
        entries,
        overlaps,
        strictly_ordered_count: strictly_ordered_cnt,
        concurrent_uncertain_count: concurrent_cnt,
        excluded_events,
    })
}
