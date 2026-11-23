# SUTRAM 2026 — Phase 1B.3 Consistency & Forensic Review
**Author:** Member 1 (AI/ML & Evidence Intelligence)  
**Date:** November 18, 2026  
**Status:** Completed  
**Branch:** `main` (Local working tree intact, uncommitted)

---

## 1. Executive Summary

Phase 1B.3 resolves documentation inconsistencies, forensic confidence score semantics, decision register collisions, and synthetic fixture hash discrepancies identified during the Phase 1B.2 review. This audit ensures that the extraction subsystem strictly maintains forensic integrity principles, avoids confidence or identity fabrications, preserves all original evidentiary data, and adheres to cross-member contract boundaries.

---

## 2. Confidence-Score Semantics & Decoupling Audit

### 2.1 Findings on Phase 1B.2 Narrative vs. Implementation
In the Phase 1B.2 review report, it was stated that valid IMEI candidates received confidence `0.95`, while invalid IMEI candidates received confidence `0.70`. 

An inspection of the codebase revealed:
1. `backend/app/schemas/extraction.py` (`ExtractedIdentifier`) had **no confidence field** implemented at all; it contained only `is_checksum_valid`.
2. The values `0.95` and `0.70` were ungrounded narrative claims from early contract discussion, not actual code or empirical probabilities.
3. Conflating a mathematical checksum failure with reduced "confidence" violates forensic integrity: a checksum is a deterministic mathematical check, not a probability that the text was extracted correctly.

### 2.2 Conceptual Separation of Forensic Metrics
To prevent misleading forensic claims, three distinct dimensions are now strictly separated:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                       FORENSIC METRIC SEPARATION                            │
├──────────────────────┬──────────────────────────────────────────────────────┤
│ Metric               │ Definition & Operational Scope                       │
├──────────────────────┼──────────────────────────────────────────────────────┤
│ is_checksum_valid    │ Deterministic Luhn mod-10 formula result for IMEIs   │
│                      │ (True = valid, False = invalid, None = not applicable)│
├──────────────────────┼──────────────────────────────────────────────────────┤
│ extraction_confidence│ Statistical or model probability in [0.0, 1.0] that  │
│                      │ a candidate span represents the claimed category.    │
│                      │ None for deterministic regex with no ML model.       │
├──────────────────────┼──────────────────────────────────────────────────────┤
│ entity_similarity    │ Multi-hop or string-distance similarity score        │
│                      │ indicating match likelihood between two entities.    │
└──────────────────────┴──────────────────────────────────────────────────────┘
```

### 2.3 Correction Implemented
1. **Schema Extension (`ExtractedIdentifier`):**
   ```python
   extraction_confidence: Optional[float] = Field(
       default=None,
       ge=0.0,
       le=1.0,
       description=(
           "Statistical model or extraction-quality confidence in [0.0, 1.0]. "
           "Set to None for deterministic regex/rule-based matches where no empirical probabilistic "
           "model is used. Completely decoupled from mathematical checksum validity (is_checksum_valid)."
       ),
   )
   ```
   A convenience property `.confidence` is provided as an alias for `extraction_confidence`.
2. **Decoupling from Checksum:**
   Deterministic regex pattern matching does not assign an arbitrary probability based on Luhn checks. Both valid and invalid IMEI candidates maintain `extraction_confidence = None`.
3. **Invalid Candidate Preservation:**
   Invalid IMEI candidates continue to be preserved in full (`is_checksum_valid = False`), retaining exact character spans and context snippets.
4. **Verification Tests:**
   Two dedicated unit tests (`test_checksum_validity_does_not_affect_extraction_confidence` and `test_extracted_identifier_confidence_decoupling`) confirm that altering Luhn checksum validity has zero impact on extraction confidence.

---

## 3. IMEI and IMSI Technical Boundary Verification

The strict boundary between device transceivers (IMEI) and subscriber profiles (IMSI) has been re-verified:

| Metric | IMEI (Handset / Terminal) | IMSI (SIM Card / Subscriber Profile) |
| :--- | :--- | :--- |
| **Standard** | 3GPP TS 23.003 §6.2 / 3GPP TS 22.016 | ITU-T Recommendation E.212 |
| **Structure** | 8 digits TAC + 6 digits SNR + 1 Check Digit (15 digits) | 3 digits MCC + 2–3 digits MNC + 9–10 digits MSIN (14–15 digits) |
| **Validation** | **Luhn mod-10 algorithm** | **E.212 allocation tables** (No Luhn algorithm) |
| **System Telemetry** | `is_checksum_valid = True | False` | `is_checksum_valid = None` |
| **Forensic Consequence** | Validates hardware manufacturing specification | Applying Luhn to IMSI causes ~90% false rejections, corrupting evidence |

---

## 4. Decision Register Reconciliation

### 4.1 Preserving Member 4's Original Decision Register (DEC-001 through DEC-006)
In `docs/member-1/phase-1b2-hardening-notes.md`, Member 1 inadvertently reused the identifiers DEC-001 through DEC-006 for new engineering topics. 

Member 4's original decision definitions from `docs/member-4/phase-3c-contract-decisions.md` are formally restored and preserved:

| Decision ID | Original Decision Topic | Proposed Canonical Value | Status | Member 1 Stance |
| :--- | :--- | :--- | :--- | :--- |
| **DEC-001** | Clock Offset Convention | Additive Correction: $\text{True UTC} = \text{Device Time} + \text{Offset}$ | `PROPOSED` | **REVIEWED BY MEMBER 1 (Endorsed)** |
| **DEC-002** | TimePrecision Enums | `EXACT_SECOND`, `MINUTE`, `HOUR`, `DAY`, `ESTIMATED` with local-first flooring | `PROPOSED` | **REVIEWED BY MEMBER 1 (Endorsed)** |
| **DEC-003** | RelationType Taxonomy | Include `CONTRADICTS` and `CO_OCCURRED_WITH`; generalize `CALL_MADE` | `PROPOSED` | **REVIEWED BY MEMBER 1 (Endorsed)** |
| **DEC-004** | Relationship.weight Nullability | `weight: Optional[float] = None` (nullable; never default null to 0.0) | `PROPOSED` | **REVIEWED BY MEMBER 1 (Endorsed)** |
| **DEC-005** | Event Duration & Uncertainty | Add `duration_seconds`, `uncertainty_window_start`, `uncertainty_window_end` | `PROPOSED` | **REVIEWED BY MEMBER 1 (Endorsed)** |
| **DEC-006** | Evidence Metadata Schema | Standardize `source_timezone`, `clock_offset_seconds`, `recording_start_timestamp`, `device_coordinates` | `PROPOSED` | **REVIEWED BY MEMBER 1 (Endorsed)** |

### 4.2 Member 1 Engineering Recommendations (M1-PROP Series)
To prevent identifier collision, Member 1's additional architectural recommendations are assigned explicit, separate proposal identifiers:

| Proposal ID | Topic | Proposed Canonical Specification | Status | Implementation in Provisional Slice |
| :--- | :--- | :--- | :--- | :--- |
| **M1-PROP-001** | UTC ISO 8601 String Serialization | All API DTOs and extraction results serialize UTC timestamps with trailing `Z` (`YYYY-MM-DDTHH:MM:SSZ`), microsecond resolution optional (`.ffffffZ`). No local offsets in transport. | `PROPOSED` | `IMPLEMENTED` internally in provisional extraction modules (`cdr.py`, `chat.py`). |
| **M1-PROP-002** | GeoJSON Coordinate Ordering | All spatial coordinate pairs serialize strictly as `[longitude, latitude]` (EPSG:4326), with optional altitude in meters. Coordinates in `[lon, lat]` order per RFC 7946. | `PROPOSED` | `IMPLEMENTED` internally in provisional extraction modules (`cdr.py`). |
| **M1-PROP-003** | Confidence Score Bounds & Telemetry Decoupling | Extraction confidence normalized to $[0.0, 1.0]$. Checksum telemetry (`is_checksum_valid`) is strictly decoupled from extraction confidence. Arbitrary values must never be labeled calibrated probabilities. | `PROPOSED` | `IMPLEMENTED` internally in `extraction.py` and `ner.py`. |
| **M1-PROP-004** | Canonical Provenance URN Scheme | All extracted entities, identifiers, and relationship candidates link to source evidence via uniform resource names: `urn:sutram:evidence:{evidence_id}:{sub_type}:{offset_or_id}`. | `PROPOSED` | `IMPLEMENTED` internally in provisional extraction DTOs (`source_record_id`, `location_reference`). |
| **M1-PROP-005** | Contradiction Severity Taxonomy | 4 standardized severity tiers (`CRITICAL`, `MAJOR`, `MODERATE`, `MINOR`) with auditable resolution workflows. Extraction preserves discordant facts without premature filtering. | `PROPOSED` | Awaiting team review; extraction engine preserves discordant facts for contradiction evaluation. |

---

## 5. Team Approval Status Definitions & Current State

To ensure compliance with **AGENTS.md Section 6**, the following decision lifecycle statuses are enforced:
1. `PROPOSED`: Formally documented by a member; open for evaluation.
2. `REVIEWED BY MEMBER X`: Evaluated and commented on by a specific member.
3. `APPROVED BY TEAM`: Formally signed off by all 4 team members.
4. `IMPLEMENTED`: Incorporated into working code.

**Current State:**
- **DEC-001 through DEC-006:** `PROPOSED` by Member 4; `REVIEWED BY MEMBER 1` (All Endorsed). Awaiting review and sign-off from Members 2 and 3.
- **M1-PROP-001 through M1-PROP-005:** `PROPOSED` by Member 1. Awaiting review from Members 2, 3, and 4.
- **Strict Rule:** **NO decision or proposal currently has `APPROVED BY TEAM` status.** Canonical shared contracts in `shared/schemas/` and `shared/enums/` remain unchanged until unanimous sign-off is achieved.

---

## 6. Synthetic Fixture SHA-256 Investigation

### 6.1 Discrepancy Overview
During Phase 1B.2, the following discrepancy appeared between recorded documentation:
- **Phase 1B / 1B.1 Recorded Hashes:**
  - CDR: `c874a7c8b0e05da52db5f29d5bb0f398439237ad69a757cb05bfe9ec76803a8d`
  - Chat: `78eaf79b73ec12a15b15f3d26386c3f2456020c914d7c496aa9efeca20fec40e`
- **Phase 1B.2 Report Claims:**
  - CDR: `c874a7c8c3e8003f56d98d28734e022f66453fb69b76c8c4a1b02ee9d6ea4e4d`
  - Chat: `78eaf79be8756ca316e6ba6813bf6b9b3c4013fcff49cfd1b82fa37397b98d23`

### 6.2 Forensic Hash Recomputation
A complete binary recomputation from the on-disk files yielded:
- **`data/synthetic/sample_cdr.csv`:**
  - Size: 624 bytes
  - Computed SHA-256: `c874a7c8b0e05da52db5f29d5bb0f398439237ad69a757cb05bfe9ec76803a8d`
  - Sidecar file (`sample_cdr.csv.sha256`): `c874a7c8b0e05da52db5f29d5bb0f398439237ad69a757cb05bfe9ec76803a8d`
  - **Status:** **100% Invariant Match with Sidecar.**
- **`data/synthetic/sample_chat_export.json`:**
  - Size: 1,241 bytes
  - Computed SHA-256: `78eaf79b73ec12a15b15f3d26386c3f2456020c914d7c496aa9efeca20fec40e`
  - Sidecar file (`sample_chat_export.json.sha256`): `78eaf79b73ec12a15b15f3d26386c3f2456020c914d7c496aa9efeca20fec40e`
  - **Status:** **100% Invariant Match with Sidecar.**

### 6.3 Root Cause Analysis
1. The synthetic evidence files and their sidecars were **never modified** after their initial generation in Phase 1B.
2. In the Phase 1B.2 session, an inline Python verification snippet truncated the hash to 16 characters (`computed[:16]`) during output:
   ```
   sample_cdr.csv: computed=c874a7c8b0e05da5... match=True
   sample_chat_export.json: computed=78eaf79b73ec12a1... match=True
   ```
3. When authoring the Phase 1B.2 markdown report and notes, the full 64-character hex strings were not read directly from the `.sha256` sidecars, resulting in hallucinated hash suffixes (`...c3e8003f...` and `...e8756ca3...`) being transcribed into the documentation.
4. **Resolution:** The documentation in `phase-1b2-hardening-notes.md` has been corrected to reflect the true, invariant binary SHA-256 hashes. Neither fixture file was modified.

---

## 7. Regression & Quality Test Results

### 7.1 NER & Identifier Extraction Test Suite
Command: `uv run --directory backend pytest tests/extraction/test_ner.py -v`
```text
tests/extraction/test_ner.py::test_email_extraction PASSED               [  8%]
tests/extraction/test_ner.py::test_phone_number_extraction PASSED        [ 16%]
tests/extraction/test_ner.py::test_contextual_imei_and_imsi_extraction PASSED [ 25%]
tests/extraction/test_ner.py::test_isolated_15_digit_number_reported_as_ambiguous PASSED [ 33%]
tests/extraction/test_ner.py::test_conflicting_context_reported_as_ambiguous PASSED [ 41%]
tests/extraction/test_ner.py::test_valid_imei_checksum_telemetry PASSED  [ 50%]
tests/extraction/test_ner.py::test_invalid_imei_checksum_preserved PASSED [ 58%]
tests/extraction/test_ner.py::test_imsi_does_not_apply_luhn_checksum PASSED [ 66%]
tests/extraction/test_ner.py::test_multiple_identifiers_in_single_message PASSED [ 75%]
tests/extraction/test_ner.py::test_luhn_checksum_validator PASSED        [ 83%]
tests/extraction/test_ner.py::test_checksum_validity_does_not_affect_extraction_confidence PASSED [ 91%]
tests/extraction/test_ner.py::test_extracted_identifier_confidence_decoupling PASSED [100%]

============================= 12 passed in 0.44s ==============================
```

### 7.2 Full Backend Regression Suite
Command: `uv run --directory backend pytest -q`
```text
122 passed, 50 skipped, 1 warning in 1.69s
```
- **Passed:** 122 tests (100% of runnable backend suite).
- **Skipped:** 50 tests (video transcoding / ffmpeg system tests skipped when binary not installed in test environment).
- **Warnings:** 1 (Starlette test client httpx deprecation notice).

### 7.3 Post-Test Fixture Hash Verification
Re-verification following test execution confirmed that test execution did not touch or modify the fixture files:
- `sample_cdr.csv`: SHA-256 `c874a7c8b0e05da52db5f29d5bb0f398439237ad69a757cb05bfe9ec76803a8d` (Match = True)
- `sample_chat_export.json`: SHA-256 `78eaf79b73ec12a15b15f3d26386c3f2456020c914d7c496aa9efeca20fec40e` (Match = True)

---

## 8. Files Changed & Preserved

### Modified Files:
- [backend/app/schemas/extraction.py](../../../backend/app/schemas/extraction.py): Added `extraction_confidence` field and `.confidence` property to `ExtractedIdentifier`, explicitly decoupled from `is_checksum_valid`.
- [backend/tests/extraction/test_ner.py](../../../backend/tests/extraction/test_ner.py): Added unit tests verifying confidence decoupling and model independence.
- [docs/member-1/phase-1b2-hardening-notes.md](phase-1b2-hardening-notes.md): Reconciled decision register, separated M1-PROP series, corrected confidence narrative, and corrected fixture hashes.

### New Documentation Created:
- [docs/member-1/phase-1b3-consistency-review.md](phase-1b3-consistency-review.md): Complete Phase 1B.3 audit report.

### Preserved Files (Untouched):
- `shared/schemas/README.md`, `shared/enums/README.md`, `shared/examples/README.md`: Canonical shared contracts preserved.
- `backend/app/modules/reconstruction/temporal.py`: Member 4 reconstruction code untouched.
- `docs/member-4/phase-3c-contract-decisions.md`: Member 4 decision proposal untouched.
- `data/synthetic/sample_cdr.csv` & `.sha256`: Synthetic fixtures preserved.
- `data/synthetic/sample_chat_export.json` & `.sha256`: Synthetic fixtures preserved.
- `docs/member-1/phase-1-extraction-contract-review.md`, `docs/member-1/phase-1b-extraction-implementation.md`, `docs/member-1/phase-1b1-readiness-audit.md`: Prior Member 1 documentation preserved.

---

## 9. Remaining Blockers & Next Steps

1. **Cross-Member Alignment on DEC-001..DEC-006 & M1-PROP-001..M1-PROP-005:**
   Formal review by Members 2 and 3 is required before any changes can be merged into `shared/schemas/`.
2. **Member 2 Ingestion Service Interfaces:**
   Member 2 must publish the canonical Ingestion API endpoints and database models (Case, Evidence, Entity, Event, Relationship) before the extraction pipeline can be wired into backend persistence.
3. **Commit & Staging Authorization:**
   All Phase 1A through Phase 1B.3 work resides cleanly in the local working tree on `main`. Once authorized by the team, these changes should be staged and committed atomically under conventional commit conventions (`feat(extraction): add multimodal extraction slice with telemetry and provenance`).

---

## 10. Recommendations for Phase 1C

1. Present the reconciled decision register to the full 4-member team for unanimous sign-off.
2. Coordinate with Member 2 to define the exact ingest-to-extract service boundary:
   - Ingestion service stores raw evidence file and calculates SHA-256 hash.
   - Ingestion service delegates parsing to `CDRExtractor` or `ChatExtractor`.
   - Extractor returns validated DTOs (`CDRExtractionResult` / `ChatExtractionResult`).
   - Ingestion service persists entities and events into PostgreSQL and Neo4j.
3. Implement Phase 1C once Member 2's ingestion endpoint skeletons are ready.
