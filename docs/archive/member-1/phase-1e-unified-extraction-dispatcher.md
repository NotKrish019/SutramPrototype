# SUTRAM 2026 — Phase 1E: Unified Evidence Extraction Dispatcher Report

**Document ID:** `docs/member-1/phase-1e-unified-extraction-dispatcher.md`  
**Phase:** 1E (Unified Evidence Extraction Dispatcher)  
**Author:** Combined Member 1 (AI/ML & Evidence Intelligence) & Member 2 (Backend & Data Engineering)  
**Target Repository:** `SutramPrototype` (`main` branch)  
**Timestamp:** 2026-11-18  

---

## 1. Architecture Overview

Phase 1E introduces the canonical, centralized extraction dispatcher for the SUTRAM 2026 Digital Forensics platform. The dispatcher decouples evidence ingestion and storage from domain-specific forensic parsing engines while strictly enforcing cryptographic data provenance, forensic immutability, and pre-extraction integrity validation.

```
+-------------------------------------------------------------------------+
|                       Phase 1D Ingestion Service                        |
|   (Multipart Upload -> Content Sniffing -> Streaming SHA-256 Storage)   |
+------------------------------------+------------------------------------+
                                     |
                                     | Invokes with EvidenceExtractionRequest
                                     v
+-------------------------------------------------------------------------+
|                  Phase 1E Unified Extraction Dispatcher                 |
|             backend/app/modules/extraction/service.py: extract_evidence |
+------------------------------------+------------------------------------+
                                     |
          +--------------------------+--------------------------+
          | Pre-Extraction Checks:                              |
          | - Target File Existence                             |
          | - Cryptographic SHA-256 Verification                |
          | - Modality Normalization & Header Verification       |
          +--------------------------+--------------------------+
                                     |
                  +------------------+------------------+
                  |                                     |
                  v                                     v
+----------------------------------+ +----------------------------------+
|          CDR CSV Parser          | |         Chat JSON Parser         |
|   (app/modules/extraction/cdr)   | |   (app/modules/extraction/chat)  |
| - Header validation              | | - Structural object validation   |
| - Telephone event candidates     | | - Regex NER (IMEI/IMSI/Phone)    |
| - Call communication links       | | - Message communication links    |
+-----------------+----------------+ +-----------------+----------------+
                  |                                    |
                  +------------------+-----------------+
                                     |
                                     v
+-------------------------------------------------------------------------+
|                Deterministic Entity Resolution Module                   |
|       (app/modules/correlation/resolution: deduplicate_identifiers)      |
| - E.164 phone canonicalization                                          |
| - Case-insensitive email grouping                                       |
| - Exact IMEI/IMSI grouping with Luhn telemetry decoupling               |
| - Forensic disclaimer binding                                           |
+------------------------------------+------------------------------------+
                                     |
                                     v
+-------------------------------------------------------------------------+
|                       UnifiedExtractionResult                           |
| - Status: SUCCESS | PARTIAL_SUCCESS | FAILED                            |
| - Structured ExtractionValidationIssues (no silent drops)               |
| - Execution Metrics (processing_time_ms)                                |
| - Audit Metadata & Provenance Citations                                 |
+-------------------------------------------------------------------------+
```

---

## 2. Dispatcher Location & Module Exports

The canonical dispatcher is implemented directly inside the extraction module domain:
- **Service Implementation:** [`backend/app/modules/extraction/service.py`](../../../backend/app/modules/extraction/service.py)
- **Canonical Public Entrypoint:** `extract_evidence(request: EvidenceExtractionRequest) -> UnifiedExtractionResult`
- **Module Interface:** Re-exported cleanly in [`backend/app/modules/extraction/__init__.py`](../../../backend/app/modules/extraction/__init__.py):
  ```python
  from app.modules.extraction.service import extract_evidence
  from app.schemas.extraction import (
      EvidenceExtractionRequest,
      ExtractionStatus,
      UnifiedExtractionResult,
  )
  ```

---

## 3. Request and Result Models

All models adhere to strict Pydantic v2 conventions (`model_config = ConfigDict(frozen=True)`), preventing accidental in-memory mutation.

### 3.1. Request DTO (`EvidenceExtractionRequest`)

Defined in [`backend/app/schemas/extraction.py`](../../../backend/app/schemas/extraction.py):

| Field Name | Type | Required | Description |
|---|---|---|---|
| `evidence_id` | `str` | Yes | Canonical UUIDv4 of the evidence item. |
| `case_id` | `str` | Yes | Associated forensic case identifier. |
| `source_type` | `str` | Yes | Evidence modality (e.g. `CALL_DETAIL_RECORD`, `CHAT_EXPORT`). |
| `file_path` | `Union[str, Path]` | Yes | Internal filesystem path originating from trusted ingestion storage. |
| `expected_sha256` | `str` (64 chars) | Yes | Cryptographic SHA-256 hash computed during initial upload. |
| `source_timezone` | `Optional[str]` | No | Stated source timezone (e.g. `Asia/Kolkata`, `UTC`). |
| `clock_offset_seconds`| `Optional[float]` | No | Physical clock drift adjustment in seconds. |
| `metadata` | `dict[str, Any]` | No | Validated ingestion metadata from the upload record. |
| `default_country_prefix`| `Optional[str]` | No | Configured dialing code (e.g. `"91"`) for deterministic E.164 phone resolution. |

### 3.2. Extraction Status (`ExtractionStatus`)

Enum specifying the discrete forensic extraction outcome:
- `SUCCESS`: One or more valid records extracted with zero rejected rows/messages.
- `PARTIAL_SUCCESS`: At least one valid record extracted, but one or more records failed validation. Valid candidates are preserved alongside structured `validation_issues`.
- `FAILED`: Zero valid records parsed, file not found, hash mismatch, unsupported source type, unhandled parser exception, or entire file malformed.

### 3.3. Result DTO (`UnifiedExtractionResult`)

| Field Name | Type | Description |
|---|---|---|
| `evidence_id` | `str` | Target evidence identifier. |
| `case_id` | `str` | Associated forensic case identifier. |
| `source_type` | `str` | Modality processed. |
| `status` | `ExtractionStatus` | `SUCCESS`, `PARTIAL_SUCCESS`, or `FAILED`. |
| `identifiers` | `list[ExtractedIdentifier]` | Discrete technical identifiers (phones, emails, IMEIs, IMSIs). |
| `resolved_entities` | `list[ResolvedEntityCandidate]` | Deduplicated entities grouped by verified canonical identifier. |
| `event_candidates` | `list[ExtractedEventCandidate]` | Temporal activity candidates synthesized from valid records. |
| `relationship_candidates` | `list[ProvisionalRelationshipCandidate]` | Directly observed communication interaction candidates. |
| `validation_issues` | `list[ExtractionValidationIssue]` | Structured diagnostic issues for all rejected/malformed rows or failures. |
| `total_records_processed` | `int` | Total rows or messages inspected in the file. |
| `valid_records_count` | `int` | Count of successfully parsed and validated records. |
| `rejected_records_count` | `int` | Count of rejected records or failure diagnostics. |
| `processing_time_ms` | `Optional[float]` | Execution duration in milliseconds. |
| `extraction_metadata` | `dict[str, Any]` | Technical audit metadata (parser name, parser version, verified hash). |

---

## 4. Supported Evidence Formats

In Phase 1E, the dispatcher accepts and routes two primary forensic evidence modalities:

1. **Call Detail Records (`CALL_DETAIL_RECORD`, `CDR`):**
   - File format: CSV (UTF-8 / ASCII).
   - Mandatory columns: `record_id`, `caller_phone`, `receiver_phone`, `call_timestamp`, `duration_seconds`.
   - Optional columns: `cell_tower_id`, `call_type`.
   - Extractor: `parse_cdr_csv` in [`backend/app/modules/extraction/cdr.py`](../../../backend/app/modules/extraction/cdr.py).

2. **Chat & Instant Messaging Exports (`CHAT_EXPORT`, `CHAT`):**
   - File format: JSON (UTF-8).
   - Expected structure: Root JSON object containing `messages` list with message attributes (`id`/`message_id`, `sender_id`/`sender`, `timestamp`, `content`/`text`, optional `recipients`).
   - Extractor: `parse_chat_json` in [`backend/app/modules/extraction/chat.py`](../../../backend/app/modules/extraction/chat.py).
   - Embedded NER: Extracts embedded phone numbers, emails, IMEIs, and IMSIs from message text.

### 4.1. Defense Against Premature CSV Classification
A major audit finding from Phase 1D.1 noted that ingestion content sniffing can classify any generic CSV as `CALL_DETAIL_RECORD`. The Phase 1E dispatcher defends against this:
- Prior to processing rows, the CDR parser checks `REQUIRED_CDR_HEADERS - set(header_columns)`.
- If mandatory headers are missing, the file is rejected immediately with `ExtractionValidationIssue(issue_type="MISSING_REQUIRED_HEADERS")`.
- The dispatcher records `valid_records_count = 0` and terminates with `status = ExtractionStatus.FAILED`. No phantom candidates are emitted.

---

## 5. Data Provenance & Forensic Integrity Guarantees

1. **Immutability of Evidence:**
   The dispatcher opens files in read-only binary or text mode (`"rb"` or `"r"`). The underlying evidence artifact is never modified in place, renamed, or truncated. Verified by unit tests comparing `st_mtime_ns` and byte-level SHA-256 before and after extraction.
2. **Cryptographic Grounding:**
   Every candidate (`ExtractedIdentifier`, `ExtractedEventCandidate`, `ProvisionalRelationshipCandidate`, `ResolvedEntityCandidate`) preserves:
   - `source_evidence_id`: Bound directly to the input `evidence_id`.
   - `source_record_id`: Originating row number or message identifier.
   - `location_reference`: Column name, metadata field, or character span offset (`char_offset:start-end`).
3. **Observed Facts vs. Inferred Entities:**
   - Raw observed values (`caller_phone_raw`, `raw_mentions`, uncorrected timestamps) are preserved verbatim.
   - Entity candidates produced by `deduplicate_identifiers` group technical communication addresses, not legal human identities. Every candidate carries the forensic disclaimer:
     > *"Normalized identifiers represent technical communication addresses or device hardware signatures, and do NOT constitute conclusive legal proof of human identity."*
4. **Checksum Telemetry Decoupled from Probabilistic Confidence:**
   - Luhn checksum results for IMEIs are recorded in `is_checksum_valid: Optional[bool]`.
   - Deterministic rule extractions have `extraction_confidence = None`. No artificial 1.0 or synthetic probabilistic confidence scores are fabricated.

---

## 6. Pre-Extraction Checks & Error Handling Strategy

The dispatcher implements an exception-safe, layered defense protocol:

| Check Stage | Condition | Dispatcher Behavior | Status | Issue Type |
|---|---|---|---|---|
| **1. File Existence** | File missing or path is not a file | Trapped before read. | `FAILED` | `FILE_NOT_FOUND` |
| **2. Cryptographic Digest** | Computed SHA-256 $\neq$ Expected SHA-256 | Streaming SHA-256 mismatch trapped. File not parsed. | `FAILED` | `HASH_MISMATCH` |
| **3. Modality Support** | Unknown source type (e.g. `PDF`, `VIDEO`) | Checked against supported registry. | `FAILED` | `UNSUPPORTED_SOURCE_TYPE` |
| **4. Header / Schema Check** | CSV missing required CDR columns | Structured validation issue generated. | `FAILED` | `MISSING_REQUIRED_HEADERS` |
| **5. File Content Syntax** | Truncated/corrupted JSON file | `json.JSONDecodeError` trapped. | `FAILED` | `JSON_DECODE_ERROR` |
| **6. Partial Degradation** | File has both valid and corrupt records | Valid candidates extracted; corrupt rows recorded. | `PARTIAL_SUCCESS` | `INVALID_DURATION`, `MISSING_REQUIRED_FIELD`, etc. |
| **7. Unhandled Parser Crash** | Bug or unexpected exception in parser | Top-level `try/except Exception` block traps error. | `FAILED` | `PARSER_EXCEPTION` |

**Zero Silent Drops:** No malformed record is ever silently discarded. Every corrupted row or invalid structure produces a structured `ExtractionValidationIssue`.
**Process Safety:** Unhandled parser exceptions cannot crash the FastAPI HTTP process.

---

## 7. Exact Integration Contract for Phase 1F

In Phase 1F, Member 2's ingestion service (`backend/app/modules/ingestion/service.py`) will invoke the extraction dispatcher upon successful upload or on-demand re-extraction.

### 7.1. Invocation Signature

```python
from app.modules.extraction import (
    EvidenceExtractionRequest,
    UnifiedExtractionResult,
    extract_evidence,
)

# Ingestion service integration:
request = EvidenceExtractionRequest(
    evidence_id=upload_response.evidence_id,
    case_id=upload_response.case_id,
    source_type=upload_response.source_type,
    file_path=Path(upload_response.storage_uri),
    expected_sha256=upload_response.sha256_hash,
    source_timezone=upload_response.metadata.get("source_timezone"),
    clock_offset_seconds=upload_response.metadata.get("clock_offset_seconds"),
    metadata=upload_response.metadata,
    default_country_prefix=upload_response.metadata.get("default_country_prefix", "91"),
)

result: UnifiedExtractionResult = extract_evidence(request)
```

### 7.2. Result Inspection & Extraction Run Mapping

Member 2 maps the returned `UnifiedExtractionResult` directly into an `ExtractionRunDTO`:
- `records_processed = result.total_records_processed`
- `valid_records_count = result.valid_records_count`
- `rejected_records_count = result.rejected_records_count`
- `status = ExtractionRunStatus(result.status.value)`
- `error_message = result.validation_issues[0].message if result.validation_issues else None`

---

## 8. Test Execution & Verification Matrix

The test suite covers all 12 mandatory Phase 1E scenarios in [`backend/tests/extraction/test_service.py`](../../../backend/tests/extraction/test_service.py):

| # | Test Function | Scenario Tested | Outcome |
|---|---|---|---|
| 1 | `test_valid_cdr_dispatch` | Valid synthetic CDR CSV file extraction | **PASSED** |
| 2 | `test_valid_chat_json_dispatch` | Valid synthetic chat JSON export extraction | **PASSED** |
| 3 | `test_unsupported_source_type_rejected` | Unsupported type (`DOCUMENT_PDF`) rejection | **PASSED** |
| 4 | `test_generic_csv_rejected_as_invalid_cdr` | Defense against CSV misclassification | **PASSED** |
| 5 | `test_missing_evidence_file_handling` | Missing file on disk error handling | **PASSED** |
| 6 | `test_sha256_mismatch_rejected` | Tampered/mismatched SHA-256 rejection | **PASSED** |
| 7 | `test_malformed_cdr_fails_extraction` | All rows malformed produces `FAILED` | **PASSED** |
| 8 | `test_malformed_json_fails_extraction` | Corrupt JSON string produces `FAILED` | **PASSED** |
| 9 | `test_partial_extraction_with_validation_issues` | Mixed valid/invalid rows produce `PARTIAL_SUCCESS` | **PASSED** |
| 10 | `test_parser_exception_handling_graceful` | Trapping unexpected parser crash as `PARSER_EXCEPTION` | **PASSED** |
| 11 | `test_provenance_preservation_across_candidates` | Verifying `source_evidence_id` across all entities | **PASSED** |
| 12 | `test_raw_evidence_remains_unchanged` | Immutability check via SHA-256 and mtime | **PASSED** |

### Test Suite Execution Results

1. **Extraction Test Suite:**
   ```bash
   uv run --directory backend pytest tests/extraction -v
   # Result: 41 passed in 0.66s
   ```

2. **Full Repository Backend Test Suite:**
   ```bash
   uv run --directory backend pytest -q
   # Result: 144 passed, 50 skipped (for ffmpeg/video), 4 warnings in 2.55s
   ```

3. **Synthetic Fixture Cryptographic Verification:**
   - `data/synthetic/sample_cdr.csv`:
     - Computed: `c874a7c8b0e05da52db5f29d5bb0f398439237ad69a757cb05bfe9ec76803a8d`
     - Sidecar: `c874a7c8b0e05da52db5f29d5bb0f398439237ad69a757cb05bfe9ec76803a8d` (Bit-for-bit MATCH)
   - `data/synthetic/sample_chat_export.json`:
     - Computed: `78eaf79b73ec12a15b15f3d26386c3f2456020c914d7c496aa9efeca20fec40e`
     - Sidecar: `78eaf79b73ec12a15b15f3d26386c3f2456020c914d7c496aa9efeca20fec40e` (Bit-for-bit MATCH)

---

## 9. Implemented vs. Planned Features

### Implemented in Phase 1E:
- Unified dispatcher entrypoint `extract_evidence(request)`.
- Pre-extraction file existence and SHA-256 verification.
- Modality routing for CDR CSV and Chat JSON.
- Generic CSV classification defense via mandatory header validation.
- Trapping of JSON decoding errors and parser crashes without process termination.
- Entity resolution and deduplication via `deduplicate_identifiers`.
- Computation of discrete statuses (`SUCCESS`, `PARTIAL_SUCCESS`, `FAILED`).
- Execution duration telemetry (`processing_time_ms`).
- Complete 12-scenario unit test suite.

### Planned for Subsequent Phases:
- **Phase 1F:** Direct invocation of `extract_evidence` from Member 2's `IngestionService.ingest_evidence()` and `trigger_extraction_run()`.
- **Phase 1G:** Alignment of `ExtractedEventCandidate` with Member 4's temporal and spatial reconstruction models (`SpatialPoint`, `TimeInterval`, `EvidenceRef`).
- **Phase 2A–2D:** Multi-modal CCTV video frame extraction, timeline alignment, and contradiction analysis.
- **Phase 2E–2F:** PostgreSQL relational persistence and Neo4j graph topology synchronization.
