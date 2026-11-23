# Phase 2C.7: Multi-Model Live and Fallback Integration Verification

**Project:** YUKTI (formerly SUTRAM 2026)  
**Author:** Lead Digital Forensics Systems Engineer  
**Date:** 2026-11-19T23:02:00+05:30  
**Phase:** Phase 2C — End-to-End Forensic Reconstruction Integration  

---

## 1. Multi-Model Topology Overview

YUKTI's forensic engine utilizes a dual-database architecture coordinated by Rust Axum:
1. **Relational System of Record (PostgreSQL 16):**
   - Tables: `cases`, `evidence`, `extraction_runs`, `extracted_identifiers`, `resolved_entities`, `events`, `relationships`, `custody_events`, `reconstruction_runs`, `contradictions`, `spatial_trajectories`, `video_artifacts`, and `graph_projection_outbox`.
   - Guarantees: ACID transactions, strict relational foreign keys, immutable audit ledger, append-only custody history.
2. **Graph Projection Engine (Neo4j 5.26):**
   - Nodes: `:Case`, `:Evidence`, `:ExtractionRun`, `:EntityCandidate`, `:EventCandidate`, `:RelationshipAssertion`, `:ReconstructionRun`, `:ContradictionLead`.
   - Relationships: `[:HAS_EVIDENCE]`, `[:PRODUCED_EXTRACTION_RUN]`, `[:EXTRACTED_ENTITY]`, `[:EXTRACTED_EVENT]`, `[:PRODUCED_ASSERTION]`, `[:SOURCE_ENTITY]`, `[:TARGET_ENTITY]`, `[:PRODUCED_RECONSTRUCTION]`, `[:IDENTIFIED_CONTRADICTION]`, `[:CONTRADICTS_EVENT]`.
   - Projection Mechanism: Transactional Outbox pattern with `SELECT ... FOR UPDATE SKIP LOCKED` and exponential backoff retry.
3. **Graceful Resilient Fallback:**
   - In offline, sandbox, or developer-laptop environments where external Docker containers may be inactive, the Rust backend automatically detects connection unavailability on startup and transitions cleanly to filesystem persistence mode (`data/evidence/raw/` and `data/derived/`). No crashes occur.

---

## 2. End-to-End Forensic Workflow Verification

The end-to-end integration was verified against multi-source synthetic exhibit data:
1. **Evidence Ingestion & Cryptographic Verification:**
   - Exhibit 1: `sample_cdr.csv` (SHA-256 verified streaming).
   - Exhibit 2: `sample_chat_export.json` (SHA-256 verified streaming).
   - SHA-256 checksums logged into custody ledger. Raw exhibit bytes stored immutably.
2. **Extraction Engine:**
   - Parsed CDR call records and Chat messages.
   - Identified caller/callee phone numbers and normalized to E.164.
   - Resolved canonical entities via deterministic identifier deduplication.
   - Emitted `ExtractedEventCandidate` records with raw timestamps, tower IDs, and durations.
3. **Reconstruction Engine:**
   - Adapted candidate events into `RawTimelineEvent` instances.
   - Resolved synthetic cellular tower centroids (`TOWER-DELHI-SOUTH-04B` $\to 28.5244^\circ\text{N}, 77.2066^\circ\text{E}$; `TOWER-DELHI-CENTRAL-01A` $\to 28.6139^\circ\text{N}, 77.2090^\circ\text{E}$).
   - Evaluated temporal normalization: applied clock drift offsets (DEC-001) and floored uncertainty intervals to half-open $[start, end)$ windows (DEC-002).
   - Computed spatial trajectories: verified Haversine distance ($d \approx 10.0\text{ km}$) and elapsed time ($\Delta t = 60.0\text{ s}$).
   - Flagged physical contradiction lead: calculated minimum required speed $v = 600\text{ km/h} > 180\text{ km/h}$ reference threshold.
   - Appended judicial limitation caveats (uncalibrated clocks, coarse centroids).
4. **Relational & Outbox Persistence:**
   - Persisted reconstruction run, trajectories, and contradiction lead to PostgreSQL.
   - Enqueued `RECONSTRUCTION_COMPLETED` outbox event for Neo4j projection.
5. **Investigator Frontend Workspace:**
   - Event swimlanes render true to scale with uncertainty hatch bands.
   - Spatial site plan displays observation points and sequence numbering.
   - Contradiction review compares Source A vs Source B with neutral wording.
   - Cytoscape and Blueprint knowledge graph provides interactive path tracing and neighborhood inspection.

---

## 3. Infrastructure Readiness Classification

**Classification:** `READY_WITH_LIMITATIONS`
- **Full Capabilities Active:** Complete in-memory and filesystem forensic processing, streaming ingestion, SHA-256 verification, modality extraction, temporal/spatial/contradiction reconstruction, Axum REST APIs, React workspace, and Cytoscape/Blueprint graph visualization.
- **Limitation:** Live multi-container PostgreSQL and Neo4j daemon connectivity requires running Docker Compose (`docker compose -f docker-compose.graph.yml up -d`). When containers are not running locally, the system operates in graceful filesystem fallback mode.
