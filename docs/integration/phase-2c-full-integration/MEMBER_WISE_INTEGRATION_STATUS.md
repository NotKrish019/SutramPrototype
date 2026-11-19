# YUKTI — Member-Wise Integration Status Report

**Project:** YUKTI (SUTRAM 2026)  
**Document ID:** MEM-P2C-004  
**Version:** 1.0.0  
**Date:** 2026-11-20  

---

## 1. Overview of the Four-Member Engineering Model

YUKTI's engineering architecture delegates domain responsibilities across four specialized tracks, strictly governed by shared contracts in `shared/schemas/` and Rust type models in `backend-rust/src/models/`.

```
┌────────────────────────────────────────────────────────────────────────┐
│                        YUKTI 4-MEMBER TEAM MATRIX                      │
├────────────────────────────────┬───────────────────────────────────────┤
│ Track / Member                 │ Core Focus & Domain Ownership         │
├────────────────────────────────┼───────────────────────────────────────┤
│ Member 1                       │ AI/ML, Evidence Extraction & Entity   │
│ Evidence Intelligence          │ Resolution, Normalization, NER        │
├────────────────────────────────┼───────────────────────────────────────┤
│ Member 2                       │ Rust Backend, Ingestion, SHA-256,     │
│ Data Engineering & Backend     │ Storage, SQLx PostgreSQL & Neo4j      │
├────────────────────────────────┼───────────────────────────────────────┤
│ Member 3                       │ Frontend, Investigator Workspace,     │
│ Visualization & UX             │ Knowledge Graph UI, Timeline & Maps   │
├────────────────────────────────┼───────────────────────────────────────┤
│ Member 4 (Swastik)             │ Forensic Reconstruction, DEC Contracts│
│ Forensic Engine & Integration  │ Contradiction Engine, Video, Laya     │
└────────────────────────────────┴───────────────────────────────────────┘
```

---

## 2. Member 1: AI/ML & Evidence Intelligence

### 2.1 Domain Responsibilities
- Parsing unstructured/semi-structured forensic evidence: Call Detail Records (CDR) CSVs and Chat Export JSONs (WhatsApp format).
- Identifier extraction and normalization:
  - Phone numbers normalized to international E.164 standard (e.g. `09811002233` -> `+919811002233`).
  - IMSI (International Mobile Subscriber Identity) validation (15 digits, MCC/MNC prefixes).
  - IMEI (International Mobile Equipment Identity) validation (15 digits, Luhn check compatibility).
  - Email address canonicalization and regex extraction.
- Entity resolution & deduplication:
  - Merging observed communication endpoints sharing exact normalized identifiers into canonical `EntityCandidate` nodes.
  - Generating pairwise `ObservedRelationship` edges with communication metadata (timestamps, duration, cell towers).

### 2.2 Integration Status & Rust Porting
- **Status:** **FULLY INTEGRATED IN RUST** (`backend-rust/src/extraction/`).
- **Files:**
  - `backend-rust/src/extraction/cdr.rs`: Robust CSV CDR parser handling standard telecom schemas.
  - `backend-rust/src/extraction/chat.rs`: WhatsApp export JSON parser handling group & direct chats.
  - `backend-rust/src/extraction/ner.rs`: Multi-identifier regex extractor with E.164 normalization.
  - `backend-rust/src/extraction/resolution.rs`: Entity deduplication and graph candidate generator.
  - `backend-rust/src/extraction/service.rs`: Unified extraction service orchestrating extraction runs.
- **Verification:**
  - Covered by 28 unit tests in `tests/extraction.rs` and 31 integration scenarios in `tests/integration_runs.rs`.

---

## 3. Member 2: Backend & Data Engineering

### 3.1 Domain Responsibilities
- High-performance, memory-safe backend engine (`backend-rust`) using Axum 0.7 and Tokio async runtime.
- Streaming evidence ingestion with constant-memory SHA-256 and SHA-3 checksum computation.
- Two-tier storage layout:
  - `data/raw/<evidence_id>/`: Immutable raw files with write-once permissions.
  - `data/derived/<evidence_id>/`: Versioned extraction run manifests (`run-xxx.json`) and `active-run.json`.
- PostgreSQL 16 persistence layer with SQLx connection pooling and compiled query checking.
- Neo4j 5.26 Cypher projection engine and graph querying APIs.
- Case management CRUD APIs (`POST /api/v1/cases`, `GET /api/v1/cases`, `GET /api/v1/cases/:case_id`, `GET /api/v1/cases/:case_id/evidence`).

### 3.2 Integration Status & Robustness
- **Status:** **FULLY INTEGRATED IN RUST** (`backend-rust/src/ingestion/`, `backend-rust/src/persistence/`, `backend-rust/src/graph/`, `backend-rust/src/api/`).
- **Files:**
  - `backend-rust/src/ingestion/storage.rs`: Immutable exhibit and manifest filesystem manager.
  - `backend-rust/src/persistence/postgres.rs`: SQLx repositories with graceful offline filesystem fallback.
  - `backend-rust/src/graph/neo4j.rs`: Neo4j graph repository with in-memory projection fallback.
  - `backend-rust/src/api/routes/cases.rs` & `evidence.rs`: REST routing handlers.
- **Verification:**
  - All 117 Rust tests passing, zero clippy warnings, zero formatting diffs, clean release build.

---

## 4. Member 3: Frontend & Visualization

### 4.1 Domain Responsibilities
- Modern, dark-mode, forensic-grade React 19 + TypeScript + Vite user interface (`apps/web`).
- Interactive Knowledge Graph visualization using Cytoscape.js canvas rendering:
  - Color-coded node taxonomy (Person, Phone, Location, CellTower, Exhibit, Vehicle).
  - Interactive edge exploration (Observed vs Inferred visual cues).
  - Neighborhood drill-down and shortest path tracing.
- Interactive Timeline & Spatial Map:
  - Chronological event cards with confidence badges and uncertainty window markers.
  - Spatial trajectory mapping with tower coverage radii and speed metrics.
- Evidence Upload & Ingestion Workspace:
  - Drag-and-drop staging area with file metadata inspection.
  - Real multipart streaming upload to `/api/v1/evidence/upload?auto_extract=true`.
  - Display of server-verified SHA-256 cryptographic hashes and extraction candidate counts.
- Judicial Export Gate:
  - Rendering the `SHIP_EXPORT_BLOCKED` badge when active physical contradictions exist, preventing non-compliant reports from reaching court submission.

### 4.2 Integration Status
- **Status:** **FULLY INTEGRATED & RECONCILED** (`apps/web/src/`).
- **Files:**
  - `apps/web/src/features/workspace/pages/EvidenceUploadPanel.tsx`: Real upload dispatch, receipt display, error handling.
  - `apps/web/src/features/workspace/pages/EvidenceUploadPanel.test.tsx`: 4 new unit tests.
  - `apps/web/src/features/workspace/pages/KnowledgeGraphView.tsx`: Cytoscape knowledge graph viewer.
  - `apps/web/src/features/workspace/pages/TimelineView.tsx`: Interactive chronologic reconstruction.
  - `apps/web/src/features/workspace/data/repository.ts`: Live API adapter with fallback for synthetic demo case.
- **Verification:**
  - 264 Vitest tests passing across 20 test files; production Vite build compiles in 23.61s with zero errors.

---

## 5. Member 4 (Swastik): Forensic Reconstruction & Integrations

### 5.1 Domain Responsibilities
- Forensic decision contracts DEC-001 through DEC-006:
  - **DEC-001:** Additive clock drift convention ($T_{true} = T_{device} + \Delta t$).
  - **DEC-002:** Half-open interval semantics $[t_{start}, t_{end})$ across millisecond, second, minute, day precisions.
  - **DEC-003:** Haversine distance, elapsed time bounds, and conservative minimum speed ($v_{min} = d / \Delta t_{max}$).
  - **DEC-004:** Physical impossibility contradiction detection ($v_{min} > 120\text{ km/h}$ for ground transit, dual co-presence).
  - **DEC-005:** Video processing cryptographic provenance, streaming SHA-256, and metadata extraction.
  - **DEC-006:** Non-guilt asserting, neutral hypothesis formulation ("physical co-presence cannot be established without corroborating video").
- Integration contracts with future Laya / ONNX decision service runtime.

### 5.2 Integration Status
- **Status:** **FULLY INTEGRATED IN RUST** (`backend-rust/src/reconstruction/`).
- **Files:**
  - `backend-rust/src/reconstruction/temporal.rs`: DEC-001 & DEC-002 interval ordering and clock drift normalization.
  - `backend-rust/src/reconstruction/spatial.rs`: DEC-003 Haversine distance and transit metric engine.
  - `backend-rust/src/reconstruction/contradictions.rs`: DEC-004 & DEC-006 speed contradiction detector.
  - `backend-rust/src/reconstruction/video.rs`: DEC-005 streaming SHA-256 frame provenance analyzer.
- **Verification:**
  - Verified by dedicated test suite `tests/reconstruction_tests.rs` with comprehensive boundary condition coverage.
