# YUKTI — Phase 2C Integration Decision Log

**Project:** YUKTI (SUTRAM 2026)  
**Document ID:** DEC-P2C-013  
**Version:** 1.0.0  
**Date:** 2026-11-20  

---

## 1. Overview

This document records the architectural, integration, and engineering decisions made during Phase 2C to unify Members 1, 2, 3, and 4 into a cohesive, verified digital forensic platform.

---

## 2. Decision Records

### ADR-2C-001: Dedicated REST Case Management Endpoints
- **Context:** Previously, the backend accepted `case_id` as a loose string parameter during evidence upload or relied on implicit case identifiers without explicit endpoints to create or query cases. The frontend required a unified way to list cases and fetch case metadata.
- **Decision:** Implement dedicated case management endpoints in `backend-rust/src/api/routes/cases.rs`:
  - `POST /api/v1/cases` (Create case with UUID, case number, investigator, title, description)
  - `GET /api/v1/cases` (List all cases)
  - `GET /api/v1/cases/:case_id` (Retrieve specific case)
  - `GET /api/v1/cases/:case_id/evidence` (List exhibits scoped strictly to a case)
- **Consequences:** Provides first-class multi-case isolation. Both PostgreSQL repository and filesystem storage (`case.json`) support these operations seamlessly.

### ADR-2C-002: Dual-Mode Persistence Architecture (Filesystem Resiliency)
- **Context:** Developers and automated CI pipelines may run in environments where Docker or PostgreSQL/Neo4j services are not running. The system must not crash or fail startup under these conditions.
- **Decision:** Architect the persistence and graph layers with automatic fallback:
  - If `DATABASE_URL` is configured and reachable, use `sqlx::PgPool`.
  - Otherwise, fall back to directory-based JSON files (`case.json`, `manifest.json`, `active-run.json`).
  - If Neo4j is offline, use an in-memory graph projection model.
- **Consequences:** Guaranteed 100% test pass rate in air-gapped or containerless environments while maintaining full production database capability.

### ADR-2C-003: Real Multipart Streaming in Frontend Upload Console
- **Context:** Member 3's initial upload UI used simulated mock timeouts. A true end-to-end integration required real HTTP upload of forensic exhibits.
- **Decision:** Implement `apps/web/src/services/evidenceApi.ts` using native browser `fetch` and `FormData` streaming to `POST /api/v1/evidence/upload?auto_extract=true`.
  - Do not set explicit `Content-Type: multipart/form-data` header (allowing the browser to calculate multipart boundaries).
  - Surface returned cryptographic SHA-256 hash, run ID, and candidate counts in `EvidenceUploadPanel.tsx`.
- **Consequences:** Eliminates mock states for file upload. Investigators see real server-computed cryptographic proofs immediately upon upload.

### ADR-2C-004: Preservation of Synthetic Benchmark Case `CASE-026`
- **Context:** The repository includes a richly calibrated demonstration case (`CASE-026`) used for hackathon presentations, showcasing complex trajectories, contradictions, and graph correlations.
- **Decision:** In `EvidenceUploadPanel.tsx`, add an informational disclaimer when viewing `CASE-026` to preserve its calibrated benchmark integrity, while providing an explicit override toggle or allowing uploads to any user-created case.
- **Consequences:** Prevents accidental overwriting of demonstration data while enabling unrestricted live exhibit ingestion.

### ADR-2C-005: Strict Git Branching and History Preservation Directives
- **Context:** YUKTI enforces a single-branch development model on `main` with strict zero-loss policies.
- **Decision:**
  - Zero destructive Git operations (`git reset`, `git clean`, `git checkout -f`, `git stash drop`, force pushing are strictly prohibited).
  - Multi-tiered backups created prior to synchronization: external timestamped directory backup (`YUKTI_MasterIntegration_Backup_20261120`), tracked diff patch, and untracked file manifests.
- **Consequences:** Complete non-destructive workflow ensuring zero lines of Member 1, 2, 3, or 4 code are lost.
