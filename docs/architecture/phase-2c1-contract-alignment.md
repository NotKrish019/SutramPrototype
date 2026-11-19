# Phase 2C.1: Canonical Contract Alignment & Preserved Decisions Report

**Project:** YUKTI (formerly SUTRAM 2026)  
**Author:** Lead Digital Forensics Architect  
**Date:** 2026-11-19T22:48:00+05:30  
**Phase:** Phase 2C — End-to-End Forensic Reconstruction Integration  

---

## 1. Compliance Matrix: Preserved Forensic Decisions (DEC-001 through DEC-007)

| Decision ID | Canonical Directive | Rust Subsystem (`backend-rust/`) | Relational Schema (`PostgreSQL`) | Graph Schema (`Neo4j`) | Frontend Workspace (`apps/web/`) | Status |
|:---|:---|:---|:---|:---|:---|:---|
| **DEC-001** | Additive clock drift convention: $\text{True UTC} = \text{Device Time} + \text{Clock Offset}$. | `temporal.rs::parse_and_normalize_timestamp` | `evidence.clock_offset_seconds` | Property on `:Evidence` | Displayed as offset & recorded time in `EventDetailPanel` | **COMPLIANT** |
| **DEC-002** | Half-open uncertainty intervals $[start, end)$ with canonical precisions (`EXACT_SECOND`, `MINUTE`, `HOUR`, `DAY`, `ESTIMATED`). Adjacent intervals do not overlap. | `temporal.rs::compute_uncertainty_window` & `intervals_overlap` | `events(uncertainty_window_start, uncertainty_window_end)` with check constraint | Projected nominal and window bounds on `:EventCandidate` | `NormalizedTimeWindow`, SVG hatch rendering in `Swimlanes` | **COMPLIANT** |
| **DEC-003** | Canonical relationship types: `COMMUNICATED_WITH`, `PRESENT_AT`, `CO_OCCURRED_WITH`, `OWNS`, `ASSOCIATE_OF`, `CONTRADICTS`. | `extraction::model`, `reconstruction::model` | `relationships.relation_type` | `[:COMMUNICATED_WITH]`, `[:CONTRADICTS_EVENT]`, etc. | `types.ts::RelationType`, Cytoscape edge types | **COMPLIANT** |
| **DEC-004** | Nullable weight (`Option<f64>`). Observed facts and contradictions do NOT invent arbitrary statistical weights (`weight = None`). | `RelationshipEntity`, `ContradictionLead` | `relationships.weight CHECK (weight IS NULL OR (weight >= 0.0 AND weight <= 1.0))` | Nullable `weight` property on `:RelationshipAssertion` | `WorkspaceRelationship.weight: number \| null` (never rendered as zero) | **COMPLIANT** |
| **DEC-005** | Separate duration and uncertainty window fields (`duration_seconds`, `uncertainty_window_start`, `uncertainty_window_end`). | `RawTimelineEvent`, `NormalizedTimeWindow` | Columns separated in `events` | Distinct node attributes on `:EventCandidate` | Distinct fields in `WorkspaceEvent` | **COMPLIANT** |
| **DEC-006** | Neutral, objective language for investigative leads. Provenance tracking: `observed`, `supplied`, `inferred`. | `contradictions.rs`, `ObservedLeadBasis`, `DerivedLeadInference` | `contradictions.explanation`, `events.is_observed` | `:ContradictionLead.neutral_explanation` | Provenance badges, disclaimers in `ContradictionReview` | **COMPLIANT** |
| **DEC-007** | Extraction preserves raw timestamps verbatim (`raw_timestamp`); temporal engine alone normalizes to UTC (`nominal_utc`). | `ExtractedEventCandidate::raw_timestamp`, `RawTimelineEvent::timestamp` | `events.source_record_id`, `events.nominal_timestamp` | `nominal_timestamp` on `:EventCandidate` | `WorkspaceEvent.source_recorded_time` vs `timestamp` | **COMPLIANT** |

---

## 2. Mathematical Verification of Core Principles

### 2.1 Additive Clock Offset (DEC-001)
If a CCTV DVR clock runs 10 minutes (600 seconds) slow:
$$\text{Offset} = +600.0\text{ s}$$
$$\text{Observed DVR Timestamp} = \text{2026-03-14 22:20:00}$$
$$\text{Normalized UTC Timestamp} = \text{2026-03-14 22:20:00} + 600.0\text{ s} = \text{2026-03-14 22:30:00 UTC}$$

### 2.2 Half-Open Intervals and Non-Overlapping Adjacency (DEC-002)
For two consecutive 1-minute call intervals:
$$I_A = [22:30:00, 22:31:00), \quad I_B = [22:31:00, 22:32:00)$$
$$\max(\text{start}_A, \text{start}_B) = 22:31:00$$
$$\min(\text{end}_A, \text{end}_B) = 22:31:00$$
Because $\max(\text{start}_A, \text{start}_B) \not< \min(\text{end}_A, \text{end}_B)$, `intervals_overlap(start_A, end_A, start_B, end_B)` evaluates strictly to `None`. Adjacent intervals do not falsely report concurrency or contradiction.

### 2.3 Existential Feasibility and Speed Thresholding (DEC-004, DEC-006)
For two located events $E_A$ and $E_B$ with duration $D_A$:
$$t_{\text{earliest completion}} = \text{start}_A + D_A$$
$$\Delta t_{\text{max}} = \text{end}_B - t_{\text{earliest completion}}$$
The minimum required speed is calculated conservatively as:
$$v_{\text{min required}} = \frac{d_{\text{Haversine}}(E_A, E_B)}{\Delta t_{\text{max}}}$$
Using $\Delta t_{\text{max}}$ guarantees that $v_{\text{min required}}$ is the absolute infimum of speed required across all valid timestamps. It is never overstated. If $v_{\text{min required}} > v_{\text{threshold}}$, an investigative lead is generated with standardized limitation caveats.

---

## 3. Schema and Adapter Alignment Summary

All relational tables, graph projections, Rust serialization contracts, and TypeScript interfaces preserve the canonical schemas without unilateral drift. Any optional fields (e.g. `weight`, `clock_offset_seconds`, `duration_seconds`) use explicit nullable types across all language boundaries.
