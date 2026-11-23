//! The investigator workbench commands against a mock API, through the library entry point:
//! timeline, spatial, contradictions, gaps, provenance, report, graph, whatif, case overview,
//! case list filters, evidence show/extract/reprocess/extraction/runs/verify, config
//! and completions.

mod support;

use std::path::PathBuf;

use clap::Parser;
use serde_json::{json, Value};
use support::{fixtures, flat, unused_base_url, MockServer, Route};
use yukti_cli::cli::Cli;
use yukti_cli::commands::evidence::sha256_hex;
use yukti_cli::config::EnvVars;
use yukti_cli::output::Rendered;
use yukti_cli::ui::{strip_ansi, TerminalEnv};

const CASE_PATH: &str = "/api/v1/cases/3f2b8c1e-7a4d-4e1b-9c3a-5d6e7f8a9b0c";
const TIMELINE_PATH: &str = "/api/v1/cases/3f2b8c1e-7a4d-4e1b-9c3a-5d6e7f8a9b0c/timeline";
const SPATIAL_PATH: &str = "/api/v1/cases/3f2b8c1e-7a4d-4e1b-9c3a-5d6e7f8a9b0c/spatial";
const CONTRA_PATH: &str = "/api/v1/cases/3f2b8c1e-7a4d-4e1b-9c3a-5d6e7f8a9b0c/contradictions";
const GAPS_PATH: &str = "/api/v1/cases/3f2b8c1e-7a4d-4e1b-9c3a-5d6e7f8a9b0c/gaps";
const PROV_PATH: &str =
    "/api/v1/cases/3f2b8c1e-7a4d-4e1b-9c3a-5d6e7f8a9b0c/reconstruction/provenance";
const FINDING_PATH: &str =
    "/api/v1/cases/3f2b8c1e-7a4d-4e1b-9c3a-5d6e7f8a9b0c/findings/lead-0001/provenance";
const REPORT_PATH: &str = "/api/v1/cases/3f2b8c1e-7a4d-4e1b-9c3a-5d6e7f8a9b0c/report";
const RECON_PATH: &str = "/api/v1/cases/3f2b8c1e-7a4d-4e1b-9c3a-5d6e7f8a9b0c/reconstruction";
const EVIDENCE_LIST_PATH: &str = "/api/v1/cases/3f2b8c1e-7a4d-4e1b-9c3a-5d6e7f8a9b0c/evidence";
const GRAPH_SUMMARY_PATH: &str = "/api/v1/cases/3f2b8c1e-7a4d-4e1b-9c3a-5d6e7f8a9b0c/graph/summary";
const NEIGHBORS_PATH: &str = "/api/v1/cases/3f2b8c1e-7a4d-4e1b-9c3a-5d6e7f8a9b0c/graph/entities/%2B919811002233/neighbors?depth=2";
const GRAPH_PATH_PATH: &str =
    "/api/v1/cases/3f2b8c1e-7a4d-4e1b-9c3a-5d6e7f8a9b0c/graph/path?from=a&to=c";
const WHATIF_PATH: &str = "/api/v1/cases/3f2b8c1e-7a4d-4e1b-9c3a-5d6e7f8a9b0c/what-if";
const EVIDENCE_PATH: &str = "/api/v1/evidence/5b1f0c2e-8d3a-4f6b-9e7c-1a2b3c4d5e6f";
const RUNS_PATH: &str = "/api/v1/evidence/5b1f0c2e-8d3a-4f6b-9e7c-1a2b3c4d5e6f/extraction/runs";
const EXTRACT_PATH: &str = "/api/v1/evidence/5b1f0c2e-8d3a-4f6b-9e7c-1a2b3c4d5e6f/extract";
const REPROCESS_PATH: &str = "/api/v1/evidence/5b1f0c2e-8d3a-4f6b-9e7c-1a2b3c4d5e6f/reprocess";
const EXTRACTION_PATH: &str = "/api/v1/evidence/5b1f0c2e-8d3a-4f6b-9e7c-1a2b3c4d5e6f/extraction";

async fn run_with(base_url: &str, args: &[&str], env: &EnvVars) -> Rendered {
    let mut argv = vec!["yukti", "--api-url", base_url];
    argv.extend_from_slice(args);
    let cli = Cli::try_parse_from(argv).expect("valid arguments");
    yukti_cli::execute(&cli, env).await
}

async fn yukti(base_url: &str, args: &[&str]) -> Rendered {
    run_with(base_url, args, &EnvVars::default()).await
}

fn parse(r: &Rendered) -> Value {
    assert!(
        r.stderr.is_empty(),
        "JSON mode writes nothing to stderr: {}",
        r.stderr
    );
    serde_json::from_str(&r.stdout).expect("stdout is one JSON document")
}

fn with_case(mut routes: Vec<Route>) -> MockServer {
    routes.insert(
        0,
        Route::json(CASE_PATH, 200, fixtures::CASE_A).method("GET"),
    );
    MockServer::start(routes)
}

fn assert_has(text: &str, needles: &[&str]) {
    let flat_text = flat(&text.replace('│', " "));
    for n in needles {
        assert!(
            flat_text.contains(&flat(&n.replace('│', " "))),
            "missing {n:?} in\n{text}"
        );
    }
}

fn scratch(name: &str, bytes: &[u8]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("yukti-cli-workbench-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, bytes).unwrap();
    path
}

fn entry(id: &str, utc: &str, ordering: &str, assumptions: Value) -> Value {
    json!({
        "event_id": id,
        "source_evidence_id": "5b1f0c2e-8d3a-4f6b-9e7c-1a2b3c4d5e6f",
        "raw_timestamp": utc,
        "time_window": {
            "nominal_utc": utc, "precision": "EXACT_SECOND",
            "window_start_utc": utc, "window_end_utc": utc,
            "uncertainty_span_seconds": 1.0, "source_timezone": "UTC", "clock_offset_seconds": 0.0
        },
        "raw_description": "Cell tower handover",
        "ordering_status": ordering,
        "assumptions": assumptions
    })
}

fn timeline_body() -> Value {
    json!({
        "total_events": 3,
        "entries": [
            entry("evt-a", "2026-11-10T16:15:12Z", "STRICTLY_ORDERED", json!([])),
            entry("evt-b", "2026-11-10T16:20:12Z", "CONCURRENT_UNCERTAIN", json!([{
                "kind": "TIMEZONE_NOT_DECLARED_ON_EVENT", "field": "source_timezone",
                "assumed_value": "UTC", "basis": "default"
            }])),
            entry("evt-c", "2026-11-10T16:25:12Z", "CONCURRENT_UNCERTAIN", json!([])),
        ],
        "overlaps": [{"a": "evt-b", "b": "evt-c"}],
        "strictly_ordered_count": 1,
        "concurrent_uncertain_count": 2
    })
}

/* ------------------------------------------------------------------------------ timeline */

#[tokio::test]
async fn timeline_draws_a_rail_and_filters_entries() {
    let server = with_case(vec![Route::json(
        TIMELINE_PATH,
        200,
        &timeline_body().to_string(),
    )]);
    let r = yukti(
        &server.base_url,
        &["timeline", "--case", fixtures::CASE_A_ID],
    )
    .await;
    assert_eq!(r.exit_code, 0, "{}{}", r.stdout, r.stderr);
    assert_has(
        &r.stdout,
        &[
            "▌ YUKTI │ TIMELINE",
            "3 events │ 1 strictly ordered │ 2 concurrent/uncertain │ 1 with assumptions",
            "● 2026-11-10T16:15:12Z",
            "◆ 2026-11-10T16:20:12Z",
            "○ 2026-11-10T16:25:12Z",
            "◆ assumed source_timezone = \"UTC\" (TIMEZONE_NOT_DECLARED_ON_EVENT)",
            "1 pair(s) of uncertainty windows overlap",
            "● observed ○ inferred ◇ supplied ◆ assumed ▲ flagged for review",
        ],
    );
    let a = r.stdout.find("16:15:12Z").unwrap();
    let c = r.stdout.find("16:25:12Z").unwrap();
    assert!(a < c, "API order kept");

    let r = yukti(
        &server.base_url,
        &["timeline", "--case", fixtures::CASE_A_ID, "--assumed-only"],
    )
    .await;
    assert_has(&r.stdout, &["1 of 3 shown", "evt-b"]);
    assert!(!r.stdout.contains("event evt-a"), "{}", r.stdout);

    let v = parse(
        &yukti(
            &server.base_url,
            &[
                "timeline",
                "--case",
                fixtures::CASE_A_ID,
                "--uncertain-only",
                "--limit",
                "1",
                "--json",
            ],
        )
        .await,
    );
    assert_eq!(v["schema"], "yukti.cli.timeline/v1");
    assert_eq!(v["shown_count"], 1);
    assert_eq!(v["entries"][0]["event_id"], "evt-b");
    assert_eq!(
        v["response"],
        timeline_body(),
        "complete response passed through"
    );
}

#[tokio::test]
async fn views_check_the_case_first() {
    let server = MockServer::start(vec![
        Route::json(CASE_PATH, 404, fixtures::NOT_FOUND),
        Route::json(TIMELINE_PATH, 200, &timeline_body().to_string()),
    ]);
    for cmd in [
        "timeline",
        "spatial",
        "contradictions",
        "gaps",
        "provenance",
        "report",
    ] {
        let r = yukti(&server.base_url, &[cmd, "--case", fixtures::CASE_A_ID]).await;
        assert_eq!(r.exit_code, 4, "{cmd}");
        assert!(
            r.stderr.contains("HTTP 404 (NOT_FOUND)"),
            "{cmd}: {}",
            r.stderr
        );
    }
    assert!(server.requests().iter().all(|r| r.path == CASE_PATH));
    let bad = yukti(&server.base_url, &["gaps", "--case", "CASE-026"]).await;
    assert_eq!(bad.exit_code, 2);
}

/* ------------------------------------------------------------------- spatial, contradictions */

fn spatial_body() -> Value {
    json!({
        "trajectories": [{
            "entity_id": "+919811002233",
            "waypoints": [{}, {}],
            "segments": [{
                "origin_event_id": "evt-a", "destination_event_id": "evt-b",
                "transit_metrics": {
                    "distance_km": 1153.2, "min_required_speed_kmh": 13931.5,
                    "configured_speed_threshold_kmh": 900.0,
                    "exceeds_configured_speed_threshold": true,
                    "threshold_evaluation": "EXCEEDS_CONFIGURED_THRESHOLD"
                },
                "has_intervening_unlocated_events": true
            }],
            "total_distance_km": 1153.2,
            "max_min_required_speed_kmh": 13931.5
        }],
        "overlaps": []
    })
}

fn contradictions_body() -> Value {
    json!({
        "leads": [{
            "lead_id": "lead-0001", "lead_type": "SPATIAL_TRANSIT_IMPOSSIBLE",
            "entity_id": "+919811002233", "event_a_id": "evt-a", "event_b_id": "evt-b",
            "neutral_explanation": "Transit would require a speed above the threshold.",
            "limitations": ["Tower-based coordinates may be offset."],
            "generated_at": "2026-11-21T10:00:00Z"
        }],
        "total_evaluated_segments": 1,
        "flagged_segments_count": 1,
        "disclaimer": "Leads are not conclusive proof."
    })
}

#[tokio::test]
async fn spatial_and_contradictions_render_as_leads_not_findings() {
    let server = with_case(vec![
        Route::json(SPATIAL_PATH, 200, &spatial_body().to_string()),
        Route::json(CONTRA_PATH, 200, &contradictions_body().to_string()),
    ]);
    let r = yukti(
        &server.base_url,
        &["spatial", "--case", fixtures::CASE_A_ID],
    )
    .await;
    assert_eq!(r.exit_code, 0, "{}", r.stderr);
    assert_has(
        &r.stdout,
        &[
            "1 trajectories │ 1 segments │ 1153.20 km │ 1 over threshold",
            "├─ evt-a → evt-b 1153.200 km",
            "min required 13931.500 km/h vs threshold 900 km/h: EXCEEDS_CONFIGURED_THRESHOLD",
            "(intervening unlocated events)",
            "a lead to review, not proof of impossibility",
        ],
    );

    let r = yukti(
        &server.base_url,
        &["contradictions", "--case", fixtures::CASE_A_ID],
    )
    .await;
    assert_eq!(r.exit_code, 0);
    assert_has(
        &r.stdout,
        &[
            "1 leads │ 1 segments evaluated │ 1 flagged",
            "▲ SPATIAL_TRANSIT_IMPOSSIBLE lead-0001",
            "investigative leads, not findings",
            "◆ Tower-based coordinates may be offset.",
            "Disclaimer: Leads are not conclusive proof.",
            "yukti provenance --case 3f2b8c1e-7a4d-4e1b-9c3a-5d6e7f8a9b0c --finding lead-0001",
        ],
    );
    let v = parse(
        &yukti(
            &server.base_url,
            &["contradictions", "--case", fixtures::CASE_A_ID, "--json"],
        )
        .await,
    );
    assert_eq!(v["schema"], "yukti.cli.contradictions/v1");
    assert_eq!(v["response"], contradictions_body());
    assert_eq!(v["request"], json!({"method": "GET", "path": CONTRA_PATH}));
}

/* ---------------------------------------------------------------------------------- gaps */

fn gap(id: &str, severity: &str) -> Value {
    json!({
        "gap_id": id, "gap_type": "EVENT_WITHOUT_COORDINATES", "severity": severity,
        "subject_type": "EVENT", "subject_id": "evt-c", "title": format!("Gap {id}"),
        "explanation": "No coordinates.", "impact_on_reconstruction": "Cannot place the event.",
        "suggested_corroborating_evidence": "CCTV or tower dump.", "rule_id": "R-GAP-03",
        "is_confirmed_missing": false
    })
}

#[tokio::test]
async fn gaps_sort_by_severity_and_filter() {
    let body = json!({
        "case_id": fixtures::CASE_A_ID, "reconstruction_run_id": "run-9", "audited_at": "t",
        "total_gaps_count": 3,
        "gaps_by_severity": {"LOW": 1, "HIGH": 1, "CRITICAL": 1},
        "gaps_by_type": {"EVENT_WITHOUT_COORDINATES": 3},
        "gaps": [gap("g-low", "LOW"), gap("g-crit", "CRITICAL"), gap("g-high", "HIGH")]
    });
    let server = with_case(vec![Route::json(GAPS_PATH, 200, &body.to_string())]);
    let r = yukti(&server.base_url, &["gaps", "--case", fixtures::CASE_A_ID]).await;
    assert_eq!(r.exit_code, 0);
    let crit = r.stdout.find("g-crit").unwrap();
    let high = r.stdout.find("g-high").unwrap();
    let low = r.stdout.find("g-low").unwrap();
    assert!(
        crit < high && high < low,
        "most severe first:\n{}",
        r.stdout
    );
    assert_has(
        &r.stdout,
        &[
            "3 gaps │ 1 critical │ 1 high │ 0 medium │ 1 low",
            "corroborate CCTV or tower dump.",
            "EVENT evt-c · rule R-GAP-03 · not yet supplied",
        ],
    );

    let v = parse(
        &yukti(
            &server.base_url,
            &[
                "gaps",
                "--case",
                fixtures::CASE_A_ID,
                "--min-severity",
                "high",
                "--json",
            ],
        )
        .await,
    );
    assert_eq!(v["schema"], "yukti.cli.gaps/v1");
    assert_eq!(v["min_severity"], "HIGH");
    assert_eq!(v["shown_count"], 2);
    assert_eq!(v["gaps"][0]["gap_id"], "g-crit", "API order kept in JSON");
    assert_eq!(v["response"], body);
}

/* ---------------------------------------------------------------------------- provenance */

fn trace(integrity: &str) -> Value {
    json!({
        "finding_id": "lead-0001", "finding_type": "CONTRADICTION_LEAD",
        "reconstruction_run_id": "run-9", "epistemic_classification": "INFERRED",
        "is_hypothetical": false, "claim_summary": "Transit is implausible.",
        "source_anchors": [{
            "event_id": "evt-a", "evidence_id": "5b1f0c2e-8d3a-4f6b-9e7c-1a2b3c4d5e6f",
            "original_filename": "cdr.csv", "evidence_sha256": "ab".repeat(32),
            "integrity_status": integrity, "source_record_id": "row-4",
            "original_source_timestamp": "10/11/2026 21:45:12"
        }],
        "derived_inferences": {"required_speed_kmh": 13931.5},
        "missing_provenance_fields": [],
        "audited_at": "2026-11-21T10:00:00Z"
    })
}

#[tokio::test]
async fn provenance_traces_findings_to_hashed_sources() {
    let report = json!({
        "case_id": fixtures::CASE_A_ID, "reconstruction_run_id": "run-9", "audited_at": "t",
        "total_findings_count": 2, "intact_provenance_count": 1, "flagged_provenance_count": 1,
        "findings": [trace("VERIFIED_MATCH"), trace("HASH_MISMATCH")]
    });
    let server = with_case(vec![
        Route::json(PROV_PATH, 200, &report.to_string()),
        Route::json(FINDING_PATH, 200, &trace("VERIFIED_MATCH").to_string()),
    ]);
    let r = yukti(
        &server.base_url,
        &["provenance", "--case", fixtures::CASE_A_ID],
    )
    .await;
    assert_eq!(r.exit_code, 0);
    assert_has(
        &r.stdout,
        &[
            "2 findings │ 1 intact │ 1 flagged",
            "○ lead-0001 CONTRADICTION_LEAD INFERRED",
            "✓ PROVENANCE INTACT",
            "✕ PROVENANCE FLAGGED",
            "✕ HASH_MISMATCH",
            "evidence 5b1f0c2e… (cdr.csv)",
            "raw \"10/11/2026 21:45:12\" · record row-4 · sha256 abababababababab…",
            "○ required_speed_kmh 13931.5",
        ],
    );

    let r = yukti(
        &server.base_url,
        &[
            "provenance",
            "--case",
            fixtures::CASE_A_ID,
            "--finding",
            "lead-0001",
        ],
    )
    .await;
    assert_eq!(r.exit_code, 0);
    assert_has(&r.stdout, &["FINDING TRACE", "✓ VERIFIED_MATCH"]);
    assert_eq!(server.requests().last().unwrap().path, FINDING_PATH);
}

/* -------------------------------------------------------------------------------- report */

fn report_body() -> Value {
    json!({
        "report_id": "rep-1", "case_id": fixtures::CASE_A_ID, "title": "Warehouse burglary",
        "generated_at": "2026-11-21T10:05:00Z",
        "judicial_export_status": "SHIP_EXPORT_BLOCKED",
        "judicial_export_reason": "1 unresolved lead.",
        "summary": {"evidence_count": 2, "total_timeline_events": 3, "excluded_events_count": 0, "open_contradictions_count": 1},
        "forensic_disclaimer": "Leads, not conclusions.",
        "markdown_dossier": "# Dossier\n\nBody.\n"
    })
}

#[tokio::test]
async fn report_exports_the_dossier_with_its_hash_and_never_overwrites() {
    let server = with_case(vec![Route::json(
        REPORT_PATH,
        200,
        &report_body().to_string(),
    )]);
    let r = yukti(&server.base_url, &["report", "--case", fixtures::CASE_A_ID]).await;
    assert_eq!(r.exit_code, 0);
    assert_has(
        &r.stdout,
        &[
            "✕ SHIP_EXPORT_BLOCKED",
            "2 exhibits",
            "1 open leads",
            "# Dossier",
            "end of dossier",
        ],
    );

    let dir = std::env::temp_dir().join(format!("yukti-cli-report-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.join("dossier.md");
    let _ = std::fs::remove_file(&out);
    let out_s = out.to_str().unwrap();
    let v = parse(
        &yukti(
            &server.base_url,
            &[
                "report",
                "--case",
                fixtures::CASE_A_ID,
                "--out",
                out_s,
                "--json",
            ],
        )
        .await,
    );
    assert_eq!(v["schema"], "yukti.cli.report/v1");
    assert_eq!(
        std::fs::read_to_string(&out).unwrap(),
        "# Dossier\n\nBody.\n"
    );
    assert_eq!(v["written"]["sha256"], sha256_hex(b"# Dossier\n\nBody.\n"));
    assert_eq!(v["written"]["format"], "markdown");

    let again = yukti(
        &server.base_url,
        &["report", "--case", fixtures::CASE_A_ID, "--out", out_s],
    )
    .await;
    assert_eq!(again.exit_code, 2);
    assert!(
        again.stderr.contains("already exists; pass --force"),
        "{}",
        again.stderr
    );

    let forced = yukti(
        &server.base_url,
        &[
            "report",
            "--case",
            fixtures::CASE_A_ID,
            "--out",
            out_s,
            "--format",
            "json",
            "--force",
        ],
    )
    .await;
    assert_eq!(forced.exit_code, 0, "{}", forced.stderr);
    let written: Value = serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
    assert_eq!(written, report_body());
    assert_has(&forced.stdout, &["EXPORTED json", "SHA-256 ✓"]);
    std::fs::remove_dir_all(&dir).unwrap();
}

/* --------------------------------------------------------------------------------- graph */

#[tokio::test]
async fn graph_summary_neighbors_and_path() {
    let summary = json!({
        "case_id": fixtures::CASE_A_ID, "evidence_count": 2, "extraction_run_count": 3,
        "entity_candidate_count": 7, "event_candidate_count": 12, "relationship_assertion_count": 9,
        "reconstruction_run_count": 1, "contradiction_lead_count": 1,
        "graph_synchronization_status": "SYNCING (2 pending)",
        "last_projection_time": null, "identifier_privacy": "hmac_sha256"
    });
    let neighbors = json!({
        "case_id": fixtures::CASE_A_ID, "root_entity_id": "+919811002233", "depth": 2,
        "nodes": [
            {"id": "n1", "label": "EntityCandidate", "properties": {"canonical_identifier": "+919811002233"}},
            {"id": "n2", "label": "EventCandidate", "properties": {"event_type": "CALL"}}
        ],
        "relationships": [{"id": "r1", "source": "n1", "target": "n2", "relation_type": "PARTICIPATED_IN", "properties": {}}]
    });
    let path = json!({
        "case_id": fixtures::CASE_A_ID, "from_entity_id": "a", "to_entity_id": "c", "found": true,
        "nodes": [
            {"id": "a", "label": "EntityCandidate", "properties": {"name": "Alpha"}},
            {"id": "b", "label": "EventCandidate", "properties": {}},
            {"id": "c", "label": "EntityCandidate", "properties": {"name": "Charlie"}}
        ],
        "relationships": [
            {"id": "r1", "source": "a", "target": "b", "relation_type": "CALLED", "properties": {}},
            {"id": "r2", "source": "b", "target": "c", "relation_type": "RECEIVED", "properties": {}}
        ]
    });
    let server = with_case(vec![
        Route::json(GRAPH_SUMMARY_PATH, 200, &summary.to_string()),
        Route::json(NEIGHBORS_PATH, 200, &neighbors.to_string()),
        Route::json(GRAPH_PATH_PATH, 200, &path.to_string()),
    ]);
    let r = yukti(
        &server.base_url,
        &["graph", "summary", "--case", fixtures::CASE_A_ID],
    )
    .await;
    assert_eq!(r.exit_code, 0);
    assert_has(
        &r.stdout,
        &[
            "◐ SYNCING (2 PENDING)",
            "7 entities │ 12 events │ 9 relationships",
            "Identifier privacy hmac_sha256",
        ],
    );

    let r = yukti(
        &server.base_url,
        &[
            "graph",
            "neighbors",
            "--case",
            fixtures::CASE_A_ID,
            "--entity",
            "+919811002233",
            "--depth",
            "2",
        ],
    )
    .await;
    assert_eq!(r.exit_code, 0, "{}", r.stderr);
    assert_has(
        &r.stdout,
        &[
            "2 nodes │ 1 relationships",
            "○ +919811002233 PARTICIPATED_IN → CALL",
            "not proof of identity",
        ],
    );

    let r = yukti(
        &server.base_url,
        &[
            "graph",
            "path",
            "--case",
            fixtures::CASE_A_ID,
            "--from",
            "a",
            "--to",
            "c",
        ],
    )
    .await;
    assert_eq!(r.exit_code, 0);
    assert_has(
        &r.stdout,
        &[
            "✓ PATH FOUND",
            "a → c · 2 hop(s)",
            "● Alpha",
            "CALLED →",
            "RECEIVED →",
            "● Charlie",
        ],
    );
    let v = parse(
        &yukti(
            &server.base_url,
            &[
                "graph",
                "path",
                "--case",
                fixtures::CASE_A_ID,
                "--from",
                "a",
                "--to",
                "c",
                "--json",
            ],
        )
        .await,
    );
    assert_eq!(v["schema"], "yukti.cli.graph_path/v1");
    assert_eq!(v["request"]["path"], GRAPH_PATH_PATH);
}

/* -------------------------------------------------------------------------------- whatif */

#[tokio::test]
async fn whatif_sends_only_the_given_assumptions_and_labels_the_result_hypothetical() {
    let response = json!({
        "baseline_run_id": "run-9", "scenario_run_id": "sim-1", "scenario_name": "Clock skew",
        "is_hypothetical": true, "simulated_at": "2026-11-21T11:00:00Z",
        "assumptions_applied": {"default_clock_offset_seconds": 120.0},
        "comparison": {
            "findings_unchanged": [],
            "findings_newly_appearing": [],
            "findings_disappearing": [{"finding_id": "lead-0001", "finding_type": "SPATIAL_TRANSIT_IMPOSSIBLE", "entity_id": "e", "event_ids": ["evt-a", "evt-b"], "summary": "Resolves.", "details": {}}],
            "findings_robustness_changed": [],
            "applied_assumptions_summary": ["Default clock offset 120 s"],
            "explanations": ["Shifting the clock widens the transit window."]
        },
        "simulated_reconstruction": {}
    });
    let server = with_case(vec![
        Route::json(WHATIF_PATH, 200, &response.to_string()).method("POST")
    ]);
    let r = yukti(
        &server.base_url,
        &[
            "whatif",
            "--case",
            fixtures::CASE_A_ID,
            "--name",
            "Clock skew",
            "--clock-offset",
            "120",
        ],
    )
    .await;
    assert_eq!(r.exit_code, 0, "{}", r.stderr);
    let post = server
        .requests()
        .into_iter()
        .find(|q| q.method == "POST")
        .unwrap();
    let body: Value = serde_json::from_str(&post.body).unwrap();
    assert_eq!(
        body,
        json!({"scenario_name": "Clock skew", "assumptions": {"default_clock_offset_seconds": 120.0}})
    );
    assert_has(
        &r.stdout,
        &[
            "◐ HYPOTHETICAL Scenario \"Clock skew\"",
            "◆ Default clock offset 120 s",
            "+0 appearing │ -1 disappearing │ ~0 changed │ =0 unchanged",
            "○ - SPATIAL_TRANSIT_IMPOSSIBLE lead-0001",
            "0 of 1 baseline finding(s) persist",
            "not what happened",
        ],
    );

    let none = yukti(&server.base_url, &["whatif", "--case", fixtures::CASE_A_ID]).await;
    assert_eq!(
        none.exit_code, 2,
        "an empty scenario is refused before any request"
    );
}

/* ------------------------------------------------------------------- case overview, list */

#[tokio::test]
async fn case_overview_combines_every_section_and_reports_missing_ones() {
    let recon = json!({
        "case_id": fixtures::CASE_A_ID,
        "timeline": timeline_body(),
        "spatial": spatial_body(),
        "contradictions": contradictions_body(),
        "export_safeguard": {"status": "BLOCKED", "reasons": ["Unresolved lead."]}
    });
    let gaps = json!({"total_gaps_count": 1, "gaps_by_severity": {"HIGH": 1}, "gaps_by_type": {}, "gaps": [gap("g-1", "HIGH")]});
    let evidence = format!("[{}]", fixtures::RECEIPT_CSV);
    let server = with_case(vec![
        Route::json(EVIDENCE_LIST_PATH, 200, &evidence),
        Route::json(RECON_PATH, 200, &recon.to_string()),
        Route::json(GAPS_PATH, 200, &gaps.to_string()),
        Route::json(
            GRAPH_SUMMARY_PATH,
            503,
            r#"{"error":{"code":"GRAPH_UNAVAILABLE","message":"Neo4j down"}}"#,
        ),
    ]);
    let r = yukti(
        &server.base_url,
        &["case", "overview", "--case", fixtures::CASE_A_ID],
    )
    .await;
    assert_eq!(
        r.exit_code, 5,
        "a missing section makes the brief incomplete"
    );
    assert_has(
        &r.stdout,
        &[
            "Warehouse burglary ✓ ACTIVE",
            "FIR FIR-112/2026",
            "1 exhibits",
            "Source types TABULAR_CSV 1",
            "3 events │ 0 excluded │ 1 assumptions │ 1 leads",
            "Export safeguard ✕ BLOCKED",
            "▲ SPATIAL_TRANSIT_IMPOSSIBLE evt-a → evt-b",
            "1 gaps │ 0 critical │ 1 high",
            "▲ HIGH Gap g-1",
            "KNOWLEDGE GRAPH",
            "Unavailable",
        ],
    );
    assert!(
        r.stderr
            .contains("warning: The graph summary section is unavailable"),
        "{}",
        r.stderr
    );

    let v = parse(
        &yukti(
            &server.base_url,
            &["case", "overview", "--case", fixtures::CASE_A_ID, "--json"],
        )
        .await,
    );
    assert_eq!(v["schema"], "yukti.cli.case_overview/v1");
    assert_eq!(v["complete"], false);
    assert_eq!(v["reconstruction"]["response"], recon);
    assert_eq!(v["graph"]["error"]["http_status"], 503);
}

#[tokio::test]
async fn case_list_filters_on_the_client_side() {
    let body = format!("[{},{}]", fixtures::CASE_A, fixtures::CASE_B);
    let server = MockServer::start(vec![
        Route::json("/api/v1/cases", 200, &body).method("GET"),
        Route::json("/health/ready", 200, fixtures::READY),
    ]);
    let v = parse(
        &yukti(
            &server.base_url,
            &["case", "list", "--search", "atm", "--json"],
        )
        .await,
    );
    assert_eq!(v["count"], 1);
    assert_eq!(v["total_count"], 2);
    assert_eq!(v["cases"][0]["case_id"], fixtures::CASE_B_ID);
    assert_eq!(v["filters"]["search"], "atm");

    let r = yukti(&server.base_url, &["case", "list", "--status", "active"]).await;
    assert!(r.stdout.contains("Warehouse burglary") && !r.stdout.contains("ATM skimming"));
    assert_has(&r.stdout, &["1 of 2 case(s) match the filters"]);

    let r = yukti(
        &server.base_url,
        &["case", "list", "--search", "nothing-matches"],
    )
    .await;
    assert_has(
        &r.stdout,
        &["No cases match the filters (2 returned by the API)."],
    );
    let bad = yukti(&server.base_url, &["case", "list", "--limit", "0"]).await;
    assert_eq!(bad.exit_code, 2);
}

/* ----------------------------------------------------------------------------- evidence */

fn csv_receipt(bytes: &[u8]) -> Value {
    let mut v: Value = serde_json::from_str(fixtures::RECEIPT_CSV).unwrap();
    v["sha256_hash"] = json!(sha256_hex(bytes));
    v["file_size_bytes"] = json!(bytes.len());
    v
}

fn run_manifest(status: &str) -> Value {
    json!({
        "extraction_run_id": "run-0002", "evidence_id": fixtures::RECEIPT_CSV_ID,
        "case_id": fixtures::CASE_A_ID, "source_sha256": "ab".repeat(32),
        "extractor_name": "cdr", "extractor_version": "1.2.0", "configuration_hash": "cfg",
        "configuration": {}, "started_at": "2026-11-21T10:00:00Z",
        "completed_at": "2026-11-21T10:00:01Z", "duration_ms": 812.0, "status": status,
        "validation_issue_count": 1, "records_processed": 10, "valid_records_count": 9,
        "rejected_records_count": 1, "result_sha256": "cd".repeat(32)
    })
}

#[tokio::test]
async fn evidence_verify_compares_a_local_copy_with_the_recorded_hash() {
    let original = b"caller,callee,start\n+91,+91,2026-11-10T16:15:12Z\n";
    let server = MockServer::start(vec![Route::json(
        EVIDENCE_PATH,
        200,
        &csv_receipt(original).to_string(),
    )]);
    let good = scratch("good.csv", original);
    let r = yukti(
        &server.base_url,
        &[
            "evidence",
            "verify",
            "--evidence",
            fixtures::RECEIPT_CSV_ID,
            good.to_str().unwrap(),
        ],
    )
    .await;
    assert_eq!(r.exit_code, 0, "{}{}", r.stdout, r.stderr);
    assert_has(
        &r.stdout,
        &["✓ VERIFIED", "byte-identical", "✓ VERIFIED MATCH"],
    );

    let tampered = scratch(
        "tampered.csv",
        b"caller,callee,start\n+91,+92,2026-11-10T16:15:12Z\n",
    );
    let r = yukti(
        &server.base_url,
        &[
            "evidence",
            "verify",
            "--evidence",
            fixtures::RECEIPT_CSV_ID,
            tampered.to_str().unwrap(),
        ],
    )
    .await;
    assert_eq!(r.exit_code, 7);
    assert_has(
        &r.stdout,
        &["✕ MISMATCH", "does NOT match", "✕ HASH MISMATCH"],
    );
    let v = parse(
        &yukti(
            &server.base_url,
            &[
                "evidence",
                "verify",
                "--evidence",
                fixtures::RECEIPT_CSV_ID,
                tampered.to_str().unwrap(),
                "--json",
            ],
        )
        .await,
    );
    assert_eq!(v["schema"], "yukti.cli.evidence_verify/v1");
    assert_eq!(v["hash_match"], false);
    assert_eq!(v["size_match"], true);
}

#[tokio::test]
async fn ingest_reports_a_transfer_hash_mismatch_as_exit_seven() {
    let bytes = b"caller,callee\n1,2\n";
    let path = scratch("mismatch.csv", bytes);
    let mut receipt = csv_receipt(bytes);
    receipt["sha256_hash"] = json!("00".repeat(32));
    let server = with_case(vec![Route::json(
        "/api/v1/evidence/upload",
        201,
        &receipt.to_string(),
    )]);
    let r = yukti(
        &server.base_url,
        &[
            "evidence",
            "ingest",
            "--case",
            fixtures::CASE_A_ID,
            path.to_str().unwrap(),
        ],
    )
    .await;
    assert_eq!(r.exit_code, 7);
    assert!(r.stderr.contains("warning: INTEGRITY"), "{}", r.stderr);
    assert_has(&r.stdout, &["✕ HASH MISMATCH"]);
}

#[tokio::test]
async fn evidence_show_runs_extract_reprocess_and_extraction() {
    let original = b"x\n";
    let runs = json!([run_manifest("SUCCESS"), run_manifest("PARTIAL_SUCCESS")]);
    let extraction = json!({
        "evidence_id": fixtures::RECEIPT_CSV_ID, "case_id": fixtures::CASE_A_ID,
        "source_type": "CDR", "status": "SUCCESS",
        "identifiers": [{}], "resolved_entities": [{"canonical_identifier": "+919811002233", "identifier_type": "MSISDN", "raw_mentions": ["a", "b"]}],
        "event_candidates": [{"candidate_id": "c1", "source_evidence_id": "e", "source_record_id": "row-1", "event_type": "CALL", "raw_timestamp": "10/11/2026 21:45:12"}],
        "relationship_candidates": [],
        "validation_issues": [{"field_name": "duration", "issue_type": "UNPARSEABLE", "message": "not a number"}],
        "total_records_processed": 2, "valid_records_count": 1, "rejected_records_count": 1,
        "processing_time_ms": 4.0, "extraction_metadata": {}
    });
    let server = MockServer::start(vec![
        Route::json(EVIDENCE_PATH, 200, &csv_receipt(original).to_string()),
        Route::json(RUNS_PATH, 200, &runs.to_string()),
        Route::json(EXTRACT_PATH, 200, &run_manifest("SUCCESS").to_string()).method("POST"),
        Route::json(REPROCESS_PATH, 200, &run_manifest("FAILED").to_string()).method("POST"),
        Route::json(EXTRACTION_PATH, 200, &extraction.to_string()),
    ]);
    let ev = fixtures::RECEIPT_CSV_ID;

    let r = yukti(&server.base_url, &["evidence", "show", "--evidence", ev]).await;
    assert_eq!(r.exit_code, 0, "{}", r.stderr);
    assert_has(
        &r.stdout,
        &[
            "cdr.csv ✓ INGESTED",
            "CHAIN OF CUSTODY",
            "EXTRACTION RUNS 2 run(s)",
            "◐ PARTIAL_SUCCESS",
        ],
    );

    let r = yukti(&server.base_url, &["evidence", "runs", "--evidence", ev]).await;
    assert_has(&r.stdout, &["run-0002", "cdr 1.2.0", "never edits one"]);

    let r = yukti(&server.base_url, &["evidence", "extract", "--evidence", ev]).await;
    assert_eq!(r.exit_code, 0);
    assert_has(
        &r.stdout,
        &[
            "✓ SUCCESS",
            "10 processed, 9 valid · 1 rejected · 1 validation issue(s)",
            "Extractor cdr 1.2.0",
        ],
    );

    let r = yukti(
        &server.base_url,
        &[
            "evidence",
            "reprocess",
            "--evidence",
            ev,
            "--source-timezone",
            "Asia/Kolkata",
            "--clock-offset-seconds",
            "-30",
            "--default-country-code",
            "91",
        ],
    )
    .await;
    assert_eq!(r.exit_code, 5, "a failed run is not success");
    let post = server
        .requests()
        .into_iter()
        .rfind(|q| q.path == REPROCESS_PATH)
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&post.body).unwrap(),
        json!({"source_timezone": "Asia/Kolkata", "clock_offset_seconds": -30.0, "default_country_code": "91"})
    );
    assert_has(&r.stdout, &["DECLARED FOR THIS RUN", "◇ Asia/Kolkata"]);

    for (args, expected) in [
        (
            vec!["evidence", "reprocess", "--evidence", ev],
            "at least one of",
        ),
        (
            vec![
                "evidence",
                "reprocess",
                "--evidence",
                ev,
                "--default-country-code",
                "+91",
            ],
            "digits only",
        ),
        (
            vec![
                "evidence",
                "reprocess",
                "--evidence",
                ev,
                "--clock-offset-seconds",
                "NaN",
            ],
            "--clock-offset-seconds",
        ),
    ] {
        let r = yukti(&server.base_url, &args).await;
        assert_eq!(r.exit_code, 2, "{args:?}");
        assert!(r.stderr.contains(expected), "{args:?}: {}", r.stderr);
    }

    let r = yukti(
        &server.base_url,
        &["evidence", "extraction", "--evidence", ev],
    )
    .await;
    assert_eq!(r.exit_code, 0);
    assert_has(
        &r.stdout,
        &[
            "1 events │ 1 identifiers │ 1 entities │ 0 relationships │ 1 issues",
            "● 10/11/2026 21:45:12",
            "inferred equivalence, not identity",
            "○ +919811002233 MSISDN · 2 mention(s)",
            "◆ UNPARSEABLE.duration not a number",
        ],
    );
}

/* ----------------------------------------------------------------------- config, colour */

#[tokio::test]
async fn config_shows_sources_without_contacting_the_api() {
    let env = EnvVars {
        timeout_secs: Some("30".into()),
        case_id: Some(fixtures::CASE_A_ID.into()),
        ..EnvVars::default()
    };
    let r = run_with(&unused_base_url(), &["config"], &env).await;
    assert_eq!(r.exit_code, 0);
    assert_has(
        &r.stdout,
        &[
            "API URL",
            "· --api-url",
            "30 s · YUKTI_TIMEOUT_SECS",
            fixtures::CASE_A_ID,
        ],
    );
    let v = parse(&run_with(&unused_base_url(), &["config", "--json"], &env).await);
    assert_eq!(v["schema"], "yukti.cli.config/v1");
    assert_eq!(v["timeout_secs"]["source"], "YUKTI_TIMEOUT_SECS");
    assert_eq!(v["working_case"], fixtures::CASE_A_ID);
}

#[tokio::test]
async fn colour_is_emitted_only_for_terminals_and_strips_to_the_plain_layout() {
    let server = MockServer::degraded_api();
    let tty = EnvVars {
        terminal: TerminalEnv {
            stdout_tty: true,
            stderr_tty: true,
            colorterm: Some("truecolor".into()),
            columns: Some(100),
            ..TerminalEnv::default()
        },
        ..EnvVars::default()
    };
    let coloured = run_with(&server.base_url, &["status"], &tty).await;
    let plain = yukti(&server.base_url, &["status"]).await;
    assert!(
        coloured.stdout.contains("\x1b[1;38;2;229;36;59m"),
        "brand crimson"
    );
    assert!(!plain.stdout.contains('\x1b'));
    assert_eq!(strip_ansi(&coloured.stdout), plain.stdout);

    let no_color = EnvVars {
        terminal: TerminalEnv {
            no_color: true,
            ..tty.terminal.clone()
        },
        ..EnvVars::default()
    };
    assert!(!run_with(&server.base_url, &["status"], &no_color)
        .await
        .stdout
        .contains('\x1b'));
    let ascii = yukti(&server.base_url, &["status", "--ascii"]).await;
    assert!(ascii.stdout.is_ascii(), "{}", ascii.stdout);
    let json = yukti(&server.base_url, &["status", "--json", "--color", "always"]).await;
    assert!(!json.stdout.contains('\x1b'), "JSON is never coloured");
}

#[tokio::test]
async fn completions_are_generated_offline() {
    let r = yukti(&unused_base_url(), &["completions", "zsh"]).await;
    assert_eq!(r.exit_code, 0);
    assert!(r.stdout.contains("#compdef yukti"));
    for cmd in ["timeline", "whatif", "provenance", "doctor"] {
        assert!(r.stdout.contains(cmd), "{cmd}");
    }
}
