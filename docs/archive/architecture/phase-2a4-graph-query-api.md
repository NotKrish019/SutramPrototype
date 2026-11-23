# Architecture Document: Forensic Knowledge Graph Query Engine & APIs

**Document ID:** ARCH-2A4  
**Status:** Approved & Implemented  
**Author:** Lead Rust Knowledge Graph Architect, YUKTI Forensics Team  
**Date:** 2026-11-19  

---

## 1. Overview

The YUKTI Knowledge Graph Query Engine exposes structured REST endpoints for investigator UI exploration, link analysis, neighborhood discovery, and path tracing.

All endpoints adhere to strict forensic query principles:
1. **Case Boundary Enforcement:** Every query explicitly requires and filters on `case_id`. Cross-case leakage is structurally impossible.
2. **Strictly Parameterized Cypher:** Zero string interpolation of user input. All user parameters are supplied through Neo4j JSON parameter maps.
3. **Bounded Traversal Depth:** Traversal depth and limits are strictly clamped at the API boundary to prevent query explosion and denial-of-service.

---

## 2. API Endpoints

### 2.1 Case Graph Summary
- **Endpoint:** `GET /api/v1/cases/:case_id/graph/summary`
- **Description:** Returns aggregate entity, event, relationship, contradiction counts, synchronization status, and privacy mode.
- **Response Format:**
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
  "last_projection_time": "2026-11-19T13:40:00Z",
  "identifier_privacy": "hmac_sha256_active"
}
```

### 2.2 Entity Neighborhood Exploration
- **Endpoint:** `GET /api/v1/cases/:case_id/graph/entities/:entity_id/neighbors?max_depth=2&limit=50`
- **Description:** Explores the local subgraph surrounding a specified entity within an authorized case boundary.
- **Constraints:**
  - `max_depth`: Clamped to $[1, 3]$ (default: 2).
  - `limit`: Clamped to $[1, 100]$ (default: 50).
- **Parameterized Cypher:**
```cypher
MATCH (root:EntityCandidate {case_id: $case_id})
WHERE root.graph_entity_id = $entity_id OR root.fingerprint = $entity_id
MATCH path = (root)-[r*1..3]-(neighbor)
WHERE ALL(n IN nodes(path) WHERE n.case_id = $case_id)
WITH root, neighbor, relationships(path) as rels
LIMIT $limit
RETURN root, collect(DISTINCT neighbor) as neighbors, collect(DISTINCT rels) as all_rels
```

### 2.3 Shortest Path Tracing
- **Endpoint:** `GET /api/v1/cases/:case_id/graph/path?from=:from_id&to=:to_id&max_hops=3`
- **Description:** Finds the shortest evidentiary path connecting two entities within an authorized case.
- **Constraints:**
  - `max_hops`: Clamped to $[1, 5]$ (default: 3).
- **Parameterized Cypher:**
```cypher
MATCH (src:EntityCandidate {case_id: $case_id}), (tgt:EntityCandidate {case_id: $case_id})
WHERE (src.graph_entity_id = $from_id OR src.fingerprint = $from_id)
  AND (tgt.graph_entity_id = $to_id OR tgt.fingerprint = $to_id)
MATCH p = shortestPath((src)-[*1..5]-(tgt))
WHERE ALL(n IN nodes(p) WHERE n.case_id = $case_id)
RETURN p
```

---

## 3. Graceful Degradation & Health Probing

When running without Neo4j (e.g. offline field laptop or tests):
- `/health/ready` reports:
  ```json
  {
    "status": "degraded",
    "version": "0.1.0",
    "storage": "filesystem",
    "database": "unconfigured_filesystem_fallback",
    "graph": "disabled_or_unreachable"
  }
  ```
- Graph API routes return `404 Not Found` with structured error code `GRAPH_NOT_CONFIGURED` or `SERVICE_UNAVAILABLE`.
