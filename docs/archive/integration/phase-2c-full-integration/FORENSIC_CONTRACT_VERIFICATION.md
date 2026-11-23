# YUKTI — Forensic Contract Verification Report

**Project:** YUKTI (SUTRAM 2026)  
**Document ID:** FORENSIC-P2C-008  
**Version:** 1.0.0  
**Date:** 2026-11-20  

---

## 1. Forensic Integrity Principles (Section 4 Compliance)

YUKTI's software architecture enforces six non-negotiable forensic integrity directives:

1. **Immutability of Evidence:** Original evidence files are written once to `data/raw/<evidence_id>/` and marked read-only. No processing pipeline mutates raw bytes in place.
2. **Cryptographic Provenance:** Every evidence ingestion computes streaming SHA-256 and SHA-3 hashes.
3. **Chain of Custody:** Source files, byte offsets, extraction timestamps, and parser version hashes are logged into immutable JSON manifests.
4. **Observed vs Inferred Distinction:** Hard telecommunication carrier facts (CDR timestamps, cell tower IDs) are strictly separated from probabilistic inferences (NLP entity extraction, cross-source name matching).
5. **No Identity Fabrications:** Heuristic confidence scores are never asserted as conclusive proof of personal identity or guilt.
6. **Synthetic Development Data:** All development and automated test suites strictly use synthetic data (`data/synthetic/`).

---

## 2. Decision Contracts DEC-001 through DEC-006

Member 4 (Swastik) established six foundational forensic decision contracts. Their implementation and verification across the Rust backend are documented below:

### 2.1 DEC-001: Additive Clock Drift Correction
- **Formula:** $T_{\text{true UTC}} = T_{\text{device}} + \Delta t_{\text{offset}}$
- **Convention:**
  - If a device clock is fast by 30 seconds, its reported time is ahead; the offset is negative ($-30.0\text{s}$). True UTC is earlier.
  - If a device clock is slow by 45 seconds, the offset is positive ($+45.0\text{s}$). True UTC is later.
- **Verification:** Verified by `test_dec001_clock_offset_additive_convention` in `backend-rust/tests/reconstruction_tests.rs`. Passed.

### 2.2 DEC-002: Half-Open Interval Semantics $[t_{\text{start}}, t_{\text{end}})$
- **Principle:** All temporal intervals are modeled as half-open: inclusive of start, exclusive of end.
- **Precision Boundaries:**
  - Minute precision (e.g. `2026-11-18T12:00`): $[12:00:00.000, 12:01:00.000)$.
  - Second precision (e.g. `12:00:15`): $[12:00:15.000, 12:00:16.000)$.
- **Overlap Criterion:** Two intervals $A = [s_A, e_A)$ and $B = [s_B, e_B)$ overlap if and only if:
  $$\max(s_A, s_B) < \min(e_A, e_B)$$
  Adjacent intervals $[12:00, 12:01)$ and $[12:01, 12:02)$ do not overlap.
- **Verification:** Verified by `test_dec002_half_open_interval_semantics` and `test_dec002_interval_overlap_logic`. Passed.

### 2.3 DEC-003: Haversine Distance & Conservative Minimum Speed
- **Distance Formula:** Great-circle distance using Haversine with Earth radius $R = 6371.0\text{ km}$:
  $$a = \sin^2\left(\frac{\Delta \phi}{2}\right) + \cos(\phi_1)\cos(\phi_2)\sin^2\left(\frac{\Delta \lambda}{2}\right)$$
  $$d = 2R \arcsin(\sqrt{a})$$
- **Elapsed Time Bounds:** Taking uncertainty windows $[s_1, e_1)$ and $[s_2, e_2)$ into account:
  $$\Delta t_{\text{min}} = \max(0, s_2 - e_1)$$
  $$\Delta t_{\text{max}} = e_2 - s_1$$
- **Conservative Speed ($v_{\text{min}}$):**
  $$v_{\text{min}} = \frac{d}{\Delta t_{\text{max}}}$$
  Using maximum possible elapsed time produces the most conservative (lowest possible) speed estimate, favoring the defense/suspect against false accusations.
- **Verification:** Verified by `test_dec003_haversine_distance_calculation` and `test_dec003_conservative_minimum_speed`. Passed.

### 2.4 DEC-004: Physical Impossibility Contradiction Detection
- **Ground Transit Threshold:** Configured at $120.0\text{ km/h}$ for road/urban transit.
- **Contradiction Trigger:** If $v_{\text{min}} > 120.0\text{ km/h}$, an impossible transit contradiction is flagged.
- **Simultaneous Co-Presence:** If a single device or entity has overlapping intervals ($A \cap B \neq \emptyset$) at two distinct locations separated by $> 500\text{ meters}$, an existential co-presence contradiction is flagged.
- **Verification:** Verified by `test_dec004_speed_threshold_contradiction_detection` and `test_dec004_simultaneous_copresence_contradiction`. Passed.

### 2.5 DEC-005: Video Evidence Cryptographic Provenance
- **Integrity Guarantee:** Video files are hashed via constant-memory streaming buffers (64 KB chunks).
- **Provenance Link:** Extracted frame timestamps, OCR detections, and CCTV vehicle plate numbers retain cryptographic SHA-256 links to the source video file hash and byte offset.
- **Verification:** Verified by `test_dec005_video_sha256_provenance`. Passed.

### 2.6 DEC-006: Neutral Non-Guilt Asserting Explanations
- **Neutral Language Mandate:** Machine explanations must state physical facts, uncertainties, and alternative explanations without using legal assertions of guilt (e.g. "suspect fled", "perpetrator lied").
- **Prescribed Phrasing:** "Physical co-presence cannot be established based on available data" or "Observed minimum speed of 384 km/h exceeds ground vehicle limits; indicates distinct devices, shared SIM card, or clock drift."
- **Verification:** Verified by `test_dec006_neutral_explanation_phrasing`. Passed.

---

## 3. Judicial Export Safeguard (`SHIP_EXPORT_BLOCKED`)

To prevent preliminary or contradictory findings from being prematurely submitted to judicial authorities, YUKTI implements an automated court export gate:
- If a case reconstruction contains unresolved physical contradictions (e.g. an unverified speed anomaly), the system flags the reconstruction with `export_safeguard: "BLOCKED"`.
- In the frontend (`ReportsView.tsx`), the export button is guarded by the amber **`SHIP_EXPORT_BLOCKED`** badge.
- The investigator must either resolve the anomaly (e.g. applying a calibrated clock offset) or document an explicit forensic override before court-ready PDF/JSON exports are unlocked.
