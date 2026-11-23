# YUKTI — Phase 1 Master Findings & Corrections Register

**Project:** YUKTI (formerly SUTRAM 2026)  
**Maintained by:** Lead Rust Backend Architect & Integration Engineer  
**Status:** ACTIVE  

This register tracks all architectural, relational, security, forensic, and behavioral findings discovered during independent quality reviews between and within phases. No critical or high-priority finding may remain unresolved before advancing to a subsequent phase.

---

## Severity Classification
- **CRITICAL:** Corrupts forensic evidence, violates chain of custody, introduces data loss, leaks credentials, or crashes application.
- **HIGH:** Relational constraint defect, concurrency race condition, inaccurate status calculation, or API contract violation.
- **MEDIUM:** Non-fatal parsing inconsistency, missing schema index, performance bottleneck, or unnormalized artifact reference.
- **LOW:** Code cleanliness, redundant import, diagnostic wording improvement, or documentation drift.
- **INFORMATIONAL:** Architectural observation, environment constraint (e.g. absent local binary), or upstream proposal notation.

---

## Register

| Finding ID | Discovery Phase | Severity | Component | Description | Required Correction | Target Phase | Status |
|---|---|---|---|---|---|---|---|
| **FIND-001** | Preflight | INFORMATIONAL | Host Environment | PostgreSQL 16 binary (`psql`/`pg_isready`) and Docker daemon are not active in the local Windows environment. | Structure SQLx migrations and repositories portably; configure `.github/workflows/backend-rust.yml` with PostgreSQL 16 service container; implement resilient pool initialization and `/health/ready` probe. | Phase 1J.2 / 1J.3 | OPEN |
| **FIND-002** | Preflight | INFORMATIONAL | Host Environment | `ffmpeg` and `ffprobe` binaries are absent from Windows PATH, skipping video integration tests in local environment. | Implement safe process abstractions with timeout/resource bounds; distinguish unit tests from live invocation; configure Linux CI with FFmpeg package. | Phase 1K | OPEN |
| **FIND-003** | Phase 1J.1 Review | MEDIUM | Relational Schema | Event uncertainty window constraint must enforce that nominal timestamp falls within the bounds when uncertainty window is specified. | Add CHECK constraint: `CHECK (uncertainty_window_start IS NULL OR uncertainty_window_end IS NULL OR (uncertainty_window_start <= nominal_timestamp AND nominal_timestamp <= uncertainty_window_end))`. | Phase 1J.2 Migrations | RESOLVED |
| **FIND-004** | Phase 1J.1 Review | LOW | Relational Schema | Uniqueness on `(case_id, sha256_hash)` would violate multi-exhibit acquisition rule. | Explicitly verified omission of unique constraint on `(case_id, sha256_hash)` to allow distinct exhibits with identical content. | Phase 1J.1 / 1J.2 | RESOLVED |
| **FIND-005** | Phase 1J.2 Review | LOW | Persistence Module | Unused import `warn` in `src/persistence/database.rs` produced compilation warning. | Removed unused import. | Phase 1J.2 | RESOLVED |
| **FIND-006** | Phase 1J.2 Review | INFORMATIONAL | Persistence Module | Compilation without local active database requires runtime queries (`query_as` / `query`) rather than compile-time database checking (`query!`). | Implemented all repositories using type-safe runtime queries with `FromRow` entity mapping. | Phase 1J.2 | RESOLVED |
| **FIND-007** | Phase 1J.3 Review | LOW | Relational Schema & API | Extraction run manifest metrics (usize) and PostgreSQL integer columns required explicit type width mapping (i32). | Standardized explicit `as i32` casting across route and reconciliation mappers. | Phase 1J.3 | RESOLVED |
