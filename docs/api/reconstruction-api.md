# Reconstruction API (Member 4 — Phase 3F/3G, report: Phase 3I)

**Owner:** Member 4 · **Router:** `backend/app/api/routes/reconstruction.py` · **Machine-readable schema:** [`openapi.json`](openapi.json)
**Contracts:** DEC-001 … DEC-007, `PROPOSED` and not team-approved; the production service fails closed ([approval status](../member-4/team-contract-approvals-2026-11-19.md)) · **Pipeline design:** [`reconstruction-integration-adapters.md`](../architecture/reconstruction-integration-adapters.md)

The backend has no global API prefix (`/health` is served at the root), so these routes are
mounted at `/cases/{case_id}/reconstruction`. `case_id` and `run_id` must match
`^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$`.

Regenerate the OpenAPI file after any API change (a test fails when it is stale):

```bash
cd backend && uv run python -m app.api.openapi_export
```

## Endpoints

| Method | Path | Response | Purpose |
| :--- | :--- | :--- | :--- |
| POST | `/cases/{case_id}/reconstruction/run` | `201 ReconstructionRunSummary` | Run the end-to-end pipeline and store the result |
| GET | `/cases/{case_id}/reconstruction/runs/{run_id}` | `ReconstructionResult` | Full result (complete snapshot) |
| GET | `…/runs/{run_id}/timeline` | `TimelineView` | Timeline events with raw and corrected UTC, plus overlaps |
| GET | `…/runs/{run_id}/spatial` | `SpatialView` | Spatial observations, movement segments (travel feasibility), uncertain overlaps |
| GET | `…/runs/{run_id}/contradictions` | `ContradictionsView` | Contradiction leads, plus pairs that could not be evaluated |
| GET | `…/runs/{run_id}/relationships` | `RelationshipsView` | Canonical relationships and `relationship_export_status` |
| GET | `…/runs/{run_id}/diagnostics` | `DiagnosticsView` | Diagnostics, limitations, evidence gaps, excluded inputs, video statuses |
| GET | `…/runs/{run_id}/report?format=json\|markdown\|html` | `ForensicReport` / text | Evidence-linked investigation report (see [Report](#report-get-runsrun_idreport)) |

Each GET view also returns `run_id`, `case_id`, `status` and `result_sha256` (the report
carries them under `metadata`, with the status as `run_status`).

## POST request

```json
{
  "evidence": [
    {
      "evidence_id": "ev-cctv-dock",
      "case_id": "case-wh-burglary-001",
      "source_type": "CCTV",
      "original_filename": "dock_cam_01.mp4",
      "file_size_bytes": 32,
      "sha256_hash": "e77a6607dd66c74a6f1bca53e372ce4ba9d88b77466d3ca479ca7fefee338890",
      "storage_uri": "urn:yukti:evidence:ev-cctv-dock:storage",
      "metadata": {
        "source_timezone": "Asia/Kolkata",
        "clock_offset_seconds": -120.0,
        "recording_start_timestamp": "2026-11-10T21:30:00",
        "device_coordinates": {"lat": 28.5355, "lon": 77.271}
      }
    }
  ],
  "event_candidates": [
    {
      "candidate_id": "evt-01-shutter-forced",
      "source_evidence_id": "ev-cctv-dock",
      "source_record_id": "osd:2026-11-10T21:42:00",
      "raw_timestamp": "2026-11-10T21:42:00",
      "timestamp_precision": "EXACT_SECOND",
      "event_type": "FORCED_ENTRY",
      "description": "Rear shutter forced open at loading dock",
      "involved_entity_raw_ids": ["ent-person-ravi"],
      "coordinates": {"lat": 28.5355, "lon": 77.271},
      "duration_seconds": 30.0,
      "extraction_confidence": 0.9
    }
  ],
  "entities": [
    {"entity_id": "ent-person-ravi", "case_id": "case-wh-burglary-001", "entity_type": "PERSON",
     "canonical_name": "Ravi Kumar", "aliases": ["R. Kumar"], "source_evidence_ids": ["ev-cctv-dock"],
     "confidence": 1.0}
  ],
  "frame_artifacts": []
}
```

- **`evidence`** (required, 1–10,000 items). `storage_uri` must be the canonical key
  `data/evidence/raw/{case_id}/{evidence_id}/{evidence_id}.raw` or the reference
  `urn:yukti:evidence:{evidence_id}:storage`. Client paths and remote URIs are rejected **per
  item**.
- **`event_candidates`** (required). Either an array of Member 1 candidates, or a
  `UnifiedExtractionResult` object with an `event_candidates` array. Both forms accept at most
  10,000 candidates; more gives `422`.
- **`entities`** (optional). Shared Entity records. References resolve only by exact ID or
  registered alias.
- **`frame_artifacts`** (optional). Previously extracted frames: `artifact_id`,
  `source_evidence_id`, `source_video_sha256`, `frame_index`, `pts_offset_seconds`,
  `sha256_hash`, and optionally `artifact_relative_path`. A `file_path` or absolute path
  excludes the frame. Supplied `timestamp_utc` values are ignored and recomputed once.
- Unknown top-level fields → `422`.

**Validation levels.** The request structure is validated up front: arrays of JSON objects,
size limits, no unknown fields. A structural error gives `422`. Each evidence item, candidate
and frame is then validated individually. **A bad item is excluded, reported, and the run
continues** with `status = COMPLETED_WITH_ERRORS`. A candidate whose evidence was excluded is
also excluded (`SOURCE_EVIDENCE_EXCLUDED`).

## Response examples (synthetic warehouse burglary)

**POST → 201**

```json
{
  "run_id": "run-60683a524b3c51fed628898a849ba64b",
  "case_id": "case-wh-burglary-001",
  "status": "COMPLETED_WITH_WARNINGS",
  "result_sha256": "c21046826e0a4a09e1e2d8c5aa718157c28d1d871f09fb73347a945cf53b4539",
  "algorithm_version": "yukti-m4-reconstruction/3F.1",
  "configuration_version": "cfg-sha256-c28c593bd11e484064ea5fe9c8e6086c",
  "relationship_export_status": "EXPORTED",
  "counts": {
    "timeline_events": 5, "movement_segments": 1, "contradiction_leads": 1,
    "relationships": 1, "excluded_inputs": 0,
    "diagnostics_by_severity": {"INFO": 2, "WARNING": 2}
  },
  "links": {
    "result": "/cases/case-wh-burglary-001/reconstruction/runs/run-60683a524b3c51fed628898a849ba64b",
    "timeline": "/cases/case-wh-burglary-001/reconstruction/runs/run-60683a524b3c51fed628898a849ba64b/timeline",
    "spatial": "…/spatial", "contradictions": "…/contradictions",
    "relationships": "…/relationships", "diagnostics": "…/diagnostics", "report": "…/report"
  }
}
```

**GET …/timeline → `timeline_events[0]`.** This is the clock-skew case: the DVR runs 120 s
fast. The raw value is preserved, and the correction is applied once.

```json
{
  "candidate_id": "evt-01-shutter-forced",
  "event_id": "evt-01-shutter-forced",
  "source_evidence_id": "ev-cctv-dock",
  "source_record_id": "osd:2026-11-10T21:42:00",
  "raw_timestamp": "2026-11-10T21:42:00",
  "timestamp_utc": "2026-11-10T16:10:00Z",
  "timestamp_precision": "EXACT_SECOND",
  "uncertainty_window_start": "2026-11-10T16:10:00Z",
  "uncertainty_window_end": "2026-11-10T16:10:01Z",
  "source_timezone": "Asia/Kolkata",
  "timezone_basis": "SOURCE_TIMEZONE_METADATA",
  "clock_offset_seconds": -120.0,
  "clock_calibration": "DECLARED_NOT_INDEPENDENTLY_VERIFIED",
  "duration_seconds": 30.0,
  "involved_entity_ids": ["ent-person-ravi"],
  "ordering_status": "STRICTLY_ORDERED",
  "evidence_sha256": "e77a6607dd66c74a6f1bca53e372ce4ba9d88b77466d3ca479ca7fefee338890",
  "supporting_frame_artifact_ids": []
}
```

**GET …/relationships → `relationships[0]`.** Impossible travel: 25.3 km in at most 300 s,
which needs at least 304 km/h against the 180 km/h reference.

```json
{
  "relationship_id": "rel-contra-6d7b1756a205fd04a7d537f3869d2351",
  "source_id": "evt-01-shutter-forced",
  "target_id": "evt-03-location-shared",
  "relation_type": "CONTRADICTS",
  "weight": null,
  "is_observed": false,
  "evidence_basis": ["ev-cctv-dock", "ev-chat-export"],
  "metadata": {
    "record_kind": "INVESTIGATIVE_LEAD",
    "observation_status": "INFERRED",
    "contradiction_type": "SPEED_THRESHOLD_EXCEEDANCE_LEAD",
    "supporting_evidence": {
      "evidence_ids": ["ev-cctv-dock", "ev-chat-export"],
      "event_ids": ["evt-01-shutter-forced", "evt-03-location-shared"]
    },
    "provenance": {"origin_event": {"source_record_id": "osd:2026-11-10T21:42:00", "raw_timestamp": "2026-11-10T21:42:00", "…": "…"}, "…": "…"},
    "derived_inference": {"distance_km": 25.33, "min_required_speed_kmh": 304.01, "configured_speed_threshold_kmh": 180.0, "threshold_evaluation": "EXCEEDS_CONFIGURED_THRESHOLD", "…": "…"},
    "weight_semantics": "NOT_COMPUTED",
    "interpretation_notice": "Algorithmically derived investigative lead for human review. … It is not proof of identity, deception, guilt or intent. …"
  }
}
```

## Errors

Every error from these routes uses the standard envelope:

```json
{"error": {"code": "RUN_NOT_FOUND", "message": "No reconstruction run 'nope' exists for case 'case-wh-burglary-001'.", "details": null}}
```

| HTTP | `code` | When |
| :--- | :--- | :--- |
| 404 | `RUN_NOT_FOUND` | Unknown run, or a run that belongs to another case |
| 422 | `REQUEST_VALIDATION_FAILED` | Bad JSON, bad structure, invalid `case_id` / `run_id`, unknown report `format`. `details` lists `loc`, `msg` and `type` only; submitted values are never echoed |
| 422 | `RECONSTRUCTION_INPUT_REJECTED` | Inputs passed the schema but cannot be processed at all |
| 500 | `INTERNAL_SERVER_ERROR` | Unexpected failure. `details` is always `null` |

## Status values (returned verbatim)

| Field | Values |
| :--- | :--- |
| `status` | `COMPLETED`, `COMPLETED_WITH_WARNINGS`, `COMPLETED_WITH_ERRORS` (some items excluded or video failed), `FAILED` (unreadable candidate payload, invalid entity registry, engine failure, or double-correction guard) |
| `relationship_export_status` | `EXPORTED`, `NO_CONTRADICTION_LEADS`, `RELATIONSHIP_EXPORT_BLOCKED` (a contract decision is withheld), `NOT_RUN` |
| `video[].status` | `EXTRACTED`, `FAILED`, `SKIPPED_NO_ARTIFACT_ROOT` |
| `video[].frame_handoff_status` | `SUBMITTED`, `NOT_SUBMITTED_NO_CONSUMER`, `NOT_SUBMITTED_NO_FRAMES`, `FAILED` |
| `timeline_events[].clock_calibration` | `DECLARED_NOT_INDEPENDENTLY_VERIFIED`, `NOT_DECLARED_UNCALIBRATED`, `NOT_APPLIED_DECISION_WITHHELD` |
| `timeline_events[].timezone_basis` | `TIMESTAMP_UTC_OFFSET`, `SOURCE_TIMEZONE_METADATA` |
| `diagnostics[].severity` | `ERROR`, `WARNING`, `INFO` |

Frequent diagnostic codes:

- **Evidence:** `EVIDENCE_INTEGRITY_MISMATCH`, `CLIENT_STORAGE_PATH_REJECTED`, `REMOTE_STORAGE_URI_REJECTED`
- **Candidates:** `INVALID_EVENT_CANDIDATE`, `SOURCE_TIMEZONE_UNKNOWN`, `SOURCE_EVIDENCE_EXCLUDED`, `DERIVED_FIELDS_IGNORED`
- **Clocks:** `CLOCK_OFFSET_APPLIED`, `CLOCK_OFFSET_NOT_DECLARED`
- **Entities:** `ENTITY_REFERENCE_UNRESOLVED`
- **Relationships:** `RELATIONSHIP_EXPORT_BLOCKED`
- **Video:** `VIDEO_STAGE_SKIPPED`, `FRAME_CONSUMER_UNAVAILABLE`, `INVALID_FRAME_ARTIFACT`

Limitation codes (`limitations[].code`) state what the run could not analyse instead of
guessing. Besides the assumption codes (`REFERENCE_SPEED_THRESHOLD_ASSUMPTION`,
`ESTIMATED_WINDOW_ASSUMPTION`, `UNCALIBRATED_CLOCKS`, `DECLARED_CLOCK_OFFSETS_NOT_VERIFIED`,
`EXCLUDED_INPUTS`, `UNRESOLVED_ENTITY_REFERENCES`, `CONCURRENT_WINDOWS_NOT_ORDERED`,
`FRAMES_NOT_SEMANTICALLY_ANALYZED`, `FRAMES_NOT_UTC_ANCHORED`), Phase 3H added the sparse-evidence
codes:

| Code | When |
| :--- | :--- |
| `NO_ACCEPTED_EVENTS` | No candidate was accepted; nothing could be reconstructed |
| `SINGLE_SOURCE_NO_CORROBORATION` | All accepted events come from one evidence item |
| `NO_SPATIAL_DATA` | No accepted event has coordinates (device coordinates are never used to place events) |
| `NO_MOVEMENT_SEGMENTS` | Events have coordinates, but no entity has two strictly ordered located events |
| `RUN_FAILED_NO_RECONSTRUCTION` | The run is `FAILED`; no findings were produced |

## Report: GET …/runs/{run_id}/report

Decision-support report for investigators, generated from the stored run (it never re-runs the
pipeline). Builder: `backend/app/modules/reporting/forensic_report.py`; renderers:
`backend/app/modules/reporting/report_render.py`. Sample: [`docs/demo/`](../demo/README.md).

| `format` | Content type | Notes |
| :--- | :--- | :--- |
| `json` (default) | `application/json` | Structured `ForensicReport` (schema in `openapi.json`) |
| `markdown` | `text/markdown; charset=utf-8` | Same content as Markdown |
| `html` | `text/html; charset=utf-8` | Self-contained and print-ready (A4 page breaks). Use the browser's *Print → Save as PDF*; no PDF library is bundled |

Response headers: `Content-Disposition: inline; filename="report-{run_id}.{json|md|html}"`,
`Cache-Control: no-store`, `X-Content-Type-Options: nosniff`, `X-Report-Content-SHA256`; the
HTML also carries `Content-Security-Policy: default-src 'none'; style-src 'unsafe-inline'`.
Errors use the standard envelope (`404 RUN_NOT_FOUND`, `422` for an unknown `format`).

**Sections.** 1 case summary and run metadata (case id, run id, status, algorithm and
configuration version, `result_sha256`, generation time in UTC); 2 evidence inventory with
SHA-256 and integrity status (`VERIFIED`, `SHA256_MISMATCH`, `FILE_UNAVAILABLE`,
`STORAGE_REFERENCE_REJECTED`, `RECORD_INVALID`, `DUPLICATE_EVIDENCE_ID`, `NOT_VERIFIED`);
3 timeline (raw timestamp, corrected UTC, precision, uncertainty window, time basis, clock
calibration, source evidence and frames); 4 spatial observations and movement segments with
travel feasibility; 5 contradictions; 6 relationships; 7 limitations, evidence gaps, clock
calibrations (none is independently verified), pending items, excluded inputs, errors and
warnings; Appendix A chain of custody and provenance (evidence hashes and storage references,
video hashes before/after processing, frame artifacts by URN, `finding_provenance`, public
configuration).

**Rules the report enforces (and tests check):**

- Every finding (timeline event, spatial observation, movement segment, contradiction lead,
  relationship, summary lead) has `citations`: evidence id, evidence SHA-256, events, source
  records and frames captured during the window.
- `basis` is `OBSERVED` or `INFERRED`. Movement segments, leads and relationships are always
  `INFERRED`; leads are classified `ANALYTICAL_LEAD_NOT_A_FINDING_OF_FACT`.
- `weight: null` stays `null`; `weight_display` is `"Not provided"`, never `0%`.
- Entities appear by registry id only. Nothing in the report claims guilt or identity, and a
  disclaimer opens and closes every format.
- `limitations` always contains `REPORT_SCOPE` plus the run's own limitations.
- HTML escapes every value and contains no scripts; Markdown puts ids in code spans and
  escapes free text, so evidence text cannot inject markup, links or table cells.
- No host paths: storage appears as the canonical key or URN, frames by `urn:yukti:artifact:…`.
- `generated_at_utc` is the only time-dependent value. `content_sha256` covers everything else,
  so the same run always gives the same `content_sha256`.

## Determinism

The same inputs and configuration always give the same `run_id`, the same ordering and the
same `result_sha256`. Input order does not matter. Re-posting stores an identical result under
the same `run_id`. Stable sort rules:

| List | Order |
| :--- | :--- |
| `timeline_events` | window start, then window end, then nominal UTC, then `event_id` |
| `evidence` | `evidence_id` |
| `excluded_inputs` | subject type, then subject id |
| `movement_segments` | entity id, then origin window start, then `segment_id` |
| `spatial_observations` | entity id, then that entity's located-event order |
| `contradiction_leads` | origin nominal UTC, then `lead_id` |
| `relationships` | `relationship_id` |
| `frame_artifacts` | source evidence, then PTS offset, then `artifact_id` |
| `finding_provenance` | finding type (event, observation, segment, lead, relationship), then id |
| `evidence_gaps` | gap type, then subject type, subject id, description |
| `diagnostics` | stage, severity (ERROR, WARNING, INFO), subject type, subject id, code, message |

`run_id` is a digest of the algorithm version, the configuration version and the
order-normalized inputs. `result_sha256` is the SHA-256 of the canonical snapshot, excluding
itself and `artifact_relative_path` of frames extracted in the run. Derived artifacts are never
overwritten, so each extraction writes to a new directory.

## Notes for Member 3 (rendering)

- **Relationships source.** Draw graph edges from `relationships` (GET …/relationships, or the
  full result) only. `contradiction_leads` hold the explanation and the numbers for a lead
  panel. They link to their edge through `relationship_id`, which is `null` when export was
  blocked. The engine's internal relationship DTO is not part of the API.
- **`weight: null`.** Means "not computed". Never show it as 0% or as a thin edge. For
  `CONTRADICTS`, use an alert style (for example a red dashed edge) and read the tooltip from
  `metadata.derived_inference`, e.g. "≥ 304 km/h needed, reference 180 km/h".
- **Inferred vs observed.** `is_observed: false` (relationships, leads, movement segments) must
  look different from extracted facts. `finding_provenance[]` lists the supporting evidence (with
  SHA-256), source records and frames for each finding.
- **Blocked and partial states.** Show `relationship_export_status`, `frame_handoff_status` and
  `status` verbatim. `RELATIONSHIP_EXPORT_BLOCKED` means leads exist but no edges were exported.
  `NOT_SUBMITTED_NO_CONSUMER` means frames were extracted but not analysed. For
  `COMPLETED_WITH_ERRORS`, surface `excluded_inputs` and `evidence_gaps`.
- **Time.** Show `timestamp_utc` with its window `[uncertainty_window_start, uncertainty_window_end)`.
  Keep `raw_timestamp` visible. Flag `clock_calibration = NOT_DECLARED_UNCALIBRATED` and
  `is_estimated_window_assumption = true`. Events with `ordering_status = CONCURRENT_UNCERTAIN`
  have no known order between them.
- **Frames.** `supporting_frame_artifact_ids` are temporal alignment only. Label them
  "captured during this window", never "shows".

## Storage and configuration

- Runs are kept by `ReconstructionRunStore`. Today that is `InMemoryRunStore`, which is lost on
  restart. **TODO(Member 2):** a PostgreSQL implementation; see
  `backend/app/modules/reconstruction/run_store.py`.
- Environment variables:
  - `YUKTI_RECONSTRUCTION_STORAGE_ROOT`: evidence key root (default: repository root).
  - `YUKTI_RECONSTRUCTION_ARTIFACT_ROOT`: frame output directory. Unset means frame extraction
    is skipped.
  - `YUKTI_RECONSTRUCTION_MAX_STORED_RUNS`: capacity of the in-memory store.
