# SUTRAM Digital Forensics — Phase 1H: Rust Forensic Extraction Engine Report

**Document ID:** `docs/member-1/phase-1h-rust-extraction-engine.md`  
**Phase:** 1H (Rust Forensic Extraction Engine)  
**Author:** Member 1 (Lead Rust AI/ML and Forensic Extraction Engineer)  
**Target Repository:** `SutramPrototype` (`main` branch)  
**Timestamp:** 2026-11-18  

---

## 1. Executive Summary & Git Safety Status

- **Role:** Lead Rust AI/ML & Forensic Extraction Engineer (SUTRAM 2026).
- **Core Mission:** Migrate the complete evidence extraction and correlation subsystem (CDR CSV parser, Chat JSON parser, pattern-based NER, deterministic entity resolution, observed communication relationship generation, and unified extraction dispatcher) into Rust (`backend-rust/`).
- **Git Safety Status:**
  - Branch: `main` (base commit `179abe0`).
  - Remote Synchronization Audit: `origin/main` advanced to `fb26972` (6 commits by Member 3 modifying `apps/web/`, frontend docs, and product wordmark renaming). In accordance with Mandatory Git Safety directives, **no automatic rebase or merge was executed** to safeguard uncommitted workspace artifacts from Phases 1A–1G. Zero commits or pushes performed.
- **Reference Parity:** Python backend preserved as an immutable reference implementation. Python pytest suite passes with **144 passed, 50 skipped, 0 failed**.
- **Rust Test Suite:** **56 tests passed; 0 failed; 0 ignored** (15 unit, 2 health, 13 ingestion, 26 extraction covering all 25 mandated forensic scenarios).

---

## 2. Rust Extraction & Correlation Architecture

The extraction engine is structured into two decoupled subsystems under `backend-rust/src/`:

```
backend-rust/src/
├── extraction/                         <- Ingestion-adjacent forensic parsers & dispatcher
│   ├── mod.rs                          <- Clean re-exports of models, parsers, and service
│   ├── model.rs                        <- Canonical extraction DTOs (Request, Result, Candidates)
│   ├── cdr.rs                          <- RFC 4180-compliant CDR CSV parser & row validator
│   ├── chat.rs                         <- Serde-based JSON chat parser (WhatsApp/Signal/Telegram)
│   ├── identifiers.rs                  <- Regex NER engine (Phone, Email, IMEI, IMSI, Ambiguous)
│   └── service.rs                      <- Unified dispatcher entrypoint (`extract_evidence`)
└── correlation/                        <- Entity resolution & relationship generation
    ├── mod.rs                          <- Clean re-exports of resolution and relationships
    ├── resolution.rs                   <- E.164 phone canonicalizer & deterministic deduplicator
    └── relationships.rs                <- Directly observed communication relationship synthesizer
```

### Architectural Separation of Responsibilities
1. **Low-Level Parsing (`cdr.rs`, `chat.rs`):** Ingests raw text or bytes, enforces format schemas, validates row/record structures, extracts structural timestamps and identifiers, and isolates failures into structured `ExtractionValidationIssue` objects without crashing.
2. **Identifier Extraction (`identifiers.rs`):** Scans message bodies and unstructured text for technical indicators (E.164 phones, RFC 5322 emails, 15-digit IMEIs/IMSIs). Maintains exact Unicode character offsets matching Python string slicing.
3. **Correlation & Resolution (`correlation/`):** Groups technical identifiers deterministically under verified canonical forms, attaches forensic audit disclaimers, and synthesizes directly observed communication relationships.
4. **Unified Dispatcher (`service.rs`):** Validates physical file existence, verifies cryptographic SHA-256 pre-read hashes, routes by modality, coordinates entity deduplication, and calculates execution telemetry.

---

## 3. Ingestion Route Audit & Compatibility

During the Phase 1G audit, a discrepancy was noted between shorthand route documentation and the registered Axum paths:
- **Actual Registered Ingestion Endpoints (`backend-rust/src/app.rs`):**
  - `GET /health` -> Basic service health check (`{"status":"ok","service":"sutram-backend-rust"}`)
  - `POST /api/v1/evidence/upload` -> Bounded multipart streaming upload
  - `GET /api/v1/evidence/:evidence_id` -> Metadata receipt lookup with filesystem manifest fallback
- **Storage Path Resolution:**
  All exhibits reside at `{evidence_storage_path}/{case_id}/{evidence_id}/{evidence_id}.raw`.
- **Safe Dispatcher Construction:**
  `EvidenceExtractionService::build_request_from_manifest(&manifest, &storage, default_prefix)` converts a trusted `EvidenceManifest` into an `EvidenceExtractionRequest`. This enforces that filesystem paths originate strictly from `StorageManager`, preventing arbitrary external path injection.

---

## 4. CDR CSV Parser (`cdr.rs`)

The CDR parser processes telephony Call Detail Records using the `csv` crate (RFC 4180 compliant) with the following forensic invariants:

1. **Mandatory Header Enforcement:**
   Headers are lowercased and trimmed. The CSV must contain:
   `record_id`, `caller_phone`, `receiver_phone`, `call_timestamp`, `duration_seconds`.
   Optional headers: `cell_tower_id`, `call_type`.
   If any required header is missing, parsing halts immediately with:
   `issue_type: "MISSING_REQUIRED_HEADERS"` and `valid_records_count = 0`.
2. **Strict Quoting & Delimiter Support:**
   Fields containing commas, line breaks, or quotation marks (e.g. `"TOWER-A, NORTH"`) are parsed without splitting or truncating columns.
3. **Row-Level Validation & Error Diagnostics:**
   - Missing `record_id`, `caller_phone`, `receiver_phone`, or `call_timestamp` -> `MISSING_REQUIRED_FIELD`.
   - Non-numeric or negative duration -> `INVALID_DURATION`.
   - Column count mismatch -> `MALFORMED_ROW_COLUMN_MISMATCH`.
4. **Partial Success Semantics:**
   Corrupt rows generate structured `ExtractionValidationIssue` items. Valid rows produce `ParsedCDRRecord`, `ExtractedEventCandidate`, and `ProvisionalRelationshipCandidate` objects.

---

## 5. Chat JSON Parser (`chat.rs`)

The messaging parser supports chat exports (WhatsApp, Signal, Telegram) formatted as JSON:

1. **Root Schema Inspection:**
   The root must be a JSON object containing either a `"messages"` array or a `"conversations"` array. Unrelated JSON or malformed syntax returns `JSON_DECODE_ERROR` or `INVALID_ROOT_OBJECT`.
2. **Message Item Validation:**
   Each message must contain:
   - `message_id` (or `id`)
   - `sender_id` (or `sender`)
   - `timestamp`
   - `content` (or `text`, must be string)
3. **Multi-Party Recipients:**
   Supports 1-on-1 and group chats. For group messages with $N$ recipients, $N$ independent `ProvisionalRelationshipCandidate` links are emitted (`sender -> recipient_i`).
4. **Embedded Text Extraction:**
   Every valid message text is routed to `extract_identifiers_from_text` to extract embedded phones, emails, IMEIs, and IMSIs.

---

## 6. Pattern-Based Identifier Extraction & Offset Semantics (`identifiers.rs`)

### Regex Patterns & Categories
- **Email:** `\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b` (Normalized to lowercase).
- **Phone Number:** Matches international formats (`+`, `00`) and domestic numbers (7–15 digits).
- **14–15 Digit Sequences:** Matches candidate IMEIs and IMSIs (`\b\d{14,15}\b`).
  - Preceding or following keywords matching `(?i)\b(?:imei|device|handset|terminal|hardware|tac)\b` classify the sequence as **`IMEI`**.
  - Keywords matching `(?i)\b(?:imsi|sim|subscriber|iccid|telecom)\b` classify the sequence as **`IMSI`**.
  - Missing keywords or conflicting keywords in context classify the sequence as **`AMBIGUOUS_NUMERIC_15`** (`is_ambiguous = true`).

### IMEI Luhn Checksum Telemetry
- Evaluated via `luhn_checksum_valid(digits)`.
- Recorded in `is_checksum_valid: Option<bool>`.
- **Forensic Guarantee:** Never applied to IMSI candidates (`is_checksum_valid = None`).
- **Confidence Decoupling:** Invalid-checksum IMEIs are preserved intact with `is_checksum_valid = Some(false)`. Regex extractions maintain `extraction_confidence = None` (no synthetic confidence scores fabricated).

### Critical Unicode Character Offset Parity
In Python, strings are indexed by Unicode code points (characters). In Rust, regex matches return UTF-8 byte offsets.
If non-ASCII characters appear in evidence (e.g. `"नमस्ते! Contact lead at user@cybercell.gov.in"`):
- The Devnagari characters occupy 3 bytes each in UTF-8, but represent 1 code point in Python.
- Rust maps byte offsets to character counts: `text[..byte_idx].chars().count()`.
- **Parity Verified:** Verified that both Python and Rust calculate `char_offset:24-45` for `user@cybercell.gov.in` in Hindi text.

---

## 7. Deterministic Entity Resolution (`resolution.rs`)

The entity resolution module deterministically deduplicates raw mentions into `ResolvedEntityCandidate` records:

1. **Phone Number Normalization (`normalize_phone_number`):**
   - Leading `+` -> `EXPLICIT_E164_PLUS`.
   - Leading `00` -> `INTERNATIONAL_TRUNK_00` (`+` replaces `00`).
   - 10-digit domestic numbers with configured `default_country_prefix` (`91` for India) -> `CONFIGURED_DEFAULT_COUNTRY_PREFIX` (strips domestic trunk `0` if present).
   - Ambiguous domestic numbers without a default prefix -> `AMBIGUOUS_COUNTRY_CODE` (not normalized).
2. **Canonical Identifier Grouping:**
   - Phone numbers: Normalized E.164 if resolved, else trimmed raw string.
   - Emails: Lowercased string.
   - IMEIs / IMSIs: Exact digit string.
   - Ambiguous sequences: Kept isolated under `AMBIGUOUS_NUMERIC_15:{raw_val}`.
3. **Forensic Disclaimer:**
   Every candidate includes the mandatory legal disclaimer:
   > *"Normalized identifiers represent technical communication addresses or device hardware signatures, and do NOT constitute conclusive legal proof of human identity."*

---

## 8. Observed Communication Relationships (`relationships.rs`)

Relationships generated in Phase 1H are strictly restricted to **directly observed communication events**:
- `CALL_COMMUNICATION`: Generated from valid CDR rows (`caller -> callee`).
- `MESSAGE_COMMUNICATION`: Generated from chat messages (`sender -> recipient`).
- **Forensic Guardrails:**
  - `is_observed = true`.
  - Evidence basis bounds the candidate to `source_evidence_id`.
  - No speculative associations, ownership inferences, guilt, or criminal conspiracy links are generated.

---

## 9. Unified Dispatcher Entrypoint (`service.rs`)

The canonical dispatcher `extract_evidence` enforces:
1. **Physical File Existence:** Rejects missing files with `FILE_NOT_FOUND`.
2. **Pre-Extraction SHA-256 Verification:** Reads disk bytes and computes SHA-256 before parsing. Rejects mismatches with `HASH_MISMATCH`.
3. **Modality Check:** Supported types: `CALL_DETAIL_RECORD` (or `CDR`) and `CHAT_EXPORT` (or `CHAT`). Rejects others with `UNSUPPORTED_SOURCE_TYPE`.
4. **Execution Telemetry:** Records `processing_time_ms`, record counts, and parser metadata.
5. **Discrete Status Values:**
   - `SUCCESS`: $\ge 1$ valid records, 0 rejected.
   - `PARTIAL_SUCCESS`: $\ge 1$ valid records, $\ge 1$ rejected.
   - `FAILED`: 0 valid records.

---

## 10. Behavioral Parity Test Results (25 Forensic Scenarios)

All 25 mandated parity scenarios were implemented in [`backend-rust/tests/extraction.rs`](file:///c:/Users/krish/Desktop/SutramPrototype/backend-rust/tests/extraction.rs) and passed:

| # | Forensic Scenario | Verification Status |
|---|---|---|
| 1 | Valid CDR parsing from synthetic fixture | **PASSED** |
| 2 | Malformed CDR rows with row-level issue reporting | **PASSED** |
| 3 | Quoted CSV fields with embedded commas | **PASSED** |
| 4 | CRLF, leading/trailing whitespace & empty row handling | **PASSED** |
| 5 | Required CDR header validation & rejection of generic CSV | **PASSED** |
| 6 | Valid chat JSON parsing from synthetic fixture | **PASSED** |
| 7 | Multiple messages and 1-to-many recipient relationship generation | **PASSED** |
| 8 | Malformed chat records (missing keys, non-string content) | **PASSED** |
| 9 | Phone number extraction from unstructured text | **PASSED** |
| 10 | Email extraction, lowercasing, and character offsets | **PASSED** |
| 11 | Contextual IMEI keyword identification | **PASSED** |
| 12 | Contextual IMSI keyword identification | **PASSED** |
| 13 | IMEI Luhn checksum validity telemetry | **PASSED** |
| 14 | Isolated / conflicting numeric strings classified as ambiguous | **PASSED** |
| 15 | E.164 normalization (+, 00, domestic trunk 0 with default country) | **PASSED** |
| 16 | Deterministic entity resolution, deduplication & disclaimer binding | **PASSED** |
| 17 | Observed call communication relationship generation | **PASSED** |
| 18 | Observed message communication relationship generation | **PASSED** |
| 19 | Provenance preservation across identifiers, events & relationships | **PASSED** |
| 20 | Unicode character offset parity matching Python string code points | **PASSED** |
| 21 | Extraction confidence decoupled from mathematical checksum | **PASSED** |
| 22 | Partial extraction status computation (`PARTIAL_SUCCESS`) | **PASSED** |
| 23 | Unsupported evidence formats rejected without crash | **PASSED** |
| 24 | Raw evidence file immutability verified (SHA-256 and mtime identical) | **PASSED** |
| 25 | SHA-256 mismatch rejection before parser execution | **PASSED** |

### Test Suite Execution Summary
- **Rust Extraction Integration Tests (`tests/extraction.rs`):** **26 passed; 0 failed**
- **Rust Ingestion Tests (`tests/ingestion.rs`):** **13 passed; 0 failed**
- **Rust Health Tests (`tests/health.rs`):** **2 passed; 0 failed**
- **Rust Library Unit Tests (`src/lib.rs`):** **15 passed; 0 failed**
- **Total Rust Tests:** **56 passed; 0 failed; 0 ignored** in 0.37s
- **Python Reference Test Suite (`pytest -q`):** **144 passed, 50 skipped, 0 failed** in 3.47s
- **Clippy:** Clean (0 warnings under `-D warnings`)
- **Fmt:** Clean (`cargo fmt --check` exit code 0)
- **Release Build:** Succeeded (`cargo build --release --locked`)

---

## 11. Exact Integration Contract for Phase 1I

Phase 1I will integrate Rust evidence ingestion (`backend-rust/src/ingestion/`) with Rust extraction (`backend-rust/src/extraction/`):

1. **Upload Trigger Hook:**
   Upon successful file intake in `POST /api/v1/evidence/upload`:
   ```rust
   // Phase 1I pipeline:
   let req = EvidenceExtractionService::build_request_from_receipt(
       &receipt,
       &storage,
       Some("91".to_string()),
   )?;
   let extraction_result = extract_evidence(&req);
   ```
2. **On-Demand Reprocessing Route:**
   `POST /api/v1/evidence/:evidence_id/reprocess`:
   - Looks up `EvidenceManifest` from `StorageManager::find_manifest(&evidence_id)`.
   - Constructs `EvidenceExtractionRequest`.
   - Executes `extract_evidence(&req)`.
   - Returns serialized `UnifiedExtractionResult`.

---

## 12. Implemented vs. Deferred Boundaries

### Implemented in Phase 1H:
- Full Rust CDR CSV parser with RFC 4180 quotation handling.
- Full Rust Chat JSON parser supporting 1-to-many recipients.
- Pattern-based NER with Unicode character offset parity.
- Luhn checksum calculation decoupled from extraction confidence.
- Deterministic entity resolution and deduplication.
- Directly observed communication relationship synthesis.
- Unified dispatcher entrypoint with pre-read SHA-256 verification.
- 26 comprehensive integration tests covering all 25 mandated scenarios.

### Deferred to Future Phases:
- **Phase 1I:** Connecting extraction dispatcher to public ingestion upload and reprocessing HTTP routes.
- **Phase 1J:** PostgreSQL relational storage via SQLx for evidence items, manifests, and extraction candidates.
- **Phase 2A–2D:** Migration of Swastik's temporal and spatial reconstruction modules and Neo4j graph persistence.
