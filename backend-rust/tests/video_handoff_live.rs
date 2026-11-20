//! Member 4 — live video frame handoff, evidence → judicial report (Phase 3E.3).
//!
//! Real code paths only: a synthetic MP4 (generated with FFmpeg `testsrc`) is uploaded through
//! the real multipart route (Member 2), reconstruction extracts keyframes with real FFmpeg,
//! anchors them only from declared calibration, hands them to Member 1's real consumer
//! (`extraction::consume_video_frames`), and the judicial report lists the frame-derived events
//! with provenance. No recording fakes.

use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use http_body_util::BodyExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use sutram_backend_rust::reconstruction::parse_showinfo_pts_times;
use sutram_backend_rust::{create_router, AppConfig, EvidenceUploadReceipt};
use tempfile::{tempdir, TempDir};
use tower::ServiceExt;

const BOUNDARY: &str = "---------------------------m4videohandoff";
const CASE_ID: &str = "550e8400-e29b-41d4-a716-4466554400f1";

/// These tests verify real video integration, so a missing FFmpeg is a failure, not a skip.
fn require_ffmpeg() {
    let ok = Command::new("ffmpeg")
        .arg("-version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    assert!(
        ok,
        "ffmpeg is required for live video handoff tests (CI installs it)"
    );
}

/// 4 s synthetic clip, 5 fps, a keyframe every second → keyframes at PTS 0, 1, 2, 3.
fn synthetic_mp4(dir: &Path) -> Vec<u8> {
    let out = dir.join("synthetic.mp4");
    let status = Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc=duration=4:size=64x48:rate=5",
            "-g",
            "5",
            "-pix_fmt",
            "yuv420p",
            "-y",
        ])
        .arg(&out)
        .status()
        .expect("ffmpeg");
    assert!(status.success());
    std::fs::read(out).unwrap()
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

struct Env {
    _temp: TempDir,
    app: Router,
    raw: PathBuf,
    derived: PathBuf,
    video: Vec<u8>,
}

fn env() -> Env {
    let temp = tempdir().unwrap();
    let raw = temp.path().join("raw");
    let derived = temp.path().join("derived");
    let video = synthetic_mp4(temp.path());
    let app = create_router(AppConfig {
        evidence_storage_path: raw.clone(),
        derived_storage_path: derived.clone(),
        ..Default::default()
    });
    Env {
        _temp: temp,
        app,
        raw,
        derived,
        video,
    }
}

async fn upload_video(e: &Env, extra: &[(&str, &str)]) -> EvidenceUploadReceipt {
    let mut fields = vec![("case_id", CASE_ID)];
    fields.extend_from_slice(extra);
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/evidence/upload")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={BOUNDARY}"),
        )
        .body(Body::from(multipart(&fields, "dock_cam.mp4", &e.video)))
        .unwrap();
    let res = e.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let receipt: EvidenceUploadReceipt = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(receipt.source_type, "CCTV_VIDEO");
    receipt
}

async fn call(app: &Router, method: &str, uri: String) -> serde_json::Value {
    let req = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(if method == "POST" { "{}" } else { "" }))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    serde_json::from_slice(&res.into_body().collect().await.unwrap().to_bytes()).unwrap()
}

async fn reconstruct(app: &Router) -> serde_json::Value {
    call(
        app,
        "POST",
        format!("/api/v1/cases/{CASE_ID}/reconstruction"),
    )
    .await
}

async fn report(app: &Router) -> serde_json::Value {
    call(app, "GET", format!("/api/v1/cases/{CASE_ID}/report")).await
}

const CALIBRATED: &[(&str, &str)] = &[
    ("source_timezone", "Asia/Kolkata"),
    ("clock_offset_seconds", "-120"),
    (
        "metadata",
        r#"{"recording_start_timestamp":"2026-11-10T21:30:00","device_coordinates":{"lat":28.5355,"lon":77.271}}"#,
    ),
];

fn frame_entries(result: &serde_json::Value, evidence_id: &str) -> Vec<serde_json::Value> {
    result["timeline"]["entries"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["source_evidence_id"] == evidence_id)
        .cloned()
        .collect()
}

#[test]
fn showinfo_parser_reads_pts_time_in_order() {
    let log = "[Parsed_showinfo_1 @ 0x1] n:   0 pts:      0 pts_time:0       duration: 2\n\
               [Parsed_showinfo_1 @ 0x1] config in time_base: 1/10240\n\
               [Parsed_showinfo_1 @ 0x1] n:   1 pts:  10240 pts_time:1.5     duration: 2\n";
    assert_eq!(parse_showinfo_pts_times(log), vec![0.0, 1.5]);
}

#[tokio::test]
async fn calibrated_video_frames_reach_reconstruction_and_report() {
    require_ffmpeg();
    let e = env();
    let receipt = upload_video(&e, CALIBRATED).await;
    let result = reconstruct(&e.app).await;

    // Handoff record: real extraction, anchored from declared calibration.
    let handoff = &result["video_handoff"][0];
    assert_eq!(handoff["evidence_id"], receipt.evidence_id.as_str());
    assert_eq!(handoff["status"], "HANDED_OFF_ANCHORED", "{handoff}");
    assert_eq!(handoff["frames_extracted"], 4);
    assert_eq!(handoff["candidates_handed_off"], 4);
    assert_eq!(handoff["reused_existing_frames"], false);

    // Frame events: real PTS, anchored once, offset applied once by the temporal engine.
    // 21:30:00 IST = 16:00:00Z; −120 s → 15:58:00Z, then + PTS (0, 1, 2, 3 s).
    let frames = frame_entries(&result, &receipt.evidence_id);
    assert_eq!(frames.len(), 4);
    for (i, f) in frames.iter().enumerate() {
        assert_eq!(f["metadata"]["pts_seconds"], i as f64);
        assert_eq!(
            f["time_window"]["nominal_utc"].as_str().unwrap(),
            format!("2026-11-10T15:58:0{i}Z")
        );
        assert_eq!(f["time_window"]["clock_offset_seconds"], -120.0);
        assert_eq!(
            f["metadata"]["source_video_sha256"],
            receipt.sha256_hash.as_str()
        );
        assert_eq!(f["metadata"]["declared_device_timezone"], "Asia/Kolkata");
        assert_eq!(f["coordinates"]["lat"], 28.5355); // declared device position
        let kinds: Vec<&str> = f["assumptions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a["kind"].as_str().unwrap())
            .collect();
        assert!(
            kinds.is_empty(),
            "fully declared frame has assumptions {kinds:?}"
        );

        // The persisted frame file exists and matches the recorded SHA-256.
        let rel = f["metadata"]["relative_path"].as_str().unwrap();
        assert!(!rel.starts_with('/'), "host path leaked: {rel}");
        let bytes = std::fs::read(e.derived.join(rel)).expect("frame persisted");
        let digest = hex::encode(<sha2::Sha256 as sha2::Digest>::digest(&bytes));
        assert_eq!(digest, f["metadata"]["sha256_hash"].as_str().unwrap());
    }

    // Judicial report lists the frame-derived events with provenance.
    let rep = report(&e.app).await;
    let items: Vec<&serde_json::Value> = rep["timeline_events"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|t| t["source_evidence_id"] == receipt.evidence_id.as_str())
        .collect();
    assert_eq!(items.len(), 4);
    for t in &items {
        assert!(t["frame_sha256"].is_string());
        assert!(t["frame_pts_seconds"].is_number());
        assert!(t["frame_artifact_reference"]
            .as_str()
            .unwrap()
            .starts_with("urn:yukti:artifact:"));
        assert_eq!(t["source_video_sha256"], receipt.sha256_hash.as_str());
    }
    assert_eq!(rep["video_handoff"][0]["status"], "HANDED_OFF_ANCHORED");
    let md = rep["markdown_dossier"].as_str().unwrap();
    assert!(md.contains("Reconstructed Timeline & Provenance"));
    assert!(md.contains(items[0]["frame_artifact_reference"].as_str().unwrap()));
    assert!(md.contains("Video frame handoff"));
}

#[tokio::test]
async fn repeated_runs_reuse_frames_without_duplicates_or_overwrites() {
    require_ffmpeg();
    let e = env();
    let receipt = upload_video(&e, CALIBRATED).await;
    let first = reconstruct(&e.app).await;
    let second = reconstruct(&e.app).await;

    assert_eq!(second["video_handoff"][0]["reused_existing_frames"], true);
    assert_eq!(first["timeline"], second["timeline"]);
    assert_eq!(first["contradictions"], second["contradictions"]);
    assert_eq!(
        frame_entries(&second, &receipt.evidence_id).len(),
        4,
        "no duplicate frame events"
    );
    let ev_dir = e.derived.join(CASE_ID).join(&receipt.evidence_id);
    let frame_sets: Vec<_> = std::fs::read_dir(&ev_dir)
        .unwrap()
        .filter_map(|d| d.ok())
        .filter(|d| d.file_name().to_string_lossy().starts_with("frames-"))
        .collect();
    assert_eq!(frame_sets.len(), 1, "exactly one published frame set");
}

#[tokio::test]
async fn frame_extraction_is_deterministic_across_fresh_stores() {
    require_ffmpeg();
    let mut runs = Vec::new();
    for _ in 0..2 {
        let e = env();
        let receipt = upload_video(&e, CALIBRATED).await;
        let result = reconstruct(&e.app).await;
        let stable: Vec<(String, f64, String)> = frame_entries(&result, &receipt.evidence_id)
            .iter()
            .map(|f| {
                (
                    f["metadata"]["sha256_hash"].as_str().unwrap().to_string(),
                    f["metadata"]["pts_seconds"].as_f64().unwrap(),
                    f["time_window"]["nominal_utc"]
                        .as_str()
                        .unwrap()
                        .to_string(),
                )
            })
            .collect();
        runs.push((receipt.sha256_hash.clone(), stable));
    }
    assert_eq!(runs[0], runs[1]);
}

#[tokio::test]
async fn uncalibrated_video_frames_are_excluded_not_timed() {
    require_ffmpeg();
    let e = env();
    let receipt = upload_video(&e, &[]).await; // nothing declared
    let result = reconstruct(&e.app).await;

    assert_eq!(
        result["video_handoff"][0]["status"],
        "HANDED_OFF_UNANCHORED"
    );
    assert!(frame_entries(&result, &receipt.evidence_id).is_empty());
    let excluded: Vec<&serde_json::Value> = result["timeline"]["excluded_events"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|x| x["source_evidence_id"] == receipt.evidence_id.as_str())
        .collect();
    assert_eq!(
        excluded.len(),
        4,
        "every unanchored frame excluded with a reason"
    );
    assert_eq!(result["export_safeguard"]["status"], "BLOCKED");
    let diags = result["video_handoff"][0]["diagnostics"].to_string();
    assert!(
        diags.contains("No recording_start_timestamp declared"),
        "{diags}"
    );
}

#[tokio::test]
async fn tampered_video_bytes_are_rejected_before_extraction() {
    require_ffmpeg();
    let e = env();
    let receipt = upload_video(&e, CALIBRATED).await;
    let raw = e
        .raw
        .join(CASE_ID)
        .join(&receipt.evidence_id)
        .join(format!("{}.raw", receipt.evidence_id));
    // Simulate tampering of the stored exhibit (test-only; the store marks it read-only).
    let mut perms = std::fs::metadata(&raw).unwrap().permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    perms.set_readonly(false);
    std::fs::set_permissions(&raw, perms).unwrap();
    let mut bytes = std::fs::read(&raw).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 0xFF;
    std::fs::write(&raw, bytes).unwrap();

    let result = reconstruct(&e.app).await;
    assert_eq!(
        result["video_handoff"][0]["status"],
        "REJECTED_INTEGRITY_MISMATCH"
    );
    assert_eq!(result["video_handoff"][0]["frames_extracted"], 0);
    assert!(frame_entries(&result, &receipt.evidence_id).is_empty());
    assert_eq!(result["export_safeguard"]["status"], "BLOCKED");
    let ev_dir = e.derived.join(CASE_ID).join(&receipt.evidence_id);
    let published = std::fs::read_dir(&ev_dir)
        .map(|rd| {
            rd.filter_map(|d| d.ok())
                .filter(|d| d.file_name().to_string_lossy().starts_with("frames-"))
                .count()
        })
        .unwrap_or(0);
    assert_eq!(published, 0, "no frame set published for tampered evidence");
}
