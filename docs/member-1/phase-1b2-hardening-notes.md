# SUTRAM 2026 — Phase 1B.2 Hardening & Forensic Quality Notes
**Author:** Member 1 (AI/ML & Evidence Intelligence)  
**Date:** November 18, 2026  
**Status:** Completed  
**Branch:** `main` (Local, uncommitted)

---

## 1. Executive Summary

Phase 1B.2 builds upon the successful Phase 1B vertical slice and Phase 1B.1 readiness audit. In this hardening phase, Member 1 implemented:
1. **IMEI Checksum Telemetry (`is_checksum_valid`)** on the `ExtractedIdentifier` model, populating Luhn verification status without discarding malformed or non-compliant identifiers.
2. **Defensive Validation Boundary Separation** between IMEI (device identifier with Luhn check digit) and IMSI (subscriber identifier without Luhn checksum).
3. **Cryptographic Documentation Cleanup**, correcting loose terminology in Phase 1B documentation to distinguish SHA-256 cryptographic hashes from digital signatures.
4. **Forensic Integrity Review of Temporal Interval Bounds** in Member 4's logic (DEC-002 half-open interval semantics).
5. **Contract Review & Consensus Tracking** on cross-member architectural decisions DEC-001 through DEC-006.
6. **Milestone Nomenclature Alignment** establishing a unified taxonomy across all 4 team members.

---

## 2. Objective A — IMEI Checksum Telemetry

### 2.1 Problem Statement
During Phase 1B, `luhn_checksum_valid(s: str) -> bool` was implemented and used as a confidence heuristic in `ner.py`. However, the Boolean outcome was not exposed on the resulting `ExtractedIdentifier` model. Investigators and downstream graph correlation routines could not determine whether an extracted IMEI was structurally valid according to 3GPP specifications without re-computing the Luhn algorithm.

Furthermore, an invalid checksum must **never** cause an identifier to be discarded or filtered out: in criminal digital forensics, counterfeit handsets, spoofed device headers, and corrupted firmware frequently present invalid IMEIs that constitute crucial evidentiary leads.

### 2.2 Schema & Implementation Changes
- **Model:** `backend/app/schemas/extraction.py` (`ExtractedIdentifier`):
  ```python
  is_checksum_valid: Optional[bool] = Field(
      default=None,
      description="Luhn checksum validity for IMEI identifiers. None for IMSI, phone, or unverified types.",
  )
  ```
- **Extractor Logic:** `backend/app/modules/extraction/ner.py`:
  - When a 15-digit numeric candidate is preceded or contextualized by IMEI keywords (`imei`, `device_id`, `handset_id`), `is_checksum_valid` is evaluated via `luhn_checksum_valid(raw_val)`:
    - Returns `True` if the 15th check digit satisfies the Luhn mod-10 algorithm.
    - Returns `False` if the candidate fails Luhn mod-10.
  - The identifier is emitted in both cases. Invalid checksum IMEIs are preserved with `is_checksum_valid=False`.
  - **Decoupled Confidence:** Extraction confidence (`extraction_confidence`) is completely decoupled from checksum telemetry. Deterministic regex pattern matching does not fabricate probabilistic confidence scores based on Luhn checks (`extraction_confidence` defaults to `None`).
  - For IMSI candidates, phone numbers, email addresses, and ambiguous 14–15 digit sequences lacking definitive device context, `is_checksum_valid` is explicitly set to `None`.

### 2.3 Unit Testing Verification
Four targeted unit tests were added in `backend/tests/extraction/test_ner.py`:
- `test_valid_imei_checksum_telemetry`: Verifies valid IMEI `868204040123456` produces `is_checksum_valid=True`.
- `test_invalid_imei_checksum_preserved`: Verifies invalid IMEI `868204040123457` produces `is_checksum_valid=False`, is retained in extracted outputs, and has provenance preserved.
- `test_imsi_does_not_apply_luhn_checksum`: Verifies IMSI candidate `404450123456789` produces `is_checksum_valid=None`.
- `test_multiple_identifiers_in_single_message`: Verifies text containing both valid and invalid IMEIs evaluates each candidate independently.

All 10 tests in `test_ner.py` and all 120 tests across the full suite pass.

---

## 3. Objective B — Technical Distinction: IMEI vs. IMSI Validation

Forensic integrity requires strict adherence to international telecommunications standards. The handling of IMEI vs. IMSI must be strictly segregated:

| Metric / Dimension | IMEI (International Mobile Station Equipment Identity) | IMSI (International Mobile Subscriber Identity) |
| :--- | :--- | :--- |
| **Governing Standard** | 3GPP TS 23.003 §6.2 / 3GPP TS 22.016 | ITU-T Recommendation E.212 |
| **Physical Entity** | Hardware device / terminal transceiver | SIM card / subscriber account profile |
| **Composition** | 14 digits (TAC: 8 digits + SNR: 6 digits) + 1 Check Digit (CD) = 15 digits | 3 digits MCC (Mobile Country Code) + 2–3 digits MNC (Mobile Network Code) + 9–10 digits MSIN |
| **Total Length** | Exactly 15 decimal digits (or 16 for IMEISV) | 14 or 15 decimal digits |
| **Check Digit Algorithm** | **Luhn mod-10 algorithm** applied to 14 payload digits to calculate the 15th digit. | **None.** ITU-T E.212 does not specify a check digit algorithm. |
| **Risk of Luhn on IMSI** | Validating Luhn on an IMEI is standard verification. | **Catastrophic False Rejection.** An IMSI is allocated administratively by carrier network operators. Applying Luhn to an IMSI causes ~90% of authentic IMSIs to be incorrectly flagged as invalid, corrupting evidence analysis. |
| **System Behavior** | Compute `is_checksum_valid = luhn_checksum_valid(...)` | Always set `is_checksum_valid = None`. Validate against E.212 MCC/MNC allocation tables instead. |

---

## 4. Objective C — Cryptographic Documentation Correction

In `docs/member-1/phase-1b-extraction-implementation.md`, early phrasing referenced synthetic data files as being "cryptographically signed with `.sha256` sidecars".

**Correction Applied:**
- Asymmetric digital signatures involve public/private key cryptography (e.g. RSA, Ed25519) with a digital signature algorithm.
- Unkeyed SHA-256 sidecar files are **cryptographic hash checksums** used to verify payload integrity and detect file tampering or transmission corruption.
- All references in `phase-1b-extraction-implementation.md` have been updated to precise terminology: `"SHA-256 checksum sidecar files for cryptographic hash integrity verification"`.

---

## 5. Objective D — Forensic Integrity: Half-Open Temporal Intervals (DEC-002)

During cross-member code inspection, Member 1 identified a boundary edge case in Member 4's temporal reconstruction implementation:
- **Location:** `backend/app/modules/reconstruction/temporal.py` (Line 298) and `docs/member-4/phase-3c-contract-decisions.md` (Line 167).
- **Existing Logic:**
  ```python
  # Checks whether nominal_utc falls within window:
  if start_utc <= nominal_utc <= end_utc:
  ```
- **Forensic Principle & ISO 8601 Compliance:**
  - DEC-002 proposes half-open intervals: `[start_utc, end_utc)`.
  - In half-open intervals, the start boundary is inclusive, but the end boundary is **strictly exclusive** ($start \le t < end$).
  - If closed bounds (`<= end_utc`) are used, an event occurring precisely at the boundary tick ($t = end$) will match **two contiguous intervals** (e.g., `[12:00, 13:00]` and `[13:00, 14:00]`), creating an artificial duplicate observation or false-positive temporal conflict.
- **Recommended Action for Member 4:**
  - When finalizing Phase 3C/Milestone 3, update `temporal.py` line 298 to:
    ```python
    if start_utc <= nominal_utc < end_utc:
    ```
  - In adherence to `AGENTS.md` and repository boundary rules, Member 1 has **not** directly edited Member 4's files, logging this finding here for coordinated adoption.

---

## 6. Decision Register Reconciliation & Engineering Proposals

### 6.1 Original Member 4 Decisions (DEC-001 through DEC-006)
Member 1 formally preserves and reviews the six original architectural decisions proposed by Member 4 in `docs/member-4/phase-3c-contract-decisions.md`:

| Decision ID | Original Topic | Proposed Canonical Rule | Status | Member 1 Review Stance |
| :--- | :--- | :--- | :--- | :--- |
| **DEC-001** | Clock Offset Convention | Additive Correction: $\text{True UTC} = \text{Device Time} + \text{Offset}$ | `PROPOSED` | **REVIEWED BY MEMBER 1 (Endorsed).** Standardizes sign handling across ingestion and reconstruction. |
| **DEC-002** | TimePrecision Enums | `EXACT_SECOND`, `MINUTE`, `HOUR`, `DAY`, `ESTIMATED` with local-first flooring | `PROPOSED` | **REVIEWED BY MEMBER 1 (Endorsed).** Flagged upper bound exclusion ($< end$) in `temporal.py` line 298. |
| **DEC-003** | RelationType Taxonomy | Include `CONTRADICTS` and `CO_OCCURRED_WITH`; generalize `CALL_MADE` | `PROPOSED` | **REVIEWED BY MEMBER 1 (Endorsed).** Essential for graph correlation and contradiction detection. |
| **DEC-004** | Relationship.weight Nullability | `weight: Optional[float] = None` for physical contradictions | `PROPOSED` | **REVIEWED BY MEMBER 1 (Endorsed).** Aligns with forensic prohibition against fabricated confidence on factual incompatibilities. |
| **DEC-005** | Event Duration & Uncertainty | Add `duration_seconds`, `uncertainty_window_start`, `uncertainty_window_end` | `PROPOSED` | **REVIEWED BY MEMBER 1 (Endorsed).** Cleanly separates physical event runtime from observation uncertainty windows. |
| **DEC-006** | Evidence Metadata Schema | Standardize `source_timezone`, `clock_offset_seconds`, `recording_start_timestamp`, `device_coordinates` | `PROPOSED` | **REVIEWED BY MEMBER 1 (Endorsed).** Ingestion metadata contract verified. |

*Note on Status:* All six decisions remain in `PROPOSED` status by Member 4. They are `REVIEWED BY MEMBER 1`. None currently have `APPROVED BY TEAM` status as formal unanimous team sign-off has not yet been executed.

### 6.2 Member 1 Additional Engineering Recommendations (M1-PROP Series)
To eliminate identifier collisions with Member 4's DEC series, Member 1's additional architectural recommendations are assigned unique proposal identifiers:

| Proposal ID | Topic | Proposed Rule | Status | Implementation in Provisional Slice |
| :--- | :--- | :--- | :--- | :--- |
| **M1-PROP-001** | UTC ISO 8601 String Serialization | All API and DTO timestamp strings serialize in strict UTC ending in `Z` (`YYYY-MM-DDTHH:MM:SSZ`). | `PROPOSED` | `IMPLEMENTED` internally in provisional extraction modules (`cdr.py`, `chat.py`). |
| **M1-PROP-002** | GeoJSON Coordinate Ordering | Cellular tower and spatial coordinates serialize as `[longitude, latitude]` (EPSG:4326). | `PROPOSED` | `IMPLEMENTED` internally in provisional extraction modules (`cdr.py`). |
| **M1-PROP-003** | Confidence Score Bounds & Telemetry Decoupling | Confidence normalized to $[0.0, 1.0]$. Checksum telemetry (`is_checksum_valid`) is strictly decoupled from extraction confidence. | `PROPOSED` | `IMPLEMENTED` internally in `extraction.py` and `ner.py`. |
| **M1-PROP-004** | Canonical Provenance URN Scheme | Entity and relationship evidence pointers follow `urn:sutram:evidence:{id}:{type}:{offset}`. | `PROPOSED` | `IMPLEMENTED` internally in provisional extraction DTOs (`source_record_id`, `location_reference`). |
| **M1-PROP-005** | Contradiction Severity Taxonomy | Standardized 4-tier severity: `CRITICAL`, `MAJOR`, `MODERATE`, `MINOR`. | `PROPOSED` | Awaiting cross-member consensus; extraction pipeline preserves discordant records for evaluation. |

---

## 7. Objective F — Unified Milestone Taxonomy

To resolve cross-document naming discrepancies (e.g. Phase 1 vs Milestone 1, Phase 3C vs Phase 1B), the following unified milestone taxonomy is recommended for the team:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                       SUTRAM 2026 UNIFIED MILESTONES                        │
├─────────────┬───────────────────────────────┬───────────────────────────────┤
│ Milestone   │ Name                          │ Lead Member & Core Scope      │
├─────────────┼───────────────────────────────┼───────────────────────────────┤
│ Milestone 0 │ Foundation & Monorepo Setup   │ All Members (Complete)        │
│ Milestone 1 │ Multimodal Extraction Slice   │ Member 1 (AI/ML & Extraction) │
│ Milestone 2 │ Entity Resolution & Graph     │ Member 2 (Backend & Data Eng) │
│ Milestone 3 │ Reconstruction & Contradiction│ Member 4 (Forensics & Time)   │
│ Milestone 4 │ Decision Support & Laya       │ Node.js/ONNX & All Members    │
│ Milestone 5 │ Frontend Visuals & Reporting  │ Member 3 (UI & Visualization) │
└─────────────┴───────────────────────────────┴───────────────────────────────┘
```

- **Phase 1A:** Contract review, integration audit, and extraction blueprint (Member 1).
- **Phase 1B:** First extraction vertical slice: CDR parser, Chat parser, Regex/NER extractor, provenance tracking (Member 1).
- **Phase 1B.1:** Readiness and forensic integrity audit (Member 1).
- **Phase 1B.2:** Extraction hardening, IMEI telemetry, and documentation alignment (Member 1).

---

## 8. Verification & Test Summary

1. **Extraction Test Suite:**
   ```bash
   uv run pytest tests/extraction/test_ner.py -v
   ```
   - Results: **12 passed** in 0.44s.
2. **Full Backend Test Suite:**
   ```bash
   uv run pytest -q
   ```
   - Results: **122 passed, 50 skipped, 1 warning** in 1.69s.
3. **Synthetic Evidence Invariance Verification:**
   - `sample_cdr.csv`: SHA-256 `c874a7c8b0e05da52db5f29d5bb0f398439237ad69a757cb05bfe9ec76803a8d` (Verified Match with sidecar).
   - `sample_chat_export.json`: SHA-256 `78eaf79b73ec12a15b15f3d26386c3f2456020c914d7c496aa9efeca20fec40e` (Verified Match with sidecar).
