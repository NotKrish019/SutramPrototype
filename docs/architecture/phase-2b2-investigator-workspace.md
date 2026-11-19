# Phase 2B.2 — Investigator Knowledge Graph Workspace Architecture

**Project:** YUKTI Digital Forensics Platform  
**Workstream:** Phase 2B — Frontend Knowledge Graph Visualization & Interactive Investigator Workspace  
**Author:** Member 3 (Frontend & Visualization Lead) / Infrastructure Engineer  
**Date:** November 19, 2026  
**Status:** IMPLEMENTED & VERIFIED  

---

## 1. Overview

The Investigator Knowledge Graph Workspace (`apps/web/src/features/graph/components/KnowledgeGraph.tsx`) provides an interactive interface for forensic investigators to explore complex multi-modal evidence topologies.

The workspace is structured into three integrated zones:
1. **Telemetry & Synchronization Header:** Real-time outbox synchronization status (`SYNCHRONIZED`, `SYNCING`, `DEGRADED`, `GRAPH_DISABLED`), HMAC identifier privacy state, and canvas/table view switcher.
2. **Interactive Graph Canvas (Left Panel, 8 Columns):** Cytoscape.js canvas with hardware-accelerated rendering, dynamic force-directed layouts, zoom/pan controls, search, and category filter chips.
3. **Forensic Inspector (Right Panel, 4 Columns):** Detailed attribute inspection for selected nodes and relationship edges, containing provenance metadata, supporting evidence links, and interactive graph exploration actions.

---

## 2. Interactive Canvas Capabilities

### 2.1 Supported Layout Algorithms
- **Force-Directed (CoSE):** Compound Spring Embedder for natural clustering of communicating entities and co-occurring events.
- **Concentric Radial:** Centers primary targets and arranges associates and evidence artifacts in radial distance rings.
- **Hierarchical Flow (Breadthfirst):** Displays causal and temporal progression from source evidence to extracted events.
- **Circular:** Fast perimeter arrangement for high-level link discovery.

### 2.2 Navigation & Viewport Controls
- **Smooth Zoom & Pan:** Keyboard shortcuts, mouse wheel zoom, click-drag canvas panning.
- **Fit View:** Auto-frames all visible nodes with padding.
- **Reset View:** Returns zoom and viewport center to default orientation.
- **Focus Node:** Center viewport directly on an entity identified via search.

---

## 3. Node & Edge Inspectors

### 3.1 Node Details (`GraphNodeDetails`)
- **Category Badge:** Visual badge distinguishing the 10 Phase 2A node categories.
- **Privacy Notice:** Clearly marks HMAC-SHA-256 protected identifiers (`[HMAC] abcdef01…`) and explains ground truth isolation in PostgreSQL.
- **Provenance & Confidence:** Displays algorithmic confidence percentages when available; never displays synthetic zeroes when confidence is missing.
- **Connected Relationships:** Lists all inbound and outbound edges with quick-inspect buttons.
- **Cross-Workspace Navigation:** Direct links to "Open in Timeline" for event candidates and "Inspect Evidence File" for raw artifacts.

### 3.2 Relationship Details (`GraphEdgeDetails`)
- **Observed vs Inferred Distinction:**
  - *Observed Links:* Primary facts from CDR or chat logs (solid edge).
  - *Inferred Links:* Heuristic correlations from spatial or temporal proximity (dashed edge).
- **Contradiction Leads:** Highlights conflicting evidence (e.g. spatial transit impossibility) with prominent red styling and a direct link to the Contradiction Review Workspace.
- **Evidence Basis:** Lists specific supporting evidence artifact IDs linking back to the raw repository.

---

## 4. Interactive Graph Exploration

### 4.1 On-Demand Neighborhood Expansion
- An investigator can select any candidate node and click **"Expand 1-Hop Neighbors"**.
- Triggers `GET /api/v1/cases/:case_id/graph/entities/:entity_id/neighbors?depth=1&limit=50`.
- The frontend adapter deduplicates incoming nodes and relationships and merges them incrementally into the Cytoscape canvas without disrupting existing node positions.

### 4.2 Evidentiary Path Tracing
- An investigator selects a source node, chooses a target node from the dropdown, and clicks **"Trace Path"**.
- Triggers `GET /api/v1/cases/:case_id/graph/path?from=:src&to=:tgt&max_hops=5`.
- Intermediate nodes and connecting edges along the shortest evidentiary path are highlighted in amber (`#FFB224`) on the canvas.

---

## 5. Accessibility & Forensic Safeguards

1. **Tabular Alternative:** A view mode switch provides an accessible, screen-reader-operable HTML table listing all visible nodes, labels, categories, and confidence scores, complete with an "Inspect" action.
2. **Forensic Integrity Notice:** Visible disclaimer states: *"Knowledge graph links represent recorded assertions or algorithmic derivations. They do not constitute conclusive proof of identity, culpability, or wrongdoing."*
