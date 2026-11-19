# Phase 2B.1 — Graph Visualization Technology Architecture

**Project:** YUKTI Digital Forensics Platform  
**Workstream:** Phase 2B — Frontend Knowledge Graph Visualization & Interactive Investigator Workspace  
**Author:** Member 3 (Frontend & Visualization Lead) / Infrastructure Engineer  
**Date:** November 19, 2026  
**Status:** APPROVED & ADOPTED  

---

## 1. Technology Selection: Cytoscape.js

Following architectural directives, **Cytoscape.js (`^3.35.0`)** has been selected and installed alongside its official TypeScript definitions (`@types/cytoscape`).

### 1.1 Key Justifications:
1. **Forensic Network Analysis:** Built specifically for large-scale graph analysis and biological/forensic network rendering.
2. **Performance & Canvas Acceleration:** Renders via HTML5 canvas, supporting hundreds of nodes and relationships smoothly without DOM overhead.
3. **Compound Nodes & Custom Stylings:** Native support for varied node shapes, edge routing, directed arrows, badges, and dynamic styling based on data attributes.
4. **Clean React Lifecycle:** Clean imperative API (`destroy()`, `resize()`, `batch()`) allowing strict separation of UI canvas lifecycle from React state updates.

---

## 2. Component Architecture

We adopt the modular architecture specified in the Phase 2B mandate under `apps/web/src/features/graph/`:

```
apps/web/src/features/graph/
    components/
        KnowledgeGraph.tsx          # Master graph workspace container with canvas & inspectors
        GraphCanvas.tsx             # Cytoscape.js canvas lifecycle and rendering host
        GraphToolbar.tsx            # Zoom, fit, center, layout selector, and export controls
        GraphLegend.tsx             # Visual semantics guide (node categories, edge styles)
        GraphNodeDetails.tsx        # Inspector panel for selected node attributes & provenance
        GraphEdgeDetails.tsx        # Inspector panel for relationship type, assertion & evidence
        GraphSyncStatus.tsx         # Backend outbox & Neo4j synchronization badge
        GraphEmptyState.tsx         # Empty / unconfigured state presentation
    hooks/
        useKnowledgeGraph.ts        # Data fetching, expansion, path tracing, and layout management
        useGraphSelection.ts        # Node & edge selection, hover state, and focus tracking
    services/
        graphApi.ts                 # Typed HTTP client calling Rust backend endpoints
        graphAdapter.ts             # Translates backend DTOs into Cytoscape element definitions
    types/
        graph.ts                    # Canonical TypeScript interfaces for graph domain & DTOs
    utils/
        graphStyles.ts              # Forensic visual styling stylesheet for Cytoscape
        graphFilters.ts             # Category, provenance, and search filtering helpers
```

---

## 3. Cytoscape Lifecycle & Memory Management

1. **Mounting:**
   - Instantiated when the container DOM node becomes available.
   - Built-in layouts: `cose` (physics-based force-directed layout) and `concentric` (radial hierarchy around key entities).
2. **Differential Updates:**
   - Instead of destroying and recreating the graph on every state change, `cy.batch()` is used to add new nodes/edges during neighborhood expansion and remove filtered items.
3. **Unmounting:**
   - Comprehensive cleanup: `cy.removeAllListeners(); cy.destroy();`
   - Clear canvas references to avoid detached DOM memory leaks.
4. **Responsive Resizing:**
   - `ResizeObserver` attached to the canvas container invokes `cy.resize()`.
