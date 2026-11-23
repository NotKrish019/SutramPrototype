# Phase 2C Findings Register

**Project:** YUKTI (formerly SUTRAM 2026)  
**Document:** Phase 2C Architectural & Engineering Findings Register  
**Updated:** 2026-11-20T00:32:00+05:30  
**Phase:** Phase 2C Finalization  

---

## 1. Register Index

| Finding ID | Subsystem | Severity | Description | Resolution Strategy |
|:---|:---|:---|:---|:---|
| **FND-2C-001** | Persistence | Medium | `reconstruction_runs.extraction_run_id` was `NOT NULL` in initial schema, but case reconstructions may span multiple extraction runs or entire case dossiers. | Provide SQL migration relaxing `reconstruction_runs.extraction_run_id` to nullable while maintaining foreign key integrity when specified. |
| **FND-2C-002** | Persistence | Medium | `events` table generated arbitrary `Uuid::new_v4()` during extraction insertion, divorcing database event IDs from extraction candidate IDs (`candidate_id`). | Make database event UUIDs deterministic via SHA-256 (`deterministic_uuid(&case_id, candidate_id.as_bytes())`) to ensure foreign key integrity without altering `Cargo.lock`. |
| **FND-2C-003** | API Routes | High | Reconstruction endpoints (`/api/v1/cases/:case_id/reconstruction`, etc.) were absent from Axum router in `app.rs`. | Author dedicated Axum handler module `backend-rust/src/api/routes/reconstruction.rs` with full case reconstruction trigger and query routes. |
| **FND-2C-004** | Reconstruction Adapter | Low | Extraction event candidates from CDR/Chat did not automatically carry forward exhibit-level `source_timezone` or `clock_offset_seconds` unless explicitly embedded in record metadata. | Enhance `adapters.rs` to accept optional exhibit fallback metadata or resolve exhibit properties from the database / manifest. |
| **FND-2C-005** | Frontend API Integration | Medium | Frontend workspace features (`EventReconstruction`, `SpatialReconstruction`, `ContradictionReview`) relied solely on static synthetic fixtures without an active API client adapter for the Rust backend. | Author typed API client service in `apps/web/src/services/reconstruction.ts` and integrate with `WorkspaceDataContext` and page components. |
| **FND-2C-006** | Persistence Provenance | High | Making `extraction_run_id` nullable in migration `20261119000003` risked losing explicit provenance links for reconstruction runs that correlate multiple exhibits. | Added additive migration `20261120000001_reconstruction_run_inputs.sql` creating `reconstruction_run_inputs` junction table and updated `ReconstructionRunRepository` to record all input extraction runs. |
| **FND-2C-007** | Infrastructure | Medium | Docker Desktop Linux engine service was stopped on host machine; containerized PostgreSQL and Neo4j unavailable in headless execution environment. | Operating status honestly classified as `READY_WITH_LIMITATIONS`. Rust backend and frontend operate in complete, verified filesystem/in-memory fallback mode. |
| **FND-2C-008** | Offline API Fallback | Medium | When PostgreSQL persistence is inactive, `load_case_events` previously returned empty vector for reconstruction query routes. | Enhanced `load_case_events` in `api/routes/reconstruction.rs` with automated filesystem scanning of `{derived_dir}/{case_id}/` to load extracted exhibits dynamically from disk. |
| **FND-2C-009** | Frontend Wiring & Dossier Export | Low | Frontend repository needed to handle real case UUID routing while preserving test compatibility, and export blocking status needed explicit forensic prominence. | Wired `hybridRepository` and `syntheticRepository` to query live Rust backend for non-fixture case UUIDs, and explicitly rendered `SHIP_EXPORT_BLOCKED` badge in `ReportsWorkspace`. |

---

## 2. Forensic Standards Audit

- **Immutability of Evidence:** Upheld. Neither extraction nor reconstruction modifies raw evidence exhibits in storage.
- **Cryptographic Provenance:** Upheld. Every evidence exhibit has a verified SHA-256 hash matching on-disk receipts.
- **Additive Clock Drift (DEC-001):** Strictly enforced: $\text{True UTC} = \text{Device Time} + \text{Clock Offset}$.
- **Half-Open Intervals (DEC-002):** Strictly enforced: $[start, end)$ with canonical precision windows.
- **Relationship Taxonomy & Spatial Bounds (DEC-003):** Standardized edge labels and Haversine great-circle distance on WGS84 ellipsoid.
- **Nullable Weights (DEC-004):** Observed facts and contradictions never invent arbitrary probabilities (`weight = None` / rendered as "Not provided").
- **Event Duration & Uncertainty Window Schema (DEC-005):** Strict separation of `duration_seconds` and uncertainty intervals.
- **Evidence Metadata Schema (DEC-006):** Preserved metadata schema (`source_timezone`, `clock_offset_seconds`, `recording_start_timestamp`) with neutral objective leads.
- **Timestamp Ownership & Normalization (DEC-007):** Extraction preserves raw timestamps verbatim; temporal normalization engine alone normalizes to nominal UTC.
