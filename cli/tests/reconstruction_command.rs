//! `yukti reconstruction run | explain | compare` against a mock API, through the library entry point.

mod support;

use std::time::Duration;

use clap::Parser;
use serde_json::{json, Value};
use support::{fixtures, unused_base_url, MockServer, Route};
use yukti_cli::cli::Cli;
use yukti_cli::commands::reconstruction::{
    LIMITATION_NO_RUN_ID, LIMITATION_PERSISTENCE_NOT_REPORTED, LIMITATION_PROVENANCE_UNKNOWN,
    LIMITATION_REPORT_SEPARATE, RECONSTRUCTION_EXPLANATION_SCHEMA, RECONSTRUCTION_RUN_SCHEMA,
};
use yukti_cli::config::EnvVars;
use yukti_cli::output::Rendered;

const CASE_PATH: &str = "/api/v1/cases/3f2b8c1e-7a4d-4e1b-9c3a-5d6e7f8a9b0c";
const RECON_PATH: &str = "/api/v1/cases/3f2b8c1e-7a4d-4e1b-9c3a-5d6e7f8a9b0c/reconstruction";
const REPORT_PATH: &str = "/api/v1/cases/3f2b8c1e-7a4d-4e1b-9c3a-5d6e7f8a9b0c/report";
const SERVER_ERROR: &str =
    r#"{"error":{"code":"INTERNAL_ERROR","message":"Reconstruction engine failure"}}"#;

const SAFEGUARD_REASON: &str =
    "2 timeline event(s) rest on labelled assumptions: TIMEZONE_NOT_DECLARED_ON_EVENT x2.";
const LEAD_EXPLANATION: &str = "Transit between the two observed locations would require a speed above the configured reference threshold.";
const LEAD_LIMITATION: &str = "Tower-based coordinates may be offset from the handset location.";
const ASSUMPTION_BASIS: &str = "Reconstruction request default applied to a stored event.";
const DOSSIER: &str = "# Forensic Reconstruction Dossier\n\n> [!NOTE]\n> Judicial export is blocked pending human investigator resolution of 1 unresolved contradiction lead(s).\n";

fn time_window(nominal: &str) -> Value {
    json!({
        "nominal_utc": nominal, "precision": "EXACT_SECOND", "raw_precision_input": "EXACT_SECOND",
        "window_start_utc": nominal, "window_end_utc": nominal, "uncertainty_span_seconds": 1.0,
        "has_explicit_bounds": false, "is_estimated_window_assumption": false,
        "source_timezone": "UTC", "clock_offset_seconds": 0.0
    })
}

fn tz_assumption() -> Value {
    json!({
        "kind": "TIMEZONE_NOT_DECLARED_ON_EVENT", "field": "source_timezone",
        "assumed_value": "UTC", "basis": ASSUMPTION_BASIS
    })
}

fn timeline_entry(id: &str, nominal: &str, lat: f64, lon: f64) -> Value {
    json!({
        "event_id": id, "source_evidence_id": fixtures::RECEIPT_CSV_ID,
        "raw_timestamp": nominal, "time_window": time_window(nominal),
        "coordinates": {"lat": lat, "lon": lon}, "raw_description": "Cell tower handover",
        "entity_ids": ["+919811002233"], "metadata": {"tower_id": "DL-0042"},
        "ordering_status": "STRICTLY_ORDERED", "concurrent_event_ids": [],
        "assumptions": [tz_assumption()]
    })
}

/// A `FullCaseReconstructionResult` as the API serializes it (`backend-rust/src/reconstruction/`).
fn reconstruction(case_id: &str) -> Value {
    let a = timeline_entry("evt-a", "2026-11-10T16:15:12Z", 28.5245, 77.2066);
    let b = timeline_entry("evt-b", "2026-11-10T16:20:12Z", 19.0760, 72.8777);
    json!({
        "case_id": case_id,
        "timeline": {
            "case_id": case_id, "total_events": 2, "entries": [a.clone(), b.clone()],
            "overlaps": [], "strictly_ordered_count": 2, "concurrent_uncertain_count": 0,
            "excluded_events": [{
                "event_id": "evt-x", "source_evidence_id": fixtures::RECEIPT_CHAT_ID,
                "reason": "Unknown timezone 'Mars/Olympus'"
            }]
        },
        "spatial": {
            "case_id": case_id,
            "trajectories": [{
                "entity_id": "+919811002233", "waypoints": [a, b],
                "segments": [{
                    "origin_event_id": "evt-a", "destination_event_id": "evt-b",
                    "entity_id": "+919811002233",
                    "origin_coordinates": {"lat": 28.5245, "lon": 77.2066},
                    "destination_coordinates": {"lat": 19.0760, "lon": 72.8777},
                    "transit_metrics": {
                        "distance_km": 1153.2, "nominal_elapsed_seconds": 300.0,
                        "min_possible_elapsed_seconds": 298.0, "max_possible_elapsed_seconds": 302.0,
                        "min_required_speed_kmh": 13931.5, "nominal_speed_kmh": 13838.4,
                        "configured_speed_threshold_kmh": 900.0,
                        "exceeds_configured_speed_threshold": true,
                        "threshold_evaluation": "EXCEEDS_CONFIGURED_THRESHOLD"
                    },
                    "has_intervening_unlocated_events": false,
                    "intervening_unlocated_event_ids": []
                }],
                "total_distance_km": 1153.2, "max_min_required_speed_kmh": 13931.5
            }],
            "overlaps": []
        },
        "contradictions": {
            "case_id": case_id,
            "leads": [{
                "lead_id": "lead-0001", "lead_type": "SPATIAL_TRANSIT_IMPOSSIBLE",
                "entity_id": "+919811002233", "event_a_id": "evt-a", "event_b_id": "evt-b",
                "observed_basis": {
                    "origin_event_id": "evt-a", "destination_event_id": "evt-b",
                    "origin_evidence_id": fixtures::RECEIPT_CSV_ID,
                    "destination_evidence_id": fixtures::RECEIPT_CSV_ID,
                    "origin_timestamp_raw": "2026-11-10T16:15:12Z",
                    "destination_timestamp_raw": "2026-11-10T16:20:12Z",
                    "observed_distance_km": 1153.2
                },
                "derived_inference": {
                    "post_activity_min_required_speed_kmh": 13931.5,
                    "reference_speed_threshold_kmh": 900.0,
                    "max_possible_elapsed_seconds": 302.0, "min_possible_elapsed_seconds": 298.0,
                    "nominal_elapsed_seconds": 300.0
                },
                "neutral_explanation": LEAD_EXPLANATION,
                "limitations": [LEAD_LIMITATION],
                "generated_at": "2026-11-21T10:00:00Z"
            }],
            "total_evaluated_segments": 1, "flagged_segments_count": 1,
            "disclaimer": "Analytical contradiction leads indicate spatio-temporal discrepancies against user-configured thresholds. They are investigative leads and do not constitute conclusive proof of falsity or identity."
        },
        "export_safeguard": {"status": "BLOCKED", "reasons": [SAFEGUARD_REASON]}
    })
}

/// A `CaseForensicReport` consistent with [`reconstruction`].
fn report(events: u64) -> Value {
    json!({
        "report_id": "0b9d7c1e-5a4f-5e3d-8c2b-1a0f9e8d7c6b",
        "case_id": fixtures::CASE_A_ID, "title": "Warehouse burglary",
        "fir_number": "FIR-112/2026", "investigator_id": "INV-7",
        "generated_at": "2026-11-21T10:05:00+00:00",
        "judicial_export_status": "SHIP_EXPORT_BLOCKED",
        "judicial_export_reason": "Judicial export is blocked pending human investigator resolution of 1 unresolved contradiction lead(s).",
        "summary": {
            "evidence_count": 2, "total_timeline_events": events, "strictly_ordered_events": 2,
            "concurrent_uncertain_events": 0, "excluded_events_count": 1,
            "spatial_trajectories_count": 1, "total_movement_segments": 1,
            "flagged_speed_segments": 1, "open_contradictions_count": 1
        },
        "evidence_inventory": [],
        "contradiction_leads": [],
        "forensic_disclaimer": "This report presents investigative leads, not conclusions of guilt or identity.",
        "markdown_dossier": DOSSIER
    })
}

fn case_route() -> Route {
    Route::json(CASE_PATH, 200, fixtures::CASE_A).method("GET")
}

fn run_api(status: u16, body: &str) -> MockServer {
    MockServer::start(vec![
        case_route(),
        Route::json(RECON_PATH, status, body).method("POST"),
    ])
}

fn explain_api(report_status: u16, report_body: &str) -> MockServer {
    MockServer::start(vec![
        case_route(),
        Route::json(
            RECON_PATH,
            200,
            &reconstruction(fixtures::CASE_A_ID).to_string(),
        )
        .method("GET"),
        Route::json(REPORT_PATH, report_status, report_body).method("GET"),
    ])
}

async fn yukti(base_url: &str, args: &[&str]) -> Rendered {
    let mut argv = vec!["yukti", "--api-url", base_url];
    argv.extend_from_slice(args);
    let cli = Cli::try_parse_from(argv).expect("valid arguments");
    yukti_cli::execute(&cli, &EnvVars::default()).await
}

fn parse(r: &Rendered) -> Value {
    assert!(
        r.stderr.is_empty(),
        "JSON mode writes nothing to stderr: {}",
        r.stderr
    );
    serde_json::from_str(&r.stdout).expect("stdout is one JSON document")
}

fn calls(server: &MockServer) -> Vec<(String, String)> {
    server
        .requests()
        .into_iter()
        .map(|r| (r.method, r.path))
        .collect()
}

fn call(method: &str, path: &str) -> (String, String) {
    (method.to_string(), path.to_string())
}

/// Every needle appears in the output, ignoring how whitespace wraps or aligns it.
fn assert_contains(haystack: &str, needles: &[&str]) {
    // Card rails (│) start wrapped continuation lines; they are layout, not text.
    let flat_haystack = support::flat(&haystack.replace('│', " "));
    for needle in needles {
        assert!(
            flat_haystack.contains(&support::flat(needle)),
            "missing {needle:?} in\n{haystack}"
        );
    }
}

/* ------------------------------------------------------------------------------------ run */

#[tokio::test]
async fn run_checks_the_case_then_posts_an_empty_request_and_summarises_the_result() {
    let server = run_api(200, &reconstruction(fixtures::CASE_A_ID).to_string());
    let r = yukti(
        &server.base_url,
        &["reconstruction", "run", "--case", fixtures::CASE_A_ID],
    )
    .await;
    assert_eq!(r.exit_code, 0, "{}{}", r.stdout, r.stderr);
    assert!(r.stderr.is_empty(), "{}", r.stderr);

    assert_eq!(
        calls(&server),
        [call("GET", CASE_PATH), call("POST", RECON_PATH)]
    );
    let post = &server.requests()[1];
    assert_eq!(post.content_type.as_deref(), Some("application/json"));
    assert_eq!(
        post.body, "{}",
        "no options, candidates or raw events are fabricated"
    );

    assert_contains(
        &r.stdout,
        &[
            &format!("Reconstruction completed for case {}", fixtures::CASE_A_ID),
            &format!("POST {RECON_PATH} → HTTP 200"),
            "run ID not returned by the API",
            &format!("Case ID (server)  {}", fixtures::CASE_A_ID),
            "2 event(s): 2 strictly ordered, 0 concurrent/uncertain 1 excluded · 0 overlapping pair(s)",
            "1 trajectory(ies), 1 movement segment(s) (1 exceed the configured speed threshold), 0 spatial-temporal overlap(s)",
            "1 lead(s) from 1 evaluated segment(s), 1 flagged",
            "2 labelled assumption(s) on 2 event(s)",
            "Export safeguard ✕ BLOCKED",
            &format!("▲ {SAFEGUARD_REASON}"),
            "◆ TIMEZONE_NOT_DECLARED_ON_EVENT source_timezone = \"UTC\" on 2 event(s)",
            ASSUMPTION_BASIS,
            "evt-x evidence 7c2d1e3f… Unknown timezone 'Mars/Olympus'",
            "▲ SPATIAL_TRANSIT_IMPOSSIBLE lead-0001",
            "entity +919811002233 · event evt-a → event evt-b",
            "do not constitute conclusive proof",
            LIMITATION_NO_RUN_ID,
            LIMITATION_PERSISTENCE_NOT_REPORTED,
        ],
    );
}

#[tokio::test]
async fn run_json_preserves_the_response_and_invents_no_run_id() {
    let body = reconstruction(fixtures::CASE_A_ID);
    let server = run_api(200, &body.to_string());
    let r = yukti(
        &server.base_url,
        &[
            "reconstruction",
            "run",
            "--case",
            fixtures::CASE_A_ID,
            "--json",
        ],
    )
    .await;
    assert_eq!(r.exit_code, 0);
    let v = parse(&r);
    assert_eq!(v["schema"], RECONSTRUCTION_RUN_SCHEMA);
    assert_eq!(v["case_id"], fixtures::CASE_A_ID);
    assert_eq!(v["server_case_id"], fixtures::CASE_A_ID);
    assert_eq!(
        v["request"],
        json!({"method": "POST", "path": RECON_PATH, "body": {}})
    );
    assert_eq!(
        v["response"], body,
        "the server body is passed through unchanged"
    );
    assert_eq!(v["export_safeguard"], body["export_safeguard"]);
    assert_eq!(v["summary"]["timeline_total_events"], 2);
    assert_eq!(v["summary"]["timeline_excluded_events"], 1);
    assert_eq!(
        v["summary"]["spatial_segments_exceeding_speed_threshold"],
        1
    );
    assert_eq!(v["summary"]["contradiction_leads"], 1);
    assert_eq!(v["summary"]["assumptions"], 2);
    assert!(v.get("run_id").is_none(), "no run ID is invented");
    assert!(v["api_limitations"]
        .as_array()
        .unwrap()
        .contains(&json!(LIMITATION_NO_RUN_ID)));
    assert_eq!(v["warnings"], json!([]));
}

#[tokio::test]
async fn unknown_case_stops_before_the_reconstruction_request() {
    let server = MockServer::start(vec![
        Route::json(CASE_PATH, 404, fixtures::NOT_FOUND).method("GET"),
        Route::json(
            RECON_PATH,
            200,
            &reconstruction(fixtures::CASE_A_ID).to_string(),
        ),
    ]);
    let r = yukti(
        &server.base_url,
        &["reconstruction", "run", "--case", fixtures::CASE_A_ID],
    )
    .await;
    assert_eq!(r.exit_code, 4);
    assert_eq!(calls(&server), [call("GET", CASE_PATH)]);
    assert_contains(
        &r.stderr,
        &["HTTP 404 (NOT_FOUND)", "Nothing was reconstructed"],
    );
}

#[tokio::test]
async fn invalid_case_ids_never_reach_the_api() {
    let server = MockServer::start(vec![]);
    for sub in ["run", "explain", "compare"] {
        let r = yukti(
            &server.base_url,
            &["reconstruction", sub, "--case", "CASE-026"],
        )
        .await;
        assert_eq!(r.exit_code, 2, "{sub}");
        assert!(
            r.stderr.contains("--case must be a case UUID"),
            "{sub}: {}",
            r.stderr
        );
    }
    assert!(server.requests().is_empty());
}

#[tokio::test]
async fn reconstruction_http_errors_map_to_exit_codes() {
    for (status, body, code, kind) in [
        (
            400,
            r#"{"error":{"code":"BAD_REQUEST","message":"Invalid case UUID format"}}"#,
            4,
            "http_client_error",
        ),
        (500, SERVER_ERROR, 5, "http_server_error"),
    ] {
        let server = run_api(status, body);
        let human = yukti(
            &server.base_url,
            &["reconstruction", "run", "--case", fixtures::CASE_A_ID],
        )
        .await;
        assert_eq!(human.exit_code, code, "{status}");
        assert!(human.stdout.is_empty(), "{status}: {}", human.stdout);
        assert!(
            human.stderr.contains(&format!("HTTP {status}")),
            "{}",
            human.stderr
        );

        let r = yukti(
            &server.base_url,
            &[
                "reconstruction",
                "run",
                "--case",
                fixtures::CASE_A_ID,
                "--json",
            ],
        )
        .await;
        assert_eq!(r.exit_code, code);
        let v = parse(&r);
        assert_eq!(v["schema"], "yukti.cli.error/v1");
        assert_eq!(v["error"]["kind"], kind);
        assert_eq!(v["error"]["http_status"], status);
    }
}

#[tokio::test]
async fn a_response_that_is_not_a_reconstruction_is_exit_six() {
    let server = run_api(200, r#"{"status":"queued"}"#);
    let r = yukti(
        &server.base_url,
        &["reconstruction", "run", "--case", fixtures::CASE_A_ID],
    )
    .await;
    assert_eq!(r.exit_code, 6);
    assert!(
        r.stderr.contains("FullCaseReconstructionResult"),
        "{}",
        r.stderr
    );
}

#[tokio::test]
async fn unreachable_api_is_exit_three() {
    let r = yukti(
        &unused_base_url(),
        &[
            "reconstruction",
            "run",
            "--case",
            fixtures::CASE_A_ID,
            "--json",
        ],
    )
    .await;
    assert_eq!(r.exit_code, 3);
    assert_eq!(parse(&r)["error"]["kind"], "unreachable");
}

#[tokio::test]
async fn a_reconstruction_timeout_is_exit_three_and_warns_the_run_may_still_complete() {
    let server = MockServer::start(vec![
        case_route(),
        Route::json(
            RECON_PATH,
            200,
            &reconstruction(fixtures::CASE_A_ID).to_string(),
        )
        .method("POST")
        .delayed(Duration::from_secs(3)),
    ]);
    let r = yukti(
        &server.base_url,
        &[
            "--timeout",
            "1",
            "reconstruction",
            "run",
            "--case",
            fixtures::CASE_A_ID,
        ],
    )
    .await;
    assert_eq!(r.exit_code, 3);
    assert_contains(&r.stderr, &["timed out", "may still complete"]);
    assert_eq!(
        calls(&server),
        [call("GET", CASE_PATH), call("POST", RECON_PATH)]
    );
}

#[tokio::test]
async fn a_different_or_missing_server_case_id_is_warned_about() {
    let server = run_api(200, &reconstruction(fixtures::CASE_B_ID).to_string());
    let r = yukti(
        &server.base_url,
        &["reconstruction", "run", "--case", fixtures::CASE_A_ID],
    )
    .await;
    assert_eq!(r.exit_code, 0);
    assert_contains(
        &r.stdout,
        &[&format!("Case ID (server)  {}", fixtures::CASE_B_ID)],
    );
    assert_contains(
        &r.stderr,
        &["warning: The reconstruction response names case_id"],
    );

    let mut body = reconstruction(fixtures::CASE_A_ID);
    body["case_id"] = Value::Null;
    let server = run_api(200, &body.to_string());
    let r = yukti(
        &server.base_url,
        &[
            "reconstruction",
            "run",
            "--case",
            fixtures::CASE_A_ID,
            "--json",
        ],
    )
    .await;
    let v = parse(&r);
    assert_eq!(v["server_case_id"], Value::Null);
    assert!(v["warnings"][0]
        .as_str()
        .unwrap()
        .contains("does not name a case_id"));
}

/* -------------------------------------------------------------------------------- explain */

#[tokio::test]
async fn explain_shows_the_reconstruction_report_and_provenance_limitation() {
    let server = explain_api(200, &report(2).to_string());
    let r = yukti(
        &server.base_url,
        &["reconstruction", "explain", "--case", fixtures::CASE_A_ID],
    )
    .await;
    assert_eq!(r.exit_code, 0, "{}{}", r.stdout, r.stderr);
    assert!(r.stderr.is_empty(), "{}", r.stderr);
    assert_eq!(
        calls(&server),
        [
            call("GET", CASE_PATH),
            call("GET", RECON_PATH),
            call("GET", REPORT_PATH)
        ]
    );
    assert_contains(
        &r.stdout,
        &[
            &format!("Reconstruction of case {}", fixtures::CASE_A_ID),
            "Provenance        UNKNOWN: stored result or computed on the fly",
            LIMITATION_PROVENANCE_UNKNOWN,
            LIMITATION_REPORT_SEPARATE,
            // Timeline.
            "2026-11-10T16:15:12Z 1s window STRICTLY_ORDERED #1",
            "event evt-a · evidence 5b1f0c2e… · EXACT_SECOND",
            "Cell tower handover",
            // Assumptions, with the events they apply to.
            "events evt-a, evt-b",
            // Excluded events.
            "Unknown timezone 'Mars/Olympus'",
            // Spatial findings.
            "entity +919811002233 2 waypoint(s), 1 segment(s), 1153.200 km total",
            "EXCEEDS_CONFIGURED_THRESHOLD",
            // Contradiction leads with their limitations.
            LEAD_EXPLANATION,
            &format!("◆ {LEAD_LIMITATION}"),
            // Export safeguard and judicial report.
            &format!("▲ {SAFEGUARD_REASON}"),
            "Judicial export ✕ SHIP_EXPORT_BLOCKED",
            "Report ID         0b9d7c1e-5a4f-5e3d-8c2b-1a0f9e8d7c6b",
            DOSSIER,
            "end of dossier",
        ],
    );
}

#[tokio::test]
async fn explain_json_carries_both_bodies_unchanged() {
    let server = explain_api(200, &report(2).to_string());
    let r = yukti(
        &server.base_url,
        &[
            "reconstruction",
            "explain",
            "--case",
            fixtures::CASE_A_ID,
            "--json",
        ],
    )
    .await;
    assert_eq!(r.exit_code, 0);
    let v = parse(&r);
    assert_eq!(v["schema"], RECONSTRUCTION_EXPLANATION_SCHEMA);
    assert_eq!(v["complete"], true);
    assert_eq!(
        v["reconstruction"]["provenance"],
        "unknown_stored_or_computed_on_the_fly"
    );
    assert_eq!(
        v["reconstruction"]["request"],
        json!({"method": "GET", "path": RECON_PATH})
    );
    assert_eq!(
        v["reconstruction"]["response"],
        reconstruction(fixtures::CASE_A_ID)
    );
    assert_eq!(v["report"]["response"], report(2));
    assert_eq!(v["report"]["error"], Value::Null);
    assert_eq!(v["warnings"], json!([]));
    assert!(v["api_limitations"]
        .as_array()
        .unwrap()
        .contains(&json!(LIMITATION_PROVENANCE_UNKNOWN)));
}

#[tokio::test]
async fn a_failed_report_is_shown_as_unavailable_with_the_reconstruction_kept() {
    let server = explain_api(500, SERVER_ERROR);

    let human = yukti(
        &server.base_url,
        &["reconstruction", "explain", "--case", fixtures::CASE_A_ID],
    )
    .await;
    assert_eq!(human.exit_code, 5);
    assert_contains(
        &human.stdout,
        &[
            "Export safeguard ✕ BLOCKED",
            &format!("JUDICIAL REPORT GET {REPORT_PATH}"),
            "Status ✕ UNAVAILABLE",
        ],
    );
    assert!(
        !human.stdout.contains("dossier"),
        "no dossier is substituted"
    );
    assert_contains(
        &human.stderr,
        &["error: the judicial report is unavailable", "HTTP 500"],
    );

    let r = yukti(
        &server.base_url,
        &[
            "reconstruction",
            "explain",
            "--case",
            fixtures::CASE_A_ID,
            "--json",
        ],
    )
    .await;
    assert_eq!(r.exit_code, 5);
    let v = parse(&r);
    assert_eq!(v["complete"], false);
    assert_eq!(v["report"]["response"], Value::Null);
    assert_eq!(v["report"]["error"]["kind"], "http_server_error");
    assert_eq!(v["report"]["error"]["http_status"], 500);
    assert_eq!(
        v["reconstruction"]["response"],
        reconstruction(fixtures::CASE_A_ID)
    );
}

#[tokio::test]
async fn explain_does_not_request_the_report_when_the_reconstruction_fails() {
    let server = MockServer::start(vec![
        case_route(),
        Route::json(RECON_PATH, 500, SERVER_ERROR).method("GET"),
        Route::json(REPORT_PATH, 200, &report(2).to_string()),
    ]);
    let r = yukti(
        &server.base_url,
        &["reconstruction", "explain", "--case", fixtures::CASE_A_ID],
    )
    .await;
    assert_eq!(r.exit_code, 5);
    assert_eq!(
        calls(&server),
        [call("GET", CASE_PATH), call("GET", RECON_PATH)]
    );
    assert!(r.stdout.is_empty());
}

#[tokio::test]
async fn explain_checks_the_case_first() {
    let server = MockServer::start(vec![
        Route::json(CASE_PATH, 404, fixtures::NOT_FOUND).method("GET"),
        Route::json(
            RECON_PATH,
            200,
            &reconstruction(fixtures::CASE_A_ID).to_string(),
        ),
    ]);
    let r = yukti(
        &server.base_url,
        &["reconstruction", "explain", "--case", fixtures::CASE_A_ID],
    )
    .await;
    assert_eq!(r.exit_code, 4);
    assert_eq!(calls(&server), [call("GET", CASE_PATH)]);
}

#[tokio::test]
async fn a_report_that_disagrees_with_the_reconstruction_is_warned_about() {
    let server = explain_api(200, &report(5).to_string());
    let r = yukti(
        &server.base_url,
        &["reconstruction", "explain", "--case", fixtures::CASE_A_ID],
    )
    .await;
    assert_eq!(r.exit_code, 0);
    assert_contains(
        &r.stderr,
        &["The report counts 5 timeline events but the reconstruction shown has 2"],
    );
}

/* -------------------------------------------------------------------------------- compare */

#[tokio::test]
async fn compare_is_unsupported_and_sends_nothing() {
    let server = MockServer::start(vec![case_route()]);

    let human = yukti(
        &server.base_url,
        &["reconstruction", "compare", "--case", fixtures::CASE_A_ID],
    )
    .await;
    assert_eq!(human.exit_code, 69);
    assert!(human.stdout.is_empty());
    assert_contains(
        &human.stderr,
        &[
            "reconstruction scenario comparison is not yet supported by the YUKTI API",
            "No request was sent to the API",
            "scenario-comparison API",
        ],
    );

    let r = yukti(
        &server.base_url,
        &[
            "reconstruction",
            "compare",
            "--case",
            fixtures::CASE_A_ID,
            "--json",
        ],
    )
    .await;
    assert_eq!(r.exit_code, 69);
    let v = parse(&r);
    assert_eq!(v["error"]["kind"], "unsupported");
    assert_eq!(v["exit_code"], 69);

    assert!(
        server.requests().is_empty(),
        "compare never contacts the API"
    );
}
