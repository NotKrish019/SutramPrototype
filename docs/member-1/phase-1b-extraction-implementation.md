# SUTRAM Phase 1B — First Evidence Extraction Vertical Slice

**Author:** Member 1 (AI/ML & Evidence Intelligence)  
**Date:** 2026-11-17  
**Monorepo Target:** SUTRAM 2026 Core Extraction, NER, Resolution & Provenance Engines  
**Status:** `IMPLEMENTED & VERIFIED` (Local Workspace)

---

## 1. Implemented Extraction Modules

In Phase 1B, Member 1 implemented the core extraction and preliminary correlation modules without premature machine learning dependency overhead:

1. **`backend/app/modules/extraction/cdr.py`:**
   - Deterministic, stream-safe CSV parser for Call Detail Records.
   - Comprehensive header and row-level validation.
   - Extracts caller and callee identifiers, timestamps, call durations, and cell-tower tags.
   - Produces structured `ParsedCDRRecord`, `ExtractedIdentifier`, `ExtractedEventCandidate`, and `ProvisionalRelationshipCandidate` models.
   - Rejects malformed or incomplete rows with structured `ExtractionValidationIssue` records without modifying the source file or crashing.
2. **`backend/app/modules/extraction/chat.py`:**
   - Deterministic JSON parser for instant messaging exports (WhatsApp, Telegram, SMS).
   - Validates message schema and mandatory identifiers (`message_id`, `sender_id`, `timestamp`, `content`).
   - Automatically scans unstructured message text for embedded phone numbers, emails, IMEIs, and IMSIs using the NER engine.
   - Generates observed message-communication relationship candidates for single or multi-recipient threads.
3. **`backend/app/modules/extraction/ner.py`:**
   - Conservative, regex-based identifier extraction engine.
   - Extracts international and domestic phone numbers, RFC-compliant email addresses, and 14–15 digit device/subscriber numbers.
   - **Contextual Disambiguation:** Differentiates IMEI vs. IMSI based on immediate surrounding context keywords (`imei`, `device`, `sim`, `imsi`).
   - **Ambiguity Reporting:** Refuses to guess the identity of isolated 14–15 digit sequences lacking contextual keywords, classifying them as `AMBIGUOUS_NUMERIC_15` with `is_ambiguous = True`.
   - Includes standard 15-digit Luhn algorithm check helper for IMEI validation.
4. **`backend/app/modules/correlation/resolution.py`:**
   - Deterministic E.164 phone number canonicalization.
   - Normalizes explicit international numbers (`+91 98110-02233` $\rightarrow$ `+919811002233`) and international trunk format (`0091...`).
   - Supports configurable default country prefixes (e.g. `"91"`) for stripping domestic trunk `0` while refusing to invent country codes for ambiguous numbers when no prefix is configured.
   - Deduplicates extracted identifiers across multi-source evidence while preserving all raw mentions, source references, and forensic audit reasons.
   - Enforces the non-assertion disclaimer: normalized identifiers represent communication endpoints, not legal proof of human identity.
5. **`backend/app/modules/correlation/relationships.py`:**
   - Synthesizes directly observed communication relationships:
     - `CALL_COMMUNICATION`: Generated directly from CDR rows (Caller $\rightarrow$ Callee) with observed call duration and cell tower metadata.
     - `MESSAGE_COMMUNICATION`: Generated directly from messaging exports (Sender $\rightarrow$ Recipient).
   - Enforces `is_observed = True` and binds candidate edges directly to `source_evidence_id`.
6. **`backend/app/schemas/extraction.py`:**
   - Provisional, internal Pydantic models for extraction outputs, validation issues, identifiers, event candidates, and relationship candidates.

---

## 2. Synthetic Evidence Formats

Two standard synthetic evidence fixtures were authored and placed in `data/synthetic/` with accompanying SHA-256 checksum sidecar files for cryptographic hash integrity verification:

### A. Call Detail Record: `data/synthetic/sample_cdr.csv`
- **Columns:** `record_id`, `caller_phone`, `receiver_phone`, `call_timestamp`, `duration_seconds`, `cell_tower_id`, `call_type`.
- **SHA-256 Checksum:** `c874a7c8b0e05da52db5f29d5bb0f398439237ad69a757cb05bfe9ec76803a8d`

### B. Messaging Export: `data/synthetic/sample_chat_export.json`
- **Structure:** Top-level metadata (`export_format`, `export_timestamp`, `chat_title`, `chat_id`) and array of `messages` (`message_id`, `sender_id`, `sender_name`, `recipients`, `timestamp`, `content`).
- **SHA-256 Checksum:** `78eaf79b73ec12a15b15f3d26386c3f2456020c914d7c496aa9efeca20fec40e`

*(Note on Terminology: The `.sha256` files represent unkeyed cryptographic hash checksums for file integrity verification. They do not constitute asymmetric public/private key digital signatures, which require PKI or signing certificates.)*

---

## 3. Input & Output Examples

### CDR Extraction Example

**Input CSV Row:**
```csv
cdr-rec-001,+919811002233,+919899887766,2026-11-10T21:45:12+05:30,184,TOWER-DELHI-SOUTH-04B,VOICE_OUTGOING
```

**Parsed CDR Record:**
```json
{
  "source_evidence_id": "ev-cdr-001",
  "record_id": "cdr-rec-001",
  "caller_phone_raw": "+919811002233",
  "receiver_phone_raw": "+919899887766",
  "call_timestamp_raw": "2026-11-10T21:45:12+05:30",
  "duration_seconds": 184.0,
  "cell_tower_id": "TOWER-DELHI-SOUTH-04B",
  "call_type": "VOICE_OUTGOING",
  "row_index": 2
}
```

**Synthesized Event Candidate:**
```json
{
  "candidate_id": "evt-cdr-ev-cdr-001-cdr-rec-001",
  "source_evidence_id": "ev-cdr-001",
  "source_record_id": "cdr-rec-001",
  "event_type": "TELEPHONY_CALL",
  "raw_timestamp": "2026-11-10T21:45:12+05:30",
  "duration_seconds": 184.0,
  "location_name": "TOWER-DELHI-SOUTH-04B",
  "involved_entity_raw_ids": ["+919811002233", "+919899887766"],
  "metadata": {
    "call_type": "VOICE_OUTGOING",
    "cell_tower_id": "TOWER-DELHI-SOUTH-04B",
    "row_index": 2
  }
}
```

**Synthesized Relationship Candidate:**
```json
{
  "candidate_id": "rel-cdr-ev-cdr-001-cdr-rec-001",
  "source_evidence_id": "ev-cdr-001",
  "source_record_id": "cdr-rec-001",
  "source_raw_id": "+919811002233",
  "target_raw_id": "+919899887766",
  "source_normalized_id": "+919811002233",
  "target_normalized_id": "+919899887766",
  "relation_type": "CALL_COMMUNICATION",
  "is_observed": true,
  "metadata": {
    "call_duration_seconds": 184.0,
    "call_type": "VOICE_OUTGOING",
    "raw_timestamp": "2026-11-10T21:45:12+05:30",
    "cell_tower_id": "TOWER-DELHI-SOUTH-04B"
  },
  "evidence_basis": ["ev-cdr-001"]
}
```

---

## 4. Provenance & Immutability Guarantees

1. **Zero Evidence Mutation:** The parsing functions open evidence files strictly in read-only mode (`r` or stream input). Automated tests explicitly hash the source files before and after extraction to confirm that cryptographic SHA-256 hashes are 100% invariant.
2. **Chain of Custody:** Every extracted identifier, event candidate, and relationship candidate retains `source_evidence_id`, `source_record_id` (or `source_message_id`), and source character/column offset.
3. **Preservation of Raw Truth:** Original phone strings (`09811002233`) and timestamp strings (`2026-11-10T21:45:12+05:30`) are preserved alongside normalized values. If an investigator later recalibrates clock drift or country prefix assumptions, raw evidence remains intact.

---

## 5. Validation Rules

- **CDR Headers:** Mandatory presence of `record_id`, `caller_phone`, `receiver_phone`, `call_timestamp`, and `duration_seconds`. Missing headers result in immediate file-level rejection.
- **Duration Non-Negativity:** `duration_seconds >= 0.0`. Negative numbers or non-numeric strings trigger row rejection with `INVALID_DURATION`.
- **Chat Structure:** Root JSON must be an object containing a `messages` array. Each message must provide non-empty `message_id`, `sender_id`, `timestamp`, and string `content`.
- **Non-Destructive Error Handling:** Valid rows in partially corrupted files are parsed and extracted normally; corrupted rows are segregated into `rejected_records` with exact row numbers and diagnostic reasons.

---

## 6. Entity Normalization & Deduplication Behavior

- **Explicit International (+):** Recognized and formatted directly (e.g. `+91 98110-02233` $\rightarrow$ `+919811002233`).
- **Trunk Replacement (00):** Initial `00` replaced with `+`.
- **Configured Country Prefix:** When `default_country_prefix="91"` is supplied, leading domestic trunk `0` is stripped and prefixed (e.g. `09811002233` $\rightarrow$ `+919811002233`).
- **Ambiguous Domestic Handling:** If `default_country_prefix` is `None`, domestic numbers are left unnormalized (`is_normalized = False`, `resolution_status = "AMBIGUOUS_COUNTRY_CODE"`), preventing accidental cross-border merges.
- **Deduplication:** Multiple occurrences of an entity across CDR and chat evidence merge into a single `ResolvedEntityCandidate`, capturing all aliases, evidence sources, and explanation reasons.

---

## 7. Relationship Candidate Behavior

- **Strictly Grounded:** Only generates relationships supported by explicit source records:
  - Telephony call row $\rightarrow$ `CALL_COMMUNICATION`
  - Chat message $\rightarrow$ `MESSAGE_COMMUNICATION`
- **Observed vs. Inferred:** All generated candidates set `is_observed = True`.
- **Zero Hallucination:** The pipeline **never** generates criminal association, guilt, unverified alibi contradictions, or co-presence links from cell-tower data alone.

---

## 8. Test Coverage & Verification Results

### 1. Windows Timezone Dependency Resolution
- Added `tzdata>=2024.1` to `backend/pyproject.toml` and updated `backend/uv.lock`.
- Resolved all 55 previous Windows `zoneinfo` test failures.

### 2. Full Test Suite Execution (`uv run pytest -q`)
- **Total Tests:** 166 tests collected.
- **Results:**
  ```text
  116 passed, 50 skipped, 1 warning in 1.66s
  ```
- **Breakdown:**
  - `backend/tests/test_health.py`: 2 passed.
  - `backend/tests/reconstruction/test_temporal.py`: 41 passed.
  - `backend/tests/reconstruction/test_spatial.py`: 26 passed.
  - `backend/tests/reconstruction/test_contradictions.py`: 24 passed.
  - `backend/tests/reconstruction/test_video.py`: 50 skipped (gracefully skipped due to absent FFmpeg/FFprobe binaries on host).
  - `backend/tests/extraction/test_cdr.py`: 5 passed.
  - `backend/tests/extraction/test_chat.py`: 4 passed.
  - `backend/tests/extraction/test_ner.py`: 6 passed.
  - `backend/tests/extraction/test_resolution.py`: 6 passed.
  - `backend/tests/extraction/test_relationships.py`: 2 passed.
- **Targeted Extraction Tests:** 23 / 23 passed in 0.41s.

---

## 9. Current Limitations

- **Structured & Text Modalities Only:** Phase 1B covers CDR CSV and chat export JSON. It does not yet parse PDF documents, TIFF scans, or video streams.
- **Rule-Based NER:** Identifiers are detected via deterministic patterns. Machine learning models (spaCy NER for person names/organizations) and deep learning OCR (PaddleOCR) are intentionally deferred to avoid premature dependency bloat.
- **Provisional Schemas:** Extraction results use internal schemas (`app/schemas/extraction.py`) pending formal team sign-off on shared canonical domain models (`Case`, `Evidence`, `Entity`, `Event`, `Relationship`).

---

## 10. Dependencies on Member 2 and Member 3

- **Member 2 (Backend & Ingestion):**
  - Requires Member 2 to provide an ingestion endpoint that saves files to disk, computes SHA-256 hashes, and invokes `parse_cdr_csv()` / `parse_chat_json()`.
  - Requires Member 2 to map provisional event and relationship candidates to PostgreSQL tables and Neo4j graph nodes.
- **Member 3 (Frontend & Visualization):**
  - Requires Member 3 to support displaying rejected records / validation errors in the evidence ingestion UI.
  - Requires Member 3 to render observed communication links in the knowledge graph.

---

## 11. Remaining Contract Approvals

- **DEC-001 through DEC-006:** Awaiting team sign-off in `docs/member-4/phase-3c-contract-decisions.md`.
- **DEC-002 Amendment:** Enforce strict half-open interval validation: $\text{window\_start\_utc} \le \text{nominal\_utc} < \text{window\_end\_utc}$.
- **Shared Canonical Domain Models:** Once approved, Member 1 and Member 2 will formalize `Case`, `Evidence`, `Entity`, `Event`, and `Relationship` Pydantic models in `backend/app/schemas/domain.py`.

---

## 12. Recommended Next Phase (Phase 1C / Phase 2)

1. **Document Text & Scanned PDF Extraction:** Implement `backend/app/modules/extraction/text.py` and OCR abstraction for case documents and FIRs.
2. **Integration with Member 4 Engines:** Wire `ExtractedEventCandidate` objects directly into Member 4's `reconstruct_timeline()` and `reconstruct_spatial_trajectories()`.
3. **Ingestion Endpoint Integration:** Connect extraction functions to Member 2's evidence upload API once available.

---
*Report prepared in compliance with AGENTS.md and SUTRAM 2026 Forensic Guidelines.*
