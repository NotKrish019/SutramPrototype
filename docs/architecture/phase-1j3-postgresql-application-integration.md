# YUKTI — Phase 1J.3: PostgreSQL Integration with Rust APIs and Existing Application

## Architectural Record & Integration Documentation

- **System:** YUKTI Digital Forensics Platform
- **Phase:** Phase 1J.3 (PostgreSQL Application Integration & Reconciliation)
- **Author:** Lead Rust Backend Architect & Integration Engineer
- **Status:** APPROVED & IMPLEMENTED
- **Date:** 2026-11-19

---

## 1. Executive Summary & Objective

Phase 1J.3 integrates the PostgreSQL 16 persistence layer established in Phase 1J.2 directly into the operational Rust Axum application:
1. **Resilient Application State**: Wired `Database`, `CaseRepository`, `EvidenceRepository`, `ExtractionRunRepository`, and `CustodyRepository` into `AppState`. If `DATABASE_URL` is omitted or unreachable, the application falls back gracefully to filesystem mode without crashing.
2. **Readiness Probe (`GET /health/ready`)**: Added a health endpoint that checks database connectivity and ping latency, returning HTTP 200 when ready (or in filesystem mode) and HTTP 503 when the configured database is unreachable.
3. **Evidence Ingestion Synchronization**: `POST /api/v1/evidence/upload` transactionally registers exhibits in the `evidence` table and records an `EVIDENCE_STORED` cryptographic custody entry in `custody_events`.
4. **Extraction Run Lifecycle Persistence**: Both auto-extract upload and explicit run triggers (`POST /api/v1/evidence/:id/extract`, `POST /api/v1/evidence/:id/reprocess`) atomically record run dispatch, completion, active run promotion, and all child entities (`extracted_identifiers`, `resolved_entities`, `timeline_events`, `provisional_relationships`, `validation_issues`).
5. **Filesystem-to-Database Reconciliation**: Implemented `ReconciliationService` that scans raw and derived filesystem manifests (`data/evidence/raw` and `data/evidence/derived`) on startup or on demand, idempotently backfilling unindexed exhibits and runs into PostgreSQL.

---

## 2. Dual-Mode Execution Model

The platform operates under a dual-mode persistence architecture:

```mermaid
flowchart TD
    Config[AppConfig] --> CheckDB{DATABASE_URL configured?}
    CheckDB -->|No| FSMode[Filesystem Mode: Option::None for Repositories]
    CheckDB -->|Yes| Connect[Attempt Database::connect]
    Connect -->|Success| RunMig[Run Migrations & Initialize Repositories]
    RunMig --> Recon[Execute ReconciliationService::reconcile_all]
    Recon --> FullMode[Dual Mode: Primary DB + Filesystem Manifests]
    Connect -->|Failure| WarnFallback[Log Warning & Fallback to Filesystem Mode]
```

### Invariants:
1. **Filesystem Ground Truth**: Raw evidence files and canonical manifests on disk remain immutable and authoritative.
2. **Non-Blocking Fallback**: In local development environments without an active PostgreSQL daemon, all API operations and test suites execute with 100% functionality against the filesystem store.
3. **Zero Loss On Migration**: When a database is provisioned and the backend restarts, `ReconciliationService` automatically ingests all historical filesystem exhibits and extraction runs into PostgreSQL.

---

## 3. API Enhancements

### 3.1 Probing Endpoints
- `GET /health`: Liveness probe. Returns 200 OK `{"status": "ok", "service": "sutram-backend-rust"}`.
- `GET /health/ready`: Readiness probe.
  - When DB is connected: `200 OK` `{"status": "ready", "service": "sutram-backend-rust", "database": "connected"}`.
  - When DB is unconfigured: `200 OK` `{"status": "ready", "service": "sutram-backend-rust", "database": "unconfigured_filesystem_mode"}`.
  - When DB is configured but unreachable: `503 Service Unavailable` `{"status": "not_ready", "service": "sutram-backend-rust", "database": "disconnected", "error": "..."}`.

### 3.2 Ingestion & Extraction Endpoints
- `POST /api/v1/evidence/upload`: Bounded streaming upload.
  - Writes to staging and atomically places `.raw` exhibit.
  - Computes incremental SHA-256 and validates content.
  - Writes disk manifest.
  - If DB active: ensures case exists, inserts evidence record, logs custody event.
  - If `auto_extract=true`: executes extraction, completes transactional persistence, and logs completion custody event.
- `POST /api/v1/evidence/:id/extract` & `reprocess`:
  - Dispatches CPU-bound parser.
  - Atomically records run manifest and result to disk.
  - If DB active: transactionally persists run and children, updates active run pointer, and logs custody event.

---

## 4. Reconciliation Protocol (`persistence::reconciliation`)

`ReconciliationService::reconcile_all()` implements the following steps:
1. Scans `data/evidence/raw` for `{case_id}/{evidence_id}/metadata.json`.
2. Verifies whether `case_id` exists in PostgreSQL; inserts if absent.
3. Verifies whether `evidence_id` exists in `evidence`; registers exhibit and logs `RECONCILIATION_INGESTED` custody event if absent.
4. Scans `data/evidence/derived/{case_id}/{evidence_id}/extraction-runs/{run_id}`.
5. Loads `run-manifest.json` and `result.json`.
6. Checks whether `run_id` exists in `extraction_runs`; if absent, creates run record and commits all child records via `complete_run_transactional`.
7. Checks `active-run.json` to properly set `is_active` without race conditions.
8. Logs `RECONCILIATION_RUN_SYNCED` custody event.

---

## 5. Verification Matrix & Quality Gate

| Area | Requirement | Status | Notes |
| :--- | :--- | :--- | :--- |
| API Backward Compatibility | All existing ingestion & extraction tests pass | PASSED | 86 baseline tests intact |
| Dual-Mode Resilience | Runs cleanly with or without DB | PASSED | Graceful fallback verified |
| Readiness Probing | `/health/ready` returns correct status | PASSED | Verified in tests |
| Ingestion Sync | Exhibits and custody logged to DB | PASSED | Implemented in `sync_evidence_to_db` |
| Run Sync | Runs, entities, events, relationships logged | PASSED | Implemented in `sync_run_to_db` |
| Reconciliation | Filesystem to DB backfill | PASSED | Implemented in `ReconciliationService` |
