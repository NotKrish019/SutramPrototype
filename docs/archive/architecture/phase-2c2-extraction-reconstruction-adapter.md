# Phase 2C.2: Extraction-to-Reconstruction Adapter Architecture

**Project:** YUKTI (formerly SUTRAM 2026)  
**Author:** Lead Rust Extraction & Reconstruction Engineer  
**Date:** 2026-11-19T22:48:45+05:30  
**Phase:** Phase 2C — End-to-End Forensic Reconstruction Integration  

---

## 1. Overview and Problem Statement

The extraction engine parses raw evidence exhibits (such as Call Detail Records in CSV and Chat Exports in JSON) into structured candidate artifacts:
- `ExtractedIdentifier`
- `ResolvedEntityCandidate`
- `ExtractedEventCandidate`
- `ProvisionalRelationshipCandidate`

The reconstruction engine consumes `RawTimelineEvent` structures to produce:
- Normalized, ordered timelines (`TimelineEntry`) with half-open intervals $[start, end)$.
- Entity spatial trajectories (`EntityTrajectory`) with Haversine distance and conservative speed bounds.
- Contradiction leads (`ContradictionLead`) grounded in physical and temporal impossibilities.

To bridge extraction outputs to reconstruction inputs seamlessly without data loss or manual mapping, the **Extraction-to-Reconstruction Adapter** (`backend-rust/src/reconstruction/adapters.rs`) serves as the canonical adapter.

---

## 2. Field-by-Field Mapping Specification

| Extraction Field (`ExtractedEventCandidate`) | Extraction Metadata / Context | Reconstruction Field (`RawTimelineEvent`) | Description & Transformation |
|:---|:---|:---|:---|
| `candidate_id` | — | `event_id` | Unique event identifier string. Deterministic UUID mapping preserved. |
| `source_evidence_id` | — | `source_evidence_id` | Exhibit UUID reference for strict cryptographic chain of custody. |
| `raw_timestamp` | — | `timestamp` | Raw timestamp string preserved verbatim (DEC-007). |
| `duration_seconds` | — | `duration_seconds` | Explicit duration in seconds ($\ge 0$). Kept separate from uncertainty (DEC-005). |
| `location_name` | — | `raw_description` | Textual description, tower name, or room identifier. |
| `involved_entity_raw_ids`| — | `entity_ids` | Raw phone numbers, handles, or canonical entity identifiers involved. |
| `metadata` | `metadata.coordinates` or `metadata.latitude/longitude` or cell tower lookup | `coordinates` | Validated WGS84 `GeoPoint` (`lat`, `lon`). |
| `metadata` | `metadata.timestamp_precision` or default | `timestamp_precision` | Canonical precision string: `EXACT_SECOND`, `MINUTE`, `HOUR`, `DAY`, `ESTIMATED`. |
| `metadata` | `metadata.uncertainty_window_start/end` | `uncertainty_window_start/end` | Explicit half-open bounds $[start, end)$ if documented by source. |
| Exhibit Context | `extraction_metadata.source_timezone` or candidate metadata | `source_timezone` | IANA timezone string (e.g. `Asia/Kolkata`, `UTC`). |
| Exhibit Context | `extraction_metadata.clock_offset_seconds` or candidate metadata | `clock_offset_seconds` | Additive drift offset in seconds (DEC-001). |

---

## 3. Contextual Propagation and Cell Tower Centroids

### 3.1 Exhibit Context Inheritance
In many forensic exhibits, clock offset and source timezone are properties of the recording system (e.g. cell carrier switch or DVR), rather than individual records. The adapter supports:
```rust
pub fn event_candidate_to_timeline_event_with_context(
    candidate: &ExtractedEventCandidate,
    default_precision: Option<&str>,
    fallback_timezone: Option<&str>,
    fallback_clock_offset: Option<f64>,
) -> RawTimelineEvent
```
If an individual event specifies an override, it takes precedence; otherwise, the parent exhibit's verified timezone and clock offset are inherited automatically.

### 3.2 Standard Cell Tower Centroid Resolution
When CDR events contain `cell_tower_id` but lack explicit GPS coordinates, the adapter can resolve known cell tower centroids (e.g. `TOWER-DELHI-SOUTH-04B` $\to 28.5244^\circ\text{N}, 77.2066^\circ\text{E}$), enabling immediate spatial trajectory and transit feasibility analysis without manual geocoding.
