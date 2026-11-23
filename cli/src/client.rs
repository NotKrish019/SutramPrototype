//! Thin HTTP client for the YUKTI Rust API.
//!
//! The CLI talks to the API over HTTP only. It never touches PostgreSQL, Neo4j or backend
//! internals, and it contains no forensic logic.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use reqwest::header::{ACCEPT, CONTENT_TYPE};
use serde::Serialize;
use serde_json::Value;

use crate::config::Config;
use crate::dto::{
    CaseId, CaseRecord, CaseReportRecord, CreateCaseRequest, EvidenceRecord, EvidenceUpload,
    ReconstructionRecord, TriggerReconstructionRequest, UploadOutcome,
};
use crate::error::{http_error, CliError};

/// Liveness probe (`backend-rust/src/app.rs`).
pub const HEALTH_PATH: &str = "/health";
/// Readiness probe.
pub const READINESS_PATH: &str = "/health/ready";
/// Case collection: `GET` lists, `POST` creates; `GET {CASES_PATH}/:case_id` shows one.
pub const CASES_PATH: &str = "/api/v1/cases";
/// Multipart evidence upload.
pub const EVIDENCE_UPLOAD_PATH: &str = "/api/v1/evidence/upload";

/// `GET /api/v1/cases/:case_id/evidence`.
pub fn case_evidence_path(case_id: &CaseId) -> String {
    format!("{CASES_PATH}/{}/evidence", case_id.as_str())
}

/// `/api/v1/cases/:case_id/reconstruction`: `POST` runs a reconstruction, `GET` retrieves one.
pub fn case_reconstruction_path(case_id: &CaseId) -> String {
    format!("{CASES_PATH}/{}/reconstruction", case_id.as_str())
}

/// `GET /api/v1/cases/:case_id/report`.
pub fn case_report_path(case_id: &CaseId) -> String {
    format!("{CASES_PATH}/{}/report", case_id.as_str())
}

/// Percent-encodes one path segment or query value (RFC 3986 unreserved characters pass).
pub fn encode_component(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for byte in raw.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

/// `/api/v1/cases/:case_id/<suffix>` for the read-only reconstruction views
/// (`timeline`, `spatial`, `contradictions`, `gaps`, `reconstruction/provenance`, `graph/summary`).
pub fn case_view_path(case_id: &CaseId, suffix: &str) -> String {
    format!("{CASES_PATH}/{}/{suffix}", case_id.as_str())
}

/// `GET /api/v1/cases/:case_id/findings/:finding_id/provenance`.
pub fn finding_provenance_path(case_id: &CaseId, finding_id: &str) -> String {
    format!(
        "{CASES_PATH}/{}/findings/{}/provenance",
        case_id.as_str(),
        encode_component(finding_id)
    )
}

/// `GET /api/v1/cases/:case_id/robustness`.
pub fn case_robustness_path(case_id: &CaseId) -> String {
    format!("{CASES_PATH}/{}/robustness", case_id.as_str())
}

/// `GET /api/v1/cases/:case_id/custody/verify`.
pub fn case_custody_path(case_id: &CaseId) -> String {
    format!("{CASES_PATH}/{}/custody/verify", case_id.as_str())
}

/// `POST /api/v1/cases/:case_id/what-if`.
pub fn case_whatif_path(case_id: &CaseId) -> String {
    format!("{CASES_PATH}/{}/what-if", case_id.as_str())
}

/// `GET /api/v1/cases/:case_id/graph/entities/:entity_id/neighbors[?depth=&limit=]`.
pub fn graph_neighbors_path(
    case_id: &CaseId,
    entity_id: &str,
    depth: Option<u32>,
    limit: Option<i64>,
) -> String {
    let mut query = Vec::new();
    if let Some(d) = depth {
        query.push(format!("depth={d}"));
    }
    if let Some(l) = limit {
        query.push(format!("limit={l}"));
    }
    let mut path = format!(
        "{CASES_PATH}/{}/graph/entities/{}/neighbors",
        case_id.as_str(),
        encode_component(entity_id)
    );
    if !query.is_empty() {
        path.push('?');
        path.push_str(&query.join("&"));
    }
    path
}

/// `GET /api/v1/cases/:case_id/graph/path?from=&to=[&max_hops=]`.
pub fn graph_path_path(case_id: &CaseId, from: &str, to: &str, max_hops: Option<u32>) -> String {
    let mut path = format!(
        "{CASES_PATH}/{}/graph/path?from={}&to={}",
        case_id.as_str(),
        encode_component(from),
        encode_component(to)
    );
    if let Some(h) = max_hops {
        path.push_str(&format!("&max_hops={h}"));
    }
    path
}

/// `/api/v1/evidence/:evidence_id[/<suffix>]` (`extract`, `reprocess`, `extraction`,
/// `extraction/runs`; empty suffix for the receipt).
pub fn evidence_path(evidence_id: &str, suffix: &str) -> String {
    let base = format!("/api/v1/evidence/{}", encode_component(evidence_id));
    if suffix.is_empty() {
        base
    } else {
        format!("{base}/{suffix}")
    }
}

/// A completed HTTP exchange, kept raw so callers can decide how to interpret any status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawResponse {
    pub url: String,
    pub status: u16,
    pub body: String,
}

impl RawResponse {
    /// True for 2xx statuses.
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// Parses the body as JSON.
    pub fn json(&self) -> Result<serde_json::Value, CliError> {
        serde_json::from_str(&self.body).map_err(|e| {
            let hint = if self.body.trim_start().starts_with('<') {
                " The server returned HTML: is the API URL pointing at the web app instead of the YUKTI Rust API?"
            } else {
                ""
            };
            CliError::BadResponse {
                url: self.url.clone(),
                reason: format!("response body is not valid JSON ({e}).{hint}"),
            }
        })
    }

    /// The error describing this (non-success) response.
    pub fn to_error(&self) -> CliError {
        http_error(&self.url, self.status, &self.body)
    }

    /// The JSON body of a 2xx response, or the matching error.
    pub fn success_json(&self) -> Result<Value, CliError> {
        if !self.is_success() {
            return Err(self.to_error());
        }
        self.json()
    }
}

fn json_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

/// HTTP client bound to one API base URL.
#[derive(Debug, Clone)]
pub struct ApiClient {
    http: reqwest::Client,
    base_url: String,
    timeout: Duration,
}

impl ApiClient {
    /// Builds a client from the resolved configuration.
    pub fn new(config: &Config) -> Result<Self, CliError> {
        let http = reqwest::Client::builder()
            .timeout(config.timeout)
            .connect_timeout(config.timeout)
            // The API does not redirect; following redirects could silently hit another server.
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(concat!("yukti-cli/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| CliError::Internal(format!("failed to build HTTP client: {e}")))?;
        Ok(Self {
            http,
            base_url: config.base_url(),
            timeout: config.timeout,
        })
    }

    /// The API base URL without a trailing slash.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Absolute URL for an API path (`path` must start with `/`).
    pub fn url_for(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    /// Sends `GET path` and returns the raw response for any HTTP status.
    /// Only transport failures (unreachable, timeout) are errors.
    pub async fn get(&self, path: &str) -> Result<RawResponse, CliError> {
        let url = self.url_for(path);
        let request = self.http.get(&url);
        self.send(url, request).await
    }

    /// Sends `POST path` with a JSON body and returns the raw response for any HTTP status.
    pub async fn post<T: Serialize>(&self, path: &str, body: &T) -> Result<RawResponse, CliError> {
        let url = self.url_for(path);
        let payload = serde_json::to_string(body)
            .map_err(|e| CliError::Internal(format!("failed to encode request body: {e}")))?;
        let request = self
            .http
            .post(&url)
            .header(CONTENT_TYPE, "application/json")
            .body(payload);
        self.send(url, request).await
    }

    /// Sends `GET path` and requires a 2xx JSON response.
    pub async fn get_json(&self, path: &str) -> Result<Value, CliError> {
        self.get(path).await?.success_json()
    }

    /// Sends `POST path` with a JSON body and requires a 2xx JSON response.
    pub async fn post_json<T: Serialize>(&self, path: &str, body: &T) -> Result<Value, CliError> {
        self.post(path, body).await?.success_json()
    }

    /// `GET /health`, raw.
    pub async fn health(&self) -> Result<RawResponse, CliError> {
        self.get(HEALTH_PATH).await
    }

    /// `GET /health/ready`, raw (a 503 still carries a `ReadinessResponse`).
    pub async fn readiness(&self) -> Result<RawResponse, CliError> {
        self.get(READINESS_PATH).await
    }

    /// `GET /api/v1/cases`: every record, in the order the API returned them.
    pub async fn list_cases(&self) -> Result<Vec<CaseRecord>, CliError> {
        let url = self.url_for(CASES_PATH);
        match self.get_json(CASES_PATH).await? {
            Value::Array(items) => items
                .into_iter()
                .map(|item| CaseRecord::from_value(&url, item))
                .collect(),
            other => Err(CliError::BadResponse {
                url,
                reason: format!("expected a JSON array of cases, got {}", json_kind(&other)),
            }),
        }
    }

    /// `POST /api/v1/cases`: the created case as returned by the API.
    pub async fn create_case(&self, request: &CreateCaseRequest) -> Result<CaseRecord, CliError> {
        let url = self.url_for(CASES_PATH);
        let value = self.post_json(CASES_PATH, request).await?;
        CaseRecord::from_value(&url, value)
    }

    /// `GET /api/v1/cases/:case_id`.
    pub async fn get_case(&self, case_id: &CaseId) -> Result<CaseRecord, CliError> {
        let path = format!("{CASES_PATH}/{}", case_id.as_str());
        let url = self.url_for(&path);
        let value = self.get_json(&path).await?;
        CaseRecord::from_value(&url, value)
    }

    /// `GET /api/v1/cases/:case_id/evidence`: every receipt, in the order the API returned them.
    pub async fn list_case_evidence(
        &self,
        case_id: &CaseId,
    ) -> Result<Vec<EvidenceRecord>, CliError> {
        let path = case_evidence_path(case_id);
        let url = self.url_for(&path);
        match self.get_json(&path).await? {
            Value::Array(items) => items
                .into_iter()
                .map(|item| EvidenceRecord::from_value(&url, item))
                .collect(),
            other => Err(CliError::BadResponse {
                url,
                reason: format!(
                    "expected a JSON array of evidence receipts, got {}",
                    json_kind(&other)
                ),
            }),
        }
    }

    /// `POST /api/v1/cases/:case_id/reconstruction` with `request` as the JSON body.
    pub async fn run_reconstruction(
        &self,
        case_id: &CaseId,
        request: &TriggerReconstructionRequest,
    ) -> Result<ReconstructionRecord, CliError> {
        let path = case_reconstruction_path(case_id);
        let url = self.url_for(&path);
        let value = self.post_json(&path, request).await?;
        ReconstructionRecord::from_value(&url, value)
    }

    /// `GET /api/v1/cases/:case_id/reconstruction`. The API returns its latest stored result,
    /// or computes one with default options when none is stored, and does not say which.
    pub async fn get_reconstruction(
        &self,
        case_id: &CaseId,
    ) -> Result<ReconstructionRecord, CliError> {
        let path = case_reconstruction_path(case_id);
        let url = self.url_for(&path);
        let value = self.get_json(&path).await?;
        ReconstructionRecord::from_value(&url, value)
    }

    /// `GET /api/v1/cases/:case_id/report`.
    pub async fn get_case_report(&self, case_id: &CaseId) -> Result<CaseReportRecord, CliError> {
        let path = case_report_path(case_id);
        let url = self.url_for(&path);
        let value = self.get_json(&path).await?;
        CaseReportRecord::from_value(&url, value)
    }

    /// `POST /api/v1/evidence/upload` as `multipart/form-data`.
    pub async fn upload_evidence(
        &self,
        upload: &EvidenceUpload,
    ) -> Result<UploadOutcome, CliError> {
        let url = self.url_for(EVIDENCE_UPLOAD_PATH);
        let form = MultipartForm::from_upload(upload);
        let request = self
            .http
            .post(&url)
            .header(CONTENT_TYPE, form.content_type())
            .body(form.body);
        let value = self.send(url.clone(), request).await?.success_json()?;
        UploadOutcome::from_value(&url, value)
    }

    async fn send(
        &self,
        url: String,
        request: reqwest::RequestBuilder,
    ) -> Result<RawResponse, CliError> {
        let response = request
            .header(ACCEPT, "application/json")
            .send()
            .await
            .map_err(|e| self.transport_error(&url, &e))?;
        let status = response.status().as_u16();
        let body = response
            .text()
            .await
            .map_err(|e| self.transport_error(&url, &e))?;
        Ok(RawResponse { url, status, body })
    }

    fn transport_error(&self, url: &str, err: &reqwest::Error) -> CliError {
        if err.is_timeout() {
            CliError::Timeout {
                url: url.to_string(),
                timeout: self.timeout,
            }
        } else if err.is_body() || err.is_decode() {
            CliError::BadResponse {
                url: url.to_string(),
                reason: format!("failed to read the response body: {}", describe(err)),
            }
        } else {
            CliError::Unreachable {
                url: url.to_string(),
                reason: describe(err),
            }
        }
    }
}

/// A `multipart/form-data` body (RFC 7578), built by hand so the CLI needs no extra
/// dependency. Text fields come first, then the single `file` part with the bytes unmodified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultipartForm {
    pub boundary: String,
    pub body: Vec<u8>,
}

impl MultipartForm {
    /// Encodes an evidence upload. Only supplied optional fields become parts.
    pub fn from_upload(upload: &EvidenceUpload) -> Self {
        let mut fields: Vec<(&str, &str)> = vec![("case_id", upload.case_id.as_str())];
        if let Some(v) = &upload.source_type {
            fields.push(("source_type", v));
        }
        if let Some(v) = &upload.source_timezone {
            fields.push(("source_timezone", v));
        }
        if let Some(v) = &upload.clock_offset_seconds {
            fields.push(("clock_offset_seconds", v));
        }
        if let Some(v) = &upload.metadata {
            fields.push(("metadata", v));
        }
        if upload.auto_extract {
            fields.push(("auto_extract", "true"));
        }
        Self::encode(&fields, &upload.file_name, &upload.bytes)
    }

    /// Encodes text `fields` and one `file` part.
    pub fn encode(fields: &[(&str, &str)], file_name: &str, bytes: &[u8]) -> Self {
        let boundary = unique_boundary(|candidate| {
            let needle = candidate.as_bytes();
            contains(bytes, needle)
                || fields.iter().any(|(_, v)| contains(v.as_bytes(), needle))
                || contains(file_name.as_bytes(), needle)
        });
        let mut body = Vec::with_capacity(bytes.len() + 512);
        for (name, value) in fields {
            body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
            body.extend_from_slice(
                format!("Content-Disposition: form-data; name=\"{name}\"\r\n\r\n").as_bytes(),
            );
            body.extend_from_slice(value.as_bytes());
            body.extend_from_slice(b"\r\n");
        }
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        body.extend_from_slice(
            format!(
                "Content-Disposition: form-data; name=\"file\"; filename=\"{}\"\r\n",
                escape_filename(file_name)
            )
            .as_bytes(),
        );
        body.extend_from_slice(b"Content-Type: application/octet-stream\r\n\r\n");
        body.extend_from_slice(bytes);
        body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
        Self { boundary, body }
    }

    /// Value of the request's `Content-Type` header.
    pub fn content_type(&self) -> String {
        format!("multipart/form-data; boundary={}", self.boundary)
    }
}

/// Escapes a filename for a quoted `Content-Disposition` parameter the way browsers do
/// (WHATWG HTML form encoding): `"` as `%22`, CR as `%0D`, LF as `%0A`.
pub fn escape_filename(name: &str) -> String {
    name.replace('"', "%22")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}

/// A boundary that `collides` reports as absent from the payload.
fn unique_boundary(collides: impl Fn(&str) -> bool) -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    loop {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let candidate = format!(
            "yukti-cli-boundary-{nanos:x}-{:x}-{n:x}",
            std::process::id()
        );
        if !collides(&candidate) {
            return candidate;
        }
    }
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty() && haystack.windows(needle.len()).any(|w| w == needle)
}

/// Human-readable cause chain of a reqwest error, without repeating the URL.
fn describe(err: &reqwest::Error) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut source = std::error::Error::source(err);
    while let Some(cause) = source {
        let text = cause.to_string();
        if !parts.iter().any(|p| p.contains(&text)) {
            parts.push(text);
        }
        source = cause.source();
    }
    if parts.is_empty() {
        err.to_string()
    } else {
        parts.join(": ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multipart_layout_is_rfc7578() {
        let form = MultipartForm::encode(&[("case_id", "abc")], "a.csv", b"x,y\r\n1,2\r\n");
        let b = &form.boundary;
        let expected = format!(
            "--{b}\r\nContent-Disposition: form-data; name=\"case_id\"\r\n\r\nabc\r\n\
             --{b}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"a.csv\"\r\n\
             Content-Type: application/octet-stream\r\n\r\nx,y\r\n1,2\r\n\r\n--{b}--\r\n"
        );
        assert_eq!(
            form.content_type(),
            format!("multipart/form-data; boundary={b}")
        );
        assert_eq!(String::from_utf8(form.body.clone()).unwrap(), expected);
    }

    #[test]
    fn boundary_never_occurs_in_the_payload() {
        let first = MultipartForm::encode(&[], "f", b"data").boundary;
        // A payload that contains the next boundaries this process could generate.
        let mut tricky = Vec::new();
        for n in 0..50u64 {
            tricky.extend_from_slice(format!("{first}{n:x}").as_bytes());
        }
        let form = MultipartForm::encode(&[], "f", &tricky);
        let needle = format!("--{}", form.boundary);
        let occurrences = form
            .body
            .windows(needle.len())
            .filter(|w| *w == needle.as_bytes())
            .count();
        assert_eq!(
            occurrences, 2,
            "only the part delimiter and the closing delimiter"
        );
    }

    #[test]
    fn filenames_are_escaped_like_browsers_do() {
        assert_eq!(escape_filename("plain.csv"), "plain.csv");
        assert_eq!(escape_filename("a\"b.csv"), "a%22b.csv");
        assert_eq!(escape_filename("a\r\nb"), "a%0D%0Ab");
        assert_eq!(escape_filename("साक्ष्य.json"), "साक्ष्य.json");
    }
}
