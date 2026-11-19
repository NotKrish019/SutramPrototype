# YUKTI — Phase 2A Findings & Resolutions Register

**Project:** YUKTI (formerly SUTRAM 2026)  
**Maintained by:** Lead Rust Knowledge Graph Architect & Forensics Engineer  
**Status:** COMPLETED — ALL FINDINGS RESOLVED  

This register tracks all architectural, graph, relational, security, and concurrency findings identified during Phase 2A. No critical or high-priority finding remains unresolved.

---

## Findings

| Finding ID | Discovery Subphase | Severity | Component | Description | Required Correction | Target Subphase | Status |
|---|---|---|---|---|---|---|---|
| **FIND-2A-01** | Phase 2A.0 | HIGH | Relational Schema & Temporal | Database check constraint `chk_event_uncertainty_window` allowed `nominal_timestamp <= uncertainty_window_end`, violating DEC-002 half-open $[start, end)$ semantics where $end$ is exclusive. | Create additive migration `20261119000001_fix_temporal_half_open_constraint.sql` enforcing `< uncertainty_window_end`. Update `temporal.rs` explicit bound check to reject `nominal_utc >= end_utc`. Add regression tests. | Phase 2A.0 | **RESOLVED** |
| **FIND-2A-02** | Phase 2A.0 | MEDIUM | Persistence Repositories | Unchecked `as i32` and `as i64` narrowing casts on `file_size_bytes`, `records_processed`, and counts could silently overflow on large values. | Replace with `i32::try_from` and `i64::try_from` with structured error handling; add boundary/overflow tests. | Phase 2A.0 | **RESOLVED** |
| **FIND-2A-03** | Phase 2A.0 | MEDIUM | API Readiness | `/health/ready` returned `status: "ready"` even when database was unconfigured, masking non-database filesystem degradation. | Update `/health/ready` to return `status: "degraded"` when running in filesystem fallback or without graph connectivity, and `"not_ready"` (503) when configured services fail. | Phase 2A.0 | **RESOLVED** |
| **FIND-2A-04** | Phase 2A.0 | INFORMATIONAL | CI & Video | Linux CI workflow lacked `ffmpeg` installation, causing video tests to skip in CI. | Add `ffmpeg` package installation to Linux CI job in `.github/workflows/backend-rust.yml`. | Phase 2A.0 | **RESOLVED** |
| **FIND-2A-05** | Phase 2A.3 | LOW | Build Portability & SQLx | `sqlx::query!` macros required live DB or query cache at compile time; caused build failure in offline environment. | Use runtime `sqlx::query` with `Row::try_get` consistent with FIND-006. | Phase 2A.3 | **RESOLVED** |
