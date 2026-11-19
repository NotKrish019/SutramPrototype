//! Contradiction Detection Engine (Member 4).
//!
//! Identifies conservative, evidence-backed temporal and spatial-transit discrepancies
//! across reconstructed timelines and entity trajectories.
//!
//! Core forensic principles enforced (DEC-004, DEC-006):
//! 1. Existential feasibility over uncertainty intervals: A lead is generated ONLY when
//!    NO valid pair (t_A, t_B) in [A_start, A_end) x [B_start, B_end) permits A to complete
//!    and transit to B within the reference speed threshold.
//! 2. Overlapping uncertainty windows are never treated as proof of contradiction.
//! 3. Chronologically unresolved events do not trigger contradiction leads.
//! 4. Neutral, objective language without assertion of guilt, intent, or conclusive identity.
//! 5. Statistical weights are omitted (`weight = None`) for factual contradictions (DEC-004).

use chrono::{Duration, Utc};
use uuid::Uuid;

use super::model::{
    ContradictionAnalysisResult, ContradictionLead, ContradictionLeadType, DerivedLeadInference,
    NormalizedTimeWindow, ObservedLeadBasis, SpatialReconstructionResult, TimelineEntry,
};
use super::spatial::haversine_distance_km;
use super::temporal::intervals_overlap;
use crate::error::AppError;

/// Tuple representing feasibility evaluation:
/// (is_feasible, post_min_elapsed_seconds, post_max_elapsed_seconds, min_required_speed_kmh)
pub type FeasibilityResult = (bool, Option<f64>, Option<f64>, Option<f64>);

/// Evaluates existential feasibility over half-open intervals [A_start, A_end) and [B_start, B_end).
pub fn check_temporal_activity_feasibility(
    origin_time_window: &NormalizedTimeWindow,
    destination_time_window: &NormalizedTimeWindow,
    distance_km: Option<f64>,
    speed_threshold_kmh: f64,
) -> Result<FeasibilityResult, AppError> {
    let duration = match origin_time_window.duration_seconds {
        Some(d) if d > 0.0 => d,
        _ => {
            // No duration or zero duration: feasibility is determined strictly by interval order
            if origin_time_window.window_start_utc >= destination_time_window.window_end_utc {
                return Ok((false, Some(0.0), Some(0.0), None));
            }
            return Ok((true, None, None, None));
        }
    };

    let duration_millis = (duration * 1000.0).round() as i64;
    let earliest_completion_utc =
        origin_time_window.window_start_utc + Duration::milliseconds(duration_millis);
    let latest_completion_utc =
        origin_time_window.window_end_utc + Duration::milliseconds(duration_millis);

    // Supremum (unattained least upper bound) of available time after A completes before B ends
    let post_max_elapsed = (destination_time_window.window_end_utc - earliest_completion_utc)
        .num_milliseconds() as f64
        / 1000.0;
    // Infimum of available time after A completes before B starts
    let post_min_elapsed = (destination_time_window.window_start_utc - latest_completion_utc)
        .num_milliseconds() as f64
        / 1000.0;

    let dist = distance_km.unwrap_or(0.0);

    if dist == 0.0 {
        // Without travel distance: feasible iff post_max_elapsed > 0.0
        let feasible = post_max_elapsed > 0.0;
        return Ok((
            feasible,
            Some(post_min_elapsed),
            Some(post_max_elapsed),
            None,
        ));
    }

    // With travel distance: travel requires post_max_elapsed > (dist * 3600.0 / threshold)
    if post_max_elapsed <= 0.0 {
        return Ok((false, Some(post_min_elapsed), Some(post_max_elapsed), None));
    }

    let min_required_speed = (dist * 3600.0) / post_max_elapsed;
    let feasible = min_required_speed <= speed_threshold_kmh;

    Ok((
        feasible,
        Some(post_min_elapsed),
        Some(post_max_elapsed),
        Some(min_required_speed),
    ))
}

/// Standardized limitations caveats appended to all contradiction leads.
pub fn build_standard_limitations(
    entity_id: &str,
    intervening_unlocated_count: usize,
) -> Vec<String> {
    let mut caveats = vec![
        "The configured speed threshold is a user-adjustable reference benchmark for investigative triage and does not establish a physical upper bound on transit.".to_string(),
        "This indicator is a derived analytical lead for human investigator review and does not establish factual inaccuracy, actor state of mind, or statistical likelihood.".to_string(),
        "Alternative explanations include uncalibrated device clock offsets, shared or transferred device possession, high-speed transport modes, or coarse cell-tower location centroids.".to_string(),
    ];

    if intervening_unlocated_count > 0 {
        caveats.push(format!(
            "Incomplete spatial continuity: {} intervening event(s) for entity '{}' lacked geographic coordinates between origin and destination.",
            intervening_unlocated_count, entity_id
        ));
    }

    caveats
}

/// Generates neutral explanation for spatial transit impossibility.
pub fn format_spatial_lead_explanation(
    entity_id: &str,
    origin_event_id: &str,
    dest_event_id: &str,
    dist_km: f64,
    min_speed: f64,
    threshold: f64,
    max_elapsed_s: f64,
) -> String {
    format!(
        "Entity '{}' is associated with event '{}' and subsequent event '{}' separated by {:.2} km. Given the uncertainty windows, transit required a conservative minimum speed of {:.1} km/h over at most {:.1} seconds, which exceeds the configured investigative reference threshold of {:.1} km/h.",
        entity_id, origin_event_id, dest_event_id, dist_km, min_speed, max_elapsed_s, threshold
    )
}

/// Detects contradictions across reconstructed trajectories and timeline entries.
pub fn detect_contradictions(
    spatial_result: &SpatialReconstructionResult,
    entries: &[TimelineEntry],
    speed_threshold_kmh: f64,
    case_id: Option<String>,
) -> Result<ContradictionAnalysisResult, AppError> {
    let mut leads: Vec<ContradictionLead> = Vec::new();
    let mut total_segments = 0;
    let mut flagged_count = 0;

    let entries_by_id: std::collections::HashMap<&str, &TimelineEntry> =
        entries.iter().map(|e| (e.event_id.as_str(), e)).collect();

    // 1. Evaluate spatial trajectories for speed threshold exceedance
    for trajectory in &spatial_result.trajectories {
        for segment in &trajectory.segments {
            total_segments += 1;

            if segment.transit_metrics.exceeds_configured_speed_threshold {
                flagged_count += 1;

                let origin_ev = entries_by_id.get(segment.origin_event_id.as_str());
                let dest_ev = entries_by_id.get(segment.destination_event_id.as_str());

                let origin_raw = origin_ev
                    .map(|e| e.time_window.nominal_utc.to_rfc3339())
                    .unwrap_or_default();
                let dest_raw = dest_ev
                    .map(|e| e.time_window.nominal_utc.to_rfc3339())
                    .unwrap_or_default();
                let origin_evidence = origin_ev
                    .map(|e| e.source_evidence_id.clone())
                    .unwrap_or_default();
                let dest_evidence = dest_ev
                    .map(|e| e.source_evidence_id.clone())
                    .unwrap_or_default();

                let min_speed = segment
                    .transit_metrics
                    .min_required_speed_kmh
                    .unwrap_or(0.0);
                let dist = segment.transit_metrics.distance_km;
                let max_elapsed = segment.transit_metrics.max_possible_elapsed_seconds;

                let explanation = format_spatial_lead_explanation(
                    &segment.entity_id,
                    &segment.origin_event_id,
                    &segment.destination_event_id,
                    dist,
                    min_speed,
                    speed_threshold_kmh,
                    max_elapsed,
                );

                let limitations = build_standard_limitations(
                    &segment.entity_id,
                    segment.intervening_unlocated_event_ids.len(),
                );

                leads.push(ContradictionLead {
                    lead_id: Uuid::new_v4().to_string(),
                    lead_type: ContradictionLeadType::SpatialTransitImpossible,
                    entity_id: segment.entity_id.clone(),
                    event_a_id: segment.origin_event_id.clone(),
                    event_b_id: segment.destination_event_id.clone(),
                    observed_basis: ObservedLeadBasis {
                        origin_event_id: segment.origin_event_id.clone(),
                        destination_event_id: segment.destination_event_id.clone(),
                        origin_evidence_id: origin_evidence,
                        destination_evidence_id: dest_evidence,
                        origin_timestamp_raw: origin_raw,
                        destination_timestamp_raw: dest_raw,
                        origin_coordinates: Some(segment.origin_coordinates),
                        destination_coordinates: Some(segment.destination_coordinates),
                        observed_distance_km: Some(dist),
                    },
                    derived_inference: DerivedLeadInference {
                        post_activity_min_required_speed_kmh: Some(min_speed),
                        reference_speed_threshold_kmh: speed_threshold_kmh,
                        max_possible_elapsed_seconds: max_elapsed,
                        min_possible_elapsed_seconds: segment
                            .transit_metrics
                            .min_possible_elapsed_seconds,
                        nominal_elapsed_seconds: segment.transit_metrics.nominal_elapsed_seconds,
                    },
                    neutral_explanation: explanation,
                    limitations,
                    generated_at: Utc::now(),
                });
            }
        }
    }

    // 2. Evaluate temporal activity infeasibility across events with non-zero duration
    for i in 0..entries.len() {
        let ev_a = &entries[i];
        if ev_a.time_window.duration_seconds.unwrap_or(0.0) <= 0.0 {
            continue;
        }

        for ev_b in entries.iter().skip(i + 1) {
            // Only evaluate pairs sharing at least one common entity
            let common_entities: Vec<&String> = ev_a
                .entity_ids
                .iter()
                .filter(|id| ev_b.entity_ids.contains(id))
                .collect();

            if common_entities.is_empty() {
                continue;
            }

            // If intervals overlap, chronological order is uncertain
            if intervals_overlap(
                ev_a.time_window.window_start_utc,
                ev_a.time_window.window_end_utc,
                ev_b.time_window.window_start_utc,
                ev_b.time_window.window_end_utc,
            )
            .is_some()
            {
                continue;
            }

            let dist = match (ev_a.coordinates, ev_b.coordinates) {
                (Some(ca), Some(cb)) => Some(haversine_distance_km(ca, cb)),
                _ => None,
            };

            let (feasible, post_min, post_max, min_speed) = check_temporal_activity_feasibility(
                &ev_a.time_window,
                &ev_b.time_window,
                dist,
                speed_threshold_kmh,
            )?;

            if !feasible {
                let entity_id = common_entities[0].clone();
                let explanation = format!(
                    "Entity '{}' is associated with event '{}' (duration {:.1}s) and subsequent event '{}'. Under half-open uncertainty interval bounds, event '{}' could not have concluded prior to event '{}' while permitting necessary physical transit.",
                    entity_id, ev_a.event_id, ev_a.time_window.duration_seconds.unwrap_or(0.0), ev_b.event_id, ev_a.event_id, ev_b.event_id
                );

                let limitations = build_standard_limitations(&entity_id, 0);

                leads.push(ContradictionLead {
                    lead_id: Uuid::new_v4().to_string(),
                    lead_type: ContradictionLeadType::TemporalActivityInfeasible,
                    entity_id,
                    event_a_id: ev_a.event_id.clone(),
                    event_b_id: ev_b.event_id.clone(),
                    observed_basis: ObservedLeadBasis {
                        origin_event_id: ev_a.event_id.clone(),
                        destination_event_id: ev_b.event_id.clone(),
                        origin_evidence_id: ev_a.source_evidence_id.clone(),
                        destination_evidence_id: ev_b.source_evidence_id.clone(),
                        origin_timestamp_raw: ev_a.time_window.nominal_utc.to_rfc3339(),
                        destination_timestamp_raw: ev_b.time_window.nominal_utc.to_rfc3339(),
                        origin_coordinates: ev_a.coordinates,
                        destination_coordinates: ev_b.coordinates,
                        observed_distance_km: dist,
                    },
                    derived_inference: DerivedLeadInference {
                        post_activity_min_required_speed_kmh: min_speed,
                        reference_speed_threshold_kmh: speed_threshold_kmh,
                        max_possible_elapsed_seconds: post_max.unwrap_or(0.0),
                        min_possible_elapsed_seconds: post_min.unwrap_or(0.0),
                        nominal_elapsed_seconds: (ev_b.time_window.nominal_utc
                            - ev_a.time_window.nominal_utc)
                            .num_milliseconds()
                            as f64
                            / 1000.0,
                    },
                    neutral_explanation: explanation,
                    limitations,
                    generated_at: Utc::now(),
                });
            }
        }
    }

    Ok(ContradictionAnalysisResult {
        case_id,
        leads,
        total_evaluated_segments: total_segments,
        flagged_segments_count: flagged_count,
        disclaimer: "Analytical contradiction leads indicate spatio-temporal discrepancies against user-configured thresholds. They are investigative leads and do not constitute conclusive proof of falsity or identity.".to_string(),
    })
}
