# Phase 2C.0: Integration Preflight Audit Report

**Project:** YUKTI (formerly SUTRAM 2026)  
**Author:** Lead Rust Backend Architect & Digital Forensics Integration Engineer  
**Date:** 2026-11-19T22:47:30+05:30  
**Phase:** Phase 2C — End-to-End Forensic Reconstruction Integration and Investigator Analysis Workspace  

---

## 1. Executive Summary

YUKTI has completed the foundation phases:
- **Phase 1F-1I:** Rust application foundation, streaming evidence ingestion, SHA-256 cryptographic verification, CDR & Chat extraction, entity resolution, and provisional relationship generation.
- **Phase 1J:** PostgreSQL 16 schema design, SQLx migrations, database repositories, custody ledger, and filesystem reconciliation.
- **Phase 1K:** Migration of temporal normalization, spatial trajectory reconstruction, contradiction detection, and video keyframe extraction into Rust.
- **Phase 2A:** Neo4j digital forensic knowledge graph engine, parameterized Cypher projection, transactional outbox pattern, and technical identifier HMAC privacy.
- **Phase 2B:** Investigator-facing knowledge graph visualization with Cytoscape.js engine, SVG blueprint view, neighborhood inspection, and provenance tracking.

The objective of **Phase 2C** is to connect all these subsystems into a single, unified, end-to-end investigative pipeline:
$$\text{Evidence Upload} \longrightarrow \text{Extraction} \longrightarrow \text{Reconstruction} \longrightarrow \text{PostgreSQL \& Outbox} \longrightarrow \text{Neo4j Projection} \longrightarrow \text{Investigator Workspace}$$

---

## 2. Component Inventory & Audit

### 2.1 Evidence Ingestion Engine (`backend-rust/src/ingestion/`)
- **Status:** Complete & verified.
- **Capabilities:** High-throughput streaming upload, SHA-256 hash calculation during streaming, MIME validation, immutable storage in raw directory (`data/evidence/raw/`), manifest generation, and custody logging.

### 2.2 Forensic Extraction Subsystem (`backend-rust/src/extraction/`)
- **Status:** Complete & verified.
- **Capabilities:**
  - `cdr.rs`: Strict CSV parser for Call Detail Records, validating mandatory fields, durations ($\ge 0$), normalizing phone numbers to E.164, emitting `ExtractedIdentifier`, `ExtractedEventCandidate`, and `ProvisionalRelationshipCandidate`.
  - `chat.rs`: JSON chat export parser, extracting message timestamps, sender/receiver identifiers, event candidates (`COMMUNICATION_MESSAGE`), and direct communication relationships.
  - `service.rs`: Modality dispatch, entity resolution across identifiers (`deduplicate_identifiers`), versioned extraction runs, and filesystem snapshot persistence.

### 2.3 Forensic Reconstruction Subsystem (`backend-rust/src/reconstruction/`)
- **Status:** Core mathematical algorithms complete in `temporal.rs`, `spatial.rs`, `contradictions.rs`, `video.rs`, `service.rs`.
- **Capabilities:**
  - `temporal.rs`: Full adherence to DEC-001 (additive clock drift: $\text{True UTC} = \text{Device Time} + \text{Clock Offset}$) and DEC-002 (half-open $[start, end)$ intervals across canonical precisions `EXACT_SECOND`, `MINUTE`, `HOUR`, `DAY`, `ESTIMATED`).
  - `spatial.rs`: WGS84 coordinate validation, Haversine great-circle distance computation (DEC-003), trajectory generation, conservative elapsed-time evaluation, and reference speed threshold checking.
  - `contradictions.rs`: Neutral, evidence-grounded contradiction detection (DEC-004, DEC-006). Evaluates existential feasibility over uncertainty intervals: flags discrepancies only when no valid pair $(t_A, t_B)$ permits completing the activity and transiting to the destination within the threshold. Omits fabricated probabilities (`weight = None`).
  - `video.rs`: Subprocess-based FFprobe metadata probing and FFmpeg keyframe extraction with individual SHA-256 hashes per frame.
  - `adapters.rs`: Bridges `ExtractedEventCandidate` to `RawTimelineEvent`.
  - `service.rs`: `ForensicReconstructionService` coordinating temporal, spatial, and contradiction reconstruction.

### 2.4 Relational Persistence Subsystem (`backend-rust/src/persistence/`)
- **Status:** Schema tables exist for `cases`, `evidence`, `extraction_runs`, `extracted_identifiers`, `resolved_entities`, `events`, `relationships`, `validation_issues`, `custody_events`, `graph_projection_outbox`, `reconstruction_runs`, `contradictions`, `spatial_trajectories`, and `video_artifacts`.
- **Gaps Identified (FND-2C-001, FND-2C-002):**
  - Typed `ReconstructionRunRepository` is missing.
  - `reconstruction_runs.extraction_run_id` was marked `NOT NULL`, which restricts case-level reconstructions that span multiple exhibit runs.
  - `events.event_id` in `extraction_run_repository.rs` generated random UUIDs rather than deterministic UUIDs matching extraction candidate IDs, creating potential foreign key lookup friction for `contradictions(event_id_1, event_id_2)`.

### 2.5 Knowledge Graph Engine & Outbox Projection (`backend-rust/src/graph/`)
- **Status:** Complete.
- **Capabilities:**
  - `outbox.rs`: Transactional outbox repository with `SKIP LOCKED` worker support.
  - `projection.rs`: Idempotent Cypher projection. Already includes support for `RECONSTRUCTION_COMPLETED`, projecting `ReconstructionRun` and `ContradictionLead` nodes with `:IDENTIFIED_CONTRADICTION` and `:CONTRADICTS_EVENT` edges.

### 2.6 Frontend Investigator Workspace (`apps/web/src/features/workspace/`)
- **Status:** Highly developed UI layouts.
- **Capabilities:**
  - `EventReconstruction.tsx`: Interactive SVG swimlanes true-to-scale, uncertainty hatch rendering, chronology table, and event details. Cross-links to Knowledge Graph (`/case/:id/graph?node=:entityId`) and Contradictions.
  - `SpatialReconstruction.tsx`: Schematic site-plan layout, place inspection, linked events, and evidence.
  - `ContradictionReview.tsx`: Review categories, observed records comparison (Source A vs Source B), suggested verification, and limitations caveats.
  - `KnowledgeGraphWorkspace.tsx`: Dual-engine Cytoscape.js interactive graph and SVG Blueprint view with neighborhood inspection and pathfinding.
- **Gaps Identified (FND-2C-005):**
  - Requires active API integration and typing for live reconstruction query and trigger endpoints.

---

## 3. End-to-End Forensic Dataflow Architecture

```
[ Exhibit Upload ] (Multipart Form / Streaming)
       │
       ▼
[ SHA-256 Integrity & Storage ] ──> [ Raw File Store ]
       │
       ▼
[ Modality Extraction Dispatch ] ──> (CDR / Chat / JSON)
       │
       ├──> [ Extracted Identifiers & Resolved Entities ]
       └──> [ Extracted Event Candidates & Relationships ]
       │
       ▼
[ Reconstruction Adapter ] (Preserve offsets, timezones, locations)
       │
       ▼
[ Forensic Reconstruction Service ]
       ├──> [ Temporal Engine: [start, end) normalization ]
       ├──> [ Spatial Engine: Haversine & Trajectories ]
       └──> [ Contradiction Engine: Existential Feasibility ]
       │
       ▼
[ PostgreSQL Persistence ] (Atomic Transaction)
       ├──> [ INSERT INTO reconstruction_runs ]
       ├──> [ INSERT INTO contradictions ]
       ├──> [ INSERT INTO spatial_trajectories ]
       └──> [ INSERT INTO graph_projection_outbox (RECONSTRUCTION_COMPLETED) ]
       │
       ▼
[ Graph Projection Engine ] ──> [ Neo4j Cypher Batch MERGE ]
       │
       ▼
[ Axum API Routes ] (/api/v1/cases/:id/reconstruction)
       │
       ▼
[ Investigator Workspace UI ] (Timeline, Map, Contradictions, Graph)
```

---

## 4. Preflight Audit Conclusion

All core algorithms and data models exist in the repository. The remaining work for Phase 2C consists of:
1. Refining the schema and contracts (`20261119000003_reconstruction_schema_refinement.sql`).
2. Building `ReconstructionRunRepository` and connecting outbox event enqueuing.
3. Adding Axum REST endpoints under `backend-rust/src/api/routes/reconstruction.rs`.
4. Enhancing the extraction adapter to ensure lossless propagation of temporal/spatial metadata.
5. Providing client API services and hooks in `apps/web` with live/synthetic fallback.
6. Performing end-to-end test verification across the entire multi-model pipeline.
