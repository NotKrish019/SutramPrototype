# SUTRAM Phase 1B.1 — Extraction Implementation Verification & Readiness Audit

**Author:** Member 1 (AI/ML & Evidence Intelligence)  
**Date:** 2026-11-17  
**Monorepo Target:** SUTRAM 2026 Extraction & Correlation Subsystem  
**Audit Scope:** Code Quality, Forensic Provenance, Checksum Verification, Integration Boundaries, and Contract Dependencies  
**Status:** `AUDIT COMPLETE — READY FOR TEAM REVIEW`

---

## 1. Git Status & Safety Verification

- **Current Branch:** `main` (verified via `git branch --show-current`).
- **Latest Remote Commit:** `179abe0 fix(ci): install dev dependencies in uv sync to make pytest available` (synchronized with `origin/main`).
- **Working Tree State:** All local changes preserved without staging, resetting, stashing, or pushing. Zero extra branches exist.
- **Git Safety Protocol:** Push authorization is withheld. Local code remains purely on `main` awaiting human approval.

---

## 2. Full Changed-File Inventory

The current working tree contains the following modified and untracked files:

| File Path | Change Type | Purpose / Description |
| :--- | :--- | :--- |
| `backend/pyproject.toml` | Tracked Modification | Added `tzdata>=2024.1` to resolve Windows IANA timezone test failures. |
| `backend/uv.lock` | Tracked Modification | Updated lockfile locking `tzdata==2026.6`. |
| `data/synthetic/README.md` | Tracked Modification | Documented schemas and column definitions for synthetic fixtures. |
| `backend/app/modules/extraction/__init__.py` | Tracked Modification | Exported `parse_cdr_csv`, `parse_chat_json`, and `extract_identifiers_from_text`. |
| `backend/app/modules/correlation/__init__.py` | Tracked Modification | Exported `normalize_phone_number`, `deduplicate_identifiers`, and relationship generators. |
| `backend/app/schemas/extraction.py` | Untracked Addition | Internal provisional DTOs for extraction results, identifiers, and candidates. |
| `backend/app/modules/extraction/cdr.py` | Untracked Addition | Deterministic Call Detail Record CSV parser with strict row-level validation. |
| `backend/app/modules/extraction/chat.py` | Untracked Addition | Deterministic instant messaging JSON export parser with free-text NER scanning. |
| `backend/app/modules/extraction/ner.py` | Untracked Addition | Deterministic pattern-based regex extractor for phone, email, IMEI, and IMSI. |
| `backend/app/modules/correlation/resolution.py` | Untracked Addition | E.164 phone canonicalizer, domestic prefix handler, and entity deduplicator. |
| `backend/app/modules/correlation/relationships.py` | Untracked Addition | Directly observed `CALL_COMMUNICATION` and `MESSAGE_COMMUNICATION` candidate builder. |
| `data/synthetic/sample_cdr.csv` | Untracked Addition | Synthetic 5-row Call Detail Record CSV fixture. |
| `data/synthetic/sample_cdr.csv.sha256` | Untracked Addition | Checksum sidecar file containing SHA-256 for `sample_cdr.csv`. |
| `data/synthetic/sample_chat_export.json` | Untracked Addition | Synthetic 3-message instant messaging export JSON fixture. |
| `data/synthetic/sample_chat_export.json.sha256` | Untracked Addition | Checksum sidecar file containing SHA-256 for `sample_chat_export.json`. |
| `backend/tests/extraction/` | Untracked Addition | 5 unit test suites (23 tests) verifying CDR, chat, NER, resolution, and relationships. |
| `docs/member-1/phase-1-extraction-contract-review.md` | Untracked Addition | Phase 1A contract review and architecture blueprint (preserved intact). |
| `docs/member-1/phase-1b-extraction-implementation.md` | Untracked Addition | Phase 1B implementation documentation (preserved intact). |
| `docs/member-1/phase-1b1-readiness-audit.md` | Untracked Addition | This audit report. |

*Note: The user query referenced `backend/app/modules/extraction/extraction.py`. The audit confirms that extraction schemas live cleanly in `backend/app/schemas/extraction.py`, and no redundant `extraction.py` module exists in `backend/app/modules/extraction/`.*

---

## 3. Extraction Implementation Review

A systematic review was conducted across all Phase 1B modules against forensic-integrity and engineering requirements:

1. **Explicit Malformed Record Handling:**
   - In `cdr.py`, missing headers reject the entire file cleanly (`MISSING_REQUIRED_HEADERS`). Row-level corruption (missing required cells, negative durations, non-numeric values, or column-count mismatches) is isolated into `ExtractionValidationIssue` records. Valid rows in the same file continue to be extracted normally without dropping data or crashing.
   - In `chat.py`, JSON syntax errors (`JSON_DECODE_ERROR`), missing `messages` arrays, non-dict message entries, or missing mandatory fields (`message_id`, `sender_id`, `timestamp`, `content`) are captured in `rejected_messages`.
2. **Preservation of Original Observed Values:**
   - Original phone strings (`09811002233`), timestamp strings (`2026-11-10T21:45:12+05:30`), and message contents are preserved in dedicated raw fields (`caller_phone_raw`, `raw_timestamp`, `content_raw`).
   - Normalization occurs into separate fields (`normalized_value`, `normalized_e164`) without overwriting raw evidence.
3. **Traceable Evidence & Record Binding:**
   - Every parsed record, extracted identifier, event candidate, and relationship candidate retains `source_evidence_id` and `source_record_id` (or `source_message_id`).
   - Identifiers captured from free text retain exact character offsets (`char_offset:start-end`) and surrounding text snippets.
4. **Original Evidence Immutability:**
   - Parsing engines open files strictly in read-only mode (`r` or stream).
   - Automated tests compute binary SHA-256 hashes before and after parsing, verifying 100% invariance.
5. **Conservative Entity Resolution:**
   - `normalize_phone_number()` normalizes explicit international formats (`+` or `00`) and domestic numbers with a configured default prefix (`"91"`).
   - Ambiguous domestic numbers lacking a country code are **never** guessed; they are flagged as `AMBIGUOUS_COUNTRY_CODE` with `is_normalized = False`.
6. **Provenance-Preserving Deduplication:**
   - Multiple mentions of an entity across sources merge into a single `ResolvedEntityCandidate`, while maintaining all distinct raw strings in `raw_mentions`, all origin evidence IDs, and auditable equivalence reasons.
   - Includes the mandatory forensic disclaimer: normalized identifiers represent communication endpoints, not conclusive proof of human identity.
7. **Observed vs. Inferred Relationship Separation:**
   - Generated candidates (`CALL_COMMUNICATION`, `MESSAGE_COMMUNICATION`) set `is_observed = True`.
   - The engine explicitly refuses to generate criminal association, guilt conclusions, or spatial co-location links from cell-tower data alone.
8. **Fabrication Prevention:**
   - Rows or messages with missing mandatory identifiers fail row-level validation; the engine never invents dummy IDs or fabricated timestamps.

---

## 4. IMEI and IMSI Validation Assessment

### Technical Architecture
- `backend/app/modules/extraction/ner.py` extracts 14–15 digit numeric sequences and performs contextual disambiguation using regex lookarounds (`IMEI_KEYWORDS`, `IMSI_KEYWORDS`).
- A standalone `luhn_checksum_valid(digits: str) -> bool` function is implemented and unit-tested for 15-digit Luhn check digit validation.

### Forensic Findings & Differentiation
1. **Luhn Algorithm Application:**
   - 3GPP TS 23.003 specifies that GSM/UMTS/LTE/5G IMEIs (15 digits) compute the 15th digit using the Luhn algorithm.
   - IMSI (International Mobile Subscriber Identity) values are 14–15 digit ITU-T E.212 identifiers composed of a 3-digit Mobile Country Code (MCC), a 2–3 digit Mobile Network Code (MNC), and a 9–10 digit MSIN. **IMSIs do NOT use Luhn checksums.**
   - The implementation correctly avoids applying Luhn checks to IMSI candidates.
2. **Current IMEI Validation Behavior:**
   - When an IMEI keyword is present in the surrounding context, the sequence is classified as `IMEI`.
   - However, `extract_identifiers_from_text()` does not currently evaluate `luhn_checksum_valid()` on the extracted IMEI candidate; the function exists only as a utility in `ner.py`.
   - *Forensic Trade-off:* If Luhn validation were used as a strict filter to discard non-conforming numbers, transcribed evidence with a single OCR character defect (e.g. OCR misreading an '8' as a '3') would be dropped, violating forensic evidence preservation. However, completely omitting the check prevents investigators from distinguishing valid hardware signatures from corrupted digits.
3. **Recommendation (Recorded for Phase 1C):**
   - Add a metadata flag to `ExtractedIdentifier` for IMEIs:
     `is_checksum_valid: Optional[bool] = luhn_checksum_valid(raw_val) if identifier_type == "IMEI" else None`
   - This provides immediate data-quality telemetry to investigators without discarding raw evidentiary text.

---

## 5. Synthetic Evidence Integrity & Checksum Verification

Binary SHA-256 hashes were recomputed from the disk files and compared against the sidecar `.sha256` files:

| Synthetic Fixture Path | Recomputed Binary SHA-256 Checksum | Sidecar File Value | Match Result | Data Nature |
| :--- | :--- | :--- | :--- | :--- |
| `data/synthetic/sample_cdr.csv` | `c874a7c8b0e05da52db5f29d5bb0f398439237ad69a757cb05bfe9ec76803a8d` | `c874a7c8b0e05da52db5f29d5bb0f398439237ad69a757cb05bfe9ec76803a8d` | **MATCH (100%)** | Fully Synthetic |
| `data/synthetic/sample_chat_export.json` | `78eaf79b73ec12a15b15f3d26386c3f2456020c914d7c496aa9efeca20fec40e` | `78eaf79b73ec12a15b15f3d26386c3f2456020c914d7c496aa9efeca20fec40e` | **MATCH (100%)** | Fully Synthetic |

### Terminology Audit: Checksum vs. Digital Signature
- In Phase 1B documentation and reporting, the sidecar files were informally referred to as "cryptographically signed".
- **Terminology Finding:** The sidecar files contain **SHA-256 cryptographic hash checksums**, which verify binary integrity against corruption or modification. They do **NOT** constitute asymmetric public/private key digital signatures (such as GPG, RSA, or ECDSA signatures), which verify authenticity and non-repudiation.
- **Correction:** All documentation and code comments have been standardized to use "SHA-256 checksum verification" rather than "cryptographic signature".

---

## 6. Test Suite Execution & Edge-Case Coverage

### Test Results Summary (`uv run pytest -q`)
```text
116 passed, 50 skipped, 1 warning in 2.08s
```
- **Total Tests Collected:** 166 tests.
- **Passed:** 116 tests.
- **Failed:** 0 tests.
- **Skipped:** 50 tests.
- **Warnings:** 1 (`StarletteDeprecationWarning` regarding `httpx` in FastAPI testclient).

### Audit of the 50 Skipped Video Tests
- Verified using `pytest -rs`.
- Every one of the 50 skipped tests explicitly reported:  
  `SKIPPED: System binaries 'ffmpeg' and 'ffprobe' are required for video reconstruction tests`
- Confirmed: The skips are entirely due to the absence of FFmpeg/FFprobe binaries on the Windows host and are governed by Swastik's fallback fixture in `backend/tests/reconstruction/test_video.py`. No tests were artificially skipped or suppressed.

### Edge-Case Coverage in Extraction Tests (23 Tests)
The 23 new extraction tests cover significant failure modes and boundary conditions:
- **Empty file handling:** Rejects empty CSV without crashing (`test_empty_csv_file`).
- **Missing headers:** Rejects CSVs lacking mandatory columns (`test_missing_required_headers`).
- **Row-level corruption:** Verifies that row failures (missing caller, missing timestamp, negative duration, string duration) produce structured issues while allowing valid rows to be extracted (`test_row_level_validation_and_rejected_records`).
- **Malformed JSON:** Tests incomplete/broken JSON syntax (`test_malformed_json_handling`).
- **Missing message keys:** Tests missing `message_id`, missing `sender_id`, missing `timestamp`, and non-string `content` (`test_missing_mandatory_fields`).
- **Ambiguous numeric sequences:** Confirms that isolated 15-digit numbers without context are marked ambiguous and not guessed (`test_isolated_15_digit_number_reported_as_ambiguous`).
- **Conflicting keywords:** Tests text containing both IMEI and IMSI in proximity (`test_conflicting_context_reported_as_ambiguous`).
- **Phone bounds:** Tests numbers with insufficient (< 7) or excessive (> 15) digits (`test_invalid_and_out_of_bounds_phone_numbers`).
- **Domestic ambiguity:** Verifies that domestic numbers without default prefixes remain unnormalized (`test_domestic_number_without_default_prefix_remains_unresolved`).
- **Multi-party messaging:** Verifies 1-to-many recipient relationship generation (`test_chat_multi_recipient_relationship_candidates`).
- **Immutability:** Confirms that neither CDR nor chat parsers alter source evidence file hashes (`test_evidence_file_immutability`, `test_chat_evidence_immutability`).

---

## 7. Forensic Provenance Assessment

The extraction subsystem satisfies all core SUTRAM forensic integrity directives:
1. **Source Traceability:** Every extracted entity and event can be traced back to its exact evidence file, record ID, and character offset.
2. **Fact vs. Inference Separation:** Directly observed records (calls, messages) are tagged `is_observed = True`. Inferred entities (deduplicated clusters) carry explicit equivalence reasons.
3. **Immutability:** Original files are never modified in place.
4. **No Identity Claims:** Normalized phone numbers and device IMEIs are explicitly tagged as communication/hardware addresses, not conclusive human identities.

---

## 8. Integration-Readiness Assessment

### Independence from Database Persistence
The extraction modules (`cdr.py`, `chat.py`, `ner.py`, `resolution.py`, `relationships.py`) are implemented as pure, deterministic functions. They accept file paths, strings, or IO streams and return in-memory Pydantic models. They do not import SQLAlchemy, asyncpg, or Neo4j drivers, and can execute in isolated worker processes, CLI scripts, or FastAPI background tasks.

### Compatibility with Member 4 (Reconstruction Engines)
- `ExtractedEventCandidate` outputs:
  - `raw_timestamp: str`
  - `duration_seconds: Optional[float]`
  - `location_name: Optional[str]`
  - `involved_entity_raw_ids: list[str]`
- When Member 2's ingestion service associates an evidence item with `case_id`, `source_timezone`, and `clock_offset_seconds`, `ExtractedEventCandidate.raw_timestamp` passes directly into Member 4's `compute_uncertainty_window()`, converting candidate events into canonical `TimelineEntry` objects for `reconstruct_timeline()` and `reconstruct_spatial_trajectories()`.

---

## 9. Contract Dependencies & Status

All canonical contracts remain protected under team governance:

| Contract File | Current Status | Member 1 Recommendation |
| :--- | :--- | :--- |
| `docs/member-4/phase-3c-contract-decisions.md` | `PROPOSED` | Approve DEC-001 through DEC-006 with the DEC-002 half-open interval amendment ($start \le nominal < end$). |
| `shared/schemas/README.md` | Unmodified | Awaiting team approval of DEC-001–006 before formal schema amendments. |
| `shared/enums/README.md` | Unmodified | Awaiting team approval. |
| `shared/examples/README.md` | Unmodified | Awaiting team approval. |
| `apps/web/src/types/index.ts` | Unmodified | Awaiting team approval. Missing domain models (`Entity`, `Event`, `Evidence`, `Relationship`). |
| `backend/app/schemas/domain.py` | Not Yet Created | Will formalize Pydantic models for domain contracts once team approves Phase 3C decisions. |

---

## 10. Audit Findings: Critical, Major, and Minor

### Critical Findings
*None.* Zero data corruption risks, zero security leaks, and zero breaking changes were identified.

### Major Findings
1. **Unresolved Team Approval on DEC-001 through DEC-006:**
   - The reconstruction DTOs and extraction candidates rely on the decisions outlined in `phase-3c-contract-decisions.md`. Until all 4 members formally sign off on these decisions, canonical domain contracts in `shared/schemas/` cannot be finalized.
2. **Missing Domain Pydantic Models in Backend:**
   - Core objects (`Case`, `Evidence`, `Entity`, `Event`, `Relationship`) exist only as markdown documentation in `shared/schemas/README.md`. Formal Pydantic schemas in `backend/app/schemas/domain.py` must be authored collaboratively with Member 2.

### Minor Findings
1. **Luhn Checksum Telemetry in IMEI Extraction:**
   - `ner.luhn_checksum_valid()` is tested but not currently surfaced on `ExtractedIdentifier`. Adding `is_checksum_valid: Optional[bool]` will improve audit telemetry.
2. **Roadmap Phase Naming Discrepancies:**
   - In `system-overview.md`, extraction is listed as Phase 2 and ingestion as Phase 1. However, Member 1 was tasked with Phase 1A/1B (extraction), and Member 4 submitted Phase 3A–3C (reconstruction). A standardized milestone taxonomy should be adopted.
3. **Terminology Precision:**
   - Clarified that `.sha256` files represent unkeyed hash checksums, not asymmetric digital signatures.

---

## 11. Exact Recommended Fixes

1. **Phase 1C Task:** Update `ExtractedIdentifier` in `backend/app/schemas/extraction.py` and `backend/app/modules/extraction/ner.py` to evaluate and store `luhn_checksum_valid(raw_val)` on IMEI candidates.
2. **Phase 1C Task:** Amend the validation in `backend/app/modules/reconstruction/temporal.py` (line 298) from:
   `start_utc <= nominal_utc <= end_utc`
   to:
   `start_utc <= nominal_utc < end_utc`
   to enforce strict half-open interval semantics $[start, end)$.
3. **Phase 1C Task:** Author canonical domain Pydantic models in `backend/app/schemas/domain.py` matching `shared/schemas/README.md`.

---

## 12. Team Approval Decisions Required

The following decisions are submitted to the team:
1. **Approval of DEC-001 through DEC-006:** Formally approve the six contract decisions with Member 1's DEC-002 amendment.
2. **Approval of `tzdata` Dependency:** Formally ratify the addition of `tzdata>=2024.1` in `backend/pyproject.toml` to ensure cross-platform test stability across Windows and Linux.
3. **Approval of Phase 1B Extraction Subsystem:** Accept the CDR, chat, NER, resolution, and relationship candidate implementations as the foundation for SUTRAM's extraction pipeline.
4. **Ratification of Milestone Roadmap:** Adopt the unified milestone taxonomy:
   - Milestone 0: Initialization & Skeletons (Completed)
   - Milestone 1: Evidence Ingestion & Structured Extraction (In Progress)
   - Milestone 2: Multimodal Extraction & Document OCR
   - Milestone 3: Knowledge Graph Persistence & Cross-Source Correlation
   - Milestone 4: Crime Reconstruction & Contradiction Detection
   - Milestone 5: Decision Support & Reporting

---
*Audit completed in accordance with AGENTS.md, SUTRAM 2026 Directives, and ISO/IEC forensic standards.*
