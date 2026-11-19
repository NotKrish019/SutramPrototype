//! Spatial Reconstruction Engine (Member 4).
//!
//! Provides WGS84 coordinate validation, Haversine great-circle distance computation (DEC-003),
//! entity-grouped spatial-temporal trajectory construction, conservative elapsed-time
//! and minimum-required-speed calculations over half-open intervals [start, end),
//! and explicit handling of overlapping/uncertain event windows.

use std::collections::BTreeMap;

use super::model::{
    EntityTrajectory, GeoPoint, MovementSegment, NormalizedTimeWindow, SpatialReconstructionResult,
    SpatialTemporalOverlap, SpeedThresholdEvaluation, TimelineEntry, TransitTimeMetrics,
    EARTH_MEAN_RADIUS_KM,
};
use super::temporal::intervals_overlap;
use crate::error::AppError;

/// Default reference speed threshold in km/h for investigative triage.
pub const DEFAULT_REFERENCE_SPEED_THRESHOLD_KMH: f64 = 180.0;

/// Calculates great-circle Haversine distance in kilometers between two WGS84 points.
pub fn haversine_distance_km(origin: GeoPoint, destination: GeoPoint) -> f64 {
    // Check identical point
    if (origin.lat == destination.lat)
        && (origin.lon == destination.lon
            || (origin.lon.abs() == 180.0 && destination.lon.abs() == 180.0)
            || origin.lat.abs() == 90.0)
    {
        return 0.0;
    }

    let phi1 = origin.lat.to_radians();
    let phi2 = destination.lat.to_radians();
    let delta_phi = (destination.lat - origin.lat).to_radians();
    let delta_lambda = (destination.lon - origin.lon).to_radians();

    let sin_half_dphi = (delta_phi / 2.0).sin();
    let sin_half_dlambda = (delta_lambda / 2.0).sin();

    let a = sin_half_dphi * sin_half_dphi
        + phi1.cos() * phi2.cos() * sin_half_dlambda * sin_half_dlambda;
    let clamped_a = a.clamp(0.0, 1.0);
    let central_angle = 2.0 * clamped_a.sqrt().asin();
    let distance = EARTH_MEAN_RADIUS_KM * central_angle;

    if distance.abs() < 1e-12 {
        0.0
    } else {
        distance
    }
}

/// Computes elapsed-time bounds and conservative minimum required speed (km/h) (DEC-003).
///
/// Forensic guarantees:
/// 1. Uses `max_possible_elapsed_seconds` for `min_required_speed_kmh` so speed is never overstated.
/// 2. Handles zero distance, negative time, and division by zero safely.
pub fn calculate_transit_metrics(
    origin_time_window: &NormalizedTimeWindow,
    destination_time_window: &NormalizedTimeWindow,
    distance_km: f64,
    speed_threshold_kmh: f64,
) -> Result<TransitTimeMetrics, AppError> {
    if !distance_km.is_finite() || distance_km < 0.0 {
        return Err(AppError::BadRequest(
            "distance_km must be a finite non-negative number.".to_string(),
        ));
    }
    if !speed_threshold_kmh.is_finite() || speed_threshold_kmh <= 0.0 {
        return Err(AppError::BadRequest(
            "speed_threshold_kmh must be a finite positive number.".to_string(),
        ));
    }

    let dist = if distance_km == 0.0 { 0.0 } else { distance_km };

    let nominal_elapsed = (destination_time_window.nominal_utc - origin_time_window.nominal_utc)
        .num_milliseconds() as f64
        / 1000.0;
    let min_elapsed = (destination_time_window.window_start_utc - origin_time_window.window_end_utc)
        .num_milliseconds() as f64
        / 1000.0;
    let max_elapsed = (destination_time_window.window_end_utc - origin_time_window.window_start_utc)
        .num_milliseconds() as f64
        / 1000.0;

    let nominal_speed = if dist == 0.0 && nominal_elapsed >= 0.0 {
        Some(0.0)
    } else if nominal_elapsed > 0.0 {
        Some((dist * 3600.0) / nominal_elapsed)
    } else {
        None
    };

    let (min_req_speed, exceeds_threshold, evaluation) = if dist == 0.0 {
        (
            Some(0.0),
            false,
            SpeedThresholdEvaluation::ZeroDistanceStationary,
        )
    } else if max_elapsed <= 0.0 {
        (None, true, SpeedThresholdEvaluation::NonPositiveElapsedTime)
    } else {
        let speed = (dist * 3600.0) / max_elapsed;
        let exceeds = speed > speed_threshold_kmh;
        let eval = if exceeds {
            SpeedThresholdEvaluation::ExceedsConfiguredThreshold
        } else {
            SpeedThresholdEvaluation::WithinConfiguredThreshold
        };
        (Some(speed), exceeds, eval)
    };

    Ok(TransitTimeMetrics {
        distance_km: dist,
        nominal_elapsed_seconds: nominal_elapsed,
        min_possible_elapsed_seconds: min_elapsed,
        max_possible_elapsed_seconds: max_elapsed,
        min_required_speed_kmh: min_req_speed,
        nominal_speed_kmh: nominal_speed,
        configured_speed_threshold_kmh: speed_threshold_kmh,
        exceeds_configured_speed_threshold: exceeds_threshold,
        threshold_evaluation: evaluation,
    })
}

/// Constructs a directed MovementSegment between two consecutive located events.
pub fn create_movement_segment(
    origin_entry: &TimelineEntry,
    destination_entry: &TimelineEntry,
    entity_id: &str,
    speed_threshold_kmh: f64,
    intervening_unlocated_event_ids: Vec<String>,
) -> Result<MovementSegment, AppError> {
    let origin_coords = origin_entry.coordinates.ok_or_else(|| {
        AppError::BadRequest(format!(
            "Origin event '{}' lacks WGS84 coordinates.",
            origin_entry.event_id
        ))
    })?;
    let dest_coords = destination_entry.coordinates.ok_or_else(|| {
        AppError::BadRequest(format!(
            "Destination event '{}' lacks WGS84 coordinates.",
            destination_entry.event_id
        ))
    })?;

    if intervals_overlap(
        origin_entry.time_window.window_start_utc,
        origin_entry.time_window.window_end_utc,
        destination_entry.time_window.window_start_utc,
        destination_entry.time_window.window_end_utc,
    )
    .is_some()
    {
        return Err(AppError::BadRequest(format!(
            "Cannot construct MovementSegment: events '{}' and '{}' have overlapping uncertainty windows.",
            origin_entry.event_id, destination_entry.event_id
        )));
    }

    if origin_entry.time_window.window_end_utc > destination_entry.time_window.window_start_utc {
        return Err(AppError::BadRequest(format!(
            "Cannot construct MovementSegment: origin event '{}' does not strictly precede destination '{}'.",
            origin_entry.event_id, destination_entry.event_id
        )));
    }

    let dist = haversine_distance_km(origin_coords, dest_coords);
    let transit_metrics = calculate_transit_metrics(
        &origin_entry.time_window,
        &destination_entry.time_window,
        dist,
        speed_threshold_kmh,
    )?;

    let has_unlocated = !intervening_unlocated_event_ids.is_empty();

    Ok(MovementSegment {
        origin_event_id: origin_entry.event_id.clone(),
        destination_event_id: destination_entry.event_id.clone(),
        entity_id: entity_id.to_string(),
        origin_coordinates: origin_coords,
        destination_coordinates: dest_coords,
        transit_metrics,
        has_intervening_unlocated_events: has_unlocated,
        intervening_unlocated_event_ids,
    })
}

/// Reconstructs spatial trajectories and detects spatio-temporal overlaps grouped by entity.
pub fn reconstruct_spatial_trajectories(
    entries: &[TimelineEntry],
    speed_threshold_kmh: f64,
    case_id: Option<String>,
) -> Result<SpatialReconstructionResult, AppError> {
    // 1. Group events by entity_id
    let mut entity_events: BTreeMap<String, Vec<&TimelineEntry>> = BTreeMap::new();
    for entry in entries {
        for entity_id in &entry.entity_ids {
            entity_events
                .entry(entity_id.clone())
                .or_default()
                .push(entry);
        }
    }

    let mut trajectories: Vec<EntityTrajectory> = Vec::new();
    let mut overlaps: Vec<SpatialTemporalOverlap> = Vec::new();

    for (entity_id, ev_list) in entity_events {
        // Collect all located events in chronological order
        let mut located_entries: Vec<&TimelineEntry> = Vec::new();
        for entry in &ev_list {
            if entry.coordinates.is_some() {
                located_entries.push(entry);
            }
        }

        // Detect spatio-temporal overlaps between located events for this entity
        let loc_len = located_entries.len();
        for i in 0..loc_len {
            for j in (i + 1)..loc_len {
                let e_a = located_entries[i];
                let e_b = located_entries[j];

                if let Some((o_start, o_end)) = intervals_overlap(
                    e_a.time_window.window_start_utc,
                    e_a.time_window.window_end_utc,
                    e_b.time_window.window_start_utc,
                    e_b.time_window.window_end_utc,
                ) {
                    let c_a = e_a.coordinates.unwrap();
                    let c_b = e_b.coordinates.unwrap();
                    let dist = haversine_distance_km(c_a, c_b);
                    let is_co_located = dist < 0.05; // Within 50 meters

                    overlaps.push(SpatialTemporalOverlap {
                        entity_id: entity_id.clone(),
                        event_a_id: e_a.event_id.clone(),
                        event_b_id: e_b.event_id.clone(),
                        overlap_start_utc: o_start,
                        overlap_end_utc: o_end,
                        distance_km: dist,
                        is_co_located,
                    });
                }
            }
        }

        // Build movement segments between strictly ordered consecutive located entries
        let mut segments: Vec<MovementSegment> = Vec::new();
        let mut total_dist = 0.0;
        let mut max_speed: Option<f64> = None;

        for window in located_entries.windows(2) {
            let origin = window[0];
            let dest = window[1];

            // Only build segment if intervals do NOT overlap and origin precedes dest
            if intervals_overlap(
                origin.time_window.window_start_utc,
                origin.time_window.window_end_utc,
                dest.time_window.window_start_utc,
                dest.time_window.window_end_utc,
            )
            .is_none()
                && origin.time_window.window_end_utc <= dest.time_window.window_start_utc
            {
                // Find any unlocated events for this entity strictly between origin and dest
                let intervening_unlocated: Vec<String> = ev_list
                    .iter()
                    .filter(|e| {
                        e.coordinates.is_none()
                            && e.time_window.window_start_utc >= origin.time_window.window_end_utc
                            && e.time_window.window_end_utc <= dest.time_window.window_start_utc
                    })
                    .map(|e| e.event_id.clone())
                    .collect();

                if let Ok(segment) = create_movement_segment(
                    origin,
                    dest,
                    &entity_id,
                    speed_threshold_kmh,
                    intervening_unlocated,
                ) {
                    total_dist += segment.transit_metrics.distance_km;
                    if let Some(spd) = segment.transit_metrics.min_required_speed_kmh {
                        max_speed = Some(max_speed.map_or(spd, |current| current.max(spd)));
                    }
                    segments.push(segment);
                }
            }
        }

        trajectories.push(EntityTrajectory {
            entity_id,
            waypoints: located_entries.into_iter().cloned().collect(),
            segments,
            total_distance_km: total_dist,
            max_min_required_speed_kmh: max_speed,
        });
    }

    Ok(SpatialReconstructionResult {
        case_id,
        trajectories,
        overlaps,
    })
}
