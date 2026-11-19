# Architecture Document: PostgreSQL-to-Neo4j Knowledge Graph Projection

**Document ID:** ARCH-2A3  
**Status:** Approved & Implemented  
**Author:** Lead Rust Knowledge Graph Architect, YUKTI Forensics Team  
**Date:** 2026-11-19  

---

## 1. Executive Summary

YUKTI enforces a multi-model storage architecture:
1. **Relational Ground Truth (PostgreSQL 16):** Immutable chain-of-custody, extraction run history, normalized forensic records, audit ledgers, and raw evidence hashes.
2. **Knowledge Graph Projection (Neo4j 5.26):** Topological indexing, cross-source link analysis, entity neighborhoods, path tracing, and contradiction discovery.

To preserve ACID transactional guarantees without distributed 2-phase commit (which is fragile and introduces runtime coupling between PostgreSQL and Neo4j), all graph updates are produced asynchronously and deterministically using the **Transactional Outbox Pattern**.

---

## 2. Transactional Outbox Design

### 2.1 Outbox Table Schema

Created via migration `backend-rust/migrations/20261119000002_create_graph_projection_outbox.sql`:

```sql
CREATE TABLE IF NOT EXISTS graph_projection_outbox (
    outbox_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    case_id UUID NOT NULL REFERENCES cases(case_id) ON DELETE CASCADE,
    aggregate_type VARCHAR(64) NOT NULL,
    aggregate_id UUID NOT NULL,
    aggregate_version INT NOT NULL DEFAULT 1,
    event_type VARCHAR(64) NOT NULL,
    payload JSONB NOT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'PENDING'
        CHECK (status IN ('PENDING', 'PROCESSING', 'SUCCEEDED', 'FAILED', 'RETRY_WAIT', 'DEAD_LETTER')),
    attempt_count INT NOT NULL DEFAULT 0,
    max_attempts INT NOT NULL DEFAULT 5,
    next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    processed_at TIMESTAMPTZ,
    CONSTRAINT uq_outbox_event_idempotency UNIQUE (case_id, aggregate_type, aggregate_id, aggregate_version, event_type)
);
```

### 2.2 Ingestion & Extraction Atomic Enqueuing

When evidence is ingested or an extraction run completes:
1. In the same database transaction (`sqlx::Transaction<Postgres>`):
   - Forensic domain records (e.g., `extraction_runs`, `extracted_entities`, `extracted_events`) are inserted.
   - An outbox record is inserted into `graph_projection_outbox` with status `'PENDING'`.
2. If the transaction rolls back, no outbox record is persisted, ensuring zero phantom graph projections.
3. If the transaction commits, the outbox record is guaranteed to exist.

---

## 3. Projection Engine (`GraphProjectionService`)

The `GraphProjectionService` consumes outbox records and emits idempotent Cypher batches using `MERGE` semantics:

### 3.1 Event Transformations

| Outbox Event Type | PostgreSQL Origin | Generated Cypher Graph Mutations |
|:---|:---|:---|
| `CASE_CREATED` | `cases` table insert | `MERGE (c:Case {case_id: $case_id}) ON CREATE SET ... ON MATCH SET ...` |
| `EVIDENCE_INGESTED` | `evidence_files` insert | `MERGE (c:Case) MERGE (e:Evidence) MERGE (c)-[:HAS_EVIDENCE]->(e)` |
| `EXTRACTION_COMPLETED` | `extraction_runs` commit | `MERGE (r:ExtractionRun)`, `(e)-[:PRODUCED_EXTRACTION_RUN]->(r)`, `(r)-[:EXTRACTED_ENTITY]->(ent)`, `(r)-[:EXTRACTED_EVENT]->(ev)`, `(r)-[:PRODUCED_ASSERTION]->(rel)` |
| `RECONSTRUCTION_COMPLETED` | `reconstruction_runs` commit | `MERGE (rec:ReconstructionRun)`, `(c)-[:PRODUCED_RECONSTRUCTION]->(rec)`, `(rec)-[:IDENTIFIED_CONTRADICTION]->(lead)` |

### 3.2 Technical Identifier Privacy

Sensitive identifiers (e.g. phone numbers, email addresses, IMEI, IMSI) are hashed before emission into Cypher:
$$\text{Fingerprint} = \text{Hex}(\text{HMAC-SHA-256}(K, \text{case\_id} \parallel \text{identifier\_type} \parallel \text{normalized\_value}))$$

- **Case isolation:** The same identifier in different cases produces completely different fingerprints.
- **Unkeyed fallback:** If no secret HMAC key is provided, a scoped fallback SHA-256 hash is computed to prevent plaintext enumeration.
- Ground truth remains strictly isolated inside PostgreSQL.

---

## 4. Concurrent Outbox Processing (`GraphOutboxRepository`)

1. **Lock-Free Claiming:**
   ```sql
   SELECT * FROM graph_projection_outbox
   WHERE status IN ('PENDING', 'RETRY_WAIT')
     AND next_attempt_at <= NOW()
   ORDER BY created_at ASC
   LIMIT $1
   FOR UPDATE SKIP LOCKED;
   ```
   `FOR UPDATE SKIP LOCKED` guarantees multiple daemon workers or CLI tools can process the outbox concurrently without lock contention or duplicate execution.
2. **Exponential Backoff:** On projection failure, `attempt_count` increments and `next_attempt_at` is scheduled using:
   $$\Delta t = 2^{\text{attempt\_count}} \times 2 \text{ seconds}$$
   If `attempt_count >= max_attempts`, the status transitions to `DEAD_LETTER`.

---

## 5. Synchronization CLI (`graph-sync`)

The project provides a dedicated standalone CLI utility: `sutram_backend_rust::bin::graph-sync`:
- `--once`: Process all pending events once and terminate (ideal for CI pipelines and cron jobs).
- `--limit <N>`: Maximum batch size per poll (default: 50).
- `--case-id <UUID>`: Filter outbox events for a specific case.
- `--dry-run`: Parse and log Cypher transformations without executing them against Neo4j.
- `--force`: Run even if `GRAPH_ENABLED=false`.
