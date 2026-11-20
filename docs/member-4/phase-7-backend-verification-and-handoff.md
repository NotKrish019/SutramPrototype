# YUKTI Backend Verification & Member 1 API Handoff Document
**Workstream:** Member 4 (Forensic Intelligence & Integration)  
**Target:** Member 1 (AI/ML & Extraction) / Member 3 (Frontend & Visualization)  
**Date:** 2026-11-21  
**Repository Branch:** `main` (Single Branch Rule)

---

## 1. Executive Summary

This handoff document provides the complete, authoritative API contract for Member 4's production Rust backend services (`backend-rust/`). Over Phases 1 through 6, the backend has been hardened for digital forensic evidentiary standards, deterministic reconstruction, stateless what-if scenario simulation, full evidence-to-claim provenance traceability, evidence-gap intelligence, and bounded India-focused normalization (TRAI/DoT NNP compliance, carrier CDR profiles, and IST timezone localization).

All **172 backend tests** and **264 web frontend tests** are passing. No synthetic fixtures or fallback data are silently substituted for missing case data.

---

## 2. API Contract Specification

All endpoints are mounted on the Axum HTTP router under `/api/v1`.

### 2.1 Trigger Full Case Reconstruction

- **Endpoint:** `POST /api/v1/cases/:case_id/reconstruction`
- **Method:** `POST`
- **Description:** Triggers full timeline reconstruction, spatial trajectory evaluation, contradiction detection, video frame handoff integration, and judicial report synthesis for a case.
- **Path Parameters:**
  - `case_id` (`string`, required): Valid UUIDv4 identifying the case.
- **Request Body (Optional):**
  ```json
  {
    "options": {
      "case_id": "550e8400-e29b-41d4-a716-446655440000",
      "default_timezone": "Asia/Kolkata",
      "default_clock_offset_seconds": 0.0,
      "max_reasonable_speed_mps": 45.0,
      "estimated_half_width_seconds": 900.0,
      "suppress_inferred_assumptions": false
    },
    "event_candidates": [],
    "raw_events": []
  }
  ```
  *Note:* If `event_candidates` or `raw_events` are omitted or empty, events are automatically loaded from prior active extraction runs and video frame exhibits on disk / PostgreSQL.
- **Response (`200 OK`):**
  ```json
  {
    "reconstruction_run_id": "7b84db56-2a81-4235-9ff2-3bfa3f885e72",
    "run_id": "7b84db56-2a81-4235-9ff2-3bfa3f885e72",
    "timeline": {
      "entries": [
        {
          "event_id": "evt-cctv-001",
          "source_evidence_id": "ev-video-gate",
          "raw_timestamp": "2026-11-10 20:15:30",
          "time_window": {
            "nominal_utc": "2026-11-10T14:45:30Z",
            "window_start_utc": "2026-11-10T14:45:30Z",
            "window_end_utc": "2026-11-10T14:45:31Z",
            "precision": "EXACT_SECOND",
            "clock_offset_seconds": 0.0
          },
          "coordinates": { "lat": 28.5355, "lon": 77.271 },
          "raw_description": "Vehicle entering loading dock",
          "assumptions": []
        }
      ],
      "overlaps": []
    },
    "spatial": {
      "segments": [],
      "spatial_overlaps": []
    },
    "contradictions": [
      {
        "lead_id": "lead-spatial-001",
        "contradiction_type": "IMPOSSIBLE_SPEED",
        "event_a_id": "evt-cctv-001",
        "event_b_id": "evt-cdr-002",
        "severity": "CRITICAL",
        "explanation": "Required speed 142.5 m/s exceeds max reasonable speed 45.0 m/s between locations.",
        "robustness": "ROBUST",
        "epistemic_level": "INFERRED"
      }
    ],
    "limitations": [],
    "report": { ... }
  }
  ```
- **Error Responses:**
  - `400 Bad Request`: `{"error": "Invalid case UUID format: 'invalid-id'"}`
  - `500 Internal Server Error`: `{"error": "<explicit persistence failure message>"}`

---

### 2.2 Retrieve Latest Case Reconstruction

- **Endpoint:** `GET /api/v1/cases/:case_id/reconstruction`
- **Method:** `GET`
- **Description:** Retrieves the latest full reconstruction snapshot from PostgreSQL or filesystem derived storage (`derived/:case_id/reconstruction/latest.json`). If none has been persisted yet, dynamically computes the baseline.
- **Response (`200 OK`):** `FullCaseReconstructionResult` (identical to 2.1).

---

### 2.3 Stateless What-If Reconstruction Simulation

- **Endpoints:**
  - `POST /api/v1/cases/:case_id/what-if`
  - `POST /api/v1/cases/:case_id/reconstruction/simulate` (alias)
- **Method:** `POST`
- **Description:** Performs an in-memory, non-mutating what-if reconstruction simulation under investigator-supplied alternative assumptions. **Guarantees that official reconstruction records, raw evidence, and custody logs are never modified.**
- **Request Body:**
  ```json
  {
    "scenario_name": "Shift DVR Clock by -120 Seconds",
    "rationale": "Test whether suspect vehicle overlap disappears under calibrated camera drift.",
    "assumptions": [
      {
        "assumption_id": "asm-clock-01",
        "target_evidence_id": "ev-video-gate",
        "target_event_id": null,
        "assumption_type": "CLOCK_OFFSET",
        "parameter_key": "clock_offset_seconds",
        "original_value": 0.0,
        "alternative_value": -120.0,
        "justification": "DVR hardware clock drift observed during seizure"
      }
    ],
    "excluded_evidence_ids": [],
    "excluded_event_ids": []
  }
  ```
- **Response (`200 OK`):**
  ```json
  {
    "scenario_id": "sim-c9f12345-6789-4abc-def0-123456789abc",
    "case_id": "550e8400-e29b-41d4-a716-446655440000",
    "scenario_name": "Shift DVR Clock by -120 Seconds",
    "created_at": "2026-11-21T03:00:00Z",
    "is_official": false,
    "applied_assumptions": [ ... ],
    "comparison": {
      "baseline_findings_count": 2,
      "scenario_findings_count": 1,
      "unchanged_findings": ["lead-spatial-002"],
      "newly_appearing_findings": [],
      "disappearing_findings": ["lead-spatial-001"],
      "robustness_changes": [],
      "explanations": [
        "Finding 'lead-spatial-001' disappears under alternative assumptions: Contradiction resolved by -120s clock correction."
      ]
    },
    "scenario_result": {
      "is_simulation": true,
      "reconstruction_run_id": "sim-c9f12345-6789-4abc-def0-123456789abc",
      ...
    }
  }
  ```
- **Error Responses:**
  - `400 Bad Request`: When alternative values contain `NaN`, infinite floats, or invalid parameter types.
  - `404 Not Found`: When the case has no evidence events.

---

### 2.4 Evidence-to-Claim Traceability (Finding Provenance)

- **Endpoints:**
  - `GET /api/v1/cases/:case_id/findings/:finding_id/provenance`
  - `GET /api/v1/cases/:case_id/reconstruction/findings/:finding_id/provenance` (alias)
- **Method:** `GET`
- **Description:** Exposes the end-to-end provenance chain for a specific finding, linking findings back through reconstruction runs, extraction runs, and raw evidence files with cryptographic verification.
- **Response (`200 OK`):**
  ```json
  {
    "finding_id": "lead-spatial-001",
    "reconstruction_run_id": "7b84db56-2a81-4235-9ff2-3bfa3f885e72",
    "finding_type": "CONTRADICTION",
    "severity": "CRITICAL",
    "description": "Required speed 142.5 m/s exceeds threshold.",
    "evidence_refs": [
      {
        "evidence_id": "ev-cctv-dock",
        "evidence_sha256": "e77a6607dd66c74a6f1bca53e372ce4ba9d88b77466d3ca479ca7fefee338890",
        "sha256_verified": true,
        "extraction_run_id": "run-60683a524b3c51fed628898a849ba64b",
        "source_record_id": "row-12",
        "original_source_timestamp": "2026-11-10 20:15:30",
        "normalized_timestamp_utc": "2026-11-10T14:45:30Z",
        "clock_correction_applied_seconds": 0.0,
        "uncertainty_half_width_seconds": 0.5,
        "spatial_interpretation": "Centroid of cell tower tower-01",
        "spatial_uncertainty_meters": 500.0,
        "epistemic_level": "OBSERVED",
        "provenance_chain_complete": true
      }
    ],
    "missing_provenance_fields": [],
    "provenance_intact": true
  }
  ```
- **Case-Wide Provenance:**
  - `GET /api/v1/cases/:case_id/reconstruction/provenance`: Returns a list of all finding traces across the case.

---

### 2.5 Evidence-Gap Intelligence

- **Endpoints:**
  - `GET /api/v1/cases/:case_id/gaps`
  - `GET /api/v1/cases/:case_id/reconstruction/gaps` (alias)
- **Method:** `GET`
- **Description:** Evaluates 11 explainable, deterministic evidence gap rules identifying uncorroborated leads, spatial continuity gaps, uncalibrated device clocks, and unextracted media.
- **Response (`200 OK`):**
  ```json
  {
    "case_id": "550e8400-e29b-41d4-a716-446655440000",
    "reconstruction_run_id": "7b84db56-2a81-4235-9ff2-3bfa3f885e72",
    "analyzed_at": "2026-11-21T03:00:00Z",
    "gaps": [
      {
        "gap_id": "GAP-NOCLOCK-08-ev-cctv-dock",
        "gap_type": "UNSUPPORTED_OR_UNCALIBRATED_DEVICE_CLOCK",
        "severity": "HIGH",
        "affected_finding_id": null,
        "affected_evidence_id": "ev-cctv-dock",
        "affected_event_id": null,
        "explanation": "Evidence item 'dock_cam_01.mp4' has no calibrated clock drift offset recorded.",
        "uncertainty_impact": "Timeline alignment relies on uncalibrated device internal clock.",
        "suggested_corroboration": "Device clock drift calibration affidavit or reference timestamp alignment."
      }
    ],
    "counts_by_severity": { "HIGH": 1, "MEDIUM": 0, "LOW": 0 },
    "total_gaps": 1
  }
  ```

---

### 2.6 Modular Breakdown Views & Report

- `GET /api/v1/cases/:case_id/timeline`: Returns `{ "entries": [...], "overlaps": [...] }`.
- `GET /api/v1/cases/:case_id/spatial`: Returns `{ "segments": [...], "spatial_overlaps": [...] }`.
- `GET /api/v1/cases/:case_id/contradictions`: Returns array of contradiction leads with robustness ratings.
- `GET /api/v1/cases/:case_id/report` (or `POST`): Returns structured Section 65B judicial report with executive summary, timeline entries, contradictions, limitations, and evidence gaps.

---

### 2.7 Evidence Ingestion & India-Focused Normalization

- **Triggering Extraction:**
  - `POST /api/v1/evidence/:evidence_id/extract`
  - Supports configurable Indian carrier profiles and timezones via request metadata:
    ```json
    {
      "evidence_id": "ev-airtel-cdr-01",
      "case_id": "550e8400-e29b-41d4-a716-446655440000",
      "source_type": "CALL_DETAIL_RECORD",
      "source_timezone": "Asia/Kolkata",
      "default_country_prefix": "91",
      "metadata": {
        "cdr_profile": "Airtel"
      }
    }
    ```
- **Phone Normalization Invariants:**
  - Mobile numbers starting with `6`, `7`, `8`, or `9` are normalized to E.164 (`+91XXXXXXXXXX`).
  - Numbers starting with `0`–`5` are rejected as mobile candidates (`INVALID_INDIAN_MOBILE_SERIES`) and preserved raw without merging.
  - Raw phone and chat bodies are **never mutated** in place.

---

## 3. Cross-Feature Verification Matrix

| Verification Item | Tested In | Result |
| :--- | :--- | :--- |
| **Finding Source Provenance** | `test_phase4_full_finding_to_source_trace_with_multiple_evidence_items` | **VERIFIED** |
| **Uncertainty Reaches Robustness Analysis** | `test_dec002_half_open_uncertainty_interval_precision`, `test_dec004_and_dec006_contradiction_detection` | **VERIFIED** |
| **What-If Baseline Protection** | `test_phase3_stateless_what_if_does_not_mutate_baseline_or_fs` | **VERIFIED** |
| **Evidence Gap References** | `test_phase5_gap_api_endpoint`, `test_phase5_gap_missing_corroboration` | **VERIFIED** |
| **Scenario vs Official Distinction** | `test_phase3_stateless_what_if_does_not_mutate_baseline_or_fs` | **VERIFIED** |
| **Reconstruction Determinism** | `test_contradiction_leads_byte_identical_across_repeated_runs` | **VERIFIED** |
| **Raw Evidence Immutability** | `test_24_raw_evidence_remains_unchanged`, `test_raw_evidence_integrity_verification` | **VERIFIED** |
| **Custody Ledger Immutability** | `test_custody_hash_chaining_deterministic` | **VERIFIED** |
| **IST Handling & Indian NNP** | `test_phase6_indian_phone_standard_formats`, `test_phase6_ist_timezone_handling_and_conversion` | **VERIFIED** |

---

## 4. Environment & Service Status

- **Rust Backend Tests:** **172 passed**, 0 failed, 0 warnings.
- **Web Frontend Tests:** **264 passed**, 0 failed.
- **PostgreSQL Database:** Offline / connection refused on port 5432 during test run. Tested via in-process SQLite/filesystem derived storage parity engine.
- **Neo4j Graph Database:** Offline / connection refused on ports 7474/7687 during test run. Graph outbox queue and Cypher generation tested via deterministic mock clients.
- **Git Policy:** Single branch `main`, no branch creation, no commits or pushes performed.
