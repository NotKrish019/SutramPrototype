# Architecture Document: Knowledge Graph Reconciliation & Failure Recovery

**Document ID:** ARCH-2A6  
**Status:** Approved & Implemented  
**Author:** Lead Rust Knowledge Graph Architect, YUKTI Forensics Team  
**Date:** 2026-11-19  

---

## 1. Overview

In distributed and multi-model systems, projection failures can occur due to network partitions, Neo4j restarts, schema migrations, or unexpected malformed payloads.

`GraphReconciliationService` and the Transactional Outbox guarantee that the knowledge graph can always be reconciled to match PostgreSQL ground truth.

---

## 2. Recovery & Reconciliation Protocols

### 2.1 Outbox Backlog Recovery
When Neo4j becomes temporarily unavailable:
1. All domain operations (ingestion, extraction, reconstruction) continue to commit successfully to PostgreSQL.
2. Outbox events accumulate in `graph_projection_outbox` with status `'PENDING'` or `'RETRY_WAIT'`.
3. When Neo4j recovers, `graph-sync` drains the outbox backlog in strictly ascending chronological order (`ORDER BY created_at ASC`), restoring topological consistency.

### 2.2 Rebuilding Graph Projections from Ground Truth (`GraphReconciliationService`)
If a catastrophic graph loss occurs or a new Neo4j cluster is provisioned:
1. `GraphReconciliationService::generate_missing_outbox_events`:
   - Scans PostgreSQL `cases` and generates missing `CASE_CREATED` outbox events.
   - Scans PostgreSQL `evidence_files` and generates missing `EVIDENCE_INGESTED` outbox events.
   - Scans completed `extraction_runs` and generates missing `EXTRACTION_COMPLETED` outbox events.
   - Scans `reconstruction_runs` and generates missing `RECONSTRUCTION_COMPLETED` outbox events.
2. All generated outbox entries use `ON CONFLICT (case_id, aggregate_type, aggregate_id, aggregate_version, event_type) DO NOTHING`, ensuring idempotent reconciliation without duplicates.
3. The outbox processor replays the events to completely reconstruct the graph.

---

## 3. Audit Verification Summary

| Test Category | Suite / File | Status | Test Cases |
|:---|:---|:---|:---|
| **Graph Schema & DDL** | `tests/graph_tests.rs` | PASSED | 1 |
| **Privacy & HMAC-SHA-256** | `tests/graph_tests.rs` | PASSED | 1 |
| **Outbox Model & Status** | `tests/graph_tests.rs` | PASSED | 1 |
| **Case Projection Cypher** | `tests/graph_tests.rs` | PASSED | 1 |
| **Evidence Projection Cypher** | `tests/graph_tests.rs` | PASSED | 1 |
| **Extraction Projection Cypher** | `tests/graph_tests.rs` | PASSED | 1 |
| **Reconstruction Projection Cypher** | `tests/graph_tests.rs` | PASSED | 1 |
| **Mock Client Batch Execution** | `tests/graph_tests.rs` | PASSED | 1 |
| **Query Depth Clamping** | `tests/graph_tests.rs` | PASSED | 1 |
| **Path Finding Query** | `tests/graph_tests.rs` | PASSED | 1 |
| **Temporal Half-Open Schema Fix** | `tests/reconstruction_tests.rs` | PASSED | 1 |
| **Checked Integer Conversions** | `tests/persistence_tests.rs` | PASSED | 1 |
| **Health Probes Ready/Degraded** | `tests/health.rs`, `integration_runs.rs`| PASSED | 2 |
