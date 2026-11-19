# Phase 2B.0 — Frontend & API Contract Audit

**Project:** YUKTI Digital Forensics Platform  
**Workstream:** Phase 2B — Frontend Knowledge Graph Visualization & Interactive Investigator Workspace  
**Author:** Member 3 (Frontend & Visualization Lead) / Infrastructure Engineer  
**Date:** November 19, 2026  
**Status:** APPROVED & AUDITED  

---

## 1. Executive Summary

This audit establishes the exact contracts between the YUKTI React + TypeScript frontend (`apps/web`) and the Rust Axum backend graph endpoints implemented in Phase 2A.

The frontend is strictly presentation, interaction, client-side validation, and API consumption. All forensic business logic, graph traversal bounds, Cypher execution, and cryptographic pseudonymization remain in the Rust backend.

---

## 2. API Endpoint Contracts

### 2.1 Case Knowledge Graph Summary
- **Route:** `GET /api/v1/cases/:case_id/graph/summary`
- **Path Parameters:**
  - `case_id`: String (UUID format, e.g. `a1b2c3d4-e5f6-7890-abcd-ef1234567890`)
- **Query Parameters:** None
- **Success Response (200 OK):**
```json
{
  "case_id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
  "evidence_count": 4,
  "extraction_run_count": 4,
  "entity_candidate_count": 28,
  "event_candidate_count": 52,
  "relationship_assertion_count": 19,
  "reconstruction_run_count": 1,
  "contradiction_lead_count": 2,
  "graph_synchronization_status": "SYNCHRONIZED",
  "last_projection_time": "2026-11-19T08:00:00Z",
  "identifier_privacy": "hmac_sha256_active"
}
```
- **Synchronization State Values:**
  - `"SYNCHRONIZED"`: PostgreSQL outbox and Neo4j projection are completely caught up.
  - `"SYNCING (N pending)"`: Outbox events are actively being claimed and projected.
  - `"DEGRADED (N failures)"`: Some outbox projections failed or reached dead-letter status.
  - `"STANDALONE_GRAPH"`: Neo4j is operational without PostgreSQL outbox tracking.
  - `"GRAPH_DISABLED"`: Backend running in filesystem mode without Neo4j configured.
- **Identifier Privacy Values:**
  - `"hmac_sha256_active"`: Keyed HMAC-SHA-256 enabled with case isolation.
  - `"hmac_key_unconfigured"`: Fallback unkeyed SHA-256 pseudonymization.
  - `"unconfigured"`: Graph engine offline.

---

### 2.2 Entity Neighborhood Exploration
- **Route:** `GET /api/v1/cases/:case_id/graph/entities/:entity_id/neighbors`
- **Path Parameters:**
  - `case_id`: String (UUID format)
  - `entity_id`: String (Target `graph_entity_id` or `fingerprint`)
- **Query Parameters:**
  - `depth`: Optional unsigned integer (default: 1, backend clamped to $[1, 3]$)
  - `limit`: Optional integer (default: 50, backend clamped to $[1, 100]$)
- **Success Response (200 OK):**
```json
{
  "case_id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
  "root_entity_id": "ent-pa",
  "depth": 1,
  "nodes": [
    {
      "id": "ent-pa",
      "label": "EntityCandidate",
      "properties": {
        "case_id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
        "canonical_name": "Pawan Arora",
        "entity_type": "PERSON",
        "confidence": 0.95
      }
    },
    {
      "id": "ent-dev",
      "label": "EntityCandidate",
      "properties": {
        "case_id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
        "fingerprint": "7f83b1657ff1fc53b92dc18148a1d65dfc2d4b1fa3d677284addd200126d9069",
        "entity_type": "PHONE",
        "confidence": 0.98
      }
    }
  ],
  "relationships": [
    {
      "id": "rel-01",
      "source": "ent-pa",
      "target": "ent-dev",
      "relation_type": "COMMUNICATED_WITH",
      "properties": {
        "is_observed": true,
        "assertion_id": "run-01:0",
        "confidence": 0.95
      }
    }
  ]
}
```

---

### 2.3 Shortest Path Tracing
- **Route:** `GET /api/v1/cases/:case_id/graph/path`
- **Path Parameters:**
  - `case_id`: String (UUID format)
- **Query Parameters:**
  - `from`: String (Source entity ID / fingerprint)
  - `to`: String (Target entity ID / fingerprint)
  - `max_hops`: Optional unsigned integer (default: 3, backend clamped to $[1, 5]$)
- **Success Response (200 OK):**
```json
{
  "case_id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
  "from_entity_id": "ent-pa",
  "to_entity_id": "ent-acc",
  "found": true,
  "nodes": [ ... ],
  "relationships": [ ... ]
}
```

---

## 3. TypeScript Data Transfer Objects (DTO)

The frontend types in `apps/web/src/features/graph/types/graph.ts` will strictly mirror this Rust backend contract:

```typescript
export interface GraphSummaryDto {
  case_id: string;
  evidence_count: number;
  extraction_run_count: number;
  entity_candidate_count: number;
  event_candidate_count: number;
  relationship_assertion_count: number;
  reconstruction_run_count: number;
  contradiction_lead_count: number;
  graph_synchronization_status: string;
  last_projection_time: string | null;
  identifier_privacy: string;
}

export interface GraphNodeDto {
  id: string;
  label: string;
  properties: Record<string, unknown>;
}

export interface GraphRelationshipDto {
  id: string;
  source: string;
  target: string;
  relation_type: string;
  properties: Record<string, unknown>;
}

export interface GraphNeighborResponseDto {
  case_id: string;
  root_entity_id: string;
  depth: number;
  nodes: GraphNodeDto[];
  relationships: GraphRelationshipDto[];
}

export interface GraphPathResponseDto {
  case_id: string;
  from_entity_id: string;
  to_entity_id: string;
  found: boolean;
  nodes: GraphNodeDto[];
  relationships: GraphRelationshipDto[];
}
```

---

## 4. Discrepancies and Frontend Integration Strategy

1. **Synthetic Case IDs vs Database UUIDs:**
   - The synthetic demonstration dataset uses formatted string IDs like `CASE-026`.
   - The Rust backend expects standard RFC 4122 UUIDs (e.g., `00000000-0000-0000-0000-000000000026`).
   - Strategy: The frontend adapter will transparently convert deterministic demo case slugs into compliant UUIDs when querying backend APIs, while preserving the user-facing display label `CASE-026`.
2. **Missing Properties and Graceful Defaults:**
   - When nodes do not supply confidence or timestamps, the UI will display "Not provided" or omit the field. It will never synthesize a zero or fake certainty.
3. **Identifier Privacy Presentation:**
   - When an entity candidate contains a cryptographic fingerprint (e.g., `7f83b1...`), the UI will display a redacted tag `[HMAC-Protected Identifier]` with a copyable short hash, clearly explaining that the raw technical identifier is secured within the PostgreSQL evidence store.
