# Phase 2B.3 — Graph API Integration Architecture

**Project:** YUKTI Digital Forensics Platform  
**Workstream:** Phase 2B — Frontend Knowledge Graph Visualization & Interactive Investigator Workspace  
**Author:** Member 3 (Frontend & Visualization Lead) / Infrastructure Engineer  
**Date:** November 19, 2026  
**Status:** IMPLEMENTED & VERIFIED  

---

## 1. Overview

Graph API consumption in YUKTI strictly adheres to the centralized HTTP client architecture defined in `apps/web/src/services/http.ts`.

No secondary API base URLs, duplicate authentication handlers, or raw `fetch` calls are introduced. All requests use `requestJson` with structured runtime validation, timeout enforcement, and abortable cancellation.

---

## 2. Consumed Backend Routes

| Route | Method | Rust Source | Purpose |
|:---|:---:|:---|:---|
| `/api/v1/cases/:case_id/graph/summary` | `GET` | `src/api/routes/graph.rs` | Returns aggregate counts, outbox sync state, privacy status |
| `/api/v1/cases/:case_id/graph/entities/:entity_id/neighbors` | `GET` | `src/api/routes/graph.rs` | Fetches 1- to 3-hop neighborhood around an entity |
| `/api/v1/cases/:case_id/graph/path` | `GET` | `src/api/routes/graph.rs` | Calculates shortest evidentiary path between two entities |

---

## 3. Case ID Normalization & Cross-Environment Compatibility

The Rust backend strictly enforces RFC 4122 UUID parameters for all case queries. To bridge the gap with demonstration case slugs (e.g. `CASE-026`), the frontend API client implements deterministic normalization:

```typescript
export function normalizeCaseId(caseId: string): string {
  const trimmed = caseId.trim();
  if (UUID_REGEX.test(trimmed)) {
    return trimmed.toLowerCase();
  }
  const numMatch = trimmed.match(/\d+/);
  if (numMatch) {
    const padded = numMatch[0].padStart(12, '0');
    return `00000000-0000-0000-0000-${padded}`;
  }
  return '00000000-0000-0000-0000-000000000001';
}
```

This ensures cross-environment compatibility without changing backend schemas or breaking frontend test fixtures.

---

## 4. Parameter Clamping & Safety Gates

To prevent accidental denial-of-service or query timeouts, the frontend client enforces boundary limits prior to dispatch:
- `depth`: Clamped to $[1, 3]$ (default: 1).
- `limit`: Clamped to $[1, 100]$ (default: 50).
- `max_hops`: Clamped to $[1, 5]$ (default: 3).

---

## 5. Synchronization State & Degraded Mode Handling

The client dynamically decodes `graph_synchronization_status`:
- `SYNCHRONIZED`: Outbox projection is current.
- `SYNCING (N pending)`: Outbox events are in transit; UI displays non-blocking spin telemetry.
- `DEGRADED (N failures)`: Outbox projection encountered retries or dead-letter events.
- `GRAPH_DISABLED`: Rust backend is operating in filesystem fallback mode without Neo4j. UI renders informative offline banner and falls back to local evidence projections without crashing.
