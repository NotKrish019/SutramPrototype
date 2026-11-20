//! Member 4 — real producer → reconstruction end-to-end tests (Phase 3E.1).
//!
//! Every step runs the team's real code through the real HTTP router, with no mocks:
//! Member 2 ingestion (multipart upload, SHA-256, immutable raw storage) →
//! Member 1 extraction (`auto_extract=true`, real CDR parser) →
//! Member 4 reconstruction (`POST /api/v1/cases/:id/reconstruction`, which loads the active
//! extraction run from derived storage) → timeline, spatial, contradictions, export gate.
//!
//! The evidence file is the committed synthetic CDR (`data/synthetic/sample_cdr.csv`).
//! The tests check that values the producers do not supply (precision, event timezone and
//! offset, tower coordinates) are labelled as assumptions and block judicial export. They are
//! never presented as observed facts.

use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use http_body_util::BodyExt;
use sha2::{Digest, Sha256};
use std::path::Path;
use sutram_backend_rust::{create_router, AppConfig, EvidenceUploadAndExtractResponse};
use tempfile::tempdir;
use tower::ServiceExt;

const BOUNDARY: &str = "---------------------------m4realproducer";
const CASE_ID: &str = "550e8400-e29b-41d4-a716-4466554400e1";

fn sample_cdr_bytes() -> Vec<u8> {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data")
        .join("synthetic")
        .join("sample_cdr.csv");
    std::fs::read(&p).expect("synthetic sample_cdr.csv")
}

fn multipart(fields: &[(&str, &str)], filename: &str, bytes: &[u8]) -> Vec<u8> {
    let mut body = Vec::new();
    for (name, value) in fields {
        body.extend_from_slice(format!("--{BOUNDARY}\r\n").as_bytes());
        body.extend_from_slice(
            format!("Content-Disposition: form-data; name=\"{name}\"\r\n\r\n").as_bytes(),
        );
        body.extend_from_slice(value.as_bytes());
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{BOUNDARY}\r\n").as_bytes());
    body.extend_from_slice(
        format!("Content-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\n")
            .as_bytes(),
    );
    body.extend_from_slice(b"Content-Type: application/octet-stream\r\n\r\n");
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());
    body
}

async fn upload(app: &Router, extra: &[(&str, &str)]) -> EvidenceUploadAndExtractResponse {
    let mut fields = vec![("case_id", CASE_ID)];
    fields.extend_from_slice(extra);
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/evidence/upload?auto_extract=true")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={BOUNDARY}"),
        )
        .body(Body::from(multipart(
            &fields,
            "calls.csv",
            &sample_cdr_bytes(),
        )))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).expect("upload+extract response")
}

async fn reconstruct(app: &Router) -> serde_json::Value {
    let req = Request::builder()
        .method("POST")
        .uri(format!("/api/v1/cases/{CASE_ID}/reconstruction"))
        .header("content-type", "application/json")
        .body(Body::from("{}"))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

fn kinds(entry: &serde_json::Value) -> Vec<String> {
    entry["assumptions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["kind"].as_str().unwrap().to_string())
        .collect()
}

#[tokio::test]
async fn real_upload_extraction_reconstruction_labels_every_assumption() {
    let temp = tempdir().unwrap();
    let raw_root = temp.path().join("raw");
    let app = create_router(AppConfig {
        evidence_storage_path: raw_root.clone(),
        derived_storage_path: temp.path().join("derived"),
        ..Default::default()
    });

    // 1–3. Member 2 ingestion + Member 1 extraction (real code paths).
    let composite = upload(&app, &[]).await;
    let receipt = &composite.receipt;
    assert!(composite.extraction.valid_records_count > 0);

    // Evidence bytes on disk match the receipt (integrity + size), and are the source bytes.
    let raw_path = raw_root
        .join(CASE_ID)
        .join(&receipt.evidence_id)
        .join(format!("{}.raw", receipt.evidence_id));
    let stored = std::fs::read(&raw_path).expect("raw evidence bytes at documented layout");
    assert_eq!(stored, sample_cdr_bytes());
    assert_eq!(stored.len() as u64, receipt.file_size_bytes);
    assert_eq!(hex::encode(Sha256::digest(&stored)), receipt.sha256_hash);

    // 4–8. Member 4 reconstruction over the active extraction run.
    let result = reconstruct(&app).await;
    let entries = result["timeline"]["entries"].as_array().unwrap();
    assert_eq!(
        entries.len() as u64,
        composite.extraction.valid_records_count as u64
    );

    for entry in entries {
        assert_eq!(entry["source_evidence_id"], receipt.evidence_id.as_str());
        // Raw producer timestamp preserved verbatim next to the corrected window.
        let raw = entry["raw_timestamp"].as_str().unwrap();
        assert!(
            raw.starts_with("2026-11-1") && raw.ends_with("+05:30"),
            "{raw}"
        );
        // Source metadata preserved (not dropped by conversion).
        assert!(entry["metadata"]["cell_tower_id"].is_string());
        assert!(entry["metadata"]["call_type"].is_string());
        // Member 1 (23a915d) supplies description and precision on the candidate.
        assert!(entry["raw_description"]
            .as_str()
            .unwrap()
            .starts_with("Telephony "));
        assert_eq!(entry["time_window"]["raw_precision_input"], "EXACT_SECOND");
        // Not supplied by the producer → labelled, never presented as observed.
        let k = kinds(entry);
        assert!(!k.contains(&"PRECISION_DEFAULTED".to_string()), "{k:?}");
        // Nothing declared at upload: Member 2 now leaves timezone/offset absent, so the engine
        // default applies and is labelled.
        assert!(
            k.contains(&"TIMEZONE_NOT_DECLARED_ON_EVENT".to_string()),
            "{k:?}"
        );
        assert!(k.contains(&"CLOCK_OFFSET_NOT_DECLARED_ON_EVENT".to_string()));
        if !entry["coordinates"].is_null() {
            assert!(k.contains(&"COORDINATES_INFERRED_FROM_TOWER_ID".to_string()));
        }
    }

    // Explicit +05:30 offsets are parsed once (no re-localization): 21:45:12+05:30 = 16:15:12Z.
    let first = entries
        .iter()
        .find(|e| e["event_id"].as_str().unwrap().contains("cdr-rec-001"))
        .expect("cdr-rec-001 reconstructed");
    assert_eq!(
        first["time_window"]["nominal_utc"].as_str().unwrap(),
        "2026-11-10T16:15:12Z"
    );

    // Leads built on assumed values say so.
    for lead in result["contradictions"]["leads"].as_array().unwrap() {
        let limits = lead["limitations"].as_array().unwrap();
        assert!(limits
            .iter()
            .any(|l| l.as_str().unwrap().contains("rests on assumed values")));
    }

    // 9. Export gate.
    assert_eq!(result["export_safeguard"]["status"], "BLOCKED");
    let reasons = result["export_safeguard"]["reasons"].to_string();
    assert!(
        reasons.contains("CLOCK_OFFSET_NOT_DECLARED_ON_EVENT"),
        "{reasons}"
    );
    assert!(!reasons.contains("PRECISION_DEFAULTED"), "{reasons}");
}

#[tokio::test]
async fn declared_clock_offset_is_applied_exactly_once() {
    let temp = tempdir().unwrap();
    let app = create_router(AppConfig {
        evidence_storage_path: temp.path().join("raw"),
        derived_storage_path: temp.path().join("derived"),
        ..Default::default()
    });
    upload(
        &app,
        &[
            ("source_timezone", "Asia/Kolkata"),
            ("clock_offset_seconds", "-300"),
        ],
    )
    .await;
    let result = reconstruct(&app).await;
    let first = result["timeline"]["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["event_id"].as_str().unwrap().contains("cdr-rec-001"))
        .cloned()
        .unwrap();
    // 16:15:12Z − 300 s, once.
    assert_eq!(
        first["time_window"]["nominal_utc"].as_str().unwrap(),
        "2026-11-10T16:10:12Z"
    );
    assert_eq!(first["time_window"]["clock_offset_seconds"], -300.0);
}

#[tokio::test]
async fn repeated_reconstruction_is_deterministic() {
    let temp = tempdir().unwrap();
    let app = create_router(AppConfig {
        evidence_storage_path: temp.path().join("raw"),
        derived_storage_path: temp.path().join("derived"),
        ..Default::default()
    });
    upload(&app, &[]).await;
    // Leads are deterministic since 52953c3 (no wall-clock stamps): compare everything.
    let a = reconstruct(&app).await;
    let b = reconstruct(&app).await;
    assert_eq!(a["timeline"], b["timeline"]);
    assert_eq!(a["spatial"], b["spatial"]);
    assert_eq!(a["contradictions"], b["contradictions"]);
    assert_eq!(a["export_safeguard"], b["export_safeguard"]);
}

#[tokio::test]
async fn real_evidence_to_judicial_report_end_to_end() {
    // Upload (Member 2) → extraction (Member 1) → reconstruction → report (Member 4), real code.
    let temp = tempdir().unwrap();
    let app = create_router(AppConfig {
        evidence_storage_path: temp.path().join("raw"),
        derived_storage_path: temp.path().join("derived"),
        ..Default::default()
    });
    let composite = upload(&app, &[]).await;
    let reconstruction = reconstruct(&app).await;

    let req = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/cases/{CASE_ID}/report"))
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let report: serde_json::Value =
        serde_json::from_slice(&res.into_body().collect().await.unwrap().to_bytes()).unwrap();

    // Evidence inventory is the uploaded exhibit, with the receipt's integrity values.
    let inventory = report["evidence_inventory"].as_array().unwrap();
    assert_eq!(inventory.len(), 1);
    assert_eq!(
        inventory[0]["evidence_id"],
        composite.receipt.evidence_id.as_str()
    );
    assert_eq!(
        inventory[0]["sha256_hash"],
        composite.receipt.sha256_hash.as_str()
    );
    assert_eq!(
        inventory[0]["file_size_bytes"],
        composite.receipt.file_size_bytes
    );

    // The report summarises the same reconstruction the API returned.
    let entries = reconstruction["timeline"]["entries"].as_array().unwrap();
    assert_eq!(
        report["summary"]["total_timeline_events"].as_u64().unwrap() as usize,
        entries.len()
    );
    assert_eq!(
        report["summary"]["open_contradictions_count"]
            .as_u64()
            .unwrap() as usize,
        reconstruction["contradictions"]["leads"]
            .as_array()
            .unwrap()
            .len()
    );

    // Undeclared clock calibration → the report must not clear judicial export.
    assert_eq!(report["judicial_export_status"], "SHIP_EXPORT_BLOCKED");
    assert_eq!(reconstruction["export_safeguard"]["status"], "BLOCKED");
    assert!(report["markdown_dossier"]
        .as_str()
        .unwrap()
        .contains(&composite.receipt.evidence_id));
}

// --- Failure and boundary cases (service level, real producer model) ------------------------

mod service_cases {
    use serde_json::json;
    use sutram_backend_rust::reconstruction::{
        ExportSafeguardStatus, ForensicReconstructionService, ReconstructionOptions,
    };
    use sutram_backend_rust::ExtractedEventCandidate;

    fn candidate(id: &str, ts: &str, metadata: serde_json::Value) -> ExtractedEventCandidate {
        ExtractedEventCandidate {
            candidate_id: id.to_string(),
            source_evidence_id: "ev-1".to_string(),
            source_record_id: format!("rec-{id}"),
            event_type: "TELEPHONY_CALL".to_string(),
            raw_timestamp: ts.to_string(),
            timestamp_precision: None,
            description: None,
            duration_seconds: None,
            location_name: None,
            involved_entity_raw_ids: vec!["+919800000001".to_string()],
            metadata,
        }
    }

    fn opts() -> ReconstructionOptions {
        ReconstructionOptions {
            case_id: Some("case-m4".to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn fully_declared_event_has_no_assumptions_and_export_is_clear() {
        let mut c = candidate(
            "c1",
            "2026-11-10T22:00:00",
            json!({
                "source_timezone": "Asia/Kolkata",
                "clock_offset_seconds": -120.0,
                "coordinates": {"lat": 28.5, "lon": 77.2}
            }),
        );
        // Precision via the typed Member 1 field (23a915d).
        c.timestamp_precision = Some("MINUTE".to_string());
        c.description = Some("Telephony call (test)".to_string());
        let r = ForensicReconstructionService::new()
            .reconstruct_from_event_candidates(&[c], &opts())
            .unwrap();
        let e = &r.timeline.entries[0];
        assert!(e.assumptions.is_empty(), "{:?}", e.assumptions);
        assert_eq!(e.raw_description.as_deref(), Some("Telephony call (test)"));
        assert_eq!(e.raw_timestamp, "2026-11-10T22:00:00");
        // 22:00 IST = 16:30Z, then −120 s once = 16:28Z (minute window floored locally first).
        assert_eq!(
            e.time_window.nominal_utc.to_rfc3339(),
            "2026-11-10T16:28:00+00:00"
        );
        assert_eq!(r.export_safeguard.status, ExportSafeguardStatus::Clear);
    }

    #[test]
    fn tower_name_coordinates_are_labelled_inferred() {
        let c = candidate(
            "c1",
            "2026-11-10T22:00:00+05:30",
            json!({"timestamp_precision": "EXACT_SECOND", "source_timezone": "Asia/Kolkata",
                   "clock_offset_seconds": 0.0, "cell_tower_id": "TOWER-DELHI-SOUTH-04B"}),
        );
        let r = ForensicReconstructionService::new()
            .reconstruct_from_event_candidates(&[c], &opts())
            .unwrap();
        let e = &r.timeline.entries[0];
        assert!(e.coordinates.is_some());
        let kinds: Vec<String> = e
            .assumptions
            .iter()
            .map(|a| {
                serde_json::to_value(a.kind)
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_string()
            })
            .collect();
        assert_eq!(kinds, vec!["COORDINATES_INFERRED_FROM_TOWER_ID"]);
        assert_eq!(r.export_safeguard.status, ExportSafeguardStatus::Blocked);
    }

    /// One bad candidate is excluded with an explicit reason; the rest of the case still
    /// reconstructs, and export is blocked (per-event exclusion from 52953c3).
    fn assert_only_bad_excluded(bad: ExtractedEventCandidate, reason_word: &str) {
        let good = candidate(
            "good",
            "2026-11-10T21:00:00+05:30",
            json!({"timestamp_precision": "EXACT_SECOND", "source_timezone": "Asia/Kolkata",
                   "clock_offset_seconds": 0.0}),
        );
        let r = ForensicReconstructionService::new()
            .reconstruct_from_event_candidates(&[good, bad], &opts())
            .unwrap();
        let kept: Vec<&str> = r
            .timeline
            .entries
            .iter()
            .map(|e| e.event_id.as_str())
            .collect();
        assert_eq!(kept, vec!["good"]);
        let excluded = &r.timeline.excluded_events;
        assert_eq!(excluded.len(), 1);
        assert_eq!(excluded[0].event_id, "c1");
        assert!(
            excluded[0].reason.to_lowercase().contains(reason_word),
            "{}",
            excluded[0].reason
        );
        assert_eq!(r.export_safeguard.status, ExportSafeguardStatus::Blocked);
        assert!(r
            .export_safeguard
            .reasons
            .iter()
            .any(|x| x.contains("excluded")));
    }

    #[test]
    fn unsupported_precision_is_excluded_not_guessed() {
        let c = candidate(
            "c1",
            "2026-11-10T22:00:00+05:30",
            json!({"timestamp_precision": "FORTNIGHT"}),
        );
        assert_only_bad_excluded(c, "precision");
    }

    #[test]
    fn invalid_declared_timezone_is_excluded() {
        let c = candidate(
            "c1",
            "2026-11-10T22:00:00",
            json!({"timestamp_precision": "EXACT_SECOND", "source_timezone": "Mars/Olympus"}),
        );
        assert_only_bad_excluded(c, "timezone");
    }

    #[test]
    fn contradiction_alone_blocks_export() {
        let decl = |lat: f64, lon: f64| {
            json!({"timestamp_precision": "EXACT_SECOND", "source_timezone": "UTC",
                   "clock_offset_seconds": 0.0, "coordinates": {"lat": lat, "lon": lon}})
        };
        let a = candidate("a", "2026-11-10T10:00:00Z", decl(28.6139, 77.2090));
        let b = candidate("b", "2026-11-10T10:10:00Z", decl(19.0760, 72.8777));
        let r = ForensicReconstructionService::new()
            .reconstruct_from_event_candidates(&[a, b], &opts())
            .unwrap();
        assert!(r.timeline.entries.iter().all(|e| e.assumptions.is_empty()));
        assert!(!r.contradictions.leads.is_empty());
        assert_eq!(r.export_safeguard.status, ExportSafeguardStatus::Blocked);
        assert!(r.export_safeguard.reasons[0].contains("contradiction lead"));
    }
}
