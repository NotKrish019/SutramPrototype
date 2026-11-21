//! Minimal HTTP/1.1 mock server on std only: no Docker, PostgreSQL, Neo4j or extra crates.
#![allow(dead_code)]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// A canned response for one request path (and optionally one method).
#[derive(Clone, Debug)]
pub struct Route {
    /// `None` matches any method.
    pub method: Option<&'static str>,
    pub path: &'static str,
    pub status: u16,
    pub content_type: &'static str,
    pub body: String,
    /// Wait this long before answering, to exercise client timeouts on one route.
    pub delay: Option<Duration>,
}

impl Route {
    /// JSON response for any method on `path`.
    pub fn json(path: &'static str, status: u16, body: &str) -> Self {
        Self {
            method: None,
            path,
            status,
            content_type: "application/json",
            body: body.to_string(),
            delay: None,
        }
    }

    pub fn text(path: &'static str, status: u16, content_type: &'static str, body: &str) -> Self {
        Self {
            method: None,
            path,
            status,
            content_type,
            body: body.to_string(),
            delay: None,
        }
    }

    /// Restricts the route to one HTTP method.
    pub fn method(mut self, method: &'static str) -> Self {
        self.method = Some(method);
        self
    }

    /// Delays the response by `delay`.
    pub fn delayed(mut self, delay: Duration) -> Self {
        self.delay = Some(delay);
        self
    }
}

/// One request as received by the mock.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordedRequest {
    pub method: String,
    pub path: String,
    pub content_type: Option<String>,
    /// Body as (lossy) UTF-8 text, for JSON requests.
    pub body: String,
    /// Body exactly as received.
    pub body_bytes: Vec<u8>,
}

/// One part of a `multipart/form-data` body.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Part {
    pub name: String,
    pub filename: Option<String>,
    pub content_type: Option<String>,
    pub data: Vec<u8>,
}

impl RecordedRequest {
    /// Parses a `multipart/form-data` request body (independently of the CLI's encoder).
    pub fn multipart_parts(&self) -> Vec<Part> {
        let content_type = self.content_type.as_deref().expect("content-type header");
        let boundary = content_type
            .split(';')
            .find_map(|p| p.trim().strip_prefix("boundary="))
            .expect("multipart boundary");
        let delimiter = format!("--{boundary}").into_bytes();
        let body = &self.body_bytes;
        let mut positions = Vec::new();
        let mut i = 0;
        while i + delimiter.len() <= body.len() {
            if body[i..i + delimiter.len()] == delimiter[..] {
                positions.push(i);
                i += delimiter.len();
            } else {
                i += 1;
            }
        }
        assert!(
            positions.len() >= 2,
            "multipart body needs an opening and closing delimiter"
        );
        let last = *positions.last().unwrap();
        assert_eq!(
            &body[last + delimiter.len()..],
            b"--\r\n",
            "closing delimiter"
        );
        positions
            .windows(2)
            .map(|w| {
                // Between "--boundary\r\n" and "\r\n--boundary".
                let start = w[0] + delimiter.len() + 2;
                let end = w[1] - 2;
                let section = &body[start..end];
                let split = section
                    .windows(4)
                    .position(|x| x == b"\r\n\r\n")
                    .expect("part headers end");
                let headers = String::from_utf8_lossy(&section[..split]).into_owned();
                let data = section[split + 4..].to_vec();
                let mut name = String::new();
                let mut filename = None;
                let mut part_type = None;
                for line in headers.split("\r\n") {
                    if let Some(rest) = line.strip_prefix("Content-Disposition: form-data;") {
                        for param in rest.split(';') {
                            let param = param.trim();
                            if let Some(v) = param.strip_prefix("name=") {
                                name = v.trim_matches('"').to_string();
                            } else if let Some(v) = param.strip_prefix("filename=") {
                                filename = Some(v.trim_matches('"').to_string());
                            }
                        }
                    } else if let Some(v) = line.strip_prefix("Content-Type: ") {
                        part_type = Some(v.to_string());
                    }
                }
                Part {
                    name,
                    filename,
                    content_type: part_type,
                    data,
                }
            })
            .collect()
    }
}

/// Bodies matching the real Rust API handlers (`backend-rust/src/api/routes/health.rs`, `cases.rs`).
pub mod fixtures {
    pub const HEALTH_OK: &str = r#"{"status":"ok","service":"sutram-backend-rust"}"#;
    pub const READY: &str = r#"{"status":"ready","service":"sutram-backend-rust","database":"connected","graph":"connected"}"#;
    pub const DEGRADED: &str = r#"{"status":"degraded","service":"sutram-backend-rust","database":"unconfigured_filesystem_mode","graph":"disabled"}"#;
    /// Graph disabled but PostgreSQL connected: degraded, yet the case list comes from the database.
    pub const DEGRADED_DB_CONNECTED: &str = r#"{"status":"degraded","service":"sutram-backend-rust","database":"connected","graph":"disabled"}"#;
    pub const NOT_READY: &str = r#"{"status":"not_ready","service":"sutram-backend-rust","database":"disconnected","graph":"disabled","error":"Database error: pool timed out"}"#;

    pub const CASE_A_ID: &str = "3f2b8c1e-7a4d-4e1b-9c3a-5d6e7f8a9b0c";
    pub const CASE_A: &str = r#"{"case_id":"3f2b8c1e-7a4d-4e1b-9c3a-5d6e7f8a9b0c","title":"Warehouse burglary","description":"Night-time break-in","fir_number":"FIR-112/2026","status":"ACTIVE","investigator_id":"INV-7","metadata":{"station":"Saket"},"created_at":"2026-11-21T09:15:00Z","updated_at":"2026-11-21T09:15:00Z"}"#;
    pub const CASE_B_ID: &str = "0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d";
    pub const CASE_B: &str = r#"{"case_id":"0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d","title":"ATM skimming","description":null,"fir_number":null,"status":"DRAFT","investigator_id":null,"metadata":{},"created_at":"2026-11-20T18:02:41Z","updated_at":"2026-11-20T18:02:41Z"}"#;
    /// The shape the API returns for a storage folder without `case.json` (filesystem mode).
    pub const FS_PLACEHOLDER: &str = r#"{"case_id":"9d8c7b6a-5f4e-4d3c-8b2a-1f0e9d8c7b6a","title":"Case 9d8c7b6a-5f4e-4d3c-8b2a-1f0e9d8c7b6a","description":"Filesystem evidence case container","fir_number":null,"status":"ACTIVE","investigator_id":null,"metadata":{"storage":"filesystem"},"created_at":"2026-11-21T10:00:00Z","updated_at":"2026-11-21T10:00:00Z"}"#;
    pub const NOT_FOUND: &str = r#"{"error":{"code":"NOT_FOUND","message":"Case not found for id '3f2b8c1e-7a4d-4e1b-9c3a-5d6e7f8a9b0c'"}}"#;

    /// `EvidenceUploadReceipt` for a CDR CSV (`backend-rust/src/ingestion/model.rs`).
    pub const RECEIPT_CSV: &str = r#"{"evidence_id":"5b1f0c2e-8d3a-4f6b-9e7c-1a2b3c4d5e6f","case_id":"3f2b8c1e-7a4d-4e1b-9c3a-5d6e7f8a9b0c","original_filename":"cdr.csv","source_type":"TABULAR_CSV","mime_type":"text/csv","file_size_bytes":76,"sha256_hash":"9f2c6d0a8b1e4f7c3a5d9e2b6c8f0a1d4e7b3c5a9f2d6e8b0c1a4f7d3e5b9c2a","storage_uri":"urn:sutram:evidence:5b1f0c2e-8d3a-4f6b-9e7c-1a2b3c4d5e6f:storage","status":"INGESTED","metadata":{"source_timezone":"Asia/Kolkata","clock_offset_seconds":-30.0},"created_at":"2026-11-21T09:30:00Z"}"#;
    pub const RECEIPT_CSV_ID: &str = "5b1f0c2e-8d3a-4f6b-9e7c-1a2b3c4d5e6f";
    pub const RECEIPT_CSV_SHA: &str =
        "9f2c6d0a8b1e4f7c3a5d9e2b6c8f0a1d4e7b3c5a9f2d6e8b0c1a4f7d3e5b9c2a";
    /// Receipt for a chat export with nothing declared (empty metadata).
    pub const RECEIPT_CHAT: &str = r#"{"evidence_id":"7c2d1e3f-9a4b-4c5d-8e6f-2b3c4d5e6f7a","case_id":"3f2b8c1e-7a4d-4e1b-9c3a-5d6e7f8a9b0c","original_filename":"chat.json","source_type":"CHAT_EXPORT","mime_type":"application/json","file_size_bytes":37,"sha256_hash":"1d4e7b3c5a9f2d6e8b0c1a4f7d3e5b9c2a9f2c6d0a8b1e4f7c3a5d9e2b6c8f0a","storage_uri":"urn:sutram:evidence:7c2d1e3f-9a4b-4c5d-8e6f-2b3c4d5e6f7a:storage","status":"INGESTED","metadata":{},"created_at":"2026-11-21T09:31:00Z"}"#;
    pub const RECEIPT_CHAT_ID: &str = "7c2d1e3f-9a4b-4c5d-8e6f-2b3c4d5e6f7a";
}

/// A server that answers each connection from a fixed route table (404 envelope otherwise)
/// and records every request it receives.
pub struct MockServer {
    pub base_url: String,
    requests: Arc<Mutex<Vec<RecordedRequest>>>,
}

impl MockServer {
    pub fn start(routes: Vec<Route>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
        let addr = listener.local_addr().expect("mock server address");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&requests);
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let routes = routes.clone();
                let log = Arc::clone(&log);
                thread::spawn(move || handle(stream, &routes, &log));
            }
        });
        Self {
            base_url: format!("http://{addr}"),
            requests,
        }
    }

    /// Accepts connections but never answers, to exercise timeouts.
    pub fn hanging() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind hanging server");
        let addr = listener.local_addr().expect("hanging server address");
        thread::spawn(move || {
            let mut held = Vec::new();
            for stream in listener.incoming().flatten() {
                held.push(stream);
            }
        });
        Self {
            base_url: format!("http://{addr}"),
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// The standard healthy, degraded (filesystem-mode) API.
    pub fn degraded_api() -> Self {
        Self::start(vec![
            Route::json("/health", 200, fixtures::HEALTH_OK),
            Route::json("/health/ready", 200, fixtures::DEGRADED),
        ])
    }

    /// Requests received so far, in arrival order.
    pub fn requests(&self) -> Vec<RecordedRequest> {
        self.requests.lock().expect("request log").clone()
    }
}

/// A base URL on which nothing is listening (connection refused).
pub fn unused_base_url() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind probe");
    let port = listener.local_addr().expect("probe address").port();
    drop(listener);
    format!("http://127.0.0.1:{port}")
}

fn handle(mut stream: TcpStream, routes: &[Route], log: &Mutex<Vec<RecordedRequest>>) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let mut buf = Vec::new();
    let mut chunk = [0u8; 1024];
    let header_end = loop {
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break pos + 4;
        }
        if buf.len() > 64 * 1024 {
            return;
        }
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => return,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
        }
    };

    let head = String::from_utf8_lossy(&buf[..header_end]).into_owned();
    let mut lines = head.lines();
    let mut request_line = lines.next().unwrap_or("").split_whitespace();
    let method = request_line.next().unwrap_or("").to_string();
    let path = request_line.next().unwrap_or("/").to_string();
    let header = |name: &str| {
        head.lines().skip(1).find_map(|l| {
            let (k, v) = l.split_once(':')?;
            k.trim()
                .eq_ignore_ascii_case(name)
                .then(|| v.trim().to_string())
        })
    };
    let content_length: usize = header("content-length")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    while buf.len() < header_end + content_length {
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
        }
    }
    let body_end = (header_end + content_length).min(buf.len());
    let body_bytes = buf[header_end..body_end].to_vec();
    let body = String::from_utf8_lossy(&body_bytes).into_owned();
    log.lock().expect("request log").push(RecordedRequest {
        method: method.clone(),
        path: path.clone(),
        content_type: header("content-type"),
        body,
        body_bytes,
    });

    let route = routes
        .iter()
        .find(|r| r.path == path && r.method.is_none_or(|m| m == method))
        .cloned()
        .unwrap_or_else(|| {
            Route::json(
                "",
                404,
                r#"{"error":{"code":"NOT_FOUND","message":"no such route in mock"}}"#,
            )
        });
    if let Some(delay) = route.delay {
        thread::sleep(delay);
    }
    let response = format!(
        "HTTP/1.1 {} Mock\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        route.status,
        route.content_type,
        route.body.len(),
        route.body
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}
