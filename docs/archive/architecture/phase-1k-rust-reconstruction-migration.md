# YUKTI — Phase 1K: Migration and Integration of Forensic Reconstruction Engines into Rust

## Architectural Record & Engineering Report

- **System:** YUKTI Digital Forensics Platform
- **Phase:** Phase 1K (Forensic Reconstruction Engine Migration into Rust)
- **Author:** Lead Rust Backend Architect & Digital Forensics Engineer
- **Status:** APPROVED & IMPLEMENTED
- **Date:** 2026-11-19

---

## 1. Executive Summary & Objective

Phase 1K completes the migration of Member 4's forensic reconstruction engines from the Python reference implementation (`backend/app/modules/reconstruction/`) into high-performance, memory-safe Rust (`backend-rust/src/reconstruction/`).

All core forensic contracts and invariants (DEC-001 through DEC-007) have been preserved with 100% mathematical and behavioral fidelity:
1. **DEC-001 (Clock Offset Convention)**: Additive clock drift correction ($\text{True UTC} = \text{Device Time} + \text{Clock Offset}$).
2. **DEC-002 (Time Precision Enums & Half-Open Intervals)**: Five canonical precisions (`EXACT_SECOND`, `MINUTE`, `HOUR`, `DAY`, `ESTIMATED`), half-open $[start, end)$ where upper boundary is strictly excluded, ensuring adjacent canonical intervals do not falsely overlap.
3. **DEC-003 (Relationship Taxonomy & Spatial Bounds)**: Canonical edge types (`COMMUNICATED_WITH`, `PRESENT_AT`, `CO_OCCURRED_WITH`, `OWNS`, `ASSOCIATE_OF`, `CONTRADICTS`), WGS84 coordinates, and Haversine great-circle distance ($R = 6371.0088\text{ km}$) with conservative minimum transit speed calculation using $T_{\max}$.
4. **DEC-004 (Relationship Weight Nullability)**: Nullable relationship weights (`Option<f64>`); observed facts and contradictions never invent arbitrary probabilities (`weight = None` / rendered as "Not provided").
5. **DEC-005 (Event Duration & Uncertainty Window Schema)**: Explicit separation of event `duration_seconds` from uncertainty bounds (`uncertainty_window_start`, `uncertainty_window_end`).
6. **DEC-006 (Evidence Metadata Schema)**: Standardized evidence metadata schema (`source_timezone` as IANA string, `clock_offset_seconds` as drift offset, `recording_start_timestamp`), accompanied by neutral, objective reporting without guilt inference.
7. **DEC-007 (Timestamp Ownership & Exactly-Once Normalization)**: Extraction engines preserve raw timestamp strings verbatim (`raw_timestamp`); the temporal engine alone normalizes to nominal UTC.
8. **Video Processing Forensics**: Implements safe FFprobe metadata extraction and FFmpeg keyframe extraction using explicit argument vectors (zero shell interpolation), resource ceilings, strict timeout enforcement, and pre- and post-operation SHA-256 integrity verification guaranteeing source file immutability.

---

## 2. Invariants & Forensic Guarantees

### 2.1 DEC-001: Additive Clock Drift Correction
$$\text{True UTC} = \text{Local Timezone Localization}(\text{Raw Device Timestamp}) + \text{Clock Offset}$$
- Fast clock: Device recorded 30s ahead of true time $\implies$ offset is negative ($-30.0\text{s}$). True UTC adjusts 30s backward.
- Slow clock: Device recorded 45s behind true time $\implies$ offset is positive ($+45.0\text{s}$). True UTC adjusts 45s forward.

### 2.2 DEC-002: Half-Open Uncertainty Intervals $[start, end)$
- `EXACT_SECOND`: $[t, t + 1\text{s})$
- `MINUTE`: Floored to minute 00 in source timezone, $[t_0, t_0 + 60\text{s})$
- `HOUR`: Floored to minute 00 second 00 in source timezone, $[t_0, t_0 + 3600\text{s})$
- `DAY`: Floored to midnight 00:00:00 in source timezone, advances 1 local calendar day (accounting for 23h and 25h DST days).
- `ESTIMATED`: $[nominal - half\_width, nominal + half\_width)$
- **Adjacent Interval Semantics**: Adjacent intervals $[t_0, t_1)$ and $[t_1, t_2)$ do not overlap.

### 2.3 DEC-003: Spatial Distance & Speed Bounds
- Haversine distance:
  $$a = \sin^2\left(\frac{\Delta \phi}{2}\right) + \cos(\phi_1)\cos(\phi_2)\sin^2\left(\frac{\Delta \lambda}{2}\right)$$
  $$d = 2 R \arcsin(\sqrt{\text{clamp}(a, 0, 1)})$$
- Conservative speed:
  $$v_{\min} = \frac{d \times 3600.0}{\max(0, B_{end} - A_{start})}$$
  Evaluated against reference speed threshold ($180.0\text{ km/h}$).

### 2.4 DEC-005: Video Subprocess Security & Immutability
- CLI arguments are passed as discrete vectors `&["-v", "quiet", "-i", ...]` without invoking an OS shell.
- Source video SHA-256 hash is computed before and verified after extraction to guarantee original evidence immutability.
- Extracted frames are hashed and stored in derived directories under `{derived_root}/{case_id}/{evidence_id}/frames/`.

### 2.5 DEC-006: Neutral Explanations & No Guilt Fabrication
- Contradiction leads strictly state observed facts and mathematical bounds without speculating on actor intent, falsity, or guilt.
- Includes mandatory standardized limitations noting clock offset drift, cell tower centroids, and alternative transport modes.

---

## 3. Decision Service Status

The Node.js 22 LTS microservice under `decision-service/` was audited:
- Reserved for future Phase 5 integration with the Laya (Receptron) ONNX inference runtime.
- Currently provides lightweight health telemetry without active model weights.

---

## 4. Test Verification Summary

| Test Area | Target File | Tests Run | Result |
| :--- | :--- | :--- | :--- |
| Clock Offset (DEC-001) | `tests/reconstruction_tests.rs` | Fast/slow/timezone offsets | PASSED |
| Interval Bounds (DEC-002) | `tests/reconstruction_tests.rs` | 5 precision levels, adjacency | PASSED |
| Spatial Transit (DEC-003) | `tests/reconstruction_tests.rs` | Haversine, stationary, speed | PASSED |
| Contradictions (DEC-004/006) | `tests/reconstruction_tests.rs` | Existential feasibility, neutral leads | PASSED |
| Video Integrity (DEC-005) | `tests/reconstruction_tests.rs` | Streaming SHA-256, immutability | PASSED |
| Timeline Concurrency | `tests/reconstruction_tests.rs` | Overlap detection & marking | PASSED |
