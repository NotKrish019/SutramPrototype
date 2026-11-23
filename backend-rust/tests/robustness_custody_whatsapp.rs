//! End-to-end tests through the HTTP router for:
//! * WhatsApp `.txt` chat exports (`CHAT_EXPORT`, minute precision, date-order labelling),
//! * `GET /api/v1/cases/:case_id/robustness` (breaking-point analysis),
//! * `GET /api/v1/cases/:case_id/custody/verify` (re-hash audit and case seal).
//!
//! Uses the synthetic demo case in `data/synthetic/demo/`.

use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use http_body_util::BodyExt;
use serde_json::Value;
use std::path::Path;
use sutram_backend_rust::{create_router, AppConfig};
use tempfile::tempdir;
use tower::ServiceExt;

const BOUNDARY: &str = "----yukti-judge-features-boundary";
const CASE: &str = "6f1c2b3a-4d5e-4f60-8a7b-9c0d1e2f3a4b";

const CARRIER_A: &str = include_str!("../../data/synthetic/demo/carrier_a_cdr.csv");
const CARRIER_B: &str = include_str!("../../data/synthetic/demo/carrier_b_tower_dump.csv");
const WHATSAPP: &str = include_str!("../../data/synthetic/demo/whatsapp_chat.txt");

fn multipart(file_name: &str, bytes: &[u8]) -> Vec<u8> {
    let mut body = Vec::new();
    for (name, value) in [
        ("case_id", CASE),
        ("source_timezone", "Asia/Kolkata"),
        ("auto_extract", "true"),
    ] {
        body.extend_from_slice(
            format!("--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n")
                .as_bytes(),
        );
    }
    body.extend_from_slice(
        format!(
            "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{file_name}\"\r\nContent-Type: application/octet-stream\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());
    body
}

async fn send(app: &Router, method: &str, uri: &str, body: Option<Vec<u8>>) -> (StatusCode, Value) {
    let mut req = Request::builder().method(method).uri(uri);
    let body = match body {
        Some(b) => {
            req = req.header(
                "content-type",
                format!("multipart/form-data; boundary={BOUNDARY}"),
            );
            Body::from(b)
        }
        None if method == "POST" => {
            req = req.header("content-type", "application/json");
            Body::from("{}")
        }
        None => Body::empty(),
    };
    let res = app.clone().oneshot(req.body(body).unwrap()).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

async fn demo_case(root: &Path) -> (Router, Vec<Value>) {
    let app = create_router(AppConfig {
        evidence_storage_path: root.join("raw"),
        derived_storage_path: root.join("derived"),
        ..Default::default()
    });
    let mut receipts = Vec::new();
    for (name, body) in [
        ("carrier_a_cdr.csv", CARRIER_A),
        ("carrier_b_tower_dump.csv", CARRIER_B),
        ("whatsapp_chat.txt", WHATSAPP),
    ] {
        let (status, v) = send(
            &app,
            "POST",
            "/api/v1/evidence/upload?auto_extract=true",
            Some(multipart(name, body.as_bytes())),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{name}: {v}");
        assert_eq!(v["extraction"]["status"], "SUCCESS", "{name}: {v}");
        receipts.push(v["receipt"].clone());
    }
    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/v1/cases/{CASE}/reconstruction"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    (app, receipts)
}

#[tokio::test]
async fn whatsapp_txt_is_a_chat_export_with_minute_precision_and_labelled_date_order() {
    let temp = tempdir().unwrap();
    let (app, receipts) = demo_case(temp.path()).await;
    let chat = &receipts[2];
    assert_eq!(chat["source_type"], "CHAT_EXPORT");
    let (status, result) = send(
        &app,
        "GET",
        &format!(
            "/api/v1/evidence/{}/extraction",
            chat["evidence_id"].as_str().unwrap()
        ),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let events = result["event_candidates"].as_array().unwrap();
    assert_eq!(
        events.len(),
        4,
        "system notice excluded, multi-line message kept whole"
    );
    assert!(events.iter().all(|e| e["timestamp_precision"] == "MINUTE"));
    assert_eq!(
        events[0]["metadata"]["original_timestamp_text"],
        "10/11/2026, 8:45 pm"
    );
    assert_eq!(
        events[0]["metadata"]["date_order_assumed"], true,
        "10/11 is ambiguous"
    );
    assert_eq!(events[0]["source_record_id"], "wa-line-2");
}

#[tokio::test]
async fn robustness_classifies_and_verifies_every_lead() {
    let temp = tempdir().unwrap();
    let (app, _) = demo_case(temp.path()).await;
    let (status, r) = send(
        &app,
        "GET",
        &format!("/api/v1/cases/{CASE}/robustness"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{r}");
    let leads = r["leads"].as_array().unwrap();
    assert!(!leads.is_empty());
    assert_eq!(r["total_leads"].as_u64().unwrap() as usize, leads.len());
    assert!(leads.iter().all(|l| l["boundary_verified"] == true), "{r}");
    let classes: Vec<&str> = leads
        .iter()
        .map(|l| l["robustness_class"].as_str().unwrap())
        .collect();
    assert!(classes.contains(&"ROBUST"), "{classes:?}");
    assert!(classes.contains(&"FRAGILE"), "{classes:?}");
    // The same-operator Saket -> Mumbai lead cannot depend on the clock.
    let same = leads
        .iter()
        .find(|l| l["same_clock"] == true)
        .expect("a single-exhibit lead");
    assert_eq!(same["breaking_points"][0]["dimension"], "CLOCK_OFFSET");
    assert_eq!(same["breaking_points"][0]["applicable"], false);
    assert_eq!(same["robustness_class"], "ROBUST");
}

#[tokio::test]
async fn custody_audit_verifies_seals_and_detects_a_tampered_exhibit() {
    let temp = tempdir().unwrap();
    let (app, receipts) = demo_case(temp.path()).await;
    let uri = format!("/api/v1/cases/{CASE}/custody/verify");
    let (status, a) = send(&app, "GET", &uri, None).await;
    assert_eq!(status, StatusCode::OK, "{a}");
    assert_eq!(a["status"], "VERIFIED");
    assert_eq!(a["exhibit_count"], 3);
    assert_eq!(a["ledger_mode"], "FILESYSTEM_NO_LEDGER");
    let seal = a["case_seal_sha256"].as_str().unwrap().to_string();
    assert_eq!(seal.len(), 64);
    // Same evidence, same seal.
    let (_, again) = send(&app, "GET", &uri, None).await;
    assert_eq!(again["case_seal_sha256"], seal);

    // Tamper with one stored exhibit (raw store layout: <case>/<evidence>/<evidence>.raw).
    let ev = receipts[0]["evidence_id"].as_str().unwrap();
    let raw = temp
        .path()
        .join("raw")
        .join(CASE)
        .join(ev)
        .join(format!("{ev}.raw"));
    assert!(raw.exists(), "raw store layout changed: {}", raw.display());
    let mut bytes = std::fs::read(&raw).unwrap();
    bytes.extend_from_slice(b"tampered\n");
    std::fs::write(&raw, bytes).unwrap();

    let (_, b) = send(&app, "GET", &uri, None).await;
    assert_eq!(b["status"], "FAILED");
    assert_eq!(b["failed_count"], 1);
    let bad = b["exhibits"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["evidence_id"] == ev)
        .unwrap();
    assert_eq!(bad["stored_file"], "HASH_MISMATCH");
    assert_eq!(bad["verified"], false);
    // The seal is over the recorded hashes, so it does not change: the audit reports the failure.
    assert_eq!(b["case_seal_sha256"], seal);
}
