# YUKTI — Comprehensive Test Results & Verification Log

**Project:** YUKTI (SUTRAM 2026)  
**Document ID:** TEST-P2C-010  
**Version:** 1.0.0  
**Date:** 2026-11-20  

---

## 1. Summary of Test Suites

| Test Suite / Tool | Command Line | Tests Executed | Passed | Failed | Skipped | Status |
|---|---|---|---|---|---|---|
| **Rust Code Formatting** | `cargo fmt --all -- --check` | Entire codebase | N/A | 0 diffs | 0 | **PASS** |
| **Rust Clippy Linter** | `cargo clippy --all-targets -- -D warnings` | All targets | N/A | 0 warnings | 0 | **PASS** |
| **Rust Unit & Integration Tests** | `cargo test --locked` | 117 tests | 117 | 0 | 0 | **PASS** |
| **Rust Release Compilation** | `cargo build --release --locked` | Optimized MSVC binary | N/A | 0 errors | 0 | **PASS** |
| **Frontend Unit & Integration** | `npm --prefix apps/web test` | 20 test files | 264 | 0 | 0 | **PASS** |
| **Frontend Production Build** | `npm --prefix apps/web run build` | `tsc && vite build` | N/A | 0 errors | 0 | **PASS** |
| **Python Reference Backend** | `python -m pytest backend/tests` | 194 test items | 144 | 0 | 50 | **PASS** |

---

## 2. Rust Backend Test Breakdown (`cargo test --locked`)

### 2.1 Unit Tests (Internal Modules)
- `tests::test_ingestion_sha256`: **ok**
- `tests::test_storage_manifest_creation`: **ok**
- `tests::test_cdr_parser_valid`: **ok**
- `tests::test_chat_parser_valid`: **ok**
- `tests::test_ner_e164_normalization`: **ok**
- `tests::test_resolution_deduplication`: **ok**
- `tests::test_temporal_clock_drift`: **ok**
- `tests::test_spatial_haversine_accuracy`: **ok**
- `tests::test_contradiction_speed_anomaly`: **ok**

### 2.2 Integration Runs (`tests/integration_runs.rs` - 31 Scenarios)
- `test_01_upload_cdr_auto_extract`: **ok**
- `test_02_upload_chat_auto_extract`: **ok**
- `test_03_upload_no_extract`: **ok**
- `test_04_upload_receipt_structure`: **ok**
- `test_05_extraction_candidate_counts`: **ok**
- `test_06_manual_extract_trigger`: **ok**
- `test_07_get_active_extraction`: **ok**
- `test_08_get_extraction_runs_history`: **ok**
- `test_09_failed_extraction_preserves_raw_file`: **ok**
- `test_10_failed_run_preserves_active_run_pointer`: **ok**
- `test_11_reprocess_creates_new_run_id`: **ok**
- `test_12_reprocess_timezone_changes_config_hash`: **ok**
- `test_13_reprocess_clock_offset_changes_config_hash`: **ok**
- `test_14_reprocess_identical_config_preserves_runs`: **ok**
- `test_15_tampered_evidence_sha256_mismatch_fails_cleanly`: **ok**
- `test_16_unsupported_format_returns_structured_error`: **ok**
- `test_17_malformed_cdr_upload_handled_without_panic`: **ok**
- `test_18_malformed_chat_json_handled_without_panic`: **ok**
- `test_19_partial_extraction_retrievable`: **ok**
- `test_20_restart_preserves_historical_run_manifests`: **ok**
- `test_21_restart_preserves_active_run_pointer`: **ok**
- `test_22_concurrent_extraction_single_evidence_serialized`: **ok**
- `test_23_concurrent_extraction_multi_evidence_parallel`: **ok**
- `test_24_interrupted_run_recovery`: **ok**
- `test_25_invalid_evidence_id_returns_400`: **ok**
- `test_26_nonexistent_evidence_id_returns_404`: **ok**
- `test_27_extraction_api_never_discloses_server_paths`: **ok**
- `test_28_health_endpoint_returns_200`: **ok**
- `test_29_manifest_listing_and_filtering`: **ok**
- `test_30_case_creation_and_retrieval`: **ok**
- `test_31_case_scoped_evidence_listing`: **ok**

### 2.3 Forensic Reconstruction Engine (`tests/reconstruction_tests.rs`)
- `test_dec001_clock_offset_additive_convention`: **ok**
- `test_dec002_half_open_interval_semantics`: **ok**
- `test_dec002_interval_overlap_logic`: **ok**
- `test_dec003_haversine_distance_calculation`: **ok**
- `test_dec003_conservative_minimum_speed`: **ok**
- `test_dec004_speed_threshold_contradiction_detection`: **ok**
- `test_dec004_simultaneous_copresence_contradiction`: **ok**
- `test_dec005_video_sha256_provenance`: **ok**
- `test_dec006_neutral_explanation_phrasing`: **ok**

### 2.4 Knowledge Graph Projection Engine (`tests/graph_tests.rs`)
- `test_graph_projection_from_extraction_run`: **ok**
- `test_graph_neighborhood_query`: **ok**
- `test_graph_shortest_path_traversal`: **ok**
- `test_graph_community_clustering`: **ok**

**Rust Test Suite Summary:** `test result: ok. 117 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`.

---

## 3. Web Frontend Test Breakdown (`apps/web`)

Executed via Vitest:
```
Test Files  20 passed (20)
     Tests  264 passed (264)
  Start at  02:35:10
  Duration  11.45s
```

### Key Test Suites Validated:
- `EvidenceUploadPanel.test.tsx`: 4 passed (staged file list, real upload streaming dispatch, success receipt rendering, error banner and retry action).
- `workspace.integration.test.tsx`: 48 passed (end-to-end workspace tabs, case switching, graph view, timeline view, spatial view, reports view).
- `KnowledgeGraphView.test.tsx`: 16 passed (Cytoscape rendering, node selection, neighborhood filters).
- `TimelineView.test.tsx`: 14 passed (chronological event sorting, uncertainty window visual cues).
- `ContradictionsPanel.test.tsx`: 12 passed (contradiction cards, severity badges, neutral explanations).
- `ReportsView.test.tsx`: 10 passed (`SHIP_EXPORT_BLOCKED` badge display, judicial export lock).

---

## 4. Python Reference Backend Test Breakdown (`pytest backend/tests`)

```
================= 144 passed, 50 skipped, 3 warnings in 6.66s =================
```
- Passed: 144 unit tests validating legacy CDR extraction, chat parsing, regex NER, and resolution algorithms.
- Skipped: 50 tests dependent on optional GPU acceleration or legacy OpenCV video bindings.
