# Reconstruction Integration Adapters (Member 4 — Phase 3E/3F)

**Status:** Implemented (uncommitted Member 4 work). Contract decisions DEC-001 … DEC-007 are `PROPOSED`. No verified team sign-off exists, so the gate fails closed (Phase 3D.2, 2026-11-20).
**Owner:** Member 4 (Forensic Intelligence & System Integration)
**Date:** 2026-11-19
**Decisions:** [`phase-3c-contract-decisions.md`](../member-4/phase-3c-contract-decisions.md) · approval record [`team-contract-approvals-2026-11-19.md`](../member-4/team-contract-approvals-2026-11-19.md)

This layer connects shared data contracts and Member 1 event candidates to the Member 4 engines
(`temporal.py`, `spatial.py`, `contradictions.py`, `video.py`). It returns a
`ReconstructionResult`, which serializes to a complete JSON snapshot. Nothing is written to a
database.

## 1. Layout

```text
backend/app/modules/reconstruction/
├── adapters/
│   ├── contracts.py             # DEC-001…007 gate; APPROVED_CONTRACT_DECISIONS = empty (fail closed)
│   ├── errors.py                # typed adapter errors with stable codes
│   ├── diagnostics.py           # structured, deterministically ordered diagnostics
│   ├── evidence_adapter.py      # canonical storage keys, URNs, SHA-256, DEC-006 metadata
│   ├── event_adapter.py         # Member 1 event candidates → engine inputs → ReconstructedEvent
│   ├── entity_adapter.py        # exact-match entity resolution (no fuzzy merging)
│   ├── relationship_adapter.py  # ContradictionLead → CONTRADICTS Relationship (weight null)
│   └── frame_handoff.py         # ExtractedFrameConsumer protocol + FrameArtifactReference
└── pipeline.py                  # ReconstructionPipeline → ReconstructionResult (snapshot)
```

## 2. Contract decisions (proposed, fail closed)

`contracts.APPROVED_CONTRACT_DECISIONS` is **empty**. The 2026-11-19 approval record is a copy of
a task brief, not a team sign-off (Phase 3D.1 audit), so the default pipeline reconstructs
nothing: every candidate is excluded with `CONTRACT_DECISION_PENDING`. A caller can enable
decisions explicitly through `ReconstructionPipelineConfig.approved_contract_decisions`.
`contracts.PROPOSED_CONTRACT_DECISIONS` holds the full proposal for tests and demos. Results list
any enabled decision that has no verified sign-off under `unverified_contract_decisions`, and the
report shows it as "proposal only" (`CONTRACT_DECISION_NOT_TEAM_APPROVED`). **Withholding a
decision never brings back the old assumptions.** The affected behaviour is refused, and a
diagnostic names the decision.

The table describes the behaviour *when a decision is enabled*. DEC-002 now gates every precision
token, including the canonical ones, because those tokens are the proposal itself.

| Decision | Implemented behaviour | When withheld |
| :--- | :--- | :--- |
| DEC-001 | `timestamp_utc = localize(raw).astimezone(UTC) + clock_offset_seconds`. A fast clock has a negative offset. | Candidates are rejected (`CONTRACT_DECISION_PENDING`). |
| DEC-002 | Precision values are `EXACT_SECOND`, `MINUTE`, `HOUR`, `DAY`, `ESTIMATED`. Calendar units are floored in local time before UTC conversion. Windows are `[start, end)` with `start <= nominal < end`. Legacy `EXACT` / `APPROXIMATE` are mapped and produce a `LEGACY_TIME_PRECISION_MAPPED` warning. | Every candidate is rejected (`CONTRACT_DECISION_PENDING`). |
| DEC-003 | Contradiction leads are exported as `CONTRADICTS` with `is_observed = false`. | `RELATIONSHIP_EXPORT_BLOCKED`. Leads are still listed. |
| DEC-004 | `weight = null`, never `0.0`. No numeric confidence anywhere in the record. | Same as DEC-003. |
| DEC-005 | Optional `duration_seconds` (finite, >= 0) is kept separate from the UTC window. `ReconstructedEvent` validates that both bounds are present or both absent, `start < end` and `start <= timestamp_utc < end`. Missing values stay null. The engine's explicit-bound check was tightened from `<=` to `<` to match. | Candidates that carry `duration_seconds` are rejected. |
| DEC-006 | `source_timezone`, `clock_offset_seconds`, `recording_start_timestamp` and `device_coordinates` are validated and exposed on the evidence record. | Metadata is ignored (`TIME_METADATA_NOT_APPLIED`). |
| DEC-007 | `raw_timestamp` is never overwritten. Only the temporal engine produces `timestamp_utc`. | Candidates are rejected, because no UTC value can be produced. |

## 3. Temporal guarantees (DEC-001 / DEC-007)

- **Exactly once.** The adapter and the engines always derive time from `raw_timestamp`, together
  with the evidence's validated `source_timezone` and `clock_offset_seconds`. They never derive it
  from an already-corrected value. After spatial reconstruction, the pipeline compares the
  engine's window with the adapter's window for each event. A difference fails the run with
  `TIME_CORRECTION_MISMATCH`.
- **Aware timestamps are not localized again.** A raw value with `Z` or `±HH:MM` keeps its
  instant (`timezone_basis = TIMESTAMP_UTC_OFFSET`). Only the declared clock offset is applied.
  A naive value is localized once with the declared `source_timezone`
  (`SOURCE_TIMEZONE_METADATA`).
- **Unknown timezone.** A naive raw value whose evidence declares no timezone is rejected with
  `SOURCE_TIMEZONE_UNKNOWN`. It is never assumed to be UTC.
- **Unverified offset.** `clock_calibration` is `DECLARED_NOT_INDEPENDENTLY_VERIFIED` when an
  offset was supplied, and `NOT_DECLARED_UNCALIBRATED` otherwise. In the second case
  `clock_offset_seconds` stays `null` and an INFO `CLOCK_OFFSET_NOT_DECLARED` is emitted. No
  value claims a verified calibration.
- **Re-running on own output.** Derived fields in an input candidate (`timestamp_utc`,
  `uncertainty_window_*`, `clock_offset_seconds`, `source_timezone`, …) are ignored, with
  `DERIVED_FIELDS_IGNORED`. `ReconstructionResult.to_event_candidates()` and
  `event_candidates_from_snapshot()` return only candidate-contract fields. Tests run the pipeline
  three times in a chain and get identical `timestamp_utc` values.

Every `ReconstructedEvent` (`timeline_events`) carries: the candidate fields unchanged,
`raw_timestamp`, `raw_timestamp_precision`, `timestamp_utc`, `timestamp_precision`,
`uncertainty_window_start/end` (UTC), `source_timezone`, `timezone_basis`,
`clock_offset_seconds`, `clock_calibration`, `duration_seconds`, resolved and unresolved entity
IDs, and the timeline ordering status.

## 4. Event candidates (Member 1 contract)

The candidate fields are `candidate_id`, `source_evidence_id`, `source_record_id`,
`raw_timestamp`, `timestamp_precision`, `event_type`, `description`,
`involved_entity_raw_ids`, `coordinates`, `duration_seconds` and `extraction_confidence`.

`ReconstructionPipeline.run(event_candidates=...)` accepts any of these:

- a list of candidate objects;
- JSON text of such a list;
- the `UnifiedExtractionResult` object or JSON text, which has an `event_candidates` array.

`NaN` / `Infinity` are rejected. Each candidate is validated individually, and all of its
structural problems are reported together in one `INVALID_EVENT_CANDIDATE` error. A candidate is
never dropped silently. Non-object items are reported by index. `event_id = candidate_id`.
`extraction_confidence` is kept as provenance only and is never used as a weight or as evidence
of identity. Fields outside the contract (for example `location_name`) are ignored with
`UNKNOWN_CANDIDATE_FIELDS_IGNORED`.

## 5. Evidence storage

- **Key:** `data/evidence/raw/{case_id}/{evidence_id}/{evidence_id}.raw`, resolved against
  `ReconstructionPipelineConfig.storage_root`. The key is derived from the record's IDs, which
  must be safe path segments.
- **Accepted `storage_uri`:** that exact key, or `urn:yukti:evidence:{evidence_id}:storage`.
  Any other value is rejected:
  - client-supplied paths (absolute, relative, `file://`, another item's key or URN) →
    `CLIENT_STORAGE_PATH_REJECTED`. The supplied value is never echoed.
  - remote URIs → `REMOTE_STORAGE_URI_REJECTED`, with the message "Remote evidence staging is
    not implemented" (Member 2).
- Every path component below the root is checked with `lstat`. Any symlink is rejected. The
  file is opened `O_RDONLY | O_NOFOLLOW`, and its SHA-256 and size are checked against the
  ingestion record.
- **No host paths in results.** Evidence records expose only `storage_key` and
  `storage_reference`. `VerifiedEvidenceFile.resolved_path` is excluded from serialization.
  Frame artifacts expose only a path relative to the artifact root, plus
  `urn:yukti:artifact:{artifact_id}`. Any path that appears in a diagnostic message is replaced
  with `<storage_root>` or `<artifact_root>`. Tests check that no form of either root appears in
  the snapshot JSON.

## 6. Video

`video.py` and the pipeline perform **frame extraction with provenance only**. FFmpeg samples
frames, and each frame is bound to the source SHA-256 and its PTS offset, and anchored to UTC
when DEC-006 metadata allows. No semantic event recognition is performed, and frames never
become events. `ExtractedFrameConsumer` stays a protocol with no default implementation.
Without a consumer, `frame_handoff_status = NOT_SUBMITTED_NO_CONSUMER`. A naive
`recording_start_timestamp` without a declared timezone is not anchored
(`FRAME_UTC_ANCHORING_WITHHELD`).

## 7. Snapshot (persistence readiness)

`ReconstructionResult.to_snapshot()` and `to_snapshot_json()` include:

- `snapshot_schema_version`, `algorithm_version` (`yukti-m4-reconstruction/3E.1`);
- `configuration_version` (SHA-256 of the path-free configuration) and `configuration`;
- `run_id`, `case_id`, `status`, and the approved and withheld decisions;
- `evidence`, `timeline_events`, `timeline_overlaps`, `spatial_observations`, `spatial`;
- `entity_resolution`, `relationships` (canonical), `contradiction_leads` (a separate record,
  linked by `relationship_id`, without the engine's legacy relationship DTO), and
  `contradiction_overlaps_not_evaluated`;
- `video` (with `frame_artifacts` references), `diagnostics` and `interpretation_notice`.

`run_id` defaults to a digest of the algorithm version, the configuration and the inputs, so it
does not depend on input order. Member 2 can pass its own `run_id`. The snapshot round-trips
through `ReconstructionResult.model_validate_json`.

## 7a. Phase 3F additions

- **Partial failure.** An invalid evidence item, candidate or supplied frame is excluded and
  reported in diagnostics, `excluded_inputs`, `evidence_gaps` and an `EXCLUDED_INPUTS`
  limitation. The run continues as `COMPLETED_WITH_ERRORS`. Duplicate IDs exclude every copy.
  `FAILED` is reserved for run-level problems.
- **Supplied frame artifacts** (`adapters/frame_artifacts.py`). A frame must be bound to
  verified CCTV evidence by SHA-256, carry no host path, and get its UTC recomputed once from
  evidence metadata. Frames link to events by temporal alignment only.
- **Findings** (`findings.py`). `finding_provenance` covers every event, spatial observation,
  movement segment, lead and relationship, with evidence SHA-256, source records and frames.
  `limitations` and `evidence_gaps` are also built here.
- **Clock skew.** Each applied non-zero offset produces an INFO `CLOCK_OFFSET_APPLIED`.
- **Determinism.** `result_sha256`. The sort rules are listed in
  [`docs/api/reconstruction-api.md`](../api/reconstruction-api.md).
- **API (Phase 3G).** `api/routes/reconstruction.py` uses `service.py`, which uses
  `run_store.py` (in-memory; PostgreSQL is a Member 2 TODO).

## 7b. Phase 3H/3I additions

- **Integration scenarios** live in `backend/tests/integration/reconstruction/` (happy path,
  clock correction, evidence integrity, malformed Member 1 JSON, partial failure, sparse
  evidence, determinism, video, API contract) plus a 5,000-event performance check. They import
  the synthetic burglary fixtures from `tests/reconstruction/adapters/synthetic_burglary.py`.
- **Sparse-evidence limitations.** `build_limitations` now states `NO_ACCEPTED_EVENTS`,
  `SINGLE_SOURCE_NO_CORROBORATION`, `NO_SPATIAL_DATA` and `NO_MOVEMENT_SEGMENTS`, and a `FAILED`
  run records `RUN_FAILED_NO_RECONSTRUCTION`. They are added only in those situations, so the
  burglary `result_sha256` is unchanged.
- **Forensic report** (`backend/app/modules/reporting/`, Member 3 and 4 module). It is a pure
  function of a stored `ReconstructionResult`: `build_forensic_report` builds the structured
  `ForensicReport`; `report_render` renders the same object as Markdown or print-ready HTML.
  `ReconstructionService.report` injects the generation time (a clock dependency, fixed in
  tests). It has no new dependencies; PDF comes from the browser's print dialog. API:
  [`reconstruction-api.md`](../api/reconstruction-api.md#report-get-runsrun_idreport).
- **Request limit fix.** A bare `event_candidates` array is now capped at 10,000 items, like the
  `UnifiedExtractionResult` form already was.

## 8. Open dependencies

**Member 1:**
- The real `UnifiedExtractionResult` producer. This adapter follows the field list in the
  Phase 3E brief, because no Rust source is present in this repository.
- An `ExtractedFrameConsumer` implementation, and semantic analysis of video frames.
- Confirmation that `source_record_id` / `extraction_confidence` are serialized as `null` (not
  omitted) when absent. Both forms are accepted today.

**Member 2:**
- Persisting the snapshot, including a nullable `weight` column.
- Staging remote evidence to the canonical storage key. Remote URIs are rejected until then.
- Populating DEC-006 metadata at ingestion.

**Member 3:**
- Rendering `weight: null` as "not computed" (never 0%).
- Showing `clock_calibration` / `timezone_basis` and the diagnostics.
- Updating `apps/web` types for the new precision values.

## 9. Remaining gaps

- **`CALL_MADE`.** The approved DEC-003 adds `CONTRADICTS` and `CO_OCCURRED_WITH`. It does not
  settle `CALL_MADE`, which stays in the schema table unchanged.
- **EntityType.** `EMAIL` vs `DEVICE` (Member 3 P-03) is unresolved. The entity adapter keeps
  `entity_type` as given.
- **Date-only raw timestamps.** These are rejected with a clear message. Supply local midnight
  with precision `DAY`.


## Phase 3D.2 remediation (2026-11-20)

- **Fail-closed gate.** See §2. `ReconstructionResult.unverified_contract_decisions` was added
  (snapshot schema `yukti-reconstruction-snapshot/3`, algorithm `yukti-m4-reconstruction/3F.2`).
- **Producer contract gaps.** A candidate that lacks `timestamp_precision` or `description`
  entirely is excluded with `PRODUCER_CONTRACT_GAP`. Neither field is ever inferred.
  Candidate fields outside the contract (Member 1 `location_name`, `metadata`, …) are kept
  verbatim in `ReconstructedEvent.source_fields_preserved` and restored by
  `event_candidates_from_snapshot`. Pydantic payloads (the real `UnifiedExtractionResult`) are
  accepted. See [`member1-candidate-contract-gap.md`](../member-4/member1-candidate-contract-gap.md).
- **Member 2 clock defaults.** On Member 2 ingestion records, `source_timezone == "UTC"` and
  `clock_offset_seconds == 0.0` are treated as undeclared
  (`CLOCK_METADATA_INGESTION_DEFAULT_IGNORED`). This is an interim safeguard; see
  [`member2-ingestion-clock-defaults-patch.md`](../member-4/member2-ingestion-clock-defaults-patch.md).
- **Collision-safe lead IDs.** `lead_id = "lead-" + sha256(json([scheme, case, entity, origin,
  destination]))` (full digest). The engine relationship ID and the exported Relationship ID are
  now the same value (`rel-contra-` + 32 hex). The pipeline refuses a duplicate `lead_id`
  instead of overwriting.
