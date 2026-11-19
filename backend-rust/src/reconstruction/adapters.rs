//! Adapters bridging extraction outputs to reconstruction inputs.

use super::model::GeoPoint;
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
        }
    }

    let precision = candidate
        .metadata
        .get("timestamp_precision")
        .and_then(|v| v.as_str())
        .or(default_precision)
        .unwrap_or("EXACT_SECOND")
        .to_string();

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

    let source_timezone = candidate
        .metadata
        .get("source_timezone")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| fallback_timezone.map(|s| s.to_string()));

    let clock_offset_seconds = candidate
        .metadata
        .get("clock_offset_seconds")
        .and_then(|v| v.as_f64())
        .or(fallback_clock_offset);

    RawTimelineEvent {
        event_id: candidate.candidate_id.clone(),
        source_evidence_id: candidate.source_evidence_id.clone(),
        timestamp: candidate.raw_timestamp.clone(),
        timestamp_precision: precision,
        duration_seconds: candidate.duration_seconds,
        uncertainty_window_start,
        uncertainty_window_end,
        coordinates: coords,
        raw_description: candidate.location_name.clone(),
        entity_ids: candidate.involved_entity_raw_ids.clone(),
        metadata: candidate.metadata.clone(),
        source_timezone,
        clock_offset_seconds,
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

    result
        .event_candidates
        .iter()
        .map(|candidate| {
            event_candidate_to_timeline_event_with_context(
                candidate,
                default_precision,
                fallback_tz,
                fallback_offset,
            )
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
