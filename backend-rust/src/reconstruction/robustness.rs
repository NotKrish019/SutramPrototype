//! Lead robustness ("breaking point") analysis.
//!
//! For every contradiction lead this answers the cross-examination question *"how wrong would
//! your assumptions have to be for this lead to disappear?"* with numbers:
//!
//! * **Clock** — the relative clock shift between the two events that would make the transit
//!   feasible. When both events come from the same exhibit they share one device clock, so a
//!   constant offset cancels out and the lead does not depend on clock calibration.
//! * **Location** — the combined location error (km) that would make the transit feasible.
//!   Relevant when coordinates are tower centroids rather than observed positions.
//! * **Speed threshold** — the reference threshold at which the lead would no longer fire. This
//!   is a user-chosen benchmark, so it is reported but does not affect the classification.
//!
//! Every boundary is checked by recomputing the lead with the same functions that raised it
//! (`calculate_transit_metrics` / `check_temporal_activity_feasibility`) just inside and just
//! outside the boundary. Nothing here changes a lead or adds statistical weight (DEC-004): the
//! class is a triage band over the computed margins, not a probability.

use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};

use super::contradictions::check_temporal_activity_feasibility;
use super::model::{ContradictionLead, ContradictionLeadType, NormalizedTimeWindow, TimelineEntry};
use super::service::FullCaseReconstructionResult;
use super::spatial::calculate_transit_metrics;

/// Clock margin below which a lead is FRAGILE (2 minutes: ordinary handset drift).
pub const CLOCK_FRAGILE_SECONDS: f64 = 120.0;
/// Clock margin at or above which a lead is ROBUST (15 minutes).
pub const CLOCK_ROBUST_SECONDS: f64 = 900.0;
/// Location margin below which a lead is FRAGILE (2 km: dense urban cell radius).
pub const LOCATION_FRAGILE_KM: f64 = 2.0;
/// Location margin at or above which a lead is ROBUST (10 km).
pub const LOCATION_ROBUST_KM: f64 = 10.0;

/// The assumption being stressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BreakingDimension {
    ClockOffset,
    LocationError,
    SpeedThreshold,
}

/// Triage band over the computed margins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RobustnessClass {
    /// A realistic error in a labelled assumption would dissolve the lead.
    Fragile,
    /// The lead survives small errors but not large ones.
    Sensitive,
    /// The lead survives every realistic error considered.
    Robust,
}

/// One breaking point.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BreakingPoint {
    pub dimension: BreakingDimension,
    /// `false` when this assumption cannot dissolve the lead (e.g. a shared device clock).
    pub applicable: bool,
    /// Margin before the lead dissolves, in `unit`. `None` when not applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub margin: Option<f64>,
    pub unit: String,
    pub explanation: String,
}

/// Robustness of one contradiction lead.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LeadRobustness {
    pub lead_id: String,
    pub lead_type: ContradictionLeadType,
    pub entity_id: String,
    pub event_a_id: String,
    pub event_b_id: String,
    /// Both events come from the same exhibit, so they share one device clock.
    pub same_clock: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distance_km: Option<f64>,
    pub max_possible_elapsed_seconds: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_required_speed_kmh: Option<f64>,
    pub speed_threshold_kmh: f64,
    pub breaking_points: Vec<BreakingPoint>,
    pub robustness_class: RobustnessClass,
    /// Assumption kinds labelled on either event (e.g. `COORDINATES_INFERRED_FROM_TOWER_ID`).
    pub assumption_dependencies: Vec<String>,
    /// Every applicable boundary was confirmed by recomputation just inside and just outside it.
    pub boundary_verified: bool,
    /// One neutral sentence for investigators and reports.
    pub verdict: String,
}

/// Reference bands used for classification, reported so the bands are never hidden.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RobustnessBands {
    pub clock_fragile_below_seconds: f64,
    pub clock_robust_from_seconds: f64,
    pub location_fragile_below_km: f64,
    pub location_robust_from_km: f64,
}

/// Case-level robustness report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CaseRobustnessReport {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub case_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reconstruction_run_id: Option<String>,
    pub generated_at: String,
    pub total_leads: usize,
    pub robust_count: usize,
    pub sensitive_count: usize,
    pub fragile_count: usize,
    pub bands: RobustnessBands,
    pub leads: Vec<LeadRobustness>,
    pub disclaimer: String,
}

fn shift(window: &NormalizedTimeWindow, seconds: f64) -> NormalizedTimeWindow {
    let d = Duration::milliseconds((seconds * 1000.0).round() as i64);
    let mut w = window.clone();
    w.nominal_utc += d;
    w.window_start_utc += d;
    w.window_end_utc += d;
    w
}

/// `true` when the lead still fires for these inputs (same rule that raised it).
fn lead_fires(
    lead_type: ContradictionLeadType,
    a: &NormalizedTimeWindow,
    b: &NormalizedTimeWindow,
    distance_km: Option<f64>,
    threshold: f64,
) -> bool {
    match lead_type {
        ContradictionLeadType::SpatialTransitImpossible => {
            calculate_transit_metrics(a, b, distance_km.unwrap_or(0.0), threshold)
                .map(|m| m.exceeds_configured_speed_threshold)
                .unwrap_or(false)
        }
        _ => check_temporal_activity_feasibility(a, b, distance_km, threshold)
            .map(|(feasible, ..)| !feasible)
            .unwrap_or(false),
    }
}

fn classify(points: &[BreakingPoint]) -> RobustnessClass {
    let mut class = RobustnessClass::Robust;
    for p in points.iter().filter(|p| p.applicable) {
        let band = match (p.dimension, p.margin) {
            (BreakingDimension::ClockOffset, Some(m)) if m < CLOCK_FRAGILE_SECONDS => {
                RobustnessClass::Fragile
            }
            (BreakingDimension::ClockOffset, Some(m)) if m < CLOCK_ROBUST_SECONDS => {
                RobustnessClass::Sensitive
            }
            (BreakingDimension::LocationError, Some(m)) if m < LOCATION_FRAGILE_KM => {
                RobustnessClass::Fragile
            }
            (BreakingDimension::LocationError, Some(m)) if m < LOCATION_ROBUST_KM => {
                RobustnessClass::Sensitive
            }
            _ => RobustnessClass::Robust,
        };
        class = class.min(band);
    }
    class
}

fn human_seconds(s: f64) -> String {
    let s = s.max(0.0);
    if s < 90.0 {
        format!("{s:.0} s")
    } else if s < 5400.0 {
        format!(
            "{} min {:02} s",
            (s / 60.0).floor(),
            (s % 60.0).round() as u64 % 60
        )
    } else {
        format!("{:.1} h", s / 3600.0)
    }
}

/// Analyses one lead against the timeline entries it references.
pub fn analyze_lead(lead: &ContradictionLead, entries: &[TimelineEntry]) -> Option<LeadRobustness> {
    let a = entries.iter().find(|e| e.event_id == lead.event_a_id)?;
    let b = entries.iter().find(|e| e.event_id == lead.event_b_id)?;
    let threshold = lead.derived_inference.reference_speed_threshold_kmh;
    let max_elapsed = lead.derived_inference.max_possible_elapsed_seconds;
    let distance = lead
        .observed_basis
        .observed_distance_km
        .filter(|d| *d > 0.0);
    let same_clock =
        !a.source_evidence_id.is_empty() && a.source_evidence_id == b.source_evidence_id;
    let fires = |aw: &NormalizedTimeWindow, bw: &NormalizedTimeWindow, d: Option<f64>, t: f64| {
        lead_fires(lead.lead_type, aw, bw, d, t)
    };

    // Seconds of extra elapsed time needed before the transit becomes feasible.
    let needed_elapsed = match distance {
        Some(d) => d * 3600.0 / threshold,
        None => 0.0,
    };
    let clock_margin = (needed_elapsed - max_elapsed).max(0.0);
    let mut verified = true;

    // Clock.
    let clock = if same_clock {
        BreakingPoint {
            dimension: BreakingDimension::ClockOffset,
            applicable: false,
            margin: None,
            unit: "seconds".into(),
            explanation: format!(
                "Both events come from exhibit {}; they share one device clock, so a constant clock offset shifts both equally and cannot dissolve this lead.",
                a.source_evidence_id
            ),
        }
    } else {
        let eps = 1.0;
        let inside = fires(
            &a.time_window,
            &shift(&b.time_window, (clock_margin - eps).max(0.0)),
            distance,
            threshold,
        );
        let outside = fires(
            &a.time_window,
            &shift(&b.time_window, clock_margin + eps),
            distance,
            threshold,
        );
        verified &= inside && !outside;
        BreakingPoint {
            dimension: BreakingDimension::ClockOffset,
            applicable: true,
            margin: Some(clock_margin),
            unit: "seconds".into(),
            explanation: format!(
                "Holds unless the two devices' clocks disagree by more than {} in the direction that lengthens the gap between the events.",
                human_seconds(clock_margin)
            ),
        }
    };

    // Location.
    let location = match distance {
        None => BreakingPoint {
            dimension: BreakingDimension::LocationError,
            applicable: false,
            margin: None,
            unit: "km".into(),
            explanation: "The lead does not depend on location: it rests on event timing alone."
                .into(),
        },
        Some(d) => {
            let reachable_km = threshold * max_elapsed.max(0.0) / 3600.0;
            let margin = (d - reachable_km).max(0.0);
            let inside = fires(
                &a.time_window,
                &b.time_window,
                Some((d - margin + 0.01).max(0.0)),
                threshold,
            );
            let outside = margin >= 0.01
                && !fires(
                    &a.time_window,
                    &b.time_window,
                    Some((d - margin - 0.01).max(0.0)),
                    threshold,
                );
            verified &= inside && (outside || margin < 0.01);
            BreakingPoint {
                dimension: BreakingDimension::LocationError,
                applicable: true,
                margin: Some(margin),
                unit: "km".into(),
                explanation: format!(
                    "Holds unless the two locations are wrong by more than {margin:.2} km combined (the events are {d:.2} km apart; at most {reachable_km:.2} km is reachable in the available time)."
                ),
            }
        }
    };

    // Speed threshold (reported, not classified).
    let speed = match lead.derived_inference.post_activity_min_required_speed_kmh {
        Some(v) => BreakingPoint {
            dimension: BreakingDimension::SpeedThreshold,
            applicable: true,
            margin: Some(v),
            unit: "km/h".into(),
            explanation: format!(
                "The lead disappears if the reference threshold is raised to {v:.1} km/h (currently {threshold:.1} km/h)."
            ),
        },
        None => BreakingPoint {
            dimension: BreakingDimension::SpeedThreshold,
            applicable: false,
            margin: None,
            unit: "km/h".into(),
            explanation: "No travel speed is involved: the events cannot be ordered as reported at any speed.".into(),
        },
    };

    let points = vec![clock, location, speed];
    let class = classify(&points[..2]);
    let mut deps: Vec<String> = a
        .assumptions
        .iter()
        .chain(&b.assumptions)
        .filter_map(|x| {
            serde_json::to_value(x.kind)
                .ok()
                .and_then(|v| v.as_str().map(str::to_string))
        })
        .collect();
    deps.sort();
    deps.dedup();

    let weakest = points[..2]
        .iter()
        .filter(|p| p.applicable)
        .map(|p| match p.dimension {
            BreakingDimension::ClockOffset => format!(
                "a clock disagreement above {}",
                human_seconds(p.margin.unwrap_or(0.0))
            ),
            _ => format!(
                "a combined location error above {:.2} km",
                p.margin.unwrap_or(0.0)
            ),
        })
        .collect::<Vec<_>>();
    let verdict = match class {
        RobustnessClass::Robust => format!(
            "ROBUST: the lead survives every realistic error considered; it would need {}.",
            if weakest.is_empty() { "a change to the evidence itself".to_string() } else { weakest.join(" or ") }
        ),
        RobustnessClass::Sensitive => format!(
            "SENSITIVE: the lead holds unless there is {}. Corroborate the weaker assumption before relying on it.",
            weakest.join(" or ")
        ),
        RobustnessClass::Fragile => format!(
            "FRAGILE: an ordinary error could dissolve this lead ({}). Treat it as unconfirmed until the assumption is verified.",
            weakest.join(" or ")
        ),
    };

    Some(LeadRobustness {
        lead_id: lead.lead_id.clone(),
        lead_type: lead.lead_type,
        entity_id: lead.entity_id.clone(),
        event_a_id: lead.event_a_id.clone(),
        event_b_id: lead.event_b_id.clone(),
        same_clock,
        distance_km: distance,
        max_possible_elapsed_seconds: max_elapsed,
        min_required_speed_kmh: lead.derived_inference.post_activity_min_required_speed_kmh,
        speed_threshold_kmh: threshold,
        breaking_points: points,
        robustness_class: class,
        assumption_dependencies: deps,
        boundary_verified: verified,
        verdict,
    })
}

/// Analyses every lead in a reconstruction.
pub fn analyze_case_robustness(
    reconstruction: &FullCaseReconstructionResult,
) -> CaseRobustnessReport {
    let leads: Vec<LeadRobustness> = reconstruction
        .contradictions
        .leads
        .iter()
        .filter_map(|l| analyze_lead(l, &reconstruction.timeline.entries))
        .collect();
    let count = |c: RobustnessClass| leads.iter().filter(|l| l.robustness_class == c).count();
    CaseRobustnessReport {
        case_id: reconstruction.case_id.clone(),
        reconstruction_run_id: reconstruction
            .reconstruction_run_id
            .clone()
            .or_else(|| reconstruction.run_id.clone()),
        generated_at: Utc::now().to_rfc3339(),
        total_leads: leads.len(),
        robust_count: count(RobustnessClass::Robust),
        sensitive_count: count(RobustnessClass::Sensitive),
        fragile_count: count(RobustnessClass::Fragile),
        bands: RobustnessBands {
            clock_fragile_below_seconds: CLOCK_FRAGILE_SECONDS,
            clock_robust_from_seconds: CLOCK_ROBUST_SECONDS,
            location_fragile_below_km: LOCATION_FRAGILE_KM,
            location_robust_from_km: LOCATION_ROBUST_KM,
        },
        leads,
        disclaimer: "Robustness bands are triage aids over computed margins, not probabilities. A ROBUST lead is still an investigative lead, not proof of falsity or identity.".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reconstruction::model::GeoPoint;
    use crate::reconstruction::service::{ForensicReconstructionService, ReconstructionOptions};
    use crate::reconstruction::temporal::RawTimelineEvent;

    fn event(id: &str, evidence: &str, ts: &str, lat: f64, lon: f64) -> RawTimelineEvent {
        RawTimelineEvent {
            event_id: id.into(),
            source_evidence_id: evidence.into(),
            timestamp: ts.into(),
            timestamp_precision: "EXACT_SECOND".into(),
            duration_seconds: None,
            uncertainty_window_start: None,
            uncertainty_window_end: None,
            coordinates: Some(GeoPoint::new(lat, lon).unwrap()),
            raw_description: None,
            entity_ids: vec!["+919811002233".into()],
            metadata: serde_json::Value::Null,
            source_timezone: Some("UTC".into()),
            clock_offset_seconds: Some(0.0),
            assumptions: vec![],
        }
    }

    fn reconstruct(events: &[RawTimelineEvent], threshold: f64) -> FullCaseReconstructionResult {
        let options = ReconstructionOptions {
            speed_threshold_kmh: threshold,
            ..Default::default()
        };
        ForensicReconstructionService::new()
            .reconstruct_case(events, &options)
            .unwrap()
    }

    // Delhi (Saket) and Mumbai (BKC): about 1,150 km apart.
    const DELHI: (f64, f64) = (28.5244, 77.2066);
    const MUMBAI: (f64, f64) = (19.0607, 72.8633);

    #[test]
    fn same_device_leads_do_not_depend_on_the_clock() {
        let r = reconstruct(
            &[
                event("a", "ev-1", "2026-11-10T16:00:00Z", DELHI.0, DELHI.1),
                event("b", "ev-1", "2026-11-10T16:30:00Z", MUMBAI.0, MUMBAI.1),
            ],
            900.0,
        );
        let report = analyze_case_robustness(&r);
        assert_eq!(report.total_leads, 1);
        let lead = &report.leads[0];
        assert!(lead.same_clock);
        let clock = &lead.breaking_points[0];
        assert_eq!(clock.dimension, BreakingDimension::ClockOffset);
        assert!(!clock.applicable);
        // ~1,150 km in ~30 min: no realistic tower error closes that gap.
        assert_eq!(lead.robustness_class, RobustnessClass::Robust);
        assert!(lead.boundary_verified);
    }

    #[test]
    fn cross_device_margin_is_exact_and_verified() {
        let r = reconstruct(
            &[
                event("a", "ev-1", "2026-11-10T16:00:00Z", DELHI.0, DELHI.1),
                event("b", "ev-2", "2026-11-10T16:30:00Z", MUMBAI.0, MUMBAI.1),
            ],
            900.0,
        );
        let lead = &analyze_case_robustness(&r).leads[0];
        assert!(!lead.same_clock);
        let d = lead.distance_km.unwrap();
        let expected = d * 3600.0 / 900.0 - lead.max_possible_elapsed_seconds;
        let clock = lead.breaking_points[0].margin.unwrap();
        assert!((clock - expected).abs() < 1e-6, "{clock} vs {expected}");
        assert!(lead.boundary_verified);
        // Shifting the later event by the margin plus a second dissolves the lead.
        let shifted = reconstruct(
            &[
                event("a", "ev-1", "2026-11-10T16:00:00Z", DELHI.0, DELHI.1),
                {
                    let t = chrono::DateTime::parse_from_rfc3339("2026-11-10T16:30:00Z").unwrap()
                        + chrono::Duration::seconds(clock.ceil() as i64 + 1);
                    event("b", "ev-2", &t.to_rfc3339(), MUMBAI.0, MUMBAI.1)
                },
            ],
            900.0,
        );
        assert!(shifted.contradictions.leads.is_empty());
    }

    #[test]
    fn near_threshold_leads_are_fragile() {
        // Saket -> Noida (~17 km) in 66 s: just over 900 km/h.
        let r = reconstruct(
            &[
                event("a", "ev-1", "2026-11-10T16:00:00Z", 28.5244, 77.2066),
                event("b", "ev-2", "2026-11-10T16:01:05Z", 28.6270, 77.3725),
            ],
            900.0,
        );
        let report = analyze_case_robustness(&r);
        assert_eq!(report.total_leads, 1);
        assert_eq!(report.leads[0].robustness_class, RobustnessClass::Fragile);
        assert_eq!(report.fragile_count, 1);
        assert!(report.leads[0].verdict.starts_with("FRAGILE"));
    }

    #[test]
    fn classification_takes_the_weakest_dimension() {
        let p = |dim, m| BreakingPoint {
            dimension: dim,
            applicable: true,
            margin: Some(m),
            unit: String::new(),
            explanation: String::new(),
        };
        use BreakingDimension::*;
        assert_eq!(
            classify(&[p(ClockOffset, 5000.0), p(LocationError, 50.0)]),
            RobustnessClass::Robust
        );
        assert_eq!(
            classify(&[p(ClockOffset, 5000.0), p(LocationError, 5.0)]),
            RobustnessClass::Sensitive
        );
        assert_eq!(
            classify(&[p(ClockOffset, 30.0), p(LocationError, 50.0)]),
            RobustnessClass::Fragile
        );
    }
}
