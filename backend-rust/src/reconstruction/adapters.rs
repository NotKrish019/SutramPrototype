//! Adapters bridging extraction outputs to reconstruction inputs.

use super::model::{AssumptionKind, GeoPoint, ReconstructionAssumption};
use super::temporal::RawTimelineEvent;
use crate::extraction::model::{ExtractedEventCandidate, UnifiedExtractionResult};

/// Resolves synthetic cell tower identifiers to representative WGS84 geographic centroids.
///
/// Used as an investigative fallback when raw CDR logs supply Base Transceiver Station (BTS)
/// tower codes without embedded GPS coordinates.
pub fn resolve_cell_tower_centroid(tower_id: &str) -> Option<GeoPoint> {
    let normalized = tower_id.trim().to_ascii_uppercase();
    if normalized.contains("SOUTH") || normalized.contains("SAKET") {
        // Saket / South Delhi centroid
        GeoPoint::new(28.5244, 77.2066).ok()
    } else if normalized.contains("CENTRAL") || normalized.contains("CONNAUGHT") {
        // Connaught Place / Central Delhi centroid
        GeoPoint::new(28.6139, 77.2090).ok()
    } else if normalized.contains("NOIDA") || normalized.contains("SEC62") {
        // Noida Sector 62 centroid
        GeoPoint::new(28.6270, 77.3725).ok()
    } else if normalized.contains("NORTH") || normalized.contains("CIVIL_LINES") {
        // Civil Lines / North Delhi centroid
        GeoPoint::new(28.6812, 77.2227).ok()
    } else if normalized.contains("AIRPORT") || normalized.contains("IGI") {
        // IGI Airport T3 centroid
        GeoPoint::new(28.5562, 77.1000).ok()
    } else if normalized.contains("MUMBAI") || normalized.contains("BKC") {
        // Bandra Kurla Complex / Mumbai centroid
        GeoPoint::new(19.0607, 72.8633).ok()
    } else {
        None
    }
}

/// Converts an `ExtractedEventCandidate` from extraction into a `RawTimelineEvent` for reconstruction,
/// with optional exhibit-level context fallbacks for source timezone and clock offset.
pub fn event_candidate_to_timeline_event_with_context(
    candidate: &ExtractedEventCandidate,
    default_precision: Option<&str>,
    fallback_timezone: Option<&str>,
    fallback_clock_offset: Option<f64>,
) -> RawTimelineEvent {
    let mut coords: Option<GeoPoint> = None;
    let mut assumptions: Vec<ReconstructionAssumption> = Vec::new();

    // 1. Direct coordinates object { "lat": ..., "lon": ... }
    if let Some(coords_val) = candidate.metadata.get("coordinates") {
        if let (Some(lat), Some(lon)) = (
            coords_val.get("lat").and_then(|v| v.as_f64()),
            coords_val.get("lon").and_then(|v| v.as_f64()),
        ) {
            coords = GeoPoint::new(lat, lon).ok();
        }
    } else if let (Some(lat), Some(lon)) = (
        candidate.metadata.get("latitude").and_then(|v| v.as_f64()),
        candidate.metadata.get("longitude").and_then(|v| v.as_f64()),
    ) {
        coords = GeoPoint::new(lat, lon).ok();
    }

    // 2. Cell tower centroid fallback
    if coords.is_none() {
        if let Some(tower_val) = candidate
            .metadata
            .get("cell_tower_id")
            .and_then(|v| v.as_str())
            .or(candidate.location_name.as_deref())
        {
            coords = resolve_cell_tower_centroid(tower_val);
            if let Some(point) = &coords {
                assumptions.push(ReconstructionAssumption {
                    kind: AssumptionKind::CoordinatesInferredFromTowerId,
                    field: "coordinates".to_string(),
                    assumed_value: format!("{},{}", point.lat, point.lon),
                    basis: format!(
                        "Representative centroid looked up from tower identifier '{}'; not an \
                         observed position. The device may have been anywhere in the cell.",
                        tower_val
                    ),
                });
            }
        }
    }

    // Producer-supplied precision: the typed candidate field (Member 1), else metadata.
    let supplied_precision = candidate.timestamp_precision.as_deref().or_else(|| {
        candidate
            .metadata
            .get("timestamp_precision")
            .and_then(|v| v.as_str())
    });
    let precision = supplied_precision
        .or(default_precision)
        .unwrap_or("EXACT_SECOND")
        .to_string();
    if supplied_precision.is_none() {
        assumptions.push(ReconstructionAssumption {
            kind: AssumptionKind::PrecisionDefaulted,
            field: "timestamp_precision".to_string(),
            assumed_value: precision.clone(),
            basis: "The extraction candidate supplies no timestamp_precision; a default was used, \
                    so the uncertainty window may be too narrow."
                .to_string(),
        });
    }

    let uncertainty_window_start = candidate
        .metadata
        .get("uncertainty_window_start")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let uncertainty_window_end = candidate
        .metadata
        .get("uncertainty_window_end")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let event_timezone = candidate
        .metadata
        .get("source_timezone")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let source_timezone = event_timezone
        .clone()
        .or_else(|| fallback_timezone.map(|s| s.to_string()));
    if event_timezone.is_none() {
        if let Some(tz) = &source_timezone {
            assumptions.push(ReconstructionAssumption {
                kind: AssumptionKind::TimezoneNotDeclaredOnEvent,
                field: "source_timezone".to_string(),
                assumed_value: tz.clone(),
                basis: "Exhibit-level or configured value, not declared on the event. Evidence \
                        upload records 'UTC' when no timezone is declared."
                    .to_string(),
            });
        }
    }

    let event_offset = candidate
        .metadata
        .get("clock_offset_seconds")
        .and_then(|v| v.as_f64());
    let clock_offset_seconds = event_offset.or(fallback_clock_offset);
    if event_offset.is_none() {
        if let Some(offset) = clock_offset_seconds {
            assumptions.push(ReconstructionAssumption {
                kind: AssumptionKind::ClockOffsetNotDeclaredOnEvent,
                field: "clock_offset_seconds".to_string(),
                assumed_value: offset.to_string(),
                basis: "Exhibit-level or configured value, not declared on the event. Evidence \
                        upload records 0.0 when no offset is declared, so the clock may be \
                        uncalibrated."
                    .to_string(),
            });
        }
    }
    assumptions.sort();

    let raw_description = candidate
        .description
        .clone()
        .or_else(|| candidate.location_name.clone());

    let mut metadata = candidate.metadata.clone();
    if let serde_json::Value::Object(ref mut map) = metadata {
        if !candidate.source_record_id.trim().is_empty() {
            map.entry("source_record_id".to_string())
                .or_insert_with(|| serde_json::Value::String(candidate.source_record_id.clone()));
        }
    }

    RawTimelineEvent {
        event_id: candidate.candidate_id.clone(),
        source_evidence_id: candidate.source_evidence_id.clone(),
        timestamp: candidate.raw_timestamp.clone(),
        timestamp_precision: precision,
        duration_seconds: candidate.duration_seconds,
        uncertainty_window_start,
        uncertainty_window_end,
        coordinates: coords,
        raw_description,
        entity_ids: candidate.involved_entity_raw_ids.clone(),
        metadata,
        source_timezone,
        clock_offset_seconds,
        assumptions,
    }
}

/// Converts an `ExtractedEventCandidate` into a `RawTimelineEvent` using standard defaults.
pub fn event_candidate_to_timeline_event(
    candidate: &ExtractedEventCandidate,
    default_precision: Option<&str>,
) -> RawTimelineEvent {
    event_candidate_to_timeline_event_with_context(candidate, default_precision, None, None)
}

/// Converts a complete `UnifiedExtractionResult` into a vector of `RawTimelineEvent` instances,
/// automatically inheriting exhibit-level timezone and clock offset from extraction metadata.
pub fn unified_extraction_to_timeline_events(
    result: &UnifiedExtractionResult,
    default_precision: Option<&str>,
) -> Vec<RawTimelineEvent> {
    let fallback_tz = result
        .extraction_metadata
        .get("source_timezone")
        .and_then(|v| v.as_str());
    let fallback_offset = result
        .extraction_metadata
        .get("clock_offset_seconds")
        .and_then(|v| v.as_f64());
    let extraction_run_id = result
        .extraction_metadata
        .get("extraction_run_id")
        .and_then(|v| v.as_str());

    result
        .event_candidates
        .iter()
        .map(|candidate| {
            let mut event = event_candidate_to_timeline_event_with_context(
                candidate,
                default_precision,
                fallback_tz,
                fallback_offset,
            );
            if let Some(run_id) = extraction_run_id {
                if let serde_json::Value::Object(ref mut map) = event.metadata {
                    map.entry("extraction_run_id".to_string())
                        .or_insert_with(|| serde_json::Value::String(run_id.to_string()));
                }
            }
            event
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_candidate_to_timeline_event_basic() {
        let candidate = ExtractedEventCandidate {
            candidate_id: "evt-001".to_string(),
            source_evidence_id: "evi-001".to_string(),
            source_record_id: "rec-001".to_string(),
            event_type: "TELEPHONY_CALL".to_string(),
            raw_timestamp: "2026-11-18T10:00:00Z".to_string(),
            timestamp_precision: Some("EXACT_SECOND".to_string()),
            description: Some("Telephony call via Saket Tower".to_string()),
            duration_seconds: Some(120.0),
            location_name: Some("Saket Tower".to_string()),
            involved_entity_raw_ids: vec!["+919811002233".to_string()],
            metadata: serde_json::json!({
                "cell_tower_id": "TOWER-DELHI-SOUTH-04B",
                "clock_offset_seconds": 15.0
            }),
        };

        let raw = event_candidate_to_timeline_event(&candidate, Some("EXACT_SECOND"));
        assert_eq!(raw.event_id, "evt-001");
        assert_eq!(raw.source_evidence_id, "evi-001");
        assert_eq!(raw.timestamp, "2026-11-18T10:00:00Z");
        assert_eq!(raw.timestamp_precision, "EXACT_SECOND");
        assert_eq!(raw.duration_seconds, Some(120.0));
        assert_eq!(raw.clock_offset_seconds, Some(15.0));

        // Tower resolved
        assert!(raw.coordinates.is_some());
        let coords = raw.coordinates.unwrap();
        assert!((coords.lat - 28.5244).abs() < 1e-4);
        assert!((coords.lon - 77.2066).abs() < 1e-4);
    }

    #[test]
    fn test_context_fallback_propagation() {
        let candidate = ExtractedEventCandidate {
            candidate_id: "evt-002".to_string(),
            source_evidence_id: "evi-002".to_string(),
            source_record_id: "rec-002".to_string(),
            event_type: "CHAT_MESSAGE".to_string(),
            raw_timestamp: "2026-11-18T12:00:00".to_string(),
            timestamp_precision: None,
            description: None,
            duration_seconds: None,
            location_name: None,
            involved_entity_raw_ids: vec!["user1".to_string()],
            metadata: serde_json::json!({}),
        };

        let raw = event_candidate_to_timeline_event_with_context(
            &candidate,
            Some("MINUTE"),
            Some("Asia/Kolkata"),
            Some(-30.0),
        );

        assert_eq!(raw.source_timezone, Some("Asia/Kolkata".to_string()));
        assert_eq!(raw.clock_offset_seconds, Some(-30.0));
        assert_eq!(raw.timestamp_precision, "MINUTE");
    }
}
