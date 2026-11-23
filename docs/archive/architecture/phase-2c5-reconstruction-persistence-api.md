# Phase 2C.5: Reconstruction Persistence and Axum API Architecture

**Project:** YUKTI (formerly SUTRAM 2026)  
**Author:** Lead Rust Backend Architect  
**Date:** 2026-11-19T22:54:20+05:30  
**Phase:** Phase 2C — End-to-End Forensic Reconstruction Integration  

---

## 1. Relational Persistence Architecture (`ReconstructionRunRepository`)

### 1.1 Atomic Relational Transaction
When a reconstruction is triggered for a case, the entire execution is saved within an ACID transaction across PostgreSQL tables:
1. **`reconstruction_runs`:**
   - Records `reconstruction_run_id`, `case_id`, `extraction_run_id` (optional/nullable), `reconstruction_type`, `status` (`SUCCESS`), `parameters` (serialized `ReconstructionOptions`), and `result_snapshot` (full JSON snapshot of `FullCaseReconstructionResult`).
2. **`contradictions`:**
   - Evaluates each detected `ContradictionLead`.
   - Derives deterministic event UUIDs:
     $$\text{UUID} = \text{Uuid::parse\_str}(\text{id}).\text{unwrap\_or\_else}(\dots)$$
   - Verifies referenced events exist in the relational `events` table before inserting to uphold foreign key constraints.
   - Stores `contradiction_type`, `severity` (`HIGH`), `explanation` (neutral description), and `metrics` (observed basis, derived inference, limitations).
3. **`spatial_trajectories`:**
   - Stores entity-grouped waypoints, `total_distance_km`, and conservative `max_speed_kmh`.
4. **`graph_projection_outbox` (Transactional Outbox Pattern):**
   - Enqueues event type `RECONSTRUCTION_COMPLETED` with aggregate type `RECONSTRUCTION_RUN`.
   - Emits outbox notification within the *same* database transaction, ensuring no partial or lost projections to Neo4j.

---

## 2. Axum REST API Route Contracts

All reconstruction endpoints are versioned under `/api/v1/cases/:case_id/`:

### 2.1 Trigger Reconstruction: `POST /api/v1/cases/:case_id/reconstruction`
- **Request Body (Optional):**
  ```json
  {
    "options": {
      "default_timezone": "Asia/Kolkata",
      "default_clock_offset_seconds": 0.0,
      "speed_threshold_kmh": 180.0,
      "estimated_half_width_seconds": 900.0
    },
    "event_candidates": [ ... ],
    "raw_events": [ ... ]
  }
  ```
- **Behavior:**
  - If `event_candidates` or `raw_events` are provided, they are processed directly.
  - If omitted, events are loaded automatically from the case's database exhibits or filesystem storage.
  - Returns `200 OK` with `FullCaseReconstructionResult`.

### 2.2 Get Reconstruction: `GET /api/v1/cases/:case_id/reconstruction`
- Returns latest stored reconstruction snapshot or dynamically computes one on the fly.

### 2.3 Subsystem Query Endpoints
- **`GET /api/v1/cases/:case_id/timeline`:** Returns normalized `TimelineReconstructionResult` (`entries`, `overlaps`, concurrency counts).
- **`GET /api/v1/cases/:case_id/spatial`:** Returns `SpatialReconstructionResult` (`trajectories`, `overlaps`).
- **`GET /api/v1/cases/:case_id/contradictions`:** Returns `ContradictionAnalysisResult` (`leads`, `flagged_segments_count`, `disclaimer`).
