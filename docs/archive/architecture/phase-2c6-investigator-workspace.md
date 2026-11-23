# Phase 2C.6: Investigator Workspace and Frontend Integration

**Project:** YUKTI (formerly SUTRAM 2026)  
**Author:** Lead Frontend & Visualization Engineer  
**Date:** 2026-11-19T22:59:55+05:30  
**Phase:** Phase 2C — End-to-End Forensic Reconstruction Integration  

---

## 1. Unified Investigator Workspace Architecture

The YUKTI investigator workspace provides four tightly integrated analytical views:
1. **Event Reconstruction (`EventReconstruction.tsx`):**
   - True-to-scale SVG event swimlanes mapped across time.
   - Uncertainty hatch rendering indicating imprecise time intervals (DEC-002).
   - Chronological events table with ordering classification (`STRICTLY_ORDERED` vs `CONCURRENT_UNCERTAIN`).
   - Detailed event inspection panel displaying nominal UTC, recorded device time, source timezone, clock drift offset (DEC-001), and supporting exhibit references.
   - Cross-links to Knowledge Graph (`/case/:id/graph?node=:entityId`) and Contradiction Review (`/case/:id/contradictions?item=:reviewId`).
2. **Spatial Reconstruction (`SpatialReconstruction.tsx`):**
   - Schematic site plan showing observation points, camera placements, cellular centroids, and Wi-Fi access areas.
   - Chronological sequence numbers matching timeline order.
   - Place detail inspector linking back to events and exhibits.
3. **Contradiction Review (`ContradictionReview.tsx`):**
   - Side-by-side comparison of observed facts between Source A and Source B.
   - Objective, neutral summary of discrepancies without conclusions of guilt (DEC-006).
   - Derived mathematical transit metrics (minimum required speed vs threshold).
   - Standardized limitation caveats (clock offsets, cell centroids, shared devices).
   - Cross-links to Event Timeline (`/case/:id/timeline?event=:eventId`) and Knowledge Graph (`/case/:id/graph?node=:entityId`).
4. **Knowledge Graph Workspace (`KnowledgeGraphWorkspace.tsx`):**
   - Dual-engine interactive visualization:
     - **Cytoscape Engine:** Interactive physics-directed graph supporting zoom, pan, neighbor exploration, shortest evidentiary path tracing, and bounding box inspection.
     - **Blueprint View:** High-contrast forensic schematic rendering.
   - Technical identifier privacy preserved via keyed HMAC-SHA-256 fingerprinting.

---

## 2. Typed Client Service and Adapters (`apps/web/src/services/reconstruction.ts`)

To communicate with the Rust Axum backend:
- **`triggerCaseReconstruction`:** Dispatches `POST /api/v1/cases/:case_id/reconstruction` with optional custom speed thresholds and timezones.
- **`getCaseReconstruction`:** Dispatches `GET /api/v1/cases/:case_id/reconstruction`.
- **`getCaseTimeline`, `getCaseSpatial`, `getCaseContradictions`:** Subsystem query routes.
- **Adapters:**
  - `adaptTimelineEntriesToWorkspaceEvents`: Translates `TimelineEntryDto` to `WorkspaceEvent`.
  - `adaptContradictionLeadsToReviewItems`: Translates `ContradictionLeadDto` to `ReviewItem`.
  - `normalizeCaseId`: Normalizes case slugs (e.g. `CASE-026`) to deterministic UUID format.

---

## 3. Graceful Multi-Mode Fallback

If the Rust backend is offline or the database exhibits are empty:
- The UI gracefully falls back to the synthetic case repository (`syntheticRepository`).
- When the backend is online, live reconstruction data replaces or enriches the dossier.
- Network and timeout errors are caught and surfaced via `ApiError` without crashing the application.
