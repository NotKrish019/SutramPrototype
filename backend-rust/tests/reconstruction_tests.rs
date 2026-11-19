//! Integration tests for Forensic Reconstruction Engines (Member 4).
//!
//! Verifies:
//! - DEC-001: Additive clock drift offset correction (True UTC = Device Time + Offset).
//! - DEC-002: Half-open interval semantics [start, end) across precision levels.
//! - DEC-003: Haversine distance, elapsed time bounds, and conservative minimum speed.
//! - DEC-004 & DEC-006: Existential feasibility, neutral explanations, no guilt assertion.
//! - DEC-005: Video cryptographic provenance, streaming SHA-256, source immutability.

use chrono::{DateTime, Utc};
use tempfile::tempdir;

use sutram_backend_rust::reconstruction::{
    calculate_transit_metrics, compute_file_sha256, compute_uncertainty_window,
    detect_contradictions, haversine_distance_km, intervals_overlap, parse_and_normalize_timestamp,
    reconstruct_spatial_trajectories, reconstruct_timeline, CanonicalTimePrecision,
    ContradictionLeadType, GeoPoint, RawTimelineEvent, SpeedThresholdEvaluation,
    TemporalOrderingStatus, DEFAULT_ESTIMATED_HALF_WIDTH_SECONDS,
};

// 1. DEC-001: Clock Offset Convention
#[test]
fn test_dec001_clock_offset_additive_convention() {
    let raw = "2026-11-18T12:00:00Z";

    // Fast clock: offset is negative (-30.0s). True UTC is 11:59:30Z.
    let dt_fast = parse_and_normalize_timestamp(raw, "UTC", -30.0).unwrap();
    assert_eq!(dt_fast.to_rfc3339(), "2026-11-18T11:59:30+00:00");

    // Slow clock: offset is positive (+45.0s). True UTC is 12:00:45Z.
    let dt_slow = parse_and_normalize_timestamp(raw, "UTC", 45.0).unwrap();
    assert_eq!(dt_slow.to_rfc3339(), "2026-11-18T12:00:45+00:00");

    // Zero offset
    let dt_zero = parse_and_normalize_timestamp(raw, "UTC", 0.0).unwrap();
    assert_eq!(dt_zero.to_rfc3339(), "2026-11-18T12:00:00+00:00");

    // Timezone localization: Asia/Kolkata naive 12:00:00 is UTC 06:30:00
    let dt_kolkata =
        parse_and_normalize_timestamp("2026-11-18T12:00:00", "Asia/Kolkata", 0.0).unwrap();
    assert_eq!(dt_kolkata.to_rfc3339(), "2026-11-18T06:30:00+00:00");
}

// 2. DEC-002: Half-Open Uncertainty Intervals [start, end)
#[test]
fn test_dec002_half_open_uncertainty_interval_precision() {
    let raw = "2026-11-18T14:32:15.500Z";

    // EXACT_SECOND: [14:32:15, 14:32:16)
    let (_, s_sec, e_sec) = compute_uncertainty_window(
        raw,
        CanonicalTimePrecision::ExactSecond,
        "UTC",
        0.0,
        DEFAULT_ESTIMATED_HALF_WIDTH_SECONDS,
        None,
        None,
    )
    .unwrap();
    assert_eq!(s_sec.to_rfc3339(), "2026-11-18T14:32:15+00:00");
    assert_eq!(e_sec.to_rfc3339(), "2026-11-18T14:32:16+00:00");
    assert_eq!((e_sec - s_sec).num_seconds(), 1);

    // MINUTE: floored to 14:32:00, [14:32:00, 14:33:00)
    let (_, s_min, e_min) = compute_uncertainty_window(
        raw,
        CanonicalTimePrecision::Minute,
        "UTC",
        0.0,
        DEFAULT_ESTIMATED_HALF_WIDTH_SECONDS,
        None,
        None,
    )
    .unwrap();
    assert_eq!(s_min.to_rfc3339(), "2026-11-18T14:32:00+00:00");
    assert_eq!(e_min.to_rfc3339(), "2026-11-18T14:33:00+00:00");
    assert_eq!((e_min - s_min).num_seconds(), 60);

    // HOUR: floored to 14:00:00, [14:00:00, 15:00:00)
    let (_, s_hr, e_hr) = compute_uncertainty_window(
        raw,
        CanonicalTimePrecision::Hour,
        "UTC",
        0.0,
        DEFAULT_ESTIMATED_HALF_WIDTH_SECONDS,
        None,
        None,
    )
    .unwrap();
    assert_eq!(s_hr.to_rfc3339(), "2026-11-18T14:00:00+00:00");
    assert_eq!(e_hr.to_rfc3339(), "2026-11-18T15:00:00+00:00");
    assert_eq!((e_hr - s_hr).num_seconds(), 3600);

    // DAY: floored to 00:00:00, [00:00:00, 24:00:00)
    let (_, s_day, e_day) = compute_uncertainty_window(
        raw,
        CanonicalTimePrecision::Day,
        "UTC",
        0.0,
        DEFAULT_ESTIMATED_HALF_WIDTH_SECONDS,
        None,
        None,
    )
    .unwrap();
    assert_eq!(s_day.to_rfc3339(), "2026-11-18T00:00:00+00:00");
    assert_eq!(e_day.to_rfc3339(), "2026-11-19T00:00:00+00:00");
    assert_eq!((e_day - s_day).num_seconds(), 86400);

    // ESTIMATED: nominal ± 900s
    let (nom, s_est, e_est) = compute_uncertainty_window(
        "2026-11-18T12:00:00Z",
        CanonicalTimePrecision::Estimated,
        "UTC",
        0.0,
        900.0,
        None,
        None,
    )
    .unwrap();
    assert_eq!((nom - s_est).num_seconds(), 900);
    assert_eq!((e_est - nom).num_seconds(), 900);
}

// 3. DEC-002: Half-Open Adjacent Intervals Do Not Overlap
#[test]
fn test_dec002_adjacent_intervals_do_not_overlap() {
    let t0 = DateTime::parse_from_rfc3339("2026-11-18T12:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let t1 = DateTime::parse_from_rfc3339("2026-11-18T12:01:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let t2 = DateTime::parse_from_rfc3339("2026-11-18T12:02:00Z")
        .unwrap()
        .with_timezone(&Utc);

    // [t0, t1) and [t1, t2) share boundary point t1.
    // In half-open semantics, t1 is exclusive in [t0, t1), so they do NOT overlap.
    assert!(intervals_overlap(t0, t1, t1, t2).is_none());

    // Truly overlapping intervals: [t0, t1 + 30s) and [t1, t2)
    let t1_plus = t1 + chrono::Duration::seconds(30);
    let overlap = intervals_overlap(t0, t1_plus, t1, t2);
    assert!(overlap.is_some());
    let (o_start, o_end) = overlap.unwrap();
    assert_eq!(o_start, t1);
    assert_eq!(o_end, t1_plus);
}

// 4. DEC-003: Haversine Distance & Transit Metrics
#[test]
fn test_dec003_haversine_and_speed_evaluation() {
    // New Delhi: 28.6139 N, 77.2090 E
    let delhi = GeoPoint::new(28.6139, 77.2090).unwrap();
    // Mumbai: 19.0760 N, 72.8777 E
    let mumbai = GeoPoint::new(19.0760, 72.8777).unwrap();

    let dist = haversine_distance_km(delhi, mumbai);
    assert!(
        dist > 1140.0 && dist < 1160.0,
        "Expected ~1150 km, got {}",
        dist
    );

    // Distance to self is 0.0
    assert_eq!(haversine_distance_km(delhi, delhi), 0.0);

    // Transit metrics over 1 hour (3600 seconds)
    let (_, s_a, e_a) = compute_uncertainty_window(
        "2026-11-18T10:00:00Z",
        CanonicalTimePrecision::ExactSecond,
        "UTC",
        0.0,
        900.0,
        None,
        None,
    )
    .unwrap();
    let (_, s_b, e_b) = compute_uncertainty_window(
        "2026-11-18T11:00:00Z",
        CanonicalTimePrecision::ExactSecond,
        "UTC",
        0.0,
        900.0,
        None,
        None,
    )
    .unwrap();

    let win_a = sutram_backend_rust::reconstruction::NormalizedTimeWindow {
        nominal_utc: s_a,
        precision: CanonicalTimePrecision::ExactSecond,
        raw_precision_input: "EXACT_SECOND".to_string(),
        window_start_utc: s_a,
        window_end_utc: e_a,
        uncertainty_span_seconds: 1.0,
        has_explicit_bounds: false,
        is_estimated_window_assumption: false,
        estimated_half_width_seconds: None,
        duration_seconds: None,
        source_timezone: "UTC".to_string(),
        clock_offset_seconds: 0.0,
    };
    let win_b = sutram_backend_rust::reconstruction::NormalizedTimeWindow {
        nominal_utc: s_b,
        precision: CanonicalTimePrecision::ExactSecond,
        raw_precision_input: "EXACT_SECOND".to_string(),
        window_start_utc: s_b,
        window_end_utc: e_b,
        uncertainty_span_seconds: 1.0,
        has_explicit_bounds: false,
        is_estimated_window_assumption: false,
        estimated_half_width_seconds: None,
        duration_seconds: None,
        source_timezone: "UTC".to_string(),
        clock_offset_seconds: 0.0,
    };

    let metrics = calculate_transit_metrics(&win_a, &win_b, dist, 180.0).unwrap();
    assert!(metrics.exceeds_configured_speed_threshold);
    assert_eq!(
        metrics.threshold_evaluation,
        SpeedThresholdEvaluation::ExceedsConfiguredThreshold
    );
    assert!(metrics.min_required_speed_kmh.unwrap() > 1140.0);
}

// 5. DEC-004 & DEC-006: Contradiction Detection & Neutral Explanations
#[test]
fn test_dec004_and_dec006_contradiction_detection() {
    let delhi = GeoPoint::new(28.6139, 77.2090).unwrap();
    let mumbai = GeoPoint::new(19.0760, 72.8777).unwrap();

    let events = vec![
        RawTimelineEvent {
            event_id: "ev-delhi".to_string(),
            source_evidence_id: "exhibit-1".to_string(),
            timestamp: "2026-11-18T10:00:00Z".to_string(),
            timestamp_precision: "EXACT_SECOND".to_string(),
            duration_seconds: None,
            uncertainty_window_start: None,
            uncertainty_window_end: None,
            coordinates: Some(delhi),
            raw_description: Some("ATM Withdrawal in Delhi".to_string()),
            entity_ids: vec!["suspect-1".to_string()],
            metadata: serde_json::json!({}),
            source_timezone: Some("UTC".to_string()),
            clock_offset_seconds: Some(0.0),
        },
        RawTimelineEvent {
            event_id: "ev-mumbai".to_string(),
            source_evidence_id: "exhibit-2".to_string(),
            timestamp: "2026-11-18T10:30:00Z".to_string(),
            timestamp_precision: "EXACT_SECOND".to_string(),
            duration_seconds: None,
            uncertainty_window_start: None,
            uncertainty_window_end: None,
            coordinates: Some(mumbai),
            raw_description: Some("Store Transaction in Mumbai".to_string()),
            entity_ids: vec!["suspect-1".to_string()],
            metadata: serde_json::json!({}),
            source_timezone: Some("UTC".to_string()),
            clock_offset_seconds: Some(0.0),
        },
    ];

    let timeline = reconstruct_timeline(&events, "UTC", 0.0, 900.0, None).unwrap();
    assert_eq!(timeline.entries.len(), 2);
    assert_eq!(timeline.strictly_ordered_count, 2);

    let spatial = reconstruct_spatial_trajectories(&timeline.entries, 180.0, None).unwrap();
    assert_eq!(spatial.trajectories.len(), 1);
    assert_eq!(spatial.trajectories[0].segments.len(), 1);

    let contradictions = detect_contradictions(&spatial, &timeline.entries, 180.0, None).unwrap();
    assert_eq!(contradictions.leads.len(), 1);

    let lead = &contradictions.leads[0];
    assert_eq!(
        lead.lead_type,
        ContradictionLeadType::SpatialTransitImpossible
    );
    assert_eq!(lead.entity_id, "suspect-1");
    assert_eq!(lead.event_a_id, "ev-delhi");
    assert_eq!(lead.event_b_id, "ev-mumbai");

    // Neutral explanation verification: no assertion of guilt, intent, or fraud
    assert!(lead.neutral_explanation.contains("Entity 'suspect-1'"));
    assert!(lead
        .neutral_explanation
        .contains("exceeds the configured investigative reference threshold"));
    assert!(!lead.neutral_explanation.to_lowercase().contains("guilty"));
    assert!(!lead.neutral_explanation.to_lowercase().contains("fraud"));
    assert!(!lead
        .neutral_explanation
        .to_lowercase()
        .contains("alibi fabrication"));

    // Limitations list present
    assert!(!lead.limitations.is_empty());
    assert!(lead
        .limitations
        .iter()
        .any(|l| l.contains("uncalibrated device clock offsets")));
}

// 6. DEC-005: Cryptographic Provenance and Source Immutability
#[test]
fn test_dec005_video_file_hashing_and_immutability() {
    let dir = tempdir().unwrap();
    let video_file = dir.path().join("cctv_sample.mp4");

    // Create synthetic video payload
    let synthetic_bytes =
        b"\x00\x00\x00\x20ftypmp42\x00\x00\x00\x00isommp42fakevideocontent12345678";
    std::fs::write(&video_file, synthetic_bytes).unwrap();

    let pre_hash = compute_file_sha256(&video_file).unwrap();
    assert_eq!(pre_hash.len(), 64);

    // Reading file does not mutate it
    let post_hash = compute_file_sha256(&video_file).unwrap();
    assert_eq!(pre_hash, post_hash);

    // Content verification
    let read_back = std::fs::read(&video_file).unwrap();
    assert_eq!(read_back, synthetic_bytes);
}

// 7. Timeline Concurrency & Overlap Marking
#[test]
fn test_timeline_concurrent_uncertain_marking() {
    let events = vec![
        RawTimelineEvent {
            event_id: "ev-overlap-1".to_string(),
            source_evidence_id: "ex-1".to_string(),
            timestamp: "2026-11-18T12:00:00Z".to_string(),
            timestamp_precision: "MINUTE".to_string(), // [12:00:00, 12:01:00)
            duration_seconds: None,
            uncertainty_window_start: None,
            uncertainty_window_end: None,
            coordinates: None,
            raw_description: None,
            entity_ids: vec!["E1".to_string()],
            metadata: serde_json::json!({}),
            source_timezone: None,
            clock_offset_seconds: None,
        },
        RawTimelineEvent {
            event_id: "ev-overlap-2".to_string(),
            source_evidence_id: "ex-2".to_string(),
            timestamp: "2026-11-18T12:00:30Z".to_string(),
            timestamp_precision: "EXACT_SECOND".to_string(), // [12:00:30, 12:00:31)
            duration_seconds: None,
            uncertainty_window_start: None,
            uncertainty_window_end: None,
            coordinates: None,
            raw_description: None,
            entity_ids: vec!["E1".to_string()],
            metadata: serde_json::json!({}),
            source_timezone: None,
            clock_offset_seconds: None,
        },
    ];

    let timeline = reconstruct_timeline(&events, "UTC", 0.0, 900.0, None).unwrap();
    assert_eq!(timeline.total_events, 2);
    assert_eq!(timeline.overlaps.len(), 1);
    assert_eq!(timeline.concurrent_uncertain_count, 2);
    assert_eq!(timeline.strictly_ordered_count, 0);

    for entry in &timeline.entries {
        assert_eq!(
            entry.ordering_status,
            TemporalOrderingStatus::ConcurrentUncertain
        );
        assert_eq!(entry.concurrent_event_ids.len(), 1);
    }
}

// 8. DEC-002 / FIND-2A-01: Half-Open Upper Boundary Rejection [start, end)
#[test]
fn test_dec002_half_open_boundary_acceptance_and_rejection() {
    let start_str = "2026-11-18T12:00:00Z";
    let end_str = "2026-11-18T13:00:00Z";

    // 1. Nominal equal to start: MUST be accepted [start, end)
    let res_start = compute_uncertainty_window(
        start_str,
        CanonicalTimePrecision::ExactSecond,
        "UTC",
        0.0,
        900.0,
        Some(start_str),
        Some(end_str),
    );
    assert!(
        res_start.is_ok(),
        "Nominal equal to start should be accepted"
    );

    // 2. Nominal strictly inside: MUST be accepted
    let inside_str = "2026-11-18T12:30:00Z";
    let res_inside = compute_uncertainty_window(
        inside_str,
        CanonicalTimePrecision::ExactSecond,
        "UTC",
        0.0,
        900.0,
        Some(start_str),
        Some(end_str),
    );
    assert!(
        res_inside.is_ok(),
        "Nominal inside interval should be accepted"
    );

    // 3. Nominal equal to end: MUST be REJECTED because end is exclusive in [start, end)
    let res_end = compute_uncertainty_window(
        end_str,
        CanonicalTimePrecision::ExactSecond,
        "UTC",
        0.0,
        900.0,
        Some(start_str),
        Some(end_str),
    );
    assert!(
        res_end.is_err(),
        "Nominal equal to end must be rejected under half-open semantics [start, end)"
    );
    let err_msg = res_end.unwrap_err().to_string();
    assert!(
        err_msg.contains("falls outside explicit uncertainty window"),
        "Unexpected error: {}",
        err_msg
    );
}

// 9. Extraction-to-Reconstruction Adapter & Cell Tower Centroid Resolution
#[test]
fn test_reconstruction_adapters_cell_tower_centroid_resolution() {
    use sutram_backend_rust::extraction::model::ExtractedEventCandidate;
    use sutram_backend_rust::reconstruction::adapters::{
        event_candidate_to_timeline_event_with_context, resolve_cell_tower_centroid,
    };

    // Test direct resolution
    let south_tower = resolve_cell_tower_centroid("TOWER-DELHI-SOUTH-04B").unwrap();
    assert!((south_tower.lat - 28.5244).abs() < 1e-4);
    assert!((south_tower.lon - 77.2066).abs() < 1e-4);

    let central_tower = resolve_cell_tower_centroid("TOWER-DELHI-CENTRAL-01A").unwrap();
    assert!((central_tower.lat - 28.6139).abs() < 1e-4);
    assert!((central_tower.lon - 77.2090).abs() < 1e-4);

    // Test candidate conversion
    let candidate = ExtractedEventCandidate {
        candidate_id: "evt-cdr-001".to_string(),
        source_evidence_id: "evi-cdr-001".to_string(),
        source_record_id: "rec-001".to_string(),
        event_type: "TELEPHONY_CALL".to_string(),
        raw_timestamp: "2026-11-18T10:00:00Z".to_string(),
        duration_seconds: Some(60.0),
        location_name: Some("TOWER-DELHI-SOUTH-04B".to_string()),
        involved_entity_raw_ids: vec!["+919811002233".to_string()],
        metadata: serde_json::json!({
            "cell_tower_id": "TOWER-DELHI-SOUTH-04B"
        }),
    };

    let raw_event = event_candidate_to_timeline_event_with_context(
        &candidate,
        Some("EXACT_SECOND"),
        Some("Asia/Kolkata"),
        Some(10.0),
    );

    assert_eq!(raw_event.event_id, "evt-cdr-001");
    assert_eq!(raw_event.source_timezone, Some("Asia/Kolkata".to_string()));
    assert_eq!(raw_event.clock_offset_seconds, Some(10.0));
    assert!(raw_event.coordinates.is_some());
    let coords = raw_event.coordinates.unwrap();
    assert!((coords.lat - 28.5244).abs() < 1e-4);
}

// 10. End-to-End Forensic Reconstruction Coordinator Service
#[test]
fn test_reconstruction_service_full_case() {
    use sutram_backend_rust::reconstruction::service::{
        ForensicReconstructionService, ReconstructionOptions,
    };

    let p1 = GeoPoint::new(28.5244, 77.2066).unwrap(); // Saket, South Delhi
    let p2 = GeoPoint::new(28.6139, 77.2090).unwrap(); // Connaught Place, Central Delhi (~10 km)

    let events = vec![
        RawTimelineEvent {
            event_id: "evt-001".to_string(),
            source_evidence_id: "evi-001".to_string(),
            timestamp: "2026-11-18T10:00:00Z".to_string(),
            timestamp_precision: "EXACT_SECOND".to_string(),
            duration_seconds: Some(60.0),
            uncertainty_window_start: None,
            uncertainty_window_end: None,
            coordinates: Some(p1),
            raw_description: Some("Saket Tower".to_string()),
            entity_ids: vec!["suspect-1".to_string()],
            metadata: serde_json::json!({}),
            source_timezone: Some("UTC".to_string()),
            clock_offset_seconds: Some(0.0),
        },
        RawTimelineEvent {
            event_id: "evt-002".to_string(),
            source_evidence_id: "evi-002".to_string(),
            timestamp: "2026-11-18T10:01:00Z".to_string(), // 1 minute later
            timestamp_precision: "EXACT_SECOND".to_string(),
            duration_seconds: Some(30.0),
            uncertainty_window_start: None,
            uncertainty_window_end: None,
            coordinates: Some(p2),
            raw_description: Some("Connaught Place".to_string()),
            entity_ids: vec!["suspect-1".to_string()],
            metadata: serde_json::json!({}),
            source_timezone: Some("UTC".to_string()),
            clock_offset_seconds: Some(0.0),
        },
    ];

    let options = ReconstructionOptions {
        default_timezone: "UTC".to_string(),
        default_clock_offset_seconds: 0.0,
        speed_threshold_kmh: 180.0,
        estimated_half_width_seconds: 900.0,
        case_id: Some("case-test-100".to_string()),
    };

    let service = ForensicReconstructionService::new();
    let res = service.reconstruct_case(&events, &options).unwrap();

    assert_eq!(res.timeline.total_events, 2);
    assert_eq!(res.spatial.trajectories.len(), 1);
    assert_eq!(res.spatial.trajectories[0].entity_id, "suspect-1");
    assert!(res.spatial.trajectories[0].total_distance_km > 9.0);

    // Speed > 180 km/h and activity duration infeasibility generate contradiction leads
    assert_eq!(res.contradictions.leads.len(), 2);
    assert!(res
        .contradictions
        .leads
        .iter()
        .any(|l| l.lead_type == ContradictionLeadType::SpatialTransitImpossible));
    assert!(res
        .contradictions
        .leads
        .iter()
        .any(|l| l.lead_type == ContradictionLeadType::TemporalActivityInfeasible));

    let lead = res
        .contradictions
        .leads
        .iter()
        .find(|l| l.lead_type == ContradictionLeadType::SpatialTransitImpossible)
        .unwrap();
    assert_eq!(
        lead.lead_type,
        ContradictionLeadType::SpatialTransitImpossible
    );
    assert_eq!(lead.entity_id, "suspect-1");
    assert!(lead
        .derived_inference
        .post_activity_min_required_speed_kmh
        .is_some());
    assert!(
        lead.derived_inference
            .post_activity_min_required_speed_kmh
            .unwrap()
            > 180.0
    );
    assert!(!lead.limitations.is_empty());
}

// 11. Axum REST Reconstruction API Endpoints
#[tokio::test]
async fn test_reconstruction_axum_endpoints() {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use sutram_backend_rust::{create_router, AppConfig};
    use tower::ServiceExt;

    let config = AppConfig::default();
    let app = create_router(config);

    let p1 = GeoPoint::new(28.5244, 77.2066).unwrap();
    let p2 = GeoPoint::new(28.6139, 77.2090).unwrap();

    let trigger_payload = serde_json::json!({
        "options": {
            "default_timezone": "UTC",
            "speed_threshold_kmh": 180.0
        },
        "raw_events": [
            {
                "event_id": "evt-001",
                "source_evidence_id": "evi-001",
                "timestamp": "2026-11-18T10:00:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "duration_seconds": 60.0,
                "coordinates": p1,
                "raw_description": "Saket",
                "entity_ids": ["suspect-1"],
                "metadata": {}
            },
            {
                "event_id": "evt-002",
                "source_evidence_id": "evi-002",
                "timestamp": "2026-11-18T10:01:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "duration_seconds": 30.0,
                "coordinates": p2,
                "raw_description": "Connaught Place",
                "entity_ids": ["suspect-1"],
                "metadata": {}
            }
        ]
    });

    let case_id = "00000000-0000-0000-0000-000000000026";

    // 1. Trigger reconstruction
    let req = Request::builder()
        .uri(format!("/api/v1/cases/{}/reconstruction", case_id))
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_vec(&trigger_payload).unwrap()))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body_bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let res: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(res["timeline"]["total_events"], 2);
    assert_eq!(res["contradictions"]["leads"].as_array().unwrap().len(), 2);

    // 2. Query timeline
    let req_tl = Request::builder()
        .uri(format!("/api/v1/cases/{}/timeline", case_id))
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let resp_tl = app.clone().oneshot(req_tl).await.unwrap();
    assert_eq!(resp_tl.status(), StatusCode::OK);

    // 3. Query spatial
    let req_sp = Request::builder()
        .uri(format!("/api/v1/cases/{}/spatial", case_id))
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let resp_sp = app.clone().oneshot(req_sp).await.unwrap();
    assert_eq!(resp_sp.status(), StatusCode::OK);

    // 4. Query contradictions
    let req_ct = Request::builder()
        .uri(format!("/api/v1/cases/{}/contradictions", case_id))
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let resp_ct = app.oneshot(req_ct).await.unwrap();
    assert_eq!(resp_ct.status(), StatusCode::OK);
}
