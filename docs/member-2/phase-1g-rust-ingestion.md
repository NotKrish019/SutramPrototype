# SUTRAM Digital Forensics — Phase 1G: Rust Forensic Evidence Ingestion Engine

**Author:** Member 2 (Backend & Data Engineering / Lead Rust Backend Engineer)  
**Date:** November 18, 2026  
**Status:** Complete  
**Repository:** `https://github.com/NotKrish019/SutramPrototype.git`  
**Crate:** `backend-rust/`

---

## 1. Architectural Overview

Phase 1G establishes the production forensic evidence ingestion subsystem within the Rust backend (`backend-rust/`). The ingestion engine serves as the secure, streaming digital evidence gateway for SUTRAM 2026.

```
                              HTTP Multipart Ingestion Request
                                             │
                                             ▼
                             ┌───────────────────────────────┐
                             │       Axum API Route          │
                             │ POST /api/v1/evidence/upload  │
                             └───────────────┬───────────────┘
                                             │
                     ┌───────────────────────┴───────────────────────┐
                     │ Bounded Chunk Streaming (64KB chunks)        │
                     │ Strict Size Guard (<= max_upload_size_bytes)  │
                     └───────────────────────┬───────────────────────┘
                                             │
                         ┌───────────────────┴───────────────────┐
                         ▼                                       ▼
             ┌───────────────────────┐               ┌───────────────────────┐
             │   IncrementalHasher   │               │   Staging Disk Sink   │
             │   SHA-256 (streaming) │               │   .staging/{uuid}.part│
             └───────────┬───────────┘               └───────────┬───────────┘
                         │                                       │
                         └───────────────────┬───────────────────┘
                                             │ Stream Exhausted
                                             ▼
                             ┌───────────────────────────────┐
                             │ Post-Storage Hash Verification│
                             │ (re-reads file & compares)    │
                             └───────────────┬───────────────┘
                                             │ Verified
                                             ▼
                             ┌───────────────────────────────┐
                             │ Content Validation & Sniffing │
                             │ (magic bytes / JSON / CSV)    │
                             └───────────────┬───────────────┘
                                             │ Valid
                                             ▼
                             ┌───────────────────────────────┐
                             │    Atomic Rename to Exhibit   │
                             │  {case_id}/{ev_id}/{ev_id}.raw│
                             └───────────────┬───────────────┘
                                             │
                                             ▼
                             ┌───────────────────────────────┐
                             │ Manifest Generation & Storage │
                             │         manifest.json         │
                             └───────────────┬───────────────┘
                                             │
                                             ▼
                                EvidenceUploadReceipt (201 Created)
```

### Core Architecture Components
1. **Bounded Streaming (`axum::extract::Multipart`):** Incoming file uploads are streamed in chunks directly from the HTTP connection to disk. Files are never buffered wholly into RAM, guaranteeing predictable $O(1)$ memory usage regardless of file size.
2. **Synchronous Cryptographic Hashing (`sha2::Sha256`):** As bytes stream through memory, they are fed into an incremental SHA-256 hasher before being flushed to disk.
3. **Post-Storage Hash Integrity Verification:** After the stream terminates and file buffers are committed, the engine re-reads the written bytes from disk and re-calculates the digest to guarantee that no bit-rot or file corruption occurred during storage.
4. **Isolated Staging Pipeline (`.staging/{staging_uuid}.part`):** Incomplete or unverified files reside in an ephemeral staging area. Only after full stream intake, post-storage hash verification, and format validation succeed is the file atomically promoted to its permanent forensic exhibit path.
5. **Fail-Safe Cleanup:** If stream transfer aborts, size limit is breached, or content validation fails, all temporary files are unlinked immediately, preventing orphaned artifacts.

---

## 2. Ingestion API Contract

All endpoints adhere to SUTRAM digital forensics specifications and return standard RFC 7807 error envelopes upon failure.

### Endpoints

#### `POST /api/v1/evidence/upload`
Uploads a multimodal digital evidence artifact.

- **Content-Type:** `multipart/form-data`
- **Response Status:** `201 Created`
- **Response Schema (`EvidenceUploadReceipt`):**
```json
{
  "evidence_id": "351d6059-77a6-404d-97fd-b8308e987fad",
  "case_id": "550e8400-e29b-41d4-a716-446655440000",
  "original_filename": "test_upload.pdf",
  "source_type": "DOCUMENT_PDF",
  "mime_type": "application/pdf",
  "file_size_bytes": 44,
  "sha256_hash": "9c7229c9cb2bff4bc01f40fa3420d7e3f3ac38ad52c09a3365754ec4cca7992d",
  "storage_uri": "urn:sutram:evidence:351d6059-77a6-404d-97fd-b8308e987fad:storage",
  "status": "INGESTED",
  "metadata": {
    "source_timezone": "UTC",
    "clock_offset_seconds": 0.0,
    "investigator": "Cooper"
  },
  "created_at": "2026-11-18T11:53:47Z"
}
```

#### `GET /api/v1/evidence/{evidence_id}`
Retrieves metadata and ingestion provenance for a previously ingested exhibit.

- **Path Parameter:** `evidence_id` (UUIDv4)
- **Response Status:** `200 OK` (or `404 Not Found`)
- **Response Schema:** Same as `EvidenceUploadReceipt`.

#### Error Envelope (RFC 7807 / SUTRAM Specification)
```json
{
  "error": {
    "code": "BAD_REQUEST",
    "message": "Missing required multipart field: 'case_id'"
  }
}
```
Standard status code mappings:
- `400 Bad Request`: Missing fields, invalid UUID syntax, malformed multipart streams, empty files.
- `404 Not Found`: Unknown evidence ID lookup.
- `413 Payload Too Large`: Upload stream exceeds configured byte ceiling (`max_upload_size_bytes`).
- `422 Unprocessable Entity`: Corrupted files, malformed JSON, or unrecognized binary signatures.
- `500 Internal Server Error`: Disk write failures or filesystem permission denials.

---

## 3. Multipart Form Field Specification

The multipart parser supports arbitrary field order (e.g. `case_id` may precede or succeed `file`).

| Field Name | Type | Required | Description | Default |
|---|---|---|---|---|
| `case_id` | String (UUID) | **Yes** | Valid UUIDv4 representing the forensic case. Validated against UUID syntax; rejects directory traversal. | *None* |
| `file` | Binary Stream | **Yes** | Raw exhibit content. Must be non-empty and conform to a recognized format. | *None* |
| `source_type` | String | No | Declared source hint (e.g., `DOCUMENT_PDF`, `CHAT_EXPORT`, `TABULAR_CSV`). Auto-detected if omitted. | Inferred from content |
| `source_device` | String | No | Device identifier (e.g. `iPhone 13`, `Hikvision NVR`). | `null` |
| `source_timezone` | String | No | Source timezone string. Validated and normalized. | `"UTC"` |
| `clock_offset_seconds` | Float | No | Clock drift adjustment for timestamp normalization. | `0.0` |
| `metadata` | JSON String | No | Arbitrary forensic key-value dictionary (e.g. investigator name, chain-of-custody notes). | `{}` |

---

## 4. Storage Directory Structure and Path Sanitization

### Filesystem Layout
All files are organized deterministically under the configured storage root (`evidence_storage_path`, defaults to `data/evidence/raw`):

```
data/evidence/raw/
├── .staging/                                   <- Ephemeral uploads in progress
│   └── 9b2d3042-3004-4fa0-8f92-9658f8ea73aa.part
└── {case_id}/                                  <- Forensic case container (UUID)
    └── {evidence_id}/                          <- Independent exhibit container (UUID)
        ├── {evidence_id}.raw                   <- Immutable raw byte-exact exhibit
        └── manifest.json                       <- Provenance manifest and metadata
```

### Path Traversal Defense and Sanitization
1. **Container Isolation:** File paths are constructed exclusively using validated UUIDs:
   $$\text{Path} = \text{Root} / \text{case\_id} / \text{evidence\_id} / (\text{evidence\_id} + \text{".raw"})$$
   Because both `case_id` and `evidence_id` are strictly validated UUID strings (`[0-9a-f-]{36}`), traversal sequences (`../`, `..\`) cannot manipulate directory depth.
2. **Declared Filename Sanitization (`sanitize_filename`):**
   - Strips directory components (`/` and `\`).
   - Strips non-ASCII characters, control characters, and null bytes (`\0`).
   - Strips filesystem-illegal characters (`< > : " / \ | ? *`).
   - Strips leading and trailing dots and whitespace.
   - Prepends `safe_` if the base filename matches Windows reserved device names (`CON`, `PRN`, `AUX`, `NUL`, `COM1`-`COM9`, `LPT1`-`LPT9`).
   - Falls back to `sanitized_evidence.bin` if sanitization empties the name.
3. **Opaque Storage URNs:** Absolute filesystem paths are never returned in client API responses. The response provides an immutable, location-independent URI:
   $$\text{storage\_uri} = \text{"urn:sutram:evidence:\{evidence\_id\}:storage"}$$

---

## 5. Cryptographic Hash Calculation & Verification

### Streaming Digest Computation
Rather than buffering complete files in memory, the engine streams incoming chunks through `sha2::Sha256`:
```rust
let mut hasher = IncrementalHasher::new();
while let Some(chunk) = field.chunk().await? {
    hasher.update(&chunk);
    file.write_all(&chunk).await?;
}
let (sha256_hex, total_bytes) = hasher.finalize();
```

### Post-Storage Integrity Verification
After the `.part` file is completely written and flushed to physical storage:
1. The engine opens the written file from disk via `tokio::fs::File`.
2. It streams the written bytes through an independent SHA-256 calculation (`verify_file_sha256`).
3. If the post-storage digest does not exactly match the streaming digest, the file is unlinked immediately and an `INTERNAL_SERVER_ERROR` is raised.

This two-phase check provides definitive mathematical proof that bytes written to disk match the stream in transit without silent storage controller corruption.

---

## 6. Content Validation and Format Detection

Content validation is format-aware, inspecting magic bytes and structural invariants.

| Declared / Detected Type | Magic Signature / Format Criteria | MIME Type |
|---|---|---|
| `DOCUMENT_PDF` | `%PDF-` (bytes `0x25, 0x50, 0x44, 0x46, 0x2D`) | `application/pdf` |
| `IMAGE_PNG` | PNG header (`\x89PNG\r\n\x1a\n`) | `image/png` |
| `IMAGE_JPEG` | JPEG SOI marker (`\xFF\xD8\xFF`) | `image/jpeg` |
| `VIDEO_CCTV` | MP4/MOV `ftyp` box at byte offset 4 | `video/mp4` |
| `CHAT_EXPORT` | Valid JSON containing `"messages"` or `"conversations"` key | `application/json` |
| `GENERIC_JSON` | Valid JSON object or array without chat semantics | `application/json` |
| `TABULAR_CSV` | Valid text/CSV structure with delimiters (RFC 4180) | `text/csv` |

### Critical Forensic Boundary: `TABULAR_CSV` vs `CALL_DETAIL_RECORD`
As mandated by SUTRAM architectural directives:
- The ingestion layer classifies generic CSVs strictly as **`TABULAR_CSV`** (MIME: `text/csv`).
- It **never** assigns `CALL_DETAIL_RECORD` at ingestion time.
- Forensic classification as a CDR requires column schema inspection, phone number normalization, and carrier dial-code validation, which belongs strictly to the **Phase 1H extraction layer**.

### JSON Chat Inspection
JSON files are parsed using `serde_json::from_slice`.
- If the top-level structure is an object containing `"messages"` or `"conversations"` keys, it is classified as `CHAT_EXPORT`.
- Otherwise, valid JSON is classified as `GENERIC_JSON`.
- Malformed JSON (syntax errors, unterminated strings) is immediately rejected with HTTP `422 Unprocessable Entity`.

---

## 7. Forensic Duplicate Exhibit Handling

In criminal investigations, multiple identical digital items may be seized from distinct sources (e.g. an identical PDF contract seized from two suspects, or the same image forwarded across different devices).

- **Principle:** SUTRAM treats every evidence submission as an independent physical/digital exhibit.
- **Deduplication Policy:** The ingestion engine **does not** reject or overwrite evidence items that have identical SHA-256 digests.
- **Exhibit Isolation:** Each upload is issued a distinct `evidence_id` (UUIDv4) and stored in its own isolated exhibit directory (`{case_id}/{evidence_id}/`).
- **Chain of Custody:** Independent metadata (source device, upload timestamp, investigator notes) is preserved per exhibit without cross-talk or mutation.

---

## 8. Atomic Writes and Partial Upload Cleanup

### Staging Lifecycle
To guarantee that incomplete, interrupted, or malicious uploads never corrupt the evidence repository:
1. Every upload writes to a temporary staging file:
   $$\text{data/evidence/raw/.staging/\{staging\_uuid\}.part}$$
2. The byte count is checked incrementally after every chunk. If `total_bytes > max_upload_size_bytes`, writing aborts immediately and the `.part` file is deleted.
3. If an error occurs during upload (client disconnect, bad JSON, unsupported magic bytes), the staging file is unlinked immediately.
4. Only upon 100% successful validation and hash verification is the staging file atomically moved (`tokio::fs::rename`) into the final exhibit path:
   $$\text{data/evidence/raw/\{case\_id\}/\{evidence\_id\}/\{evidence\_id\}.raw}$$
5. If rename succeeds, the accompanying `manifest.json` is atomically written.

No `.part` or temporary file ever remains on disk following an error.

---

## 9. Metadata Format and Filesystem Manifest

In the absence of a live relational database (scheduled for Phase 1J), metadata persistence is achieved through a filesystem-backed `manifest.json` stored alongside each `.raw` evidence file.

### Schema (`manifest.json`)
```json
{
  "evidence_id": "351d6059-77a6-404d-97fd-b8308e987fad",
  "case_id": "550e8400-e29b-41d4-a716-446655440000",
  "original_filename": "test_upload.pdf",
  "source_type": "DOCUMENT_PDF",
  "mime_type": "application/pdf",
  "file_size_bytes": 44,
  "sha256_hash": "9c7229c9cb2bff4bc01f40fa3420d7e3f3ac38ad52c09a3365754ec4cca7992d",
  "storage_path": "data/evidence/raw/550e8400-e29b-41d4-a716-446655440000/351d6059-77a6-404d-97fd-b8308e987fad/351d6059-77a6-404d-97fd-b8308e987fad.raw",
  "storage_uri": "urn:sutram:evidence:351d6059-77a6-404d-97fd-b8308e987fad:storage",
  "status": "INGESTED",
  "metadata": {
    "clock_offset_seconds": 0.0,
    "investigator": "Cooper",
    "source_timezone": "UTC"
  },
  "created_at": "2026-11-18T11:53:47Z"
}
```

### Survival Across Process Restarts
When `GET /api/v1/evidence/{evidence_id}` is requested:
1. The engine checks its in-memory cache.
2. If not cached (e.g. after a process restart), it scans the filesystem under `{storage_root}/*/{evidence_id}/manifest.json`.
3. If found, it deserializes the manifest and serves the receipt immediately, ensuring zero state loss across server restarts.

---

## 10. Security Boundaries & Resource Limits

1. **Size Limits:** Enforced via `max_upload_size_bytes` (configured via `MAX_UPLOAD_SIZE_BYTES` environment variable; default: 50 MB / 52,428,800 bytes).
2. **Memory Boundedness:** Uploads use Axum streaming with 64KB buffers. Memory usage remains bounded to $O(1)$ regardless of concurrent transfer sizes.
3. **Safe Storage IDs:** Directory creation strictly verifies `case_id` and `evidence_id` against UUID character sets (`[0-9a-f-]{36}`). Any traversal character (`..`, `/`, `\`) results in an immediate rejection.
4. **Original Evidence Immutability:** Stored files are opened in write mode only during staging creation; once atomically moved to `.raw`, they are read-only and never modified in place.

---

## 11. Test Suite Summary and Quality Metrics

The test suite covers all 25 mandated forensic scenarios:

| Test ID | Scenario | Status |
|---|---|---|
| 1 | Successful small-file upload | Verified |
| 2 | Successful large streaming upload within configured limit | Verified |
| 3 | SHA-256 accuracy for uploaded content | Verified |
| 4 | Stored file bytes match uploaded bytes exactly | Verified |
| 5 | Post-storage SHA-256 verification | Verified |
| 6 | Empty file rejection (HTTP 400) | Verified |
| 7 | Missing multipart file rejection (HTTP 400) | Verified |
| 8 | Invalid case UUID rejection (HTTP 400) | Verified |
| 9 | Unsupported content rejection (HTTP 422) | Verified |
| 10 | Invalid content signature rejection (HTTP 422) | Verified |
| 11 | Malformed JSON rejection (HTTP 422) | Verified |
| 12 | Generic CSV classified as `TABULAR_CSV`, NOT `CALL_DETAIL_RECORD` | Verified |
| 13 | Maximum upload size enforcement (HTTP 413) | Verified |
| 14 | Interrupted upload cleanup | Verified |
| 15 | Storage failure handling & atomicity | Verified |
| 16 | Temporary staging file cleanup verification | Verified |
| 17 | Path traversal filename attack sanitization | Verified |
| 18 | Windows reserved filename sanitization (`CON`, `PRN`, etc.) | Verified |
| 19 | Duplicate filename isolation | Verified |
| 20 | Identical file contents uploaded twice receive distinct UUIDs | Verified |
| 21 | No existing evidence file is overwritten | Verified |
| 22 | Absolute filesystem paths not disclosed in API responses | Verified |
| 23 | Metadata retrieval after upload via `GET` | Verified |
| 24 | Metadata survives process restart via filesystem manifest | Verified |
| 25 | Original evidence remains unchanged and immutable | Verified |

### Test Execution Results
- **Rust Unit Tests (`src/lib.rs`):** 15 passed, 0 failed, 0 ignored.
- **Rust Health Tests (`tests/health.rs`):** 2 passed, 0 failed, 0 ignored.
- **Rust Ingestion Tests (`tests/ingestion.rs`):** 13 passed (covering all 25 scenarios), 0 failed, 0 ignored.
- **Total Rust Tests:** **30 passed; 0 failed; 0 ignored.**
- **Python Reference Test Suite (`pytest -q`):** **144 passed, 50 skipped, 0 failed.**
- **Clippy:** `cargo clippy --locked --tests --target x86_64-pc-windows-gnullvm -- -D warnings` passed with 0 warnings.
- **Fmt:** `cargo fmt --check` passed cleanly.
- **Release Build:** `cargo build --release --locked --target x86_64-pc-windows-gnullvm` succeeded.
- **Live HTTP Test:** Verified live release binary on localhost with `curl.exe` for both `POST /upload` and `GET /:id`.

---

## 12. Future PostgreSQL Persistence Requirements (Phase 1J)

In Phase 1J, relational database persistence will be integrated using SQLx:
1. **Table Schema (`evidence_items`):**
   ```sql
   CREATE TABLE evidence_items (
       evidence_id UUID PRIMARY KEY,
       case_id UUID NOT NULL REFERENCES cases(id),
       original_filename TEXT NOT NULL,
       source_type TEXT NOT NULL,
       mime_type TEXT NOT NULL,
       file_size_bytes BIGINT NOT NULL,
       sha256_hash CHAR(64) NOT NULL,
       storage_path TEXT NOT NULL,
       storage_uri TEXT NOT NULL,
       status TEXT NOT NULL,
       metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
       created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
   );
   CREATE INDEX idx_evidence_case_id ON evidence_items(case_id);
   CREATE INDEX idx_evidence_sha256 ON evidence_items(sha256_hash);
   ```
2. **Transactional Boundary:** After the atomic `.part` -> `.raw` rename succeeds on disk, a SQL transaction will insert the `evidence_items` record. If the database insert fails, the `.raw` file will be rolled back.
3. **Audit Trail:** An append-only `chain_of_custody_log` table will log ingestion timestamps, IP address, user/agent credentials, and verification checksums.

---

## 13. Extraction Dispatcher Integration Preparation (Phase 1H)

Phase 1H will implement the unified forensic extraction subsystem in Rust:
1. **Consumption Contract:** The extraction dispatcher will receive an `evidence_id` and query `StorageManager::get_raw_file_path(case_id, evidence_id)`.
2. **MIME/Type Routing:**
   - `DOCUMENT_PDF`: Routed to the PDF text extraction and OCR dispatcher.
   - `IMAGE_PNG` / `IMAGE_JPEG`: Routed to image EXIF extraction and OCR.
   - `VIDEO_CCTV`: Routed to CCTV frame sampling and timestamp analysis.
   - `CHAT_EXPORT`: Routed to structured message parsing (WhatsApp, Signal, Telegram formats).
   - `TABULAR_CSV`: Inspected for CDR header structures (`timestamp`, `caller_msisdn`, `cell_tower_id`). If CDR signatures match, promoted to `CALL_DETAIL_RECORD` and extracted into call graph events.
3. **Immutability Contract:** The extraction modules will open `.raw` files with read-only file descriptors (`tokio::fs::File::open`), producing derived extraction manifests without modifying original exhibits.
