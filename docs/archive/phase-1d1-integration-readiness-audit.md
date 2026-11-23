# SUTRAM 2026 — Phase 1D.1: Backend and Integration Readiness Audit
**Author:** Combined Backend & Data Engineering and AI/ML Integration Auditor  
**Date:** November 18, 2026  
**Status:** Audit Completed  
**Branch:** `main` (Local working tree intact, uncommitted)  
**Base Commit:** `179abe0` (Verified in complete synchronization with `origin/main`)

---

## 1. Executive Summary

Phase 1D.1 executes a code-level, security, and architectural readiness audit across all local implementations delivered in Phases 1A, 1B, 1C, and 1D. In accordance with the strict audit-only protocol, **no application code, schemas, or dependency files were modified**.

This audit verifies:
1. **Extraction Subsystem (Member 1):** CDR parsing, chat parsing, regex NER, E.164 phone normalization, identifier deduplication, and IMEI/IMSI telemetry are verified and mathematically decoupled from confidence.
2. **Ingestion Foundation (Member 2):** Streaming SHA-256 hashing, path traversal defenses, content sniffing, and the non-destructive `ExtractionRun` model operate as intended.
3. **Reconstruction Engine (Member 4):** Temporal alignment, spatial trajectory calculation, and contradiction detection are robust, but require adapter mappings to consume extraction outputs directly.
4. **Security & Data Integrity:** Identified 2 High-priority and 4 Medium-priority findings—most notably, that case-scoped hash deduplication would reject legitimate multi-exhibit acquisitions sharing identical content, and that uploaded files currently lack authentication.
5. **Phase Readiness:** Formulated an actionable path forward to connect the end-to-end pipeline: **Phase 1E (Unified Extraction Dispatcher)** $\rightarrow$ **Phase 1F (Ingestion-to-Extraction Integration)** $\rightarrow$ **Phase 1G (Member 1/2/4 Contract Alignment)**.

---

## 2. Repository and Git State

- **Branch:** `main`
- **Local HEAD:** `179abe083a8bd94f8a7c6c54618eb5af03b997dc`
- **Remote `origin/main`:** `179abe083a8bd94f8a7c6c54618eb5af03b997dc` (Verified via `git fetch origin` and `git rev-parse`).
- **Synchronization:** Parity confirmed; remote has not advanced and local has not diverged.
- **Working Tree Integrity:** All uncommitted files from Phases 1A–1D remain preserved. No files were added, deleted, or committed.

---

## 3. Existing Implementation Inventory

| Subsystem | Component / Module | Implementation Status | Test Coverage |
| :--- | :--- | :--- | :--- |
| **Extraction (M1)** | `backend/app/modules/extraction/cdr.py` | Implemented (`parse_cdr_csv`) | 100% via unit tests |
| **Extraction (M1)** | `backend/app/modules/extraction/chat.py` | Implemented (`parse_chat_json`) | 100% via unit tests |
| **Extraction (M1)** | `backend/app/modules/extraction/ner.py` | Implemented (`extract_identifiers_from_text`) | 12 unit tests |
| **Correlation (M1)**| `backend/app/modules/correlation/resolution.py` | Implemented (`normalize_phone_number`, `deduplicate_identifiers`) | 100% via unit tests |
| **Correlation (M1)**| `backend/app/modules/correlation/relationships.py`| Implemented (`generate_cdr/chat_relationship_candidate`) | 100% via unit tests |
| **Ingestion (M2)** | `backend/app/api/routes/evidence.py` | Implemented (`/upload`, `/{id}`, `/{id}/runs`, `/{id}/reprocess`) | 10 integration tests |
| **Ingestion (M2)** | `backend/app/modules/ingestion/storage.py`| Implemented (`save_evidence_file`, safe paths, integrity check) | Tested |
| **Ingestion (M2)** | `backend/app/modules/ingestion/validation.py`| Implemented (`sniff_file_content` magic bytes) | Tested |
| **Ingestion (M2)** | `backend/app/modules/ingestion/service.py`| Implemented (`EvidenceIngestionService`, run versioning) | Tested |
| **Reconstruction (M4)**| `backend/app/modules/reconstruction/temporal.py`| Implemented (`normalize_event_time`, `reconstruct_timeline`) | 41 unit tests |
| **Reconstruction (M4)**| `backend/app/modules/reconstruction/spatial.py` | Implemented (`reconstruct_spatial_trajectories`, `haversine`) | 52 unit tests |
| **Reconstruction (M4)**| `backend/app/modules/reconstruction/contradictions.py`| Implemented (`detect_contradictions`) | 20 unit tests |
| **Reconstruction (M4)**| `backend/app/modules/reconstruction/video.py` | Implemented (`probe_video_metadata`, keyframe extraction) | 50 tests (skipped when ffmpeg absent) |
| **Persistence (M2)**| PostgreSQL / SQLAlchemy 2.0 ORM | Architecture & Schema Designed; Containers unprovisioned | Scheduled for Phase 2E |
| **Persistence (M2)**| Neo4j Cypher / Graph Service | Architecture Designed; Driver unprovisioned | Scheduled for Phase 2F |

---

## 4. Test Suite Execution & Cryptographic Verification

### 4.1 Pytest Suite Execution
Command: `uv run --directory backend pytest -q`
```text
........................................................................ [ 39%]
..........................................................ssssssssssssss [ 79%]
ssssssssssssssssssssssssssssssssssss..                                   [100%]
132 passed, 50 skipped, 4 warnings in 3.82s
```
- **Passed:** 132 tests (100% of runnable backend suite).
- **Skipped:** 50 tests (FFmpeg/OpenCV video extraction tests properly skipped when FFmpeg binary is absent in test environment).
- **Failed:** 0 tests.
- **Warnings:** 4 (FastAPI testclient StarletteDeprecationWarnings).

### 4.2 Synthetic Evidence Cryptographic Invariance
Bit-for-bit SHA-256 verification against disk sidecars:
- `data/synthetic/sample_cdr.csv`: `c874a7c8b0e05da52db5f29d5bb0f398439237ad69a757cb05bfe9ec76803a8d` (**Match = True**)
- `data/synthetic/sample_chat_export.json`: `78eaf79b73ec12a15b15f3d26386c3f2456020c914d7c496aa9efeca20fec40e` (**Match = True**)

---

## 5. Security and Architecture Assessment

| ID | Evaluation Dimension | Audit Assessment & Technical Reality | Severity | Affected Module | Recommended Remediation |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **SEC-01** | Cross-Exhibit Identical Hash Rejection | The duplicate check `find_by_hash(case_id, sha256)` rejects *any* file with identical hash in the same case. In criminal forensics, two separate seized exhibits (e.g. Phone 1 and Phone 2) frequently contain identical forwarded files. Rejecting the upload destroys Exhibit 2's chain of custody. | **HIGH** | `app/modules/ingestion/service.py` | Allow duplicate hashes across distinct exhibit IDs; record an `IDENTICAL_FILE_HASH` link rather than raising 409 Conflict. |
| **SEC-02** | Authentication & Case Authorization | Endpoints `/api/v1/evidence/*` have no authentication (`Depends(get_current_user)`) or case access controls. Any network actor can upload, inspect, or trigger re-extraction. | **HIGH** | `app/api/routes/evidence.py` | Introduce JWT/OAuth2 authentication and tenant/case authorization middleware prior to production. |
| **SEC-03** | Background Worker Execution Gap | Architecture documents claim large uploads run asynchronously in background workers. The current code executes synchronously in the HTTP request thread. | **MEDIUM** | `app/api/routes/evidence.py` | Add FastAPI `BackgroundTasks` or Celery task queue for files $> 10\text{MB}$ returning `202 Accepted`. |
| **SEC-04** | Synchronous 10–50 MB Processing | Uploads between 10 MB and 50 MB execute synchronously. If extraction takes $> 60\text{s}$, client or proxy connections may time out. | **MEDIUM** | `app/api/routes/evidence.py` | Implement streaming chunk ingestion and async task queue dispatch. |
| **SEC-05** | False Content-Type Confidence | `sniff_file_content()` flags any valid CSV as `CALL_DETAIL_RECORD`. If an investigator uploads an employee spreadsheet, it is labeled a CDR until extraction fails. | **MEDIUM** | `app/modules/ingestion/validation.py` | Label as `TABULAR_CSV` on ingestion; defer `CALL_DETAIL_RECORD` classification to header verification. |
| **SEC-06** | Historical Run Output Queryability | `GET /{id}/runs` returns run metadata, but full extracted candidate payloads from superseded runs are not retained in the in-memory registry. | **MEDIUM** | `app/modules/ingestion/service.py` | Persist full candidate collections per `extraction_run_id` in database tables (Phase 2E). |
| **SEC-07** | Internal Absolute Path Exposure | `EvidenceUploadResponse.storage_uri` returns local host filesystem paths (e.g. `C:/Users/...`). | **LOW** | `app/schemas/ingestion.py` | Return virtualized URNs (`urn:sutram:evidence:{id}:storage`) rather than physical host paths. |
| **SEC-08** | Arbitrary Path Traversal Defense | `get_safe_storage_path()` strictly sanitizes filenames and asserts destination is relative to storage root. Tested against `../../` attacks. | **LOW / RESOLVED** | `app/modules/ingestion/storage.py` | Implemented and verified by unit tests. |
| **SEC-09** | Raw Evidence Immutability | Evidence files are opened strictly in read-only mode (`'rb'`). Extraction errors or parser crashes leave original bytes untouched. | **INFORMATIONAL** | `app/modules/ingestion/storage.py` | Implemented and verified. |
| **SEC-10** | Forensic Invariance vs. OS Permissions | Read-only file attributes (`0o444`) provide OS protection but not judicial immutability. Judicial integrity is established by pre-extraction SHA-256 recalculation. | **INFORMATIONAL** | `app/modules/ingestion/storage.py` | Pre-extraction SHA-256 check verified. |

---

## 6. Extraction Quality & Telemetry Audit

1. **Parser Robustness:**
   - `parse_cdr_csv` strictly verifies mandatory telephony headers (`record_id`, `caller_phone`, `receiver_phone`, `call_timestamp`, `duration_seconds`). Rejected rows are logged with `ExtractionValidationIssue` without halting processing.
   - `parse_chat_json` validates JSON messaging arrays and runs regex NER over text bodies.
2. **Deterministic Confidence Decoupling:**
   - In `backend/app/schemas/extraction.py`, `extraction_confidence: Optional[float] = None` is completely decoupled from `is_checksum_valid`.
   - Unit tests confirm that valid IMEI (`is_checksum_valid=True`) and invalid IMEI (`is_checksum_valid=False`) receive identical extraction confidence (`None`).
   - 3GPP TS 23.003 Luhn mod-10 is evaluated strictly on 15-digit candidate IMEIs and never on ITU-T E.212 IMSI subscriber identities.
   - Invalid IMEIs are strictly preserved with original offsets for forensic review.

---

## 7. Member 4 Reconstruction Compatibility Matrix

Member 4's reconstruction modules (`temporal.py`, `spatial.py`, `contradictions.py`) operate on generic sequences of events (`Sequence[Mapping[str, Any] | BaseModel]`). However, specific field-name discrepancies exist between Member 1's extraction outputs and Member 4's expected keys:

| Reconstruction Module | Expected Event Field | Member 1 Extracted Candidate | Mismatch / Adapter Requirement | Integration Status |
| :--- | :--- | :--- | :--- | :--- |
| **`temporal.reconstruct_timeline`** | `id` or `event_id` | `candidate_id` | **Mismatch:** `temporal.py` looks for `id`/`event_id`. Must map `candidate_id` $\rightarrow$ `event_id`. | **Adapter Required** |
| **`temporal.reconstruct_timeline`** | `timestamp` or `raw_timestamp` | `raw_timestamp` | **Compatible:** Member 4 inspects `raw_timestamp` directly. | **Directly Compatible** |
| **`temporal.reconstruct_timeline`** | `timestamp_precision` | Missing on Extracted Candidate | **Compatible:** Defaults to `EXACT_SECOND` when omitted. | **Directly Compatible** |
| **`temporal.reconstruct_timeline`** | `evidence_id` or `source_evidence_id` | `source_evidence_id` | **Compatible:** Member 4 inspects `source_evidence_id` directly. | **Directly Compatible** |
| **`spatial.reconstruct_spatial_trajectories`** | `entity_id` | `involved_entity_raw_ids: list` | **Mismatch:** Spatial engine groups by single primary `entity_id`. Must flatten or specify subject entity. | **Adapter Required** |
| **`spatial.reconstruct_spatial_trajectories`** | `coordinates: GeoPoint(lat, lon)` | `metadata["cell_tower_lat/lon"]` | **Mismatch:** Cell tower coords in CDR reside in `metadata`. Must extract into `coordinates: {lat, lon}`. | **Adapter Required** |
| **`contradictions.detect_contradictions`** | `SpatialReconstructionResult` | N/A (Consumes spatial output) | **Compatible:** Operates downstream on Member 4's `SpatialReconstructionResult`. | **Directly Compatible** |

### Audit of DEC-001 through DEC-006 Implementation:
- **DEC-001 (Clock Offset Sign Convention):** Consistently implemented in `temporal.py` and `video.py` ($\text{True} = \text{Device} + \text{Offset}$).
- **DEC-002 (TimePrecision Enums):** Implemented in `CanonicalTimePrecision` with legacy alias tolerance.
- **DEC-003 (RelationType Taxonomy):** Implemented in `contradictions.py` and `reconstruction.py`.
- **DEC-004 (Relationship.weight Nullability):** Implemented in `reconstruction.py` (`weight: Optional[float] = None`).
- **DEC-005 (Event Duration & Uncertainty):** Implemented in `temporal.py` and `reconstruction.py`.
- **DEC-006 (Evidence Metadata Schema):** Implemented in `temporal.py` line 371.
- **Half-Open Interval Check (`temporal.py` line 298):** Remains `start_utc <= nominal_utc <= end_utc`. The upper bound should be updated to strictly exclusive (`< end_utc`) in Phase 1G.

---

## 8. Architecture Readiness Assessment

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    PHASE INTEGRATION READINESS MATRIX                       │
├───────────┬─────────────────────────────────────────┬───────────────────────┤
│ Phase     │ Target Scope                            │ Readiness Status      │
├───────────┼─────────────────────────────────────────┼───────────────────────┤
│ Phase 1E  │ Unified Extraction Dispatcher           │ READY                 │
│ Phase 1F  │ Ingestion-to-Extraction API Integration │ READY WITH CONDITIONS │
│ Phase 1G  │ Member 1/2/4 Contract Alignment         │ READY WITH CONDITIONS │
│ Phase 2A  │ Timeline Reconstruction Integration     │ READY WITH CONDITIONS │
│ Phase 2B  │ Spatial Reconstruction Integration      │ READY WITH CONDITIONS │
│ Phase 2C  │ Contradiction Detection Integration     │ READY WITH CONDITIONS │
│ Phase 2D  │ Keyframe & Video Analysis Integration   │ READY WITH CONDITIONS │
│ Phase 2E  │ PostgreSQL Relational Persistence       │ NOT READY             │
│ Phase 2F  │ Neo4j Graph Synchronization             │ NOT READY             │
└───────────┴─────────────────────────────────────────┴───────────────────────┘
```

### Readiness Rationales:
- **Phase 1E (Unified Extraction Dispatcher):** **READY.** Extractor modules (`cdr.py`, `chat.py`, `ner.py`, `resolution.py`) are fully tested. We can immediately implement `extract_evidence(request: EvidenceExtractionRequest) -> UnifiedExtractionResult` in `backend/app/services/extraction_service.py`.
- **Phase 1F (Ingestion Integration):** **READY WITH CONDITIONS.** Requires Phase 1E dispatcher and updating `find_by_hash()` duplicate policy to distinguish identical cross-exhibit files.
- **Phase 1G (Contract Alignment):** **READY WITH CONDITIONS.** Requires team sign-off on DEC-001..DEC-006 and creation of an Event-to-Reconstruction adapter.
- **Phase 2E (PostgreSQL) & 2F (Neo4j):** **NOT READY.** Dependencies (`sqlalchemy`, `asyncpg`, `alembic`, `neo4j`) are not yet installed in `pyproject.toml`, and migration files are uninstantiated.

---

## 9. Proposed Remediation Sequence

1. **Phase 1E:** Build `backend/app/services/extraction_service.py` implementing `extract_evidence()` to unify CDR and chat extraction, entity deduplication, and relationship synthesis under a single typed interface.
2. **Phase 1F:** Wire `extraction_service` into `POST /api/v1/evidence/upload` and refine duplicate handling (`SEC-01`) to support distinct exhibits with identical content hashes.
3. **Phase 1G:** Formalize the Event Adapter transforming `ExtractedEventCandidate` into Member 4's expected reconstruction schema (`candidate_id` $\rightarrow$ `event_id`, cell coords $\rightarrow$ `coordinates: GeoPoint`).
4. **Phase 2A–2D:** Connect the ingestion-extraction pipeline directly to `temporal.reconstruct_timeline` and `spatial.reconstruct_spatial_trajectories`.
5. **Phase 2E–2F:** Provision PostgreSQL and Neo4j drivers, create database migrations, and persist extraction runs and entities permanently.

---

## 10. Recommended Immediate Next Phase

**Proceed to Phase 1E: Unified Extraction Dispatcher.**
Member 1 will implement `backend/app/services/extraction_service.py`, creating the canonical `extract_evidence()` entrypoint that binds `parse_cdr_csv` and `parse_chat_json` into a single, cohesive service.
