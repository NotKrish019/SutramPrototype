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
            assumptions: Vec::new(),
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
            assumptions: Vec::new(),
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
            assumptions: Vec::new(),
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
            assumptions: Vec::new(),
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
        timestamp_precision: Some("EXACT_SECOND".to_string()),
        description: Some("Telephony call via TOWER-DELHI-SOUTH-04B".to_string()),
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
            assumptions: Vec::new(),
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
            assumptions: Vec::new(),
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
    assert!(res["reconstruction_run_id"].is_string());
    assert!(res["run_id"].is_string());
    let run_id_str = res["reconstruction_run_id"].as_str().unwrap();
    assert_eq!(res["run_id"].as_str().unwrap(), run_id_str);

    // 1b. Query full reconstruction result
    let req_recon = Request::builder()
        .uri(format!("/api/v1/cases/{}/reconstruction", case_id))
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let resp_recon = app.clone().oneshot(req_recon).await.unwrap();
    assert_eq!(resp_recon.status(), StatusCode::OK);
    let recon_bytes = resp_recon.into_body().collect().await.unwrap().to_bytes();
    let recon_res: serde_json::Value = serde_json::from_slice(&recon_bytes).unwrap();
    assert_eq!(recon_res["reconstruction_run_id"], run_id_str);
    assert_eq!(recon_res["run_id"], run_id_str);

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
    let resp_ct = app.clone().oneshot(req_ct).await.unwrap();
    assert_eq!(resp_ct.status(), StatusCode::OK);

    // 5. Query case forensic report
    let req_rep = Request::builder()
        .uri(format!("/api/v1/cases/{}/report", case_id))
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let resp_rep = app.oneshot(req_rep).await.unwrap();
    assert_eq!(resp_rep.status(), StatusCode::OK);

    let rep_bytes = resp_rep.into_body().collect().await.unwrap().to_bytes();
    let rep: serde_json::Value = serde_json::from_slice(&rep_bytes).unwrap();
    assert_eq!(rep["case_id"], case_id.to_string());
    assert_eq!(rep["judicial_export_status"], "SHIP_EXPORT_BLOCKED");
    assert!(rep["markdown_dossier"]
        .as_str()
        .unwrap()
        .contains("SHIP_EXPORT_BLOCKED"));
}

#[test]
fn test_unsupported_precision_or_invalid_timezone_excludes_single_event_without_failing_case() {
    let valid_event = RawTimelineEvent {
        event_id: "evt-valid-001".to_string(),
        source_evidence_id: "evi-001".to_string(),
        timestamp: "2026-11-18T10:00:00Z".to_string(),
        timestamp_precision: "EXACT_SECOND".to_string(),
        duration_seconds: None,
        uncertainty_window_start: None,
        uncertainty_window_end: None,
        coordinates: None,
        raw_description: Some("Valid event".to_string()),
        entity_ids: vec!["entity-1".to_string()],
        metadata: serde_json::json!({}),
        source_timezone: Some("UTC".to_string()),
        clock_offset_seconds: Some(0.0),
        assumptions: Vec::new(),
    };

    let bad_precision_event = RawTimelineEvent {
        event_id: "evt-bad-precision".to_string(),
        source_evidence_id: "evi-001".to_string(),
        timestamp: "2026-11-18T10:05:00Z".to_string(),
        timestamp_precision: "INVALID_UNKNOWN_PRECISION".to_string(),
        duration_seconds: None,
        uncertainty_window_start: None,
        uncertainty_window_end: None,
        coordinates: None,
        raw_description: Some("Bad precision event".to_string()),
        entity_ids: vec!["entity-1".to_string()],
        metadata: serde_json::json!({}),
        source_timezone: Some("UTC".to_string()),
        clock_offset_seconds: Some(0.0),
        assumptions: Vec::new(),
    };

    let bad_timezone_event = RawTimelineEvent {
        event_id: "evt-bad-tz".to_string(),
        source_evidence_id: "evi-001".to_string(),
        timestamp: "2026-11-18T10:10:00".to_string(),
        timestamp_precision: "EXACT_SECOND".to_string(),
        duration_seconds: None,
        uncertainty_window_start: None,
        uncertainty_window_end: None,
        coordinates: None,
        raw_description: Some("Bad timezone event".to_string()),
        entity_ids: vec!["entity-1".to_string()],
        metadata: serde_json::json!({}),
        source_timezone: Some("Mars/Olympus_Mons".to_string()),
        clock_offset_seconds: Some(0.0),
        assumptions: Vec::new(),
    };

    let events = vec![valid_event, bad_precision_event, bad_timezone_event];
    let result = reconstruct_timeline(&events, "UTC", 0.0, 30.0, Some("case-001".to_string()))
        .expect("Timeline reconstruction should not fail the whole case");

    // The single valid event is reconstructed cleanly
    assert_eq!(result.total_events, 1);
    assert_eq!(result.entries[0].event_id, "evt-valid-001");

    // The 2 invalid events are recorded in excluded_events
    assert_eq!(result.excluded_events.len(), 2);
    assert_eq!(result.excluded_events[0].event_id, "evt-bad-precision");
    assert!(result.excluded_events[0]
        .reason
        .contains("Unsupported timestamp precision"));
    assert_eq!(result.excluded_events[1].event_id, "evt-bad-tz");
    assert!(result.excluded_events[1]
        .reason
        .contains("Invalid timestamp or timezone"));
}

#[test]
fn test_contradiction_leads_byte_identical_across_repeated_runs() {
    let t1 = RawTimelineEvent {
        event_id: "ev-1".to_string(),
        source_evidence_id: "evi-1".to_string(),
        timestamp: "2026-11-18T12:00:00Z".to_string(),
        timestamp_precision: "EXACT_SECOND".to_string(),
        duration_seconds: None,
        uncertainty_window_start: None,
        uncertainty_window_end: None,
        coordinates: GeoPoint::new(28.5244, 77.2066).ok(),
        raw_description: None,
        entity_ids: vec!["suspect-1".to_string()],
        metadata: serde_json::json!({}),
        source_timezone: None,
        clock_offset_seconds: None,
        assumptions: Vec::new(),
    };

    let t2 = RawTimelineEvent {
        event_id: "ev-2".to_string(),
        source_evidence_id: "evi-1".to_string(),
        timestamp: "2026-11-18T12:01:00Z".to_string(),
        timestamp_precision: "EXACT_SECOND".to_string(),
        duration_seconds: None,
        uncertainty_window_start: None,
        uncertainty_window_end: None,
        coordinates: GeoPoint::new(19.0760, 72.8777).ok(),
        raw_description: None,
        entity_ids: vec!["suspect-1".to_string()],
        metadata: serde_json::json!({}),
        source_timezone: None,
        clock_offset_seconds: None,
        assumptions: Vec::new(),
    };

    let events = vec![t1, t2];
    let service =
        sutram_backend_rust::reconstruction::service::ForensicReconstructionService::new();
    let opts = sutram_backend_rust::reconstruction::service::ReconstructionOptions::default();

    let run_1 = service.reconstruct_case(&events, &opts).unwrap();
    let bytes_1 = serde_json::to_vec(&run_1).unwrap();

    // Sleep briefly to confirm wall-clock time does not alter serialized output
    std::thread::sleep(std::time::Duration::from_millis(50));

    let run_2 = service.reconstruct_case(&events, &opts).unwrap();
    let bytes_2 = serde_json::to_vec(&run_2).unwrap();

    assert_eq!(
        bytes_1, bytes_2,
        "Repeated runs must produce byte-identical serialized output"
    );
    assert_eq!(
        run_1.contradictions.leads[0].lead_id,
        run_2.contradictions.leads[0].lead_id
    );
    assert_eq!(
        run_1.contradictions.leads[0].generated_at,
        run_2.contradictions.leads[0].generated_at
    );
}

#[test]
fn test_video_frame_consumer_creates_valid_candidates() {
    let frames = vec![
        sutram_backend_rust::reconstruction::ExtractedFrameArtifact {
            frame_index: 1,
            pts_seconds: 0.0,
            sha256_hash: "abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890"
                .to_string(),
            relative_path: "derived/frames/frame_0001.jpg".to_string(),
            estimated_utc: Some(
                DateTime::parse_from_rfc3339("2026-11-18T12:00:00Z")
                    .unwrap()
                    .with_timezone(&Utc),
            ),
        },
        sutram_backend_rust::reconstruction::ExtractedFrameArtifact {
            frame_index: 2,
            pts_seconds: 5.5,
            sha256_hash: "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"
                .to_string(),
            relative_path: "derived/frames/frame_0002.jpg".to_string(),
            estimated_utc: None,
        },
    ];

    let candidates = sutram_backend_rust::extraction::consume_video_frames(
        "ev-cctv-001",
        &frames,
        Some("2026-11-18T12:00:00Z"),
    );

    assert_eq!(candidates.len(), 2);
    assert_eq!(candidates[0].candidate_id, "evt-frame-ev-cctv-001-1");
    assert_eq!(
        candidates[0].timestamp_precision,
        Some("EXACT_SECOND".to_string())
    );
    assert!(candidates[0]
        .description
        .as_ref()
        .unwrap()
        .contains("Video keyframe 1"));
    assert_eq!(candidates[0].raw_timestamp, "2026-11-18T12:00:00+00:00");

    assert_eq!(candidates[1].candidate_id, "evt-frame-ev-cctv-001-2");
    assert_eq!(
        candidates[1].timestamp_precision,
        Some("EXACT_SECOND".to_string())
    );
    assert!(candidates[1]
        .description
        .as_ref()
        .unwrap()
        .contains("Video keyframe 2"));
    assert_eq!(candidates[1].raw_timestamp, "2026-11-18T12:00:05.500+00:00");
}

// 13. Phase 1 Hardening: Strict UUID Validation on Case Endpoints
#[tokio::test]
async fn test_phase1_case_uuid_strict_validation() {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use sutram_backend_rust::{create_router, AppConfig};
    use tower::ServiceExt;

    let config = AppConfig::default();
    let app = create_router(config);

    // Invalid non-UUID case identifier must be strictly rejected with 400 Bad Request
    let invalid_case_id = "arbitrary-non-uuid-case-123";
    let req = Request::builder()
        .uri(format!("/api/v1/cases/{}/reconstruction", invalid_case_id))
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"options":{}}"#))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    let req_get = Request::builder()
        .uri(format!("/api/v1/cases/{}/reconstruction", invalid_case_id))
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let resp_get = app.clone().oneshot(req_get).await.unwrap();
    assert_eq!(resp_get.status(), StatusCode::BAD_REQUEST);

    let req_report = Request::builder()
        .uri(format!("/api/v1/cases/{}/report", invalid_case_id))
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let resp_report = app.oneshot(req_report).await.unwrap();
    assert_eq!(resp_report.status(), StatusCode::BAD_REQUEST);
}

// 14. Phase 1 Hardening: Cell Tower Centroid Coordinates Labeled as Inferred
#[test]
fn test_phase1_cell_tower_centroid_labeled_as_inferred_assumption() {
    use sutram_backend_rust::extraction::model::ExtractedEventCandidate;
    use sutram_backend_rust::reconstruction::adapters::event_candidate_to_timeline_event_with_context;
    use sutram_backend_rust::reconstruction::model::AssumptionKind;

    let candidate = ExtractedEventCandidate {
        candidate_id: "evt-tower-001".to_string(),
        source_evidence_id: "evi-001".to_string(),
        source_record_id: "rec-001".to_string(),
        event_type: "TELEPHONY_CALL".to_string(),
        raw_timestamp: "2026-11-18T10:00:00Z".to_string(),
        timestamp_precision: Some("EXACT_SECOND".to_string()),
        description: Some("Call via tower centroid".to_string()),
        duration_seconds: Some(60.0),
        location_name: Some("Saket Tower".to_string()),
        involved_entity_raw_ids: vec!["+919811002233".to_string()],
        metadata: serde_json::json!({
            "cell_tower_id": "TOWER-DELHI-SOUTH-04B",
        }),
    };

    let event = event_candidate_to_timeline_event_with_context(
        &candidate,
        Some("EXACT_SECOND"),
        Some("UTC"),
        Some(0.0),
    );

    // Coordinates are looked up from centroid
    assert!(event.coordinates.is_some());
    let assumption = event
        .assumptions
        .iter()
        .find(|a| a.kind == AssumptionKind::CoordinatesInferredFromTowerId);
    assert!(
        assumption.is_some(),
        "Inferred cell tower centroid must carry CoordinatesInferredFromTowerId assumption"
    );
    assert!(assumption
        .unwrap()
        .basis
        .contains("not an observed position"));
}

// 15. Phase 1 Hardening: UTC-Stored Events Have No Double Clock Offset
#[test]
fn test_phase1_utc_stored_event_no_double_clock_offset() {
    let t_nominal = "2026-11-18T12:00:00Z";
    // When an event is retrieved from DB, its nominal timestamp is already in UTC.
    // Specifying clock_offset_seconds: 0.0 prevents shifting it again.
    let (nom, start, end) = compute_uncertainty_window(
        t_nominal,
        CanonicalTimePrecision::ExactSecond,
        "UTC",
        0.0,
        DEFAULT_ESTIMATED_HALF_WIDTH_SECONDS,
        None,
        None,
    )
    .unwrap();

    assert_eq!(nom.to_rfc3339(), "2026-11-18T12:00:00+00:00");
    assert_eq!(start.to_rfc3339(), "2026-11-18T12:00:00+00:00");
    assert_eq!(end.to_rfc3339(), "2026-11-18T12:00:01+00:00");
}

// 16. Phase 3: Stateless What-If Simulation - Immutability & Baseline Protection
#[tokio::test]
async fn test_phase3_stateless_what_if_does_not_mutate_baseline_or_fs() {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use sutram_backend_rust::{
        create_router,
        reconstruction::{
            scenario::{
                ScenarioAssumptions, ScenarioSimulationRequest, ScenarioSimulationResponse,
            },
            FullCaseReconstructionResult, GeoPoint,
        },
        AppConfig,
    };
    use tempfile::tempdir;
    use tower::ServiceExt;

    let dir = tempdir().unwrap();
    let evidence_dir = dir.path().join("evidence");
    let derived_dir = dir.path().join("derived");
    std::fs::create_dir_all(&evidence_dir).unwrap();
    std::fs::create_dir_all(&derived_dir).unwrap();

    let config = AppConfig {
        evidence_storage_path: evidence_dir,
        derived_storage_path: derived_dir.clone(),
        ..Default::default()
    };
    let app = create_router(config);

    let case_id = "11111111-1111-1111-1111-111111111111";

    // 1. Establish baseline reconstruction with two events
    let p1 = GeoPoint::new(28.5244, 77.2066).unwrap();
    let p2 = GeoPoint::new(28.6139, 77.2090).unwrap();

    let baseline_payload = serde_json::json!({
        "options": {
            "default_timezone": "UTC",
            "speed_threshold_kmh": 180.0
        },
        "raw_events": [
            {
                "event_id": "evt-base-01",
                "source_evidence_id": "evi-01",
                "timestamp": "2026-11-18T10:00:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "duration_seconds": 60.0,
                "coordinates": p1,
                "raw_description": "Location 1",
                "entity_ids": ["suspect-1"],
                "metadata": {}
            },
            {
                "event_id": "evt-base-02",
                "source_evidence_id": "evi-02",
                "timestamp": "2026-11-18T10:01:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "duration_seconds": 30.0,
                "coordinates": p2,
                "raw_description": "Location 2",
                "entity_ids": ["suspect-1"],
                "metadata": {}
            }
        ]
    });

    let req_base = Request::builder()
        .uri(format!("/api/v1/cases/{}/reconstruction", case_id))
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_vec(&baseline_payload).unwrap()))
        .unwrap();

    let resp_base = app.clone().oneshot(req_base).await.unwrap();
    assert_eq!(resp_base.status(), StatusCode::OK);
    let base_bytes = resp_base.into_body().collect().await.unwrap().to_bytes();
    let base_result: FullCaseReconstructionResult = serde_json::from_slice(&base_bytes).unwrap();
    let baseline_run_id = base_result.reconstruction_run_id.clone().unwrap();

    // Verify baseline saved to disk
    let latest_path = derived_dir
        .join(case_id)
        .join("reconstruction")
        .join("latest.json");
    assert!(latest_path.exists());
    let initial_latest_bytes = std::fs::read(&latest_path).unwrap();

    // 2. Trigger what-if simulation with alternative assumptions
    let sim_payload = ScenarioSimulationRequest {
        scenario_name: Some("Test Immutability Hypothesis".to_string()),
        assumptions: ScenarioAssumptions {
            speed_threshold_kmh: Some(50.0),
            default_clock_offset_seconds: Some(120.0),
            ..Default::default()
        },
        raw_events: None,
    };

    let req_sim = Request::builder()
        .uri(format!("/api/v1/cases/{}/reconstruction/simulate", case_id))
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_vec(&sim_payload).unwrap()))
        .unwrap();

    let resp_sim = app.clone().oneshot(req_sim).await.unwrap();
    assert_eq!(resp_sim.status(), StatusCode::OK);
    let sim_bytes = resp_sim.into_body().collect().await.unwrap().to_bytes();
    let sim_response: ScenarioSimulationResponse = serde_json::from_slice(&sim_bytes).unwrap();

    // Verify response metadata
    assert!(sim_response.is_hypothetical);
    assert_eq!(sim_response.baseline_run_id, baseline_run_id);
    assert!(sim_response.scenario_run_id.starts_with("sim-"));
    assert_ne!(sim_response.scenario_run_id, baseline_run_id);
    assert_eq!(
        sim_response
            .simulated_reconstruction
            .export_safeguard
            .status,
        sutram_backend_rust::reconstruction::ExportSafeguardStatus::Blocked
    );

    // 3. CRUCIAL CHECK: Verify latest.json on disk was NOT modified
    let after_latest_bytes = std::fs::read(&latest_path).unwrap();
    assert_eq!(
        initial_latest_bytes, after_latest_bytes,
        "Filesystem latest.json must remain completely identical and unmutated by scenario simulation"
    );

    // 4. Verify GET /reconstruction still returns the baseline
    let req_get = Request::builder()
        .uri(format!("/api/v1/cases/{}/reconstruction", case_id))
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let resp_get = app.clone().oneshot(req_get).await.unwrap();
    assert_eq!(resp_get.status(), StatusCode::OK);
    let get_bytes = resp_get.into_body().collect().await.unwrap().to_bytes();
    let get_result: FullCaseReconstructionResult = serde_json::from_slice(&get_bytes).unwrap();
    assert_eq!(get_result.reconstruction_run_id, Some(baseline_run_id));
}

// 17. Phase 3: What-If Disappearing Contradiction Under Exhibit Exclusion or Relaxed Bounds
#[tokio::test]
async fn test_phase3_what_if_disappearing_contradiction() {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use sutram_backend_rust::{
        create_router,
        reconstruction::{
            scenario::{
                ScenarioAssumptions, ScenarioSimulationRequest, ScenarioSimulationResponse,
            },
            FullCaseReconstructionResult, GeoPoint,
        },
        AppConfig,
    };
    use tower::ServiceExt;

    let app = create_router(AppConfig::default());
    let case_id = "22222222-2222-2222-2222-222222222222";

    // Delhi and Mumbai ~1150 km apart in 10 minutes -> Impossible transit (>6000 km/h)
    let p_delhi = GeoPoint::new(28.6139, 77.2090).unwrap();
    let p_mumbai = GeoPoint::new(19.0760, 72.8777).unwrap();

    let baseline_payload = serde_json::json!({
        "options": {
            "default_timezone": "UTC",
            "speed_threshold_kmh": 180.0
        },
        "raw_events": [
            {
                "event_id": "evt-delhi",
                "source_evidence_id": "evi-delhi",
                "timestamp": "2026-11-18T10:00:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "duration_seconds": 60.0,
                "coordinates": p_delhi,
                "raw_description": "Delhi Meeting",
                "entity_ids": ["suspect-1"],
                "metadata": {}
            },
            {
                "event_id": "evt-mumbai",
                "source_evidence_id": "evi-mumbai",
                "timestamp": "2026-11-18T10:10:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "duration_seconds": 60.0,
                "coordinates": p_mumbai,
                "raw_description": "Mumbai Sight",
                "entity_ids": ["suspect-1"],
                "metadata": {}
            }
        ]
    });

    let req_base = Request::builder()
        .uri(format!("/api/v1/cases/{}/reconstruction", case_id))
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_vec(&baseline_payload).unwrap()))
        .unwrap();

    let resp_base = app.clone().oneshot(req_base).await.unwrap();
    assert_eq!(resp_base.status(), StatusCode::OK);
    let base_bytes = resp_base.into_body().collect().await.unwrap().to_bytes();
    let base_result: FullCaseReconstructionResult = serde_json::from_slice(&base_bytes).unwrap();
    assert_eq!(base_result.contradictions.leads.len(), 2);

    // Scenario A: Exclude the questioned Mumbai exhibit
    let sim_payload = ScenarioSimulationRequest {
        scenario_name: Some("Hypothesis: Disputed Mumbai Exhibit Excluded".to_string()),
        assumptions: ScenarioAssumptions {
            excluded_evidence_ids: vec!["evi-mumbai".to_string()],
            ..Default::default()
        },
        raw_events: None,
    };

    let req_sim = Request::builder()
        .uri(format!("/api/v1/cases/{}/reconstruction/simulate", case_id))
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_vec(&sim_payload).unwrap()))
        .unwrap();

    let resp_sim = app.clone().oneshot(req_sim).await.unwrap();
    assert_eq!(resp_sim.status(), StatusCode::OK);
    let sim_bytes = resp_sim.into_body().collect().await.unwrap().to_bytes();
    let sim_response: ScenarioSimulationResponse = serde_json::from_slice(&sim_bytes).unwrap();

    // Both contradiction leads must have disappeared
    assert_eq!(sim_response.comparison.findings_disappearing.len(), 2);
    assert!(sim_response
        .comparison
        .findings_disappearing
        .iter()
        .any(|f| f.finding_type == "SPATIAL_TRANSIT_IMPOSSIBLE"));
    assert!(sim_response
        .comparison
        .findings_disappearing
        .iter()
        .any(|f| f.finding_type == "TEMPORAL_ACTIVITY_INFEASIBLE"));
    assert!(sim_response.comparison.findings_newly_appearing.is_empty());
    assert!(sim_response.comparison.findings_unchanged.is_empty());
    assert!(sim_response
        .comparison
        .explanations
        .iter()
        .any(|e| e.contains("resolved/disappeared") && e.contains("excluded")));
}

// 18. Phase 3: What-If Newly Appearing Contradiction Under Stricter Speed Threshold
#[tokio::test]
async fn test_phase3_what_if_newly_appearing_contradiction() {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use sutram_backend_rust::{
        create_router,
        reconstruction::{
            scenario::{
                ScenarioAssumptions, ScenarioSimulationRequest, ScenarioSimulationResponse,
            },
            FullCaseReconstructionResult, GeoPoint,
        },
        AppConfig,
    };
    use tower::ServiceExt;

    let app = create_router(AppConfig::default());
    let case_id = "33333333-3333-3333-3333-333333333333";

    // 5 km apart in 5 minutes = 60 km/h (feasible under default 180 km/h threshold)
    let p1 = GeoPoint::new(28.5244, 77.2066).unwrap();
    let p2 = GeoPoint::new(28.5600, 77.2300).unwrap();

    let baseline_payload = serde_json::json!({
        "options": {
            "default_timezone": "UTC",
            "speed_threshold_kmh": 180.0
        },
        "raw_events": [
            {
                "event_id": "evt-sub-01",
                "source_evidence_id": "evi-01",
                "timestamp": "2026-11-18T10:00:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "duration_seconds": 0.0,
                "coordinates": p1,
                "raw_description": "Loc 1",
                "entity_ids": ["suspect-1"],
                "metadata": {}
            },
            {
                "event_id": "evt-sub-02",
                "source_evidence_id": "evi-02",
                "timestamp": "2026-11-18T10:05:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "duration_seconds": 0.0,
                "coordinates": p2,
                "raw_description": "Loc 2",
                "entity_ids": ["suspect-1"],
                "metadata": {}
            }
        ]
    });

    let req_base = Request::builder()
        .uri(format!("/api/v1/cases/{}/reconstruction", case_id))
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_vec(&baseline_payload).unwrap()))
        .unwrap();

    let resp_base = app.clone().oneshot(req_base).await.unwrap();
    assert_eq!(resp_base.status(), StatusCode::OK);
    let base_bytes = resp_base.into_body().collect().await.unwrap().to_bytes();
    let base_result: FullCaseReconstructionResult = serde_json::from_slice(&base_bytes).unwrap();
    assert_eq!(base_result.contradictions.leads.len(), 0);

    // Scenario: Test pedestrian assumption (speed threshold = 10 km/h)
    let sim_payload = ScenarioSimulationRequest {
        scenario_name: Some("Hypothesis: Pedestrian Movement Only (10 km/h threshold)".to_string()),
        assumptions: ScenarioAssumptions {
            speed_threshold_kmh: Some(10.0),
            ..Default::default()
        },
        raw_events: None,
    };

    let req_sim = Request::builder()
        .uri(format!("/api/v1/cases/{}/reconstruction/simulate", case_id))
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_vec(&sim_payload).unwrap()))
        .unwrap();

    let resp_sim = app.clone().oneshot(req_sim).await.unwrap();
    assert_eq!(resp_sim.status(), StatusCode::OK);
    let sim_bytes = resp_sim.into_body().collect().await.unwrap().to_bytes();
    let sim_response: ScenarioSimulationResponse = serde_json::from_slice(&sim_bytes).unwrap();

    assert_eq!(sim_response.comparison.findings_newly_appearing.len(), 1);
    assert_eq!(
        sim_response.comparison.findings_newly_appearing[0].finding_type,
        "SPATIAL_TRANSIT_IMPOSSIBLE"
    );
    assert!(sim_response.comparison.findings_disappearing.is_empty());
}

// 19. Phase 3: What-If Finding Robustness & Metric Shift Under Clock Drift
#[tokio::test]
async fn test_phase3_what_if_robustness_changed() {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use std::collections::HashMap;
    use sutram_backend_rust::{
        create_router,
        reconstruction::{
            scenario::{
                ScenarioAssumptions, ScenarioSimulationRequest, ScenarioSimulationResponse,
            },
            FullCaseReconstructionResult, GeoPoint,
        },
        AppConfig,
    };
    use tower::ServiceExt;

    let app = create_router(AppConfig::default());
    let case_id = "44444444-4444-4444-4444-444444444444";

    // 20 km apart in 2 minutes = 600 km/h (>180 km/h baseline)
    let p1 = GeoPoint::new(28.5244, 77.2066).unwrap();
    let p2 = GeoPoint::new(28.7000, 77.2500).unwrap();

    let baseline_payload = serde_json::json!({
        "options": {
            "default_timezone": "UTC",
            "speed_threshold_kmh": 180.0
        },
        "raw_events": [
            {
                "event_id": "evt-rob-01",
                "source_evidence_id": "evi-01",
                "timestamp": "2026-11-18T10:00:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "duration_seconds": 0.0,
                "coordinates": p1,
                "raw_description": "Start Point",
                "entity_ids": ["suspect-1"],
                "metadata": {}
            },
            {
                "event_id": "evt-rob-02",
                "source_evidence_id": "evi-02",
                "timestamp": "2026-11-18T10:02:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "duration_seconds": 0.0,
                "coordinates": p2,
                "raw_description": "End Point",
                "entity_ids": ["suspect-1"],
                "metadata": {}
            }
        ]
    });

    let req_base = Request::builder()
        .uri(format!("/api/v1/cases/{}/reconstruction", case_id))
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_vec(&baseline_payload).unwrap()))
        .unwrap();

    let resp_base = app.clone().oneshot(req_base).await.unwrap();
    assert_eq!(resp_base.status(), StatusCode::OK);
    let base_bytes = resp_base.into_body().collect().await.unwrap().to_bytes();
    let base_result: FullCaseReconstructionResult = serde_json::from_slice(&base_bytes).unwrap();
    assert_eq!(base_result.contradictions.leads.len(), 1);

    // Scenario: Apply +60s clock drift to exhibit 2 -> elapsed time becomes 3 minutes -> speed drops to ~400 km/h (still > 180 km/h)
    let mut exhibit_offsets = HashMap::new();
    exhibit_offsets.insert("evi-02".to_string(), 60.0);

    let sim_payload = ScenarioSimulationRequest {
        scenario_name: Some("Hypothesis: Clock Drift Shift".to_string()),
        assumptions: ScenarioAssumptions {
            exhibit_clock_offsets: exhibit_offsets,
            ..Default::default()
        },
        raw_events: None,
    };

    let req_sim = Request::builder()
        .uri(format!("/api/v1/cases/{}/what-if", case_id)) // using what-if alias
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_vec(&sim_payload).unwrap()))
        .unwrap();

    let resp_sim = app.clone().oneshot(req_sim).await.unwrap();
    assert_eq!(resp_sim.status(), StatusCode::OK);
    let sim_bytes = resp_sim.into_body().collect().await.unwrap().to_bytes();
    let sim_response: ScenarioSimulationResponse = serde_json::from_slice(&sim_bytes).unwrap();

    assert_eq!(sim_response.comparison.findings_robustness_changed.len(), 1);
    let delta = &sim_response.comparison.findings_robustness_changed[0];
    assert_eq!(delta.metric_name, "min_required_speed_kmh");
    assert!(delta.baseline_value > delta.scenario_value);
    assert!(delta.explanation.contains("Minimum required transit speed"));
}

// 20. Phase 3: Rejection of Invalid Scenario Assumptions
#[tokio::test]
async fn test_phase3_what_if_invalid_assumptions_rejected() {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use std::collections::HashMap;
    use sutram_backend_rust::{
        create_router,
        reconstruction::scenario::{
            ExplicitWindowOverride, ScenarioAssumptions, ScenarioSimulationRequest,
        },
        AppConfig,
    };
    use tower::ServiceExt;

    let app = create_router(AppConfig::default());
    let case_id = "55555555-5555-5555-5555-555555555555";

    // Test A: Negative speed threshold
    let invalid_speed = ScenarioSimulationRequest {
        scenario_name: None,
        assumptions: ScenarioAssumptions {
            speed_threshold_kmh: Some(-15.0),
            ..Default::default()
        },
        raw_events: None,
    };
    let req_a = Request::builder()
        .uri(format!("/api/v1/cases/{}/reconstruction/simulate", case_id))
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_vec(&invalid_speed).unwrap()))
        .unwrap();
    let resp_a = app.clone().oneshot(req_a).await.unwrap();
    assert_eq!(resp_a.status(), StatusCode::BAD_REQUEST);

    // Test B: Invalid IANA timezone
    let invalid_tz = ScenarioSimulationRequest {
        scenario_name: None,
        assumptions: ScenarioAssumptions {
            default_timezone: Some("NonExistent/Mars_Base".to_string()),
            ..Default::default()
        },
        raw_events: None,
    };
    let req_b = Request::builder()
        .uri(format!("/api/v1/cases/{}/reconstruction/simulate", case_id))
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_vec(&invalid_tz).unwrap()))
        .unwrap();
    let resp_b = app.clone().oneshot(req_b).await.unwrap();
    assert_eq!(resp_b.status(), StatusCode::BAD_REQUEST);

    // Test C: Inverted uncertainty bounds (start >= end)
    let mut overrides = HashMap::new();
    overrides.insert(
        "evt-01".to_string(),
        ExplicitWindowOverride {
            start_utc: "2026-11-18T12:00:00Z".to_string(),
            end_utc: "2026-11-18T10:00:00Z".to_string(), // inverted!
        },
    );
    let invalid_win = ScenarioSimulationRequest {
        scenario_name: None,
        assumptions: ScenarioAssumptions {
            event_uncertainty_overrides: overrides,
            ..Default::default()
        },
        raw_events: None,
    };
    let req_c = Request::builder()
        .uri(format!("/api/v1/cases/{}/reconstruction/simulate", case_id))
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_vec(&invalid_win).unwrap()))
        .unwrap();
    let resp_c = app.clone().oneshot(req_c).await.unwrap();
    assert_eq!(resp_c.status(), StatusCode::BAD_REQUEST);
}

// 21. Phase 4: Full Finding-to-Source Trace with Multiple Evidence Items
#[tokio::test]
async fn test_phase4_full_finding_to_source_trace_with_multiple_evidence_items() {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use sutram_backend_rust::{
        create_router,
        reconstruction::{
            traceability::{
                CaseProvenanceReport, EpistemicClassification, EvidenceIntegrityStatus,
                FindingProvenanceTrace,
            },
            FullCaseReconstructionResult, GeoPoint,
        },
        AppConfig,
    };
    use tempfile::tempdir;
    use tower::ServiceExt;

    let dir = tempdir().unwrap();
    let evidence_dir = dir.path().join("evidence");
    let derived_dir = dir.path().join("derived");
    std::fs::create_dir_all(&evidence_dir).unwrap();
    std::fs::create_dir_all(&derived_dir).unwrap();

    let config = AppConfig {
        evidence_storage_path: evidence_dir.clone(),
        derived_storage_path: derived_dir.clone(),
        ..Default::default()
    };
    let app = create_router(config);

    let case_id = "66666666-6666-6666-6666-666666666666";

    // 1. Create two real evidence files on disk with valid manifests and active run pointers
    let cctv_dir = evidence_dir.join(case_id).join("evi-cctv");
    std::fs::create_dir_all(&cctv_dir).unwrap();
    let cctv_bytes = b"synthetic cctv binary video content 12345678";
    let cctv_file = cctv_dir.join("evi-cctv.raw");
    std::fs::write(&cctv_file, cctv_bytes).unwrap();
    let cctv_sha = sutram_backend_rust::reconstruction::compute_file_sha256(&cctv_file).unwrap();

    let cctv_manifest = serde_json::json!({
        "receipt": {
            "evidence_id": "evi-cctv",
            "case_id": case_id,
            "original_filename": "saket_junction_cctv.mp4",
            "source_type": "VIDEO_CCTV",
            "mime_type": "video/mp4",
            "file_size_bytes": cctv_bytes.len(),
            "sha256_hash": cctv_sha,
            "storage_uri": format!("file://{}", cctv_file.display()),
            "status": "EXTRACTED",
            "metadata": {},
            "created_at": "2026-11-18T09:00:00Z"
        },
        "relative_raw_path": format!("{}/evi-cctv/evi-cctv.raw", case_id),
        "stored_at": "2026-11-18T09:00:00Z"
    });
    std::fs::write(
        cctv_dir.join("metadata.json"),
        serde_json::to_vec_pretty(&cctv_manifest).unwrap(),
    )
    .unwrap();
    std::fs::write(
        cctv_dir.join("manifest.json"),
        serde_json::to_vec_pretty(&cctv_manifest).unwrap(),
    )
    .unwrap();
    std::fs::write(
        cctv_dir.join("evi-cctv.manifest.json"),
        serde_json::to_vec_pretty(&cctv_manifest).unwrap(),
    )
    .unwrap();

    let cdr_dir = evidence_dir.join(case_id).join("evi-cdr");
    std::fs::create_dir_all(&cdr_dir).unwrap();
    let cdr_bytes = b"caller,receiver,timestamp,duration,tower\n+919811002233,+919811004455,2026-11-18 10:05:00,60,MUMBAI-SOUTH-01\n";
    let cdr_file = cdr_dir.join("evi-cdr.raw");
    std::fs::write(&cdr_file, cdr_bytes).unwrap();
    let cdr_sha = sutram_backend_rust::reconstruction::compute_file_sha256(&cdr_file).unwrap();

    let cdr_manifest = serde_json::json!({
        "receipt": {
            "evidence_id": "evi-cdr",
            "case_id": case_id,
            "original_filename": "carrier_cdr_airtel.csv",
            "source_type": "TELEPHONY_CDR",
            "mime_type": "text/csv",
            "file_size_bytes": cdr_bytes.len(),
            "sha256_hash": cdr_sha,
            "storage_uri": format!("file://{}", cdr_file.display()),
            "status": "EXTRACTED",
            "metadata": {},
            "created_at": "2026-11-18T09:05:00Z"
        },
        "relative_raw_path": format!("{}/evi-cdr/evi-cdr.raw", case_id),
        "stored_at": "2026-11-18T09:05:00Z"
    });
    std::fs::write(
        cdr_dir.join("metadata.json"),
        serde_json::to_vec_pretty(&cdr_manifest).unwrap(),
    )
    .unwrap();
    std::fs::write(
        cdr_dir.join("manifest.json"),
        serde_json::to_vec_pretty(&cdr_manifest).unwrap(),
    )
    .unwrap();
    std::fs::write(
        cdr_dir.join("evi-cdr.manifest.json"),
        serde_json::to_vec_pretty(&cdr_manifest).unwrap(),
    )
    .unwrap();

    // 2. Trigger baseline reconstruction with events referencing both exhibits
    let p_delhi = GeoPoint::new(28.5244, 77.2066).unwrap();
    let p_mumbai = GeoPoint::new(19.0760, 72.8777).unwrap();

    let reconstruction_payload = serde_json::json!({
        "options": {
            "default_timezone": "UTC",
            "speed_threshold_kmh": 180.0
        },
        "raw_events": [
            {
                "event_id": "evt-cctv-01",
                "source_evidence_id": "evi-cctv",
                "timestamp": "2026-11-18T10:00:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "duration_seconds": 0.0,
                "coordinates": p_delhi,
                "raw_description": "CCTV Saket Junction",
                "entity_ids": ["suspect-1"],
                "metadata": {
                    "source_record_id": "frame_0042",
                    "extraction_run_id": "ext-cctv-run-001"
                }
            },
            {
                "event_id": "evt-cdr-01",
                "source_evidence_id": "evi-cdr",
                "timestamp": "2026-11-18T10:05:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "duration_seconds": 0.0,
                "coordinates": p_mumbai,
                "raw_description": "CDR Mumbai Tower",
                "entity_ids": ["suspect-1"],
                "metadata": {
                    "source_record_id": "cdr_row_12",
                    "extraction_run_id": "ext-cdr-run-002"
                }
            }
        ]
    });

    let req_recon = Request::builder()
        .uri(format!("/api/v1/cases/{}/reconstruction", case_id))
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::to_vec(&reconstruction_payload).unwrap(),
        ))
        .unwrap();

    let resp_recon = app.clone().oneshot(req_recon).await.unwrap();
    assert_eq!(resp_recon.status(), StatusCode::OK);
    let recon_bytes = resp_recon.into_body().collect().await.unwrap().to_bytes();
    let recon_res: FullCaseReconstructionResult = serde_json::from_slice(&recon_bytes).unwrap();
    assert_eq!(recon_res.contradictions.leads.len(), 1);
    let lead_id = &recon_res.contradictions.leads[0].lead_id;

    // 3. Query finding-specific provenance trace via API
    let req_prov = Request::builder()
        .uri(format!(
            "/api/v1/cases/{}/reconstruction/findings/{}/provenance",
            case_id, lead_id
        ))
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let resp_prov = app.clone().oneshot(req_prov).await.unwrap();
    assert_eq!(resp_prov.status(), StatusCode::OK);
    let prov_bytes = resp_prov.into_body().collect().await.unwrap().to_bytes();
    let trace: FindingProvenanceTrace = serde_json::from_slice(&prov_bytes).unwrap();

    // Verify all 11 required traceability points
    assert_eq!(&trace.finding_id, lead_id);
    assert_eq!(trace.finding_type, "CONTRADICTION_LEAD");
    assert_eq!(
        Some(trace.reconstruction_run_id.clone()),
        recon_res.reconstruction_run_id
    );
    assert_eq!(
        trace.epistemic_classification,
        EpistemicClassification::Inferred
    );
    assert!(!trace.is_hypothetical);
    assert!(trace.missing_provenance_fields.is_empty());

    // Multiple evidence items supporting one finding
    assert_eq!(trace.source_anchors.len(), 2);

    let anchor_cctv = &trace.source_anchors[0];
    assert_eq!(anchor_cctv.event_id, "evt-cctv-01");
    assert_eq!(anchor_cctv.evidence_id, "evi-cctv");
    assert_eq!(
        anchor_cctv.original_filename,
        Some("saket_junction_cctv.mp4".to_string())
    );
    assert_eq!(anchor_cctv.evidence_sha256, Some(cctv_sha));
    assert_eq!(
        anchor_cctv.integrity_status,
        EvidenceIntegrityStatus::VerifiedMatch
    );
    assert_eq!(
        anchor_cctv.extraction_run_id,
        Some("ext-cctv-run-001".to_string())
    );
    assert_eq!(anchor_cctv.source_record_id, Some("frame_0042".to_string()));
    assert_eq!(
        anchor_cctv.original_source_timestamp,
        "2026-11-18T10:00:00Z"
    );
    assert_eq!(
        anchor_cctv.normalized_timestamp_utc,
        Some("2026-11-18T10:00:00+00:00".to_string())
    );
    assert!(anchor_cctv.uncertainty_window.is_some());
    assert!(anchor_cctv.spatial_interpretation.is_some());

    let anchor_cdr = &trace.source_anchors[1];
    assert_eq!(anchor_cdr.event_id, "evt-cdr-01");
    assert_eq!(anchor_cdr.evidence_id, "evi-cdr");
    assert_eq!(
        anchor_cdr.original_filename,
        Some("carrier_cdr_airtel.csv".to_string())
    );
    assert_eq!(anchor_cdr.evidence_sha256, Some(cdr_sha));
    assert_eq!(
        anchor_cdr.integrity_status,
        EvidenceIntegrityStatus::VerifiedMatch
    );
    assert_eq!(
        anchor_cdr.extraction_run_id,
        Some("ext-cdr-run-002".to_string())
    );
    assert_eq!(anchor_cdr.source_record_id, Some("cdr_row_12".to_string()));
    assert_eq!(anchor_cdr.original_source_timestamp, "2026-11-18T10:05:00Z");

    // 4. Query full case provenance report
    let req_report = Request::builder()
        .uri(format!(
            "/api/v1/cases/{}/reconstruction/provenance",
            case_id
        ))
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let resp_report = app.clone().oneshot(req_report).await.unwrap();
    assert_eq!(resp_report.status(), StatusCode::OK);
    let report_bytes = resp_report.into_body().collect().await.unwrap().to_bytes();
    let report: CaseProvenanceReport = serde_json::from_slice(&report_bytes).unwrap();
    assert_eq!(report.total_findings_count, 1);
    assert_eq!(report.intact_provenance_count, 1);
    assert_eq!(report.flagged_provenance_count, 0);
}

// 22. Phase 4: Missing Source Record and Missing Extraction Run Are Explicitly Flagged
#[tokio::test]
async fn test_phase4_missing_source_record_and_extraction_run() {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use sutram_backend_rust::{
        create_router,
        reconstruction::{
            traceability::FindingProvenanceTrace, FullCaseReconstructionResult, GeoPoint,
        },
        AppConfig,
    };
    use tower::ServiceExt;

    let app = create_router(AppConfig::default());
    let case_id = "77777777-7777-7777-7777-777777777777";

    let p1 = GeoPoint::new(28.5244, 77.2066).unwrap();
    let p2 = GeoPoint::new(19.0760, 72.8777).unwrap();

    // Event A has NO source_record_id and NO extraction_run_id in metadata
    let payload = serde_json::json!({
        "options": {
            "default_timezone": "UTC",
            "speed_threshold_kmh": 180.0
        },
        "raw_events": [
            {
                "event_id": "evt-bare-01",
                "source_evidence_id": "evi-unlinked",
                "timestamp": "2026-11-18T10:00:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "duration_seconds": 0.0,
                "coordinates": p1,
                "raw_description": "Bare Event 1",
                "entity_ids": ["suspect-1"],
                "metadata": {} // empty: no source_record_id, no extraction_run_id
            },
            {
                "event_id": "evt-bare-02",
                "source_evidence_id": "evi-unlinked",
                "timestamp": "2026-11-18T10:05:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "duration_seconds": 0.0,
                "coordinates": p2,
                "raw_description": "Bare Event 2",
                "entity_ids": ["suspect-1"],
                "metadata": {}
            }
        ]
    });

    let req_recon = Request::builder()
        .uri(format!("/api/v1/cases/{}/reconstruction", case_id))
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_vec(&payload).unwrap()))
        .unwrap();

    let resp_recon = app.clone().oneshot(req_recon).await.unwrap();
    assert_eq!(resp_recon.status(), StatusCode::OK);
    let recon_bytes = resp_recon.into_body().collect().await.unwrap().to_bytes();
    let recon_res: FullCaseReconstructionResult = serde_json::from_slice(&recon_bytes).unwrap();
    let lead_id = &recon_res.contradictions.leads[0].lead_id;

    let req_prov = Request::builder()
        .uri(format!(
            "/api/v1/cases/{}/findings/{}/provenance",
            case_id, lead_id
        ))
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let resp_prov = app.clone().oneshot(req_prov).await.unwrap();
    assert_eq!(resp_prov.status(), StatusCode::OK);
    let prov_bytes = resp_prov.into_body().collect().await.unwrap().to_bytes();
    let trace: FindingProvenanceTrace = serde_json::from_slice(&prov_bytes).unwrap();

    let anchor = &trace.source_anchors[0];
    assert!(
        anchor.source_record_id.is_none(),
        "Missing source record must NOT be invented"
    );
    assert!(
        anchor.extraction_run_id.is_none(),
        "Missing extraction run must NOT be invented"
    );

    // Missing fields explicitly flagged
    assert!(trace
        .missing_provenance_fields
        .iter()
        .any(|f| f.contains("source_record_id")));
    assert!(trace
        .missing_provenance_fields
        .iter()
        .any(|f| f.contains("extraction_run_id")));
}

// 23. Phase 4: Unknown Evidence Exhibit Association Flagged
#[tokio::test]
async fn test_phase4_unknown_evidence_association() {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use sutram_backend_rust::{
        create_router,
        reconstruction::{
            traceability::{EvidenceIntegrityStatus, FindingProvenanceTrace},
            FullCaseReconstructionResult, GeoPoint,
        },
        AppConfig,
    };
    use tower::ServiceExt;

    let app = create_router(AppConfig::default());
    let case_id = "88888888-8888-8888-8888-888888888888";

    let p1 = GeoPoint::new(28.5244, 77.2066).unwrap();
    let p2 = GeoPoint::new(19.0760, 72.8777).unwrap();

    // Events reference an unknown, non-existent evidence exhibit ID
    let payload = serde_json::json!({
        "options": {
            "default_timezone": "UTC",
            "speed_threshold_kmh": 180.0
        },
        "raw_events": [
            {
                "event_id": "evt-ghost-01",
                "source_evidence_id": "evi-ghost-9999",
                "timestamp": "2026-11-18T10:00:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "duration_seconds": 0.0,
                "coordinates": p1,
                "raw_description": "Ghost Event 1",
                "entity_ids": ["suspect-1"],
                "metadata": {}
            },
            {
                "event_id": "evt-ghost-02",
                "source_evidence_id": "evi-ghost-9999",
                "timestamp": "2026-11-18T10:05:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "duration_seconds": 0.0,
                "coordinates": p2,
                "raw_description": "Ghost Event 2",
                "entity_ids": ["suspect-1"],
                "metadata": {}
            }
        ]
    });

    let req_recon = Request::builder()
        .uri(format!("/api/v1/cases/{}/reconstruction", case_id))
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_vec(&payload).unwrap()))
        .unwrap();

    let resp_recon = app.clone().oneshot(req_recon).await.unwrap();
    assert_eq!(resp_recon.status(), StatusCode::OK);
    let recon_bytes = resp_recon.into_body().collect().await.unwrap().to_bytes();
    let recon_res: FullCaseReconstructionResult = serde_json::from_slice(&recon_bytes).unwrap();
    let lead_id = &recon_res.contradictions.leads[0].lead_id;

    let req_prov = Request::builder()
        .uri(format!(
            "/api/v1/cases/{}/findings/{}/provenance",
            case_id, lead_id
        ))
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let resp_prov = app.clone().oneshot(req_prov).await.unwrap();
    assert_eq!(resp_prov.status(), StatusCode::OK);
    let prov_bytes = resp_prov.into_body().collect().await.unwrap().to_bytes();
    let trace: FindingProvenanceTrace = serde_json::from_slice(&prov_bytes).unwrap();

    assert_eq!(
        trace.source_anchors[0].integrity_status,
        EvidenceIntegrityStatus::UnknownEvidence
    );
    assert!(trace
        .missing_provenance_fields
        .iter()
        .any(|f| f.contains("unknown_evidence")));
}

// 24. Phase 4: Tampered Evidence File Triggers Real-Time Hash Mismatch
#[tokio::test]
async fn test_phase4_tampered_evidence_file_hash_mismatch() {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use sutram_backend_rust::{
        create_router,
        reconstruction::{
            traceability::{EvidenceIntegrityStatus, FindingProvenanceTrace},
            FullCaseReconstructionResult, GeoPoint,
        },
        AppConfig,
    };
    use tempfile::tempdir;
    use tower::ServiceExt;

    let dir = tempdir().unwrap();
    let evidence_dir = dir.path().join("evidence");
    let derived_dir = dir.path().join("derived");
    std::fs::create_dir_all(&evidence_dir).unwrap();
    std::fs::create_dir_all(&derived_dir).unwrap();

    let config = AppConfig {
        evidence_storage_path: evidence_dir.clone(),
        derived_storage_path: derived_dir.clone(),
        ..Default::default()
    };
    let app = create_router(config);

    let case_id = "99999999-9999-9999-9999-999999999999";

    // 1. Create original evidence file
    let ev_dir = evidence_dir.join(case_id).join("evi-tamper");
    std::fs::create_dir_all(&ev_dir).unwrap();
    let raw_file = ev_dir.join("evi-tamper.raw");
    let original_bytes = b"legitimate digital evidence content 001";
    std::fs::write(&raw_file, original_bytes).unwrap();
    let original_sha = sutram_backend_rust::reconstruction::compute_file_sha256(&raw_file).unwrap();

    let manifest = serde_json::json!({
        "receipt": {
            "evidence_id": "evi-tamper",
            "case_id": case_id,
            "original_filename": "tamper_test.bin",
            "source_type": "GENERIC_BINARY",
            "mime_type": "application/octet-stream",
            "file_size_bytes": original_bytes.len(),
            "sha256_hash": original_sha,
            "storage_uri": format!("file://{}", raw_file.display()),
            "status": "EXTRACTED",
            "metadata": {},
            "created_at": "2026-11-18T10:00:00Z"
        },
        "relative_raw_path": format!("{}/evi-tamper/evi-tamper.raw", case_id),
        "stored_at": "2026-11-18T10:00:00Z"
    });
    std::fs::write(
        ev_dir.join("metadata.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    std::fs::write(
        ev_dir.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    std::fs::write(
        ev_dir.join("evi-tamper.manifest.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();

    // 2. Trigger reconstruction with this evidence
    let p1 = GeoPoint::new(28.5244, 77.2066).unwrap();
    let p2 = GeoPoint::new(19.0760, 72.8777).unwrap();

    let payload = serde_json::json!({
        "options": {
            "default_timezone": "UTC",
            "speed_threshold_kmh": 180.0
        },
        "raw_events": [
            {
                "event_id": "evt-t1",
                "source_evidence_id": "evi-tamper",
                "timestamp": "2026-11-18T10:00:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "duration_seconds": 0.0,
                "coordinates": p1,
                "raw_description": "Loc 1",
                "entity_ids": ["suspect-1"],
                "metadata": { "source_record_id": "rec-1" }
            },
            {
                "event_id": "evt-t2",
                "source_evidence_id": "evi-tamper",
                "timestamp": "2026-11-18T10:05:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "duration_seconds": 0.0,
                "coordinates": p2,
                "raw_description": "Loc 2",
                "entity_ids": ["suspect-1"],
                "metadata": { "source_record_id": "rec-2" }
            }
        ]
    });

    let req_recon = Request::builder()
        .uri(format!("/api/v1/cases/{}/reconstruction", case_id))
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_vec(&payload).unwrap()))
        .unwrap();

    let resp_recon = app.clone().oneshot(req_recon).await.unwrap();
    assert_eq!(resp_recon.status(), StatusCode::OK);
    let recon_bytes = resp_recon.into_body().collect().await.unwrap().to_bytes();
    let recon_res: FullCaseReconstructionResult = serde_json::from_slice(&recon_bytes).unwrap();
    let lead_id = &recon_res.contradictions.leads[0].lead_id;

    // 3. TAMPER WITH THE FILE ON DISK
    std::fs::write(&raw_file, b"MALICIOUSLY ALTERED BYTES 9999").unwrap();

    // 4. Query provenance trace -> must detect hash mismatch
    let req_prov = Request::builder()
        .uri(format!(
            "/api/v1/cases/{}/findings/{}/provenance",
            case_id, lead_id
        ))
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let resp_prov = app.clone().oneshot(req_prov).await.unwrap();
    assert_eq!(resp_prov.status(), StatusCode::OK);
    let prov_bytes = resp_prov.into_body().collect().await.unwrap().to_bytes();
    let trace: FindingProvenanceTrace = serde_json::from_slice(&prov_bytes).unwrap();

    assert_eq!(
        trace.source_anchors[0].integrity_status,
        EvidenceIntegrityStatus::HashMismatch
    );
    assert!(trace
        .missing_provenance_fields
        .iter()
        .any(|f| f.contains("sha256_hash_mismatch")));
}

// 25. Phase 4: Original Source Timestamp Is Preserved and Never Overwritten by Normalized Value
#[tokio::test]
async fn test_phase4_original_source_timestamp_preservation() {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use sutram_backend_rust::{
        create_router,
        reconstruction::{
            traceability::FindingProvenanceTrace, FullCaseReconstructionResult, GeoPoint,
        },
        AppConfig,
    };
    use tower::ServiceExt;

    let app = create_router(AppConfig::default());
    let case_id = "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa";

    let p1 = GeoPoint::new(28.5244, 77.2066).unwrap();
    let p2 = GeoPoint::new(19.0760, 72.8777).unwrap();

    // Event carries naive timestamp + positive clock drift offset (+45.0s)
    let payload = serde_json::json!({
        "options": {
            "default_timezone": "UTC",
            "speed_threshold_kmh": 180.0
        },
        "raw_events": [
            {
                "event_id": "evt-ts-01",
                "source_evidence_id": "evi-ts",
                "timestamp": "2026-11-18 14:30:00", // original raw string format
                "timestamp_precision": "EXACT_SECOND",
                "duration_seconds": 0.0,
                "coordinates": p1,
                "raw_description": "Loc 1",
                "entity_ids": ["suspect-1"],
                "clock_offset_seconds": 45.0,
                "metadata": { "source_record_id": "rec-1" }
            },
            {
                "event_id": "evt-ts-02",
                "source_evidence_id": "evi-ts",
                "timestamp": "2026-11-18 14:35:00",
                "timestamp_precision": "EXACT_SECOND",
                "duration_seconds": 0.0,
                "coordinates": p2,
                "raw_description": "Loc 2",
                "entity_ids": ["suspect-1"],
                "clock_offset_seconds": 45.0,
                "metadata": { "source_record_id": "rec-2" }
            }
        ]
    });

    let req_recon = Request::builder()
        .uri(format!("/api/v1/cases/{}/reconstruction", case_id))
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_vec(&payload).unwrap()))
        .unwrap();

    let resp_recon = app.clone().oneshot(req_recon).await.unwrap();
    assert_eq!(resp_recon.status(), StatusCode::OK);
    let recon_bytes = resp_recon.into_body().collect().await.unwrap().to_bytes();
    let recon_res: FullCaseReconstructionResult = serde_json::from_slice(&recon_bytes).unwrap();
    let lead_id = &recon_res.contradictions.leads[0].lead_id;

    let req_prov = Request::builder()
        .uri(format!(
            "/api/v1/cases/{}/findings/{}/provenance",
            case_id, lead_id
        ))
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let resp_prov = app.clone().oneshot(req_prov).await.unwrap();
    assert_eq!(resp_prov.status(), StatusCode::OK);
    let prov_bytes = resp_prov.into_body().collect().await.unwrap().to_bytes();
    let trace: FindingProvenanceTrace = serde_json::from_slice(&prov_bytes).unwrap();

    let anchor = &trace.source_anchors[0];
    // Original source timestamp MUST match raw input exactly
    assert_eq!(anchor.original_source_timestamp, "2026-11-18 14:30:00");
    // Normalized UTC timestamp has offset applied (+45.0s -> 14:30:45Z)
    assert_eq!(
        anchor.normalized_timestamp_utc,
        Some("2026-11-18T14:30:45+00:00".to_string())
    );
    assert_ne!(
        &anchor.original_source_timestamp,
        anchor.normalized_timestamp_utc.as_ref().unwrap()
    );
}

// 26. Phase 4: Derived versus Observed and Simulated Epistemic Distinction
#[tokio::test]
async fn test_phase4_epistemic_distinction_observed_inferred_simulated() {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use sutram_backend_rust::{
        create_router,
        extraction::model::ExtractedEventCandidate,
        reconstruction::{
            adapters::event_candidate_to_timeline_event_with_context,
            scenario::{
                ScenarioAssumptions, ScenarioSimulationRequest, ScenarioSimulationResponse,
            },
            traceability::{
                resolve_finding_provenance, EpistemicClassification, FindingProvenanceTrace,
            },
            FullCaseReconstructionResult,
        },
        AppConfig,
    };
    use tower::ServiceExt;

    let app = create_router(AppConfig::default());
    let case_id = "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb";

    // Candidate 1: Inferred tower centroid
    let cand_tower = ExtractedEventCandidate {
        candidate_id: "evt-tower-01".to_string(),
        source_evidence_id: "evi-01".to_string(),
        source_record_id: "rec-1".to_string(),
        event_type: "TELEPHONY_CALL".to_string(),
        raw_timestamp: "2026-11-18T10:00:00Z".to_string(),
        timestamp_precision: Some("EXACT_SECOND".to_string()),
        description: Some("Tower Call".to_string()),
        duration_seconds: None,
        location_name: Some("Saket Tower".to_string()),
        involved_entity_raw_ids: vec!["suspect-1".to_string()],
        metadata: serde_json::json!({
            "cell_tower_id": "TOWER-DELHI-SOUTH-04B",
        }),
    };
    let event_tower = event_candidate_to_timeline_event_with_context(
        &cand_tower,
        Some("EXACT_SECOND"),
        Some("UTC"),
        Some(0.0),
    );

    // Candidate 2: Directly observed GPS
    let cand_gps = ExtractedEventCandidate {
        candidate_id: "evt-gps-01".to_string(),
        source_evidence_id: "evi-02".to_string(),
        source_record_id: "rec-2".to_string(),
        event_type: "GPS_FIX".to_string(),
        raw_timestamp: "2026-11-18T10:05:00Z".to_string(),
        timestamp_precision: Some("EXACT_SECOND".to_string()),
        description: Some("Observed GPS Fix".to_string()),
        duration_seconds: None,
        location_name: None,
        involved_entity_raw_ids: vec!["suspect-1".to_string()],
        metadata: serde_json::json!({
            "coordinates": { "lat": 19.0760, "lon": 72.8777 }
        }),
    };
    let event_gps = event_candidate_to_timeline_event_with_context(
        &cand_gps,
        Some("EXACT_SECOND"),
        Some("UTC"),
        Some(0.0),
    );

    let payload = serde_json::json!({
        "options": {
            "default_timezone": "UTC",
            "speed_threshold_kmh": 180.0
        },
        "raw_events": [event_tower, event_gps]
    });

    let req_recon = Request::builder()
        .uri(format!("/api/v1/cases/{}/reconstruction", case_id))
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_vec(&payload).unwrap()))
        .unwrap();

    let resp_recon = app.clone().oneshot(req_recon).await.unwrap();
    assert_eq!(resp_recon.status(), StatusCode::OK);
    let recon_bytes = resp_recon.into_body().collect().await.unwrap().to_bytes();
    let recon_res: FullCaseReconstructionResult = serde_json::from_slice(&recon_bytes).unwrap();
    let lead_id = &recon_res.contradictions.leads[0].lead_id;

    // Check baseline finding provenance
    let req_prov = Request::builder()
        .uri(format!(
            "/api/v1/cases/{}/findings/{}/provenance",
            case_id, lead_id
        ))
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let resp_prov = app.clone().oneshot(req_prov).await.unwrap();
    assert_eq!(resp_prov.status(), StatusCode::OK);
    let prov_bytes = resp_prov.into_body().collect().await.unwrap().to_bytes();
    let trace: FindingProvenanceTrace = serde_json::from_slice(&prov_bytes).unwrap();

    // Baseline contradiction finding is Inferred
    assert_eq!(
        trace.epistemic_classification,
        EpistemicClassification::Inferred
    );
    assert!(!trace.is_hypothetical);

    // Tower anchor has inferred location, GPS anchor has observed location
    assert!(
        !trace.source_anchors[0]
            .spatial_interpretation
            .as_ref()
            .unwrap()
            .is_observed_gps
    );
    assert_eq!(
        trace.source_anchors[0].epistemic_level,
        EpistemicClassification::Inferred
    );

    assert!(
        trace.source_anchors[1]
            .spatial_interpretation
            .as_ref()
            .unwrap()
            .is_observed_gps
    );
    assert_eq!(
        trace.source_anchors[1].epistemic_level,
        EpistemicClassification::Extracted
    );

    // Now test What-If simulated finding
    let sim_req = ScenarioSimulationRequest {
        scenario_name: Some("What-If Test".to_string()),
        assumptions: ScenarioAssumptions {
            speed_threshold_kmh: Some(50.0),
            ..Default::default()
        },
        raw_events: None,
    };

    let req_sim = Request::builder()
        .uri(format!("/api/v1/cases/{}/what-if", case_id))
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_vec(&sim_req).unwrap()))
        .unwrap();

    let resp_sim = app.clone().oneshot(req_sim).await.unwrap();
    assert_eq!(resp_sim.status(), StatusCode::OK);
    let sim_bytes = resp_sim.into_body().collect().await.unwrap().to_bytes();
    let sim_res: ScenarioSimulationResponse = serde_json::from_slice(&sim_bytes).unwrap();

    let storage =
        sutram_backend_rust::ingestion::StorageManager::new(std::path::PathBuf::from("/tmp"));
    let sim_trace = resolve_finding_provenance(
        case_id,
        lead_id,
        &sim_res.simulated_reconstruction,
        &storage,
        None,
    )
    .await
    .unwrap();

    // Simulated finding must be strictly classified as Simulated
    assert_eq!(
        sim_trace.epistemic_classification,
        EpistemicClassification::Simulated
    );
    assert!(sim_trace.is_hypothetical);
}

// =====================================================================================
// PHASE 5: EVIDENCE-GAP INTELLIGENCE TESTS
// =====================================================================================

// 27. Phase 5: Gap Category - Evidence Excluded
#[tokio::test]
async fn test_phase5_gap_evidence_excluded() {
    use sutram_backend_rust::{
        ingestion::{EvidenceManifest, EvidenceUploadReceipt, IngestionStatus},
        reconstruction::{analyze_evidence_gaps, EvidenceGapType, ReconstructionOptions},
    };

    let manifest_corrupted = EvidenceManifest {
        receipt: EvidenceUploadReceipt {
            evidence_id: "evi-bad-01".to_string(),
            case_id: "c1".to_string(),
            original_filename: "corrupted_drive.img".to_string(),
            source_type: "DISK_IMAGE".to_string(),
            mime_type: "application/octet-stream".to_string(),
            file_size_bytes: 1024,
            sha256_hash: "abcd".to_string(),
            storage_uri: "file:///tmp/corrupted_drive.img".to_string(),
            status: IngestionStatus::Corrupted,
            metadata: serde_json::json!({}),
            created_at: "2026-11-18T10:00:00Z".to_string(),
        },
        relative_raw_path: "c1/evi-bad-01/evi-bad-01.raw".to_string(),
        stored_at: "2026-11-18T10:00:00Z".to_string(),
    };

    let options = ReconstructionOptions::default();
    let service =
        sutram_backend_rust::reconstruction::service::ForensicReconstructionService::new();
    let recon = service.reconstruct_case(&[], &options).unwrap();

    let report = analyze_evidence_gaps("c1", &recon, &[manifest_corrupted], None);

    let gap = report
        .gaps
        .iter()
        .find(|g| g.gap_type == EvidenceGapType::EvidenceExcluded)
        .expect("Must identify EvidenceExcluded gap");

    assert_eq!(gap.subject_type, "EVIDENCE");
    assert_eq!(gap.subject_id, "evi-bad-01");
    assert_eq!(gap.rule_id, "GAP-RULE-001-EVIDENCE-EXCLUDED");
    assert!(gap.is_confirmed_missing);
    assert!(gap.explanation.contains("corrupted_drive.img"));
    assert!(gap
        .impact_on_reconstruction
        .contains("absent from the timeline"));
    assert!(gap
        .suggested_corroborating_evidence
        .contains("custody checksum"));
}

// 28. Phase 5: Gap Category - Evidence Without Accepted Events
#[tokio::test]
async fn test_phase5_gap_evidence_without_events() {
    use sutram_backend_rust::{
        ingestion::{EvidenceManifest, EvidenceUploadReceipt, IngestionStatus},
        reconstruction::{analyze_evidence_gaps, EvidenceGapType, ReconstructionOptions},
    };

    let manifest_orphan = EvidenceManifest {
        receipt: EvidenceUploadReceipt {
            evidence_id: "evi-orphan-01".to_string(),
            case_id: "c2".to_string(),
            original_filename: "unparsed_notes.pdf".to_string(),
            source_type: "DOCUMENT_PDF".to_string(),
            mime_type: "application/pdf".to_string(),
            file_size_bytes: 2048,
            sha256_hash: "12345678".to_string(),
            storage_uri: "file:///tmp/unparsed_notes.pdf".to_string(),
            status: IngestionStatus::Ingested,
            metadata: serde_json::json!({}),
            created_at: "2026-11-18T10:00:00Z".to_string(),
        },
        relative_raw_path: "c2/evi-orphan-01/evi-orphan-01.raw".to_string(),
        stored_at: "2026-11-18T10:00:00Z".to_string(),
    };

    let options = ReconstructionOptions::default();
    let service =
        sutram_backend_rust::reconstruction::service::ForensicReconstructionService::new();
    let recon = service.reconstruct_case(&[], &options).unwrap();

    let report = analyze_evidence_gaps("c2", &recon, &[manifest_orphan], None);

    let gap = report
        .gaps
        .iter()
        .find(|g| g.gap_type == EvidenceGapType::EvidenceWithoutEvents)
        .expect("Must identify EvidenceWithoutEvents gap");

    assert_eq!(gap.subject_id, "evi-orphan-01");
    assert!(!gap.is_confirmed_missing); // Exists in storage, but unextracted
    assert!(gap.explanation.contains("zero accepted timeline events"));
    assert!(gap
        .suggested_corroborating_evidence
        .contains("extraction pipeline"));
}

// 29. Phase 5: Gap Category - Candidate Excluded
#[tokio::test]
async fn test_phase5_gap_candidate_excluded() {
    use sutram_backend_rust::reconstruction::{
        analyze_evidence_gaps, EvidenceGapType, RawTimelineEvent, ReconstructionOptions,
    };

    let options = ReconstructionOptions::default();
    let service =
        sutram_backend_rust::reconstruction::service::ForensicReconstructionService::new();

    // Event with malformed timestamp
    let bad_event = RawTimelineEvent {
        event_id: "evt-malformed-01".to_string(),
        source_evidence_id: "evi-01".to_string(),
        timestamp: "NOT_A_VALID_TIMESTAMP".to_string(),
        timestamp_precision: "EXACT_SECOND".to_string(),
        duration_seconds: None,
        uncertainty_window_start: None,
        uncertainty_window_end: None,
        coordinates: None,
        raw_description: None,
        entity_ids: vec!["suspect-1".to_string()],
        metadata: serde_json::json!({}),
        source_timezone: None,
        clock_offset_seconds: None,
        assumptions: vec![],
    };

    let recon = service.reconstruct_case(&[bad_event], &options).unwrap();
    assert_eq!(recon.timeline.excluded_events.len(), 1);

    let report = analyze_evidence_gaps("c3", &recon, &[], None);

    let gap = report
        .gaps
        .iter()
        .find(|g| g.gap_type == EvidenceGapType::CandidateExcluded)
        .expect("Must identify CandidateExcluded gap");

    assert_eq!(gap.subject_type, "EVENT_CANDIDATE");
    assert_eq!(gap.subject_id, "evt-malformed-01");
    assert!(gap.is_confirmed_missing);
    assert!(gap
        .impact_on_reconstruction
        .contains("cannot be positioned"));
}

// 30. Phase 5: Gap Category - Event Without Coordinates
#[tokio::test]
async fn test_phase5_gap_event_without_coordinates() {
    use sutram_backend_rust::reconstruction::{
        analyze_evidence_gaps, EvidenceGapType, RawTimelineEvent, ReconstructionOptions,
    };

    let options = ReconstructionOptions::default();
    let service =
        sutram_backend_rust::reconstruction::service::ForensicReconstructionService::new();

    // Valid timestamp, but coordinates are None
    let unlocated_event = RawTimelineEvent {
        event_id: "evt-nocoords-01".to_string(),
        source_evidence_id: "evi-call-01".to_string(),
        timestamp: "2026-11-18T10:00:00Z".to_string(),
        timestamp_precision: "EXACT_SECOND".to_string(),
        duration_seconds: None,
        uncertainty_window_start: None,
        uncertainty_window_end: None,
        coordinates: None, // No coords
        raw_description: Some("Incoming voice call".to_string()),
        entity_ids: vec!["suspect-1".to_string()],
        metadata: serde_json::json!({
            "source_record_id": "row_1",
            "extraction_run_id": "ext_1"
        }),
        source_timezone: Some("UTC".to_string()),
        clock_offset_seconds: Some(0.0),
        assumptions: vec![],
    };

    let recon = service
        .reconstruct_case(&[unlocated_event], &options)
        .unwrap();
    let report = analyze_evidence_gaps("c4", &recon, &[], None);

    let gap = report
        .gaps
        .iter()
        .find(|g| g.gap_type == EvidenceGapType::EventWithoutCoordinates)
        .expect("Must identify EventWithoutCoordinates gap");

    assert_eq!(gap.subject_id, "evt-nocoords-01");
    assert!(gap.is_confirmed_missing);
    assert!(gap
        .impact_on_reconstruction
        .contains("cannot be placed on spatial maps"));
    assert!(gap
        .suggested_corroborating_evidence
        .contains("Incoming voice call"));
}

// 31. Phase 5: Gap Category - Unresolved Entity Reference
#[tokio::test]
async fn test_phase5_gap_unresolved_entity_reference() {
    use sutram_backend_rust::reconstruction::{
        analyze_evidence_gaps, EvidenceGapType, GeoPoint, RawTimelineEvent, ReconstructionOptions,
    };

    let options = ReconstructionOptions::default();
    let service =
        sutram_backend_rust::reconstruction::service::ForensicReconstructionService::new();

    let p = GeoPoint::new(28.5244, 77.2066).unwrap();
    let anon_event = RawTimelineEvent {
        event_id: "evt-anon-01".to_string(),
        source_evidence_id: "evi-cctv-01".to_string(),
        timestamp: "2026-11-18T10:00:00Z".to_string(),
        timestamp_precision: "EXACT_SECOND".to_string(),
        duration_seconds: None,
        uncertainty_window_start: None,
        uncertainty_window_end: None,
        coordinates: Some(p),
        raw_description: Some("Unidentified person in hoodie".to_string()),
        entity_ids: vec!["UNKNOWN".to_string()], // Unknown entity!
        metadata: serde_json::json!({
            "source_record_id": "frame_100",
            "extraction_run_id": "ext_1"
        }),
        source_timezone: Some("UTC".to_string()),
        clock_offset_seconds: Some(0.0),
        assumptions: vec![],
    };

    let recon = service.reconstruct_case(&[anon_event], &options).unwrap();
    let report = analyze_evidence_gaps("c5", &recon, &[], None);

    let gap = report
        .gaps
        .iter()
        .find(|g| g.gap_type == EvidenceGapType::UnresolvedEntityReference)
        .expect("Must identify UnresolvedEntityReference gap");

    assert_eq!(gap.subject_id, "evt-anon-01");
    assert!(gap.explanation.contains("UNKNOWN"));
    assert!(gap.suggested_corroborating_evidence.contains("biometric"));
}

// 32. Phase 5: Gap Category - Spatial Continuity Gap
#[tokio::test]
async fn test_phase5_gap_spatial_continuity_gap() {
    use sutram_backend_rust::reconstruction::{
        analyze_evidence_gaps, EvidenceGapType, GeoPoint, RawTimelineEvent, ReconstructionOptions,
    };

    let options = ReconstructionOptions::default();
    let service =
        sutram_backend_rust::reconstruction::service::ForensicReconstructionService::new();

    let p1 = GeoPoint::new(28.5244, 77.2066).unwrap();
    let p2 = GeoPoint::new(28.6000, 77.2200).unwrap();

    // Event 1 (located) -> Event 2 (unlocated intervening) -> Event 3 (located)
    let ev1 = RawTimelineEvent {
        event_id: "evt-sp-01".to_string(),
        source_evidence_id: "evi-01".to_string(),
        timestamp: "2026-11-18T10:00:00Z".to_string(),
        timestamp_precision: "EXACT_SECOND".to_string(),
        duration_seconds: None,
        uncertainty_window_start: None,
        uncertainty_window_end: None,
        coordinates: Some(p1),
        raw_description: None,
        entity_ids: vec!["suspect-1".to_string()],
        metadata: serde_json::json!({ "source_record_id": "1", "extraction_run_id": "1" }),
        source_timezone: Some("UTC".to_string()),
        clock_offset_seconds: Some(0.0),
        assumptions: vec![],
    };

    let ev2_unlocated = RawTimelineEvent {
        event_id: "evt-sp-02-unlocated".to_string(),
        source_evidence_id: "evi-01".to_string(),
        timestamp: "2026-11-18T10:15:00Z".to_string(),
        timestamp_precision: "EXACT_SECOND".to_string(),
        duration_seconds: None,
        uncertainty_window_start: None,
        uncertainty_window_end: None,
        coordinates: None, // unlocated intervening!
        raw_description: None,
        entity_ids: vec!["suspect-1".to_string()],
        metadata: serde_json::json!({ "source_record_id": "2", "extraction_run_id": "1" }),
        source_timezone: Some("UTC".to_string()),
        clock_offset_seconds: Some(0.0),
        assumptions: vec![],
    };

    let ev3 = RawTimelineEvent {
        event_id: "evt-sp-03".to_string(),
        source_evidence_id: "evi-01".to_string(),
        timestamp: "2026-11-18T10:30:00Z".to_string(),
        timestamp_precision: "EXACT_SECOND".to_string(),
        duration_seconds: None,
        uncertainty_window_start: None,
        uncertainty_window_end: None,
        coordinates: Some(p2),
        raw_description: None,
        entity_ids: vec!["suspect-1".to_string()],
        metadata: serde_json::json!({ "source_record_id": "3", "extraction_run_id": "1" }),
        source_timezone: Some("UTC".to_string()),
        clock_offset_seconds: Some(0.0),
        assumptions: vec![],
    };

    let recon = service
        .reconstruct_case(&[ev1, ev2_unlocated, ev3], &options)
        .unwrap();
    let report = analyze_evidence_gaps("c6", &recon, &[], None);

    let gap = report
        .gaps
        .iter()
        .find(|g| g.gap_type == EvidenceGapType::SpatialContinuityGap)
        .expect("Must identify SpatialContinuityGap");

    assert_eq!(gap.subject_type, "MOVEMENT_SEGMENT");
    assert!(gap.subject_id.contains("evt-sp-01"));
    assert!(gap.subject_id.contains("evt-sp-03"));
    assert!(gap.explanation.contains("unlocated intervening"));
    assert!(gap.suggested_corroborating_evidence.contains("FASTag"));
}

// 33. Phase 5: Gap Category - Video Evidence Without Extracted Frames
#[tokio::test]
async fn test_phase5_gap_video_without_frames() {
    use sutram_backend_rust::{
        ingestion::{EvidenceManifest, EvidenceUploadReceipt, IngestionStatus},
        reconstruction::{analyze_evidence_gaps, EvidenceGapType, ReconstructionOptions},
    };

    let video_manifest = EvidenceManifest {
        receipt: EvidenceUploadReceipt {
            evidence_id: "evi-vid-01".to_string(),
            case_id: "c7".to_string(),
            original_filename: "traffic_cam.mp4".to_string(),
            source_type: "VIDEO_CCTV".to_string(),
            mime_type: "video/mp4".to_string(),
            file_size_bytes: 10485760,
            sha256_hash: "vidhash123".to_string(),
            storage_uri: "file:///tmp/traffic_cam.mp4".to_string(),
            status: IngestionStatus::Ingested,
            metadata: serde_json::json!({}),
            created_at: "2026-11-18T10:00:00Z".to_string(),
        },
        relative_raw_path: "c7/evi-vid-01/evi-vid-01.raw".to_string(),
        stored_at: "2026-11-18T10:00:00Z".to_string(),
    };

    let options = ReconstructionOptions::default();
    let service =
        sutram_backend_rust::reconstruction::service::ForensicReconstructionService::new();
    let recon = service.reconstruct_case(&[], &options).unwrap();
    // No video handoff records attached

    let report = analyze_evidence_gaps("c7", &recon, &[video_manifest], None);

    let gap = report
        .gaps
        .iter()
        .find(|g| g.gap_type == EvidenceGapType::VideoWithoutFrames)
        .expect("Must identify VideoWithoutFrames gap");

    assert_eq!(gap.subject_id, "evi-vid-01");
    assert!(gap.explanation.contains("traffic_cam.mp4"));
    assert!(gap
        .suggested_corroborating_evidence
        .contains("I-frame keyframes"));
}

// 34. Phase 5: Gap Category - Uncalibrated Device Clock
#[tokio::test]
async fn test_phase5_gap_uncalibrated_clock() {
    use sutram_backend_rust::extraction::model::ExtractedEventCandidate;
    use sutram_backend_rust::reconstruction::{
        adapters::event_candidate_to_timeline_event_with_context, analyze_evidence_gaps,
        EvidenceGapType, ReconstructionOptions,
    };

    let candidate = ExtractedEventCandidate {
        candidate_id: "evt-clk-01".to_string(),
        source_evidence_id: "evi-cctv-01".to_string(),
        source_record_id: "rec_1".to_string(),
        event_type: "CCTV_DETECTION".to_string(),
        raw_timestamp: "2026-11-18T10:00:00Z".to_string(),
        timestamp_precision: Some("EXACT_SECOND".to_string()),
        description: Some("Passerby".to_string()),
        duration_seconds: None,
        location_name: None,
        involved_entity_raw_ids: vec!["suspect-1".to_string()],
        metadata: serde_json::json!({
            "coordinates": { "lat": 28.5, "lon": 77.2 }
        }),
    };

    // When context has None for clock_offset, adapters.rs generates ClockOffsetNotDeclaredOnEvent assumption
    let event = event_candidate_to_timeline_event_with_context(
        &candidate,
        Some("EXACT_SECOND"),
        Some("UTC"),
        None,
    );

    let options = ReconstructionOptions::default();
    let service =
        sutram_backend_rust::reconstruction::service::ForensicReconstructionService::new();
    let recon = service.reconstruct_case(&[event], &options).unwrap();

    let report = analyze_evidence_gaps("c8", &recon, &[], None);

    let gap = report
        .gaps
        .iter()
        .find(|g| g.gap_type == EvidenceGapType::UncalibratedClock)
        .expect("Must identify UncalibratedClock gap");

    assert_eq!(gap.subject_id, "evt-clk-01");
    assert!(gap
        .explanation
        .contains("uncalibrated clock offset assumption"));
    assert!(gap.suggested_corroborating_evidence.contains("NTP"));
}

// 35. Phase 5: Gap Category - Unresolved Source Record
#[tokio::test]
async fn test_phase5_gap_unresolved_source_record() {
    use sutram_backend_rust::reconstruction::{
        analyze_evidence_gaps, EvidenceGapType, GeoPoint, RawTimelineEvent, ReconstructionOptions,
    };

    let p = GeoPoint::new(28.5244, 77.2066).unwrap();
    let ev = RawTimelineEvent {
        event_id: "evt-nosource-01".to_string(),
        source_evidence_id: "evi-cdr-01".to_string(),
        timestamp: "2026-11-18T10:00:00Z".to_string(),
        timestamp_precision: "EXACT_SECOND".to_string(),
        duration_seconds: None,
        uncertainty_window_start: None,
        uncertainty_window_end: None,
        coordinates: Some(p),
        raw_description: None,
        entity_ids: vec!["suspect-1".to_string()],
        metadata: serde_json::json!({
            // Empty metadata: no source_record_id!
            "extraction_run_id": "ext-1"
        }),
        source_timezone: Some("UTC".to_string()),
        clock_offset_seconds: Some(0.0),
        assumptions: vec![],
    };

    let options = ReconstructionOptions::default();
    let service =
        sutram_backend_rust::reconstruction::service::ForensicReconstructionService::new();
    let recon = service.reconstruct_case(&[ev], &options).unwrap();

    let report = analyze_evidence_gaps("c9", &recon, &[], None);

    let gap = report
        .gaps
        .iter()
        .find(|g| g.gap_type == EvidenceGapType::UnresolvedSourceRecord)
        .expect("Must identify UnresolvedSourceRecord gap");

    assert_eq!(gap.subject_id, "evt-nosource-01");
    assert!(gap
        .explanation
        .contains("lacks a specific source record identifier"));
}

// 36. Phase 5: Gap Category - Missing Extraction Provenance
#[tokio::test]
async fn test_phase5_gap_missing_extraction_provenance() {
    use sutram_backend_rust::reconstruction::{
        analyze_evidence_gaps, EvidenceGapType, GeoPoint, RawTimelineEvent, ReconstructionOptions,
    };

    let p = GeoPoint::new(28.5244, 77.2066).unwrap();
    let ev = RawTimelineEvent {
        event_id: "evt-noext-01".to_string(),
        source_evidence_id: "evi-cdr-01".to_string(),
        timestamp: "2026-11-18T10:00:00Z".to_string(),
        timestamp_precision: "EXACT_SECOND".to_string(),
        duration_seconds: None,
        uncertainty_window_start: None,
        uncertainty_window_end: None,
        coordinates: Some(p),
        raw_description: None,
        entity_ids: vec!["suspect-1".to_string()],
        metadata: serde_json::json!({
            "source_record_id": "row_42"
            // missing extraction_run_id!
        }),
        source_timezone: Some("UTC".to_string()),
        clock_offset_seconds: Some(0.0),
        assumptions: vec![],
    };

    let options = ReconstructionOptions::default();
    let service =
        sutram_backend_rust::reconstruction::service::ForensicReconstructionService::new();
    let recon = service.reconstruct_case(&[ev], &options).unwrap();

    let report = analyze_evidence_gaps("c10", &recon, &[], None);

    let gap = report
        .gaps
        .iter()
        .find(|g| g.gap_type == EvidenceGapType::MissingExtractionProvenance)
        .expect("Must identify MissingExtractionProvenance gap");

    assert_eq!(gap.subject_id, "evt-noext-01");
    assert!(gap
        .explanation
        .contains("lacks an associated extraction run ID"));
}

// 37. Phase 5: Gap Category - Missing Corroboration for Contradiction Lead
#[tokio::test]
async fn test_phase5_gap_missing_corroboration() {
    use sutram_backend_rust::reconstruction::{
        analyze_evidence_gaps, EvidenceGapType, GeoPoint, RawTimelineEvent, ReconstructionOptions,
    };

    let options = ReconstructionOptions {
        speed_threshold_kmh: 180.0,
        ..Default::default()
    };
    let service =
        sutram_backend_rust::reconstruction::service::ForensicReconstructionService::new();

    // 2 events separated by 1000 km in 5 minutes, but both from the EXACT SAME EXHIBIT ("evi-cdr-single")
    let p1 = GeoPoint::new(28.5244, 77.2066).unwrap();
    let p2 = GeoPoint::new(19.0760, 72.8777).unwrap();

    let ev1 = RawTimelineEvent {
        event_id: "evt-lead-01".to_string(),
        source_evidence_id: "evi-cdr-single".to_string(),
        timestamp: "2026-11-18T10:00:00Z".to_string(),
        timestamp_precision: "EXACT_SECOND".to_string(),
        duration_seconds: None,
        uncertainty_window_start: None,
        uncertainty_window_end: None,
        coordinates: Some(p1),
        raw_description: None,
        entity_ids: vec!["suspect-1".to_string()],
        metadata: serde_json::json!({ "source_record_id": "r1", "extraction_run_id": "e1" }),
        source_timezone: Some("UTC".to_string()),
        clock_offset_seconds: Some(0.0),
        assumptions: vec![],
    };

    let ev2 = RawTimelineEvent {
        event_id: "evt-lead-02".to_string(),
        source_evidence_id: "evi-cdr-single".to_string(), // SAME EXHIBIT!
        timestamp: "2026-11-18T10:05:00Z".to_string(),
        timestamp_precision: "EXACT_SECOND".to_string(),
        duration_seconds: None,
        uncertainty_window_start: None,
        uncertainty_window_end: None,
        coordinates: Some(p2),
        raw_description: None,
        entity_ids: vec!["suspect-1".to_string()],
        metadata: serde_json::json!({ "source_record_id": "r2", "extraction_run_id": "e1" }),
        source_timezone: Some("UTC".to_string()),
        clock_offset_seconds: Some(0.0),
        assumptions: vec![],
    };

    let recon = service.reconstruct_case(&[ev1, ev2], &options).unwrap();
    assert_eq!(recon.contradictions.leads.len(), 1);

    let report = analyze_evidence_gaps("c11", &recon, &[], None);

    let gap = report
        .gaps
        .iter()
        .find(|g| g.gap_type == EvidenceGapType::MissingCorroboration)
        .expect("Must identify MissingCorroboration gap");

    assert_eq!(gap.subject_type, "CONTRADICTION_LEAD");
    assert!(gap
        .explanation
        .contains("single evidence exhibit 'evi-cdr-single'"));
    assert!(gap
        .suggested_corroborating_evidence
        .contains("independent secondary evidence"));
}

// 38. Phase 5: False-Positive Prevention
#[tokio::test]
async fn test_phase5_gap_false_positive_prevention() {
    use sutram_backend_rust::{
        ingestion::{EvidenceManifest, EvidenceUploadReceipt, IngestionStatus},
        reconstruction::{
            analyze_evidence_gaps, GeoPoint, RawTimelineEvent, ReconstructionOptions,
        },
    };

    let p1 = GeoPoint::new(28.5244, 77.2066).unwrap();
    let p2 = GeoPoint::new(28.5300, 77.2100).unwrap();

    let ev1 = RawTimelineEvent {
        event_id: "evt-clean-01".to_string(),
        source_evidence_id: "evi-clean-01".to_string(),
        timestamp: "2026-11-18T10:00:00Z".to_string(),
        timestamp_precision: "EXACT_SECOND".to_string(),
        duration_seconds: None,
        uncertainty_window_start: None,
        uncertainty_window_end: None,
        coordinates: Some(p1),
        raw_description: Some("Verified Shop CCTV".to_string()),
        entity_ids: vec!["suspect-1".to_string()],
        metadata: serde_json::json!({
            "source_record_id": "frame_10",
            "extraction_run_id": "ext_run_01"
        }),
        source_timezone: Some("UTC".to_string()),
        clock_offset_seconds: Some(0.0),
        assumptions: vec![], // No uncalibrated clock assumptions
    };

    let ev2 = RawTimelineEvent {
        event_id: "evt-clean-02".to_string(),
        source_evidence_id: "evi-clean-01".to_string(),
        timestamp: "2026-11-18T10:15:00Z".to_string(),
        timestamp_precision: "EXACT_SECOND".to_string(),
        duration_seconds: None,
        uncertainty_window_start: None,
        uncertainty_window_end: None,
        coordinates: Some(p2),
        raw_description: Some("Verified Street Camera".to_string()),
        entity_ids: vec!["suspect-1".to_string()],
        metadata: serde_json::json!({
            "source_record_id": "frame_20",
            "extraction_run_id": "ext_run_01"
        }),
        source_timezone: Some("UTC".to_string()),
        clock_offset_seconds: Some(0.0),
        assumptions: vec![],
    };

    let manifest = EvidenceManifest {
        receipt: EvidenceUploadReceipt {
            evidence_id: "evi-clean-01".to_string(),
            case_id: "c-clean".to_string(),
            original_filename: "verified_shop.cctv".to_string(),
            source_type: "SECURITY_CCTV".to_string(), // not matching "VIDEO" so no frames check
            mime_type: "application/octet-stream".to_string(),
            file_size_bytes: 5000,
            sha256_hash: "hashclean".to_string(),
            storage_uri: "file:///tmp/shop.cctv".to_string(),
            status: IngestionStatus::Ingested,
            metadata: serde_json::json!({}),
            created_at: "2026-11-18T10:00:00Z".to_string(),
        },
        relative_raw_path: "c-clean/evi-clean-01/raw".to_string(),
        stored_at: "2026-11-18T10:00:00Z".to_string(),
    };

    let options = ReconstructionOptions::default();
    let service =
        sutram_backend_rust::reconstruction::service::ForensicReconstructionService::new();
    let recon = service.reconstruct_case(&[ev1, ev2], &options).unwrap();

    let report = analyze_evidence_gaps(
        "c-clean",
        &recon,
        &[manifest],
        Some(&["suspect-1".to_string()]),
    );

    // Fully calibrated, valid coordinates, full provenance, resolved entity, no continuity jump
    assert_eq!(
        report.total_gaps_count, 0,
        "Fully verified reconstruction must have zero evidence gaps, found: {:?}",
        report.gaps
    );
}

// 39. Phase 5: Missing-Data Handling (Empty inputs do not panic)
#[tokio::test]
async fn test_phase5_gap_missing_data_handling() {
    use sutram_backend_rust::reconstruction::{analyze_evidence_gaps, ReconstructionOptions};

    let options = ReconstructionOptions::default();
    let service =
        sutram_backend_rust::reconstruction::service::ForensicReconstructionService::new();
    let recon = service.reconstruct_case(&[], &options).unwrap();

    let report = analyze_evidence_gaps("c-empty", &recon, &[], None);
    assert_eq!(report.total_gaps_count, 0);
    assert!(report.gaps_by_severity.is_empty());
    assert!(report.gaps_by_type.is_empty());
}

// 40. Phase 5: Duplicate-Gap Handling
#[tokio::test]
async fn test_phase5_gap_duplicate_gap_handling() {
    use sutram_backend_rust::reconstruction::{
        analyze_evidence_gaps, RawTimelineEvent, ReconstructionOptions,
    };

    let options = ReconstructionOptions::default();
    let service =
        sutram_backend_rust::reconstruction::service::ForensicReconstructionService::new();

    let ev = RawTimelineEvent {
        event_id: "evt-dup-01".to_string(),
        source_evidence_id: "evi-dup".to_string(),
        timestamp: "2026-11-18T10:00:00Z".to_string(),
        timestamp_precision: "EXACT_SECOND".to_string(),
        duration_seconds: None,
        uncertainty_window_start: None,
        uncertainty_window_end: None,
        coordinates: None, // causes EventWithoutCoordinates gap
        raw_description: None,
        entity_ids: vec!["suspect-1".to_string()],
        metadata: serde_json::json!({ "source_record_id": "r1", "extraction_run_id": "e1" }),
        source_timezone: Some("UTC".to_string()),
        clock_offset_seconds: Some(0.0),
        assumptions: vec![],
    };

    let recon = service.reconstruct_case(&[ev], &options).unwrap();

    let report = analyze_evidence_gaps("c-dup", &recon, &[], None);

    let coord_gaps: Vec<_> = report
        .gaps
        .iter()
        .filter(|g| g.gap_id == "gap-nocoord-evt-dup-01")
        .collect();

    assert_eq!(
        coord_gaps.len(),
        1,
        "Duplicate gap IDs must be strictly deduplicated"
    );
}

// 41. Phase 5: Deterministic Output
#[tokio::test]
async fn test_phase5_gap_deterministic_output() {
    use sutram_backend_rust::reconstruction::{
        analyze_evidence_gaps, RawTimelineEvent, ReconstructionOptions,
    };

    let options = ReconstructionOptions::default();
    let service =
        sutram_backend_rust::reconstruction::service::ForensicReconstructionService::new();

    let ev = RawTimelineEvent {
        event_id: "evt-det-01".to_string(),
        source_evidence_id: "evi-det".to_string(),
        timestamp: "2026-11-18T10:00:00Z".to_string(),
        timestamp_precision: "EXACT_SECOND".to_string(),
        duration_seconds: None,
        uncertainty_window_start: None,
        uncertainty_window_end: None,
        coordinates: None,
        raw_description: None,
        entity_ids: vec![],
        metadata: serde_json::json!({}),
        source_timezone: Some("UTC".to_string()),
        clock_offset_seconds: Some(0.0),
        assumptions: vec![],
    };

    let recon = service.reconstruct_case(&[ev], &options).unwrap();

    let report1 = analyze_evidence_gaps("c-det", &recon, &[], None);
    let report2 = analyze_evidence_gaps("c-det", &recon, &[], None);

    // Must be byte-for-byte identical (excluding audited_at timestamp)
    assert_eq!(report1.total_gaps_count, report2.total_gaps_count);
    assert_eq!(report1.gaps, report2.gaps);
}

// 42. Phase 5: Evidence Added Later Resolves Relevant Gap
#[tokio::test]
async fn test_phase5_gap_resolved_when_evidence_added() {
    use sutram_backend_rust::{
        ingestion::{EvidenceManifest, EvidenceUploadReceipt, IngestionStatus},
        reconstruction::{
            analyze_evidence_gaps,
            video_handoff::{VideoHandoffRecord, VideoHandoffStatus},
            EvidenceGapType, ReconstructionOptions,
        },
    };

    let video_manifest = EvidenceManifest {
        receipt: EvidenceUploadReceipt {
            evidence_id: "evi-resolve-vid".to_string(),
            case_id: "c-res".to_string(),
            original_filename: "junction.mp4".to_string(),
            source_type: "VIDEO_CCTV".to_string(),
            mime_type: "video/mp4".to_string(),
            file_size_bytes: 10000,
            sha256_hash: "vidhash".to_string(),
            storage_uri: "file:///tmp/junction.mp4".to_string(),
            status: IngestionStatus::Ingested,
            metadata: serde_json::json!({}),
            created_at: "2026-11-18T10:00:00Z".to_string(),
        },
        relative_raw_path: "c-res/evi-resolve-vid/raw".to_string(),
        stored_at: "2026-11-18T10:00:00Z".to_string(),
    };

    let options = ReconstructionOptions::default();
    let service =
        sutram_backend_rust::reconstruction::service::ForensicReconstructionService::new();

    // 1. Initial reconstruction: video has NO frames extracted
    let recon_initial = service.reconstruct_case(&[], &options).unwrap();
    let report_initial =
        analyze_evidence_gaps("c-res", &recon_initial, &[video_manifest.clone()], None);
    assert!(report_initial
        .gaps
        .iter()
        .any(|g| g.gap_type == EvidenceGapType::VideoWithoutFrames));

    // 2. Later: Video frames extracted and handed off
    let mut recon_resolved = service.reconstruct_case(&[], &options).unwrap();
    recon_resolved.attach_video_handoff(vec![VideoHandoffRecord {
        evidence_id: "evi-resolve-vid".to_string(),
        source_video_sha256: "vidhash".to_string(),
        status: VideoHandoffStatus::HandedOffAnchored,
        frames_extracted: 24,
        candidates_handed_off: 24,
        reused_existing_frames: false,
        frames_reference: Some("frames.json".to_string()),
        anchor_basis: Some("calibration_receipt".to_string()),
        diagnostics: vec![],
    }]);

    let report_resolved = analyze_evidence_gaps("c-res", &recon_resolved, &[video_manifest], None);
    assert!(
        !report_resolved
            .gaps
            .iter()
            .any(|g| g.gap_type == EvidenceGapType::VideoWithoutFrames),
        "VideoWithoutFrames gap must be resolved when verified frames are provided"
    );
}

// 43. Phase 5: Gap Intelligence API Endpoint Integration
#[tokio::test]
async fn test_phase5_gap_api_endpoint() {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use sutram_backend_rust::{
        create_router, reconstruction::gaps::EvidenceGapAnalysisReport, AppConfig,
    };
    use tower::ServiceExt;

    let app = create_router(AppConfig::default());
    let case_id = "cccccccc-cccc-cccc-cccc-cccccccccccc";

    // 1. Post a reconstruction with an unlocated event
    let payload = serde_json::json!({
        "options": {
            "default_timezone": "UTC",
            "speed_threshold_kmh": 180.0
        },
        "raw_events": [
            {
                "event_id": "evt-api-gap-01",
                "source_evidence_id": "evi-gap-test",
                "timestamp": "2026-11-18T10:00:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "duration_seconds": 0.0,
                "coordinates": null, // triggers EventWithoutCoordinates
                "raw_description": "Unlocated event",
                "entity_ids": ["suspect-1"],
                "metadata": {}
            }
        ]
    });

    let req_recon = Request::builder()
        .uri(format!("/api/v1/cases/{}/reconstruction", case_id))
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_vec(&payload).unwrap()))
        .unwrap();

    let resp_recon = app.clone().oneshot(req_recon).await.unwrap();
    assert_eq!(resp_recon.status(), StatusCode::OK);

    // 2. Query gap report via GET /api/v1/cases/:case_id/reconstruction/gaps
    let req_gaps = Request::builder()
        .uri(format!("/api/v1/cases/{}/reconstruction/gaps", case_id))
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let resp_gaps = app.clone().oneshot(req_gaps).await.unwrap();
    assert_eq!(resp_gaps.status(), StatusCode::OK);
    let bytes = resp_gaps.into_body().collect().await.unwrap().to_bytes();
    let report: EvidenceGapAnalysisReport = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(report.case_id, case_id);
    assert!(report.total_gaps_count >= 1);
    assert!(report.gaps.iter().any(|g| g.subject_id == "evt-api-gap-01"));

    // 3. Query via alias GET /api/v1/cases/:case_id/gaps
    let req_alias = Request::builder()
        .uri(format!("/api/v1/cases/{}/gaps", case_id))
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let resp_alias = app.clone().oneshot(req_alias).await.unwrap();
    assert_eq!(resp_alias.status(), StatusCode::OK);
}
