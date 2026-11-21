# YUKTI — USP & Gap Analysis Audit

**Audit date:** 2026-11-21
**Scope:** whole repository at `main` @ `8053b09`, plus one uncommitted working-tree change (`backend-rust/src/app.rs`: adds `CorsLayer::permissive()`), which was present when the Rust tests were compiled.
**Mode:** read-only. The only file this audit writes is this report. Nothing was staged, committed, pushed or deleted. Stashes `stash@{0}` and `stash@{1}` are untouched.
**Method:** source code first, tests second, documentation last. DTO fields, fixtures, placeholder UI and docs do **not** count as proof that a feature works.

---

## 1. Executive summary

YUKTI has a **strong deterministic reconstruction core**. Time is handled as half-open uncertainty windows, clock offsets are applied once, DST-ambiguous times are rejected instead of guessed, contradictions are tested for existential feasibility, and assumptions are labelled with an export gate. That core is real and tested in Rust (`backend-rust/src/reconstruction/`). It is implemented again, with more depth, in the Python reference (`backend/app/modules/reconstruction/`).

Most of the proposed USPs are **not finished yet**. The pattern is the same each time:

- **The Python reference is ahead of Rust.** Finding-level provenance, evidence gaps, structured limitations, clock-calibration status and `result_sha256` all exist in Python and are tested there. None of them were ported to the Rust backend, which is what the frontend calls.
- **The frontend throws away the backend's honesty signals.** `assumptions` and `export_safeguard` are not in the frontend DTOs. Every live event is hard-coded `provenance: 'observed'`. The tower-centroid warning checks a metadata key (`location_synthetic_centroid`) that no backend sets.
- **The workspace mostly shows fixtures.** By default, `syntheticRepository` serves hand-written case data. Live reconstruction appears only for a case ID that isn't in the fixtures, and the case list is always the fixture list.
- **Nothing exists yet for sensitivity analysis, alternative clock scenarios, what-if comparison, gap recommendations or Hindi-English processing.**

### Overall readiness: **3.7 / 10**

| # | USP | Classification | Score |
|---|-----|----------------|------:|
| 1 | Uncertainty-Aware Crime Reconstruction | PARTIALLY_IMPLEMENTED | **5.5** |
| 2 | Evidence-to-Claim Traceability | PARTIALLY_IMPLEMENTED | **5.0** |
| 3 | Evidence Gap Intelligence | IMPLEMENTED_BUT_NOT_INTEGRATED (Python only) → FOUNDATION_EXISTS_NEAR_USP for the live stack | **3.0** |
| 4 | Investigation What-If Simulator | FOUNDATION_EXISTS_NEAR_USP | **2.5** |
| 5 | India-Focused Digital Evidence Intelligence | FOUNDATION_EXISTS_NEAR_USP | **2.5** |

Scores weight working backend logic about 40%, API and persistence integration 20%, frontend integration 20% and executed tests 20%.

---

## 2. Test execution record (actually run in this audit)

| Suite | Command | Result |
|-------|---------|--------|
| Rust (`backend-rust`) | `cargo test --offline` | **129 passed, 0 failed** (lib 20, extraction 26, graph 10, health 3, ingestion 13, integration_runs 32, persistence 3, reconstruction_real_producers 8, reconstruction_tests 14) |
| Python (`backend`) | `pytest -q` | **582 passed, 5 failed**. All 5 failures are symlink tests that fail during setup at `Path.symlink_to` because Windows does not grant symlink privilege by default. This is an environment problem, not a product defect, and should be re-run on Linux or in Windows Developer Mode. |
| Web (`apps/web`) | `npx vitest run` | **264 passed, 0 failed** (20 files) |
| Decision service | `npm test` | **2 passed** (health only) |

**Not verified:**
- **No live PostgreSQL or Neo4j connection was made.** `persistence_tests.rs` only checks configuration defaults, `graph_tests.rs` uses generated Cypher and mocks, and no test connects to a database. The `reconstruction_repository.rs` SQL paths have **never been executed** by any test in this repo.
- Almost every workspace UI test renders the **synthetic fixture repository**. The live adapters (`adaptTimelineEntriesToWorkspaceEvents`, `adaptContradictionLeadsToReviewItems`) are unit-tested once each in `apps/web/src/services/reconstruction.test.ts:38,78`. `hybridRepository` has no test and no caller.

---

## 3. USP-by-USP findings

### USP 1 — Uncertainty-Aware Crime Reconstruction · PARTIALLY_IMPLEMENTED · 5.5/10

**What exists and is tested**

| Capability | Evidence |
|---|---|
| Five precision levels converted into half-open windows `[start, end)`, floored in local time | `backend-rust/src/reconstruction/temporal.rs:139` `compute_uncertainty_window`; tests `reconstruction_tests.rs:46,126,387` |
| Clock offset applied additively once (DEC-001) | `temporal.rs:62` `parse_and_normalize_timestamp` (offset at `:125`); `reconstruction_real_producers.rs:198` `declared_clock_offset_is_applied_exactly_once` |
| DST fold and gap **rejected** rather than guessed | `temporal.rs:106-121` |
| IANA timezone validation | `temporal.rs:21` `validate_timezone` |
| Overlapping windows marked `CONCURRENT_UNCERTAIN` | `temporal.rs:432-471`; test `reconstruction_tests.rs:334` |
| Existential feasibility: a lead is raised only if **no** pair of instants in the two windows allows the transit | `contradictions.rs:31` `check_temporal_activity_feasibility`; `spatial.rs:58` `calculate_transit_metrics` (`max_elapsed = B.end − A.start`, `:84`) |
| Every non-observed value labelled as a typed `ReconstructionAssumption` (`PRECISION_DEFAULTED`, `TIMEZONE_NOT_DECLARED_ON_EVENT`, `CLOCK_OFFSET_NOT_DECLARED_ON_EVENT`, `COORDINATES_INFERRED_FROM_TOWER_ID`) | `model.rs:95-118`; `temporal.rs:341-361`; `adapters.rs:35-180`; tests `reconstruction_real_producers.rs:105,288,317` |
| Leads that rest on assumptions get an extra limitation | `service.rs:136` `annotate_leads_with_assumptions` |
| Judicial export gate (`CLEAR`/`BLOCKED` with reasons) | `service.rs:82` `evaluate_export_safeguard`; `report.rs:138-169`; tests `reconstruction_real_producers.rs:288,400` |
| Python reference: `ClockCalibrationStatus` (`DECLARED_NOT_INDEPENDENTLY_VERIFIED`, `NOT_DECLARED_UNCALIBRATED`) plus a double-correction guard | `backend/app/modules/reconstruction/adapters/evidence_adapter.py:148-203`; `tests/integration/reconstruction/test_scenario_b_clock_correction.py` (8 tests) |

**Missing (these are what would make it a USP)**

- **No alternative clock-offset scenarios.** Each run takes one global `default_clock_offset_seconds` (`service.rs:27` `ReconstructionOptions`). It applies only to events with no declared offset, and there is no per-evidence override.
- **No contradiction sensitivity analysis.** Nothing computes how much clock error or location error would make a lead disappear.
- **No robust / assumption-sensitive / inconclusive classification.** `ContradictionLead` (`model.rs:300-311`) has no such field. The `contradictions.severity` column is hard-coded to `'HIGH'` (`persistence/reconstruction_repository.rs:170`), which contradicts the "no fabricated weights" principle in DEC-004.
- **The frontend drops the uncertainty labels.** `apps/web/src/services/reconstruction.ts` has no `assumptions` or `export_safeguard` in its DTOs. The adapter at `:352` sets `provenance: 'observed'` for every live event, including events placed by a tower centroid. The centroid warning at `:323` and `features/workspace/data/repository.ts:78` checks `metadata.location_synthetic_centroid`, which **no backend produces** (searched across Rust, Python and TS), so live data never shows the warning.
- **What the UI does show:** uncertainty windows drawn as hatched bands (`features/workspace/pages/EventReconstruction.tsx:59-140`). That part is real.
- **Mislabelled field:** `ObservedLeadBasis.origin_timestamp_raw` / `destination_timestamp_raw` are filled with the **corrected** `nominal_utc` (`contradictions.rs:154-159, 313-314`), not the source string. The label says "raw" but the value isn't.

**Reusable:** `check_temporal_activity_feasibility`, `calculate_transit_metrics`, `ReconstructionAssumption`, `ExportSafeguard`, and Python `ClockCalibrationStatus`.

**Complexity to reach USP:** MEDIUM (see §8 for the design).

---

### USP 2 — Evidence-to-Claim Traceability · PARTIALLY_IMPLEMENTED · 5.0/10

**What exists**

| Link in the chain | Evidence |
|---|---|
| SHA-256 at ingestion; raw store separate from derived store | `backend-rust/src/ingestion/hashing.rs`, `ingestion/storage.rs`; tests `tests/ingestion.rs` (13) |
| Versioned extraction runs with `configuration_hash`, `result_sha256` and one active run per evidence | migration `20261118000001_initial_forensic_schema.sql:49-82`; `extraction/service.rs:438-566` |
| Events persisted with `extraction_run_id`, `evidence_id`, `source_record_id`, `is_observed` | migration `:119-144`; `persistence/extraction_run_repository.rs:333` |
| Character-offset and column locations for identifiers | `extraction/identifiers.rs:104,143`; Unicode parity test `tests/extraction.rs:516` |
| Append-only, hash-chained custody ledger | migration `:187-199`; `persistence/custody_repository.rs` |
| Neo4j provenance edges: `Evidence-[:PRODUCED_EXTRACTION_RUN]->ExtractionRun-[:PRODUCED_ASSERTION]->RelationshipAssertion-[:SUPPORTED_BY_EVIDENCE]->Evidence`, with `is_observed` on assertions | `graph/projection.rs:141-146, 289-308`. Cypher generation is tested; **never run against Neo4j**. |
| Contradiction lead carries the evidence IDs for both events | `model.rs:272-285` `ObservedLeadBasis` |
| UI click-through from event, lead and place to the evidence item | `EventReconstruction.tsx:196`, `ContradictionReview.tsx:95`, `SpatialReconstruction.tsx:292` |
| Observed / supplied / inferred badges in the UI | `features/workspace/types.ts:28`; `features/graph/services/graphAdapter.ts:148-155` |
| **Python reference:** `FindingProvenance` for each finding (type, `is_observed`, evidence IDs **with SHA-256**, event IDs, source record IDs, frame artifacts) | `backend/app/modules/reconstruction/findings.py:35-145` `ProvenanceBuilder`; used throughout `pipeline.py` |

**Missing / broken**

- **Rust reconstruction has no finding-level provenance.** `TimelineEntry` (`model.rs:139`) has no `source_record_id`, `extraction_run_id` or evidence hash. `RawTimelineEvent` drops `source_record_id` (`adapters.rs:161-176`).
- **The run ID is not returned.** `trigger_case_reconstruction` creates a fresh `Uuid::new_v4()` (`api/routes/reconstruction.rs:99`) and returns the result without it. A finding can't be cited by run.
- **Input provenance over-claims.** `reconstruction_run_inputs` is filled with **every** extraction run of the case, including inactive ones (`reconstruction_repository.rs:113-127`), not the runs that were actually used.
- **Errors are swallowed.** Examples: `let _ = sqlx::query(...)` at `reconstruction_repository.rs:99,114`; `let _ = tokio::fs::write` at `api/routes/reconstruction.rs:382`; persistence failure only logged at `:112-118`. This conflicts with AGENTS.md §3.
- **The report doesn't trace.** `CaseForensicReport` (`report.rs:66`) has no reconstruction run ID, no result hash and no per-lead evidence hashes. `report_id` is derived from `case_id + exhibits.len()` (`report.rs:97-101`), so different reconstructions of the same case share one report ID.
- **No lead → evidence-hash → extraction-run → raw-byte drill-down in the UI.** Click-through stops at the evidence item.
- **Live graph data in the UI loses inference marking.** Every live event is labelled `observed` (`reconstruction.ts:352`), and live `entities`/`relationships` are empty arrays (`repository.ts:99-101`).

**Reusable:** Python `ProvenanceBuilder`; the Neo4j projection; the extraction-run versioning; the custody ledger.

**Complexity:** MEDIUM. Port `FindingProvenance`, carry `source_record_id` and `extraction_run_id` through `RawTimelineEvent`, return the `run_id`, fix the UI labels.

---

### USP 3 — Evidence Gap Intelligence · IMPLEMENTED_BUT_NOT_INTEGRATED (Python) · 3.0/10

**What exists**

- **Python only:** `EvidenceGapType` with 8 types (`EVIDENCE_EXCLUDED`, `EVIDENCE_WITHOUT_EVENTS`, `CANDIDATE_EXCLUDED`, `FRAME_EXCLUDED`, `EVENT_WITHOUT_COORDINATES`, `UNRESOLVED_ENTITY_REFERENCE`, `SPATIAL_CONTINUITY_GAP`, `VIDEO_WITHOUT_FRAMES`) at `backend/app/modules/reconstruction/findings.py:63-81`; produced by `pipeline.py:1355` `_gaps`.
- **Python only:** structured `Limitation` codes, including `SINGLE_SOURCE_NO_CORROBORATION`, `UNCALIBRATED_CLOCKS`, `DECLARED_CLOCK_OFFSETS_NOT_VERIFIED`, `NO_SPATIAL_DATA` and `CONCURRENT_WINDOWS_NOT_ORDERED` (`findings.py:155-268`). Exposed at `GET /cases/{id}/reconstruction/runs/{run}/diagnostics` (`backend/app/api/routes/reconstruction.py:236-247`). Tested in `test_scenario_ef_partial_and_sparse.py`, `test_reconstruction_api.py` and `test_pipeline.py`.
- **Rust:** only `excluded_events` (`model.rs:171`) and the "intervening unlocated events" caveat (`contradictions.rs:103-108`).
- **Frontend:** a `missing` review category with help text (`ContradictionReview.tsx:13-21`). Its only data source is a **hand-written fixture item** (`fixtures/synthetic/workspace.ts:270`).

**Missing**

- No Rust port of gaps or limitations, and no Rust API.
- **No "missing corroboration" logic per finding.** For example: "this contradiction rests on one CDR row from one carrier, and no second source covers 21:40–22:20".
- **No unsupported-assumption detection tied to findings**, beyond the Rust assumption labels.
- **No recommendation engine.** For example: "request the tower dump for cell X", "request the DVR clock calibration certificate", "obtain the carrier CDR for the second number". No rule table exists.
- Python runs are kept in `InMemoryRunStore` (`run_store.py:1-12`, TODO for Member 2), so they are lost on restart.

**Reusable:** port Python `_gaps` and `build_limitations` directly. Assumption kinds map one-to-one to recommendations, for example `COORDINATES_INFERRED_FROM_TOWER_ID` → tower dump / sector azimuth, and `CLOCK_OFFSET_NOT_DECLARED_ON_EVENT` → DVR clock calibration.

**Complexity:** LOW–MEDIUM. The rule-based recommendations are deterministic and need no ML.

---

### USP 4 — Investigation What-If Simulator · FOUNDATION_EXISTS_NEAR_USP · 2.5/10

**What exists**

- Per-request `ReconstructionOptions` (timezone, `default_clock_offset_seconds`, `speed_threshold_kmh`, estimated half-width) at `service.rs:27`. `POST /api/v1/cases/:id/reconstruction` accepts it, along with inline `raw_events` (`api/routes/reconstruction.rs:31-82`).
- Evidence reprocessing with a new timezone or offset creates a **new, hashed extraction run** and never modifies the original (`api/routes/evidence.rs:428-475`; `extraction/model.rs:381` `ReprocessRequest`).
- Python: deterministic run IDs and `result_sha256` (`pipeline.py:24-30`). In principle, two configurations can be compared by hash.

**Missing**

- **Every run overwrites "latest".** The filesystem store keeps a single `reconstruction/latest.json` (`api/routes/reconstruction.rs:368-385`), and every GET reads "latest" (`reconstruction_repository.rs:265-275`). A what-if run would **replace** the investigator's baseline view.
- **Reprocessing moves the active-run pointer** (`extraction/service.rs:566`), so a hypothetical offset becomes the case's canonical extraction. That is wrong for a what-if.
- No scenario entity: no `scenario_id`, parent baseline, per-evidence offset map or label. No migration for it.
- No diff or compare of leads between baseline and scenario (appeared / dissolved / unchanged).
- No UI. `triggerCaseReconstruction` exists in `services/reconstruction.ts:218` but **no component calls it**.

**Complexity:** MEDIUM for a stateless compare endpoint; HIGH for fully persisted, versioned scenarios with a UI.

---

### USP 5 — India-Focused Digital Evidence Intelligence · FOUNDATION_EXISTS_NEAR_USP · 2.5/10

**What exists**

- E.164 normalization with an optional `default_country_prefix` (`correlation/resolution.rs:29-120`), wired into CDR and chat parsing (`extraction/cdr.rs:306,326`; `chat.rs:323,344`). This is **generic, not India-specific**; `+91` only applies if the caller passes `91`.
- Reprocessing accepts `default_country_code` (`api/routes/evidence.rs:449`).
- IST: generic IANA support handles `Asia/Kolkata`. The UI shows IST (`features/workspace/format.ts:5-6`). The synthetic CDR uses `+05:30` and `+91` numbers (`data/synthetic/sample_cdr.csv`).
- Cell-tower centroids: **5 hard-coded Delhi substrings** (`reconstruction/adapters.rs:11-31`, `resolve_cell_tower_centroid`). This is a mock and must not be described as a tower database.
- Hindi: **one test** checks that character offsets are correct in Devanagari text (`tests/extraction.rs:516`).

**Missing**

- **No Indian telecom CDR formats.** The parser requires a project-specific 5-column header (`cdr.rs:19-25`). Real Indian carrier CDR or tower-dump exports aren't supported: their columns (calling/called party, first/last cell ID, IMEI, IMSI, LRN, roaming circle, date and time in separate columns, often `dd/mm/yyyy`) have no mapping.
- **Phones inside chat text are not normalized.** `extract_identifiers_from_text` ignores its `_default_country_code` argument (`identifiers.rs:75`) and leaves `normalized_value: None` (`:248`). Chat-body numbers therefore never link to CDR numbers.
- No Indian mobile-number rules: 10 digits starting 6–9, the `0` trunk prefix, `+91`/`91`/`091` variants. These depend on the caller passing a prefix.
- **No WhatsApp `.txt` export parser.** WhatsApp's actual export format is `dd/mm/yy, h:mm am/pm - Name: msg`, with locale variants. Only a custom JSON schema is accepted (`chat.rs:30-90`; `data/synthetic/sample_chat_export.json`).
- **No Hindi-English (Hinglish) or Devanagari handling:** no normalization, transliteration, language ID or keyword lexicon (searched: no matches).
- No UPI, Indian bank-statement or Indian device-dump formats.

**Complexity:** LOW for Indian phone rules plus text-number normalization; MEDIUM for a WhatsApp `.txt` parser and CDR column-mapping profiles; HIGH for meaningful Hinglish NLP.

---

## 4. Implementation matrix (backend → API → DB → frontend → tests)

| USP capability | Rust logic | Rust API | Postgres | Frontend | Tests run | Python ref |
|---|---|---|---|---|---|---|
| Uncertainty windows / clock offset | ✅ | ✅ | ⚠ events lose per-event tz/offset, defaults re-applied (`api/routes/reconstruction.rs:299-319`) | ✅ hatch bands | ✅ | ✅ |
| Assumption labels + export gate | ✅ | ✅ in JSON | ✅ in `result_snapshot` | ❌ dropped; report export hard-disabled, status hard-coded (`ReportsWorkspace.tsx:52-61`) | ✅ Rust | ✅ |
| Sensitivity / robustness class | ❌ | ❌ | ❌ (`severity='HIGH'` hard-coded) | ❌ | ❌ | ❌ |
| Finding-level provenance | ❌ | ❌ | ⚠ run inputs over-claimed | ⚠ evidence click-through only | ❌ | ✅ |
| Observed vs inferred graph | ✅ projection | ✅ graph routes | ✅ `is_observed` | ⚠ live events forced `observed` | ⚠ mocks only | ✅ |
| Evidence gaps / limitations | ❌ | ❌ | ❌ | ⚠ fixture only | ❌ | ✅ |
| Gap recommendations | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| Scenario / what-if | ⚠ options only | ⚠ overwrites latest | ❌ | ❌ | ❌ | ⚠ deterministic hash |
| Indian phone normalization | ⚠ generic + prefix | ⚠ reprocess param | ✅ normalized column | ❌ | ✅ | ✅ |
| WhatsApp txt / Indian CDR / Hinglish | ❌ | ❌ | — | ❌ | ❌ | ❌ |
| Forensic report | ⚠ thin | ✅ `/report` | — | ❌ endpoint not called | ⚠ | ✅ richer (`reporting/forensic_report.py`, 1030 lines) |

`decision-service/` exposes only `/health` (`src/index.ts:9-22`). Laya / ONNX is deferred and supports no USP today. `shared/` contains only READMEs; there are no machine-readable schemas to change.

---

## 5. Cross-cutting gaps that affect several USPs

1. **The workspace is fixture-first.** `App.tsx:34` renders `WorkspaceApp` with no repository, so it falls back to `syntheticRepository` (`WorkspaceDataContext.tsx:6`). `listCases` always returns `SYNTHETIC_CASES` (`repository.ts:110`), so real backend cases are never listed. `normalizeCaseId` maps any non-UUID string containing digits to `00000000-0000-0000-0000-<digits>`, and any other string to a fixed default UUID (`services/reconstruction.ts:14-25`). That could silently show the wrong case.
2. **Python and Rust have diverged.** The Python pipeline is the more forensically mature implementation (provenance, gaps, limitations, calibration status, result hashing, a rich report). The frontend calls Rust (`services/config.ts:2`, `localhost:8080`). Every USP gains most from **porting Python → Rust**, not from writing new logic.
3. **The persistence layer is unexecuted.** No test touches PostgreSQL. `reconstruction_repository.rs` ignores errors with `let _` and `unwrap_or(0)`.

---

## 6. Test coverage and verification gaps

| Area | Present | Needed |
|---|---|---|
| Sensitivity margin | — | Property tests: the lead dissolves exactly at the computed offset (±1 ms); same-clock pairs are invariant to a shared offset |
| Robustness classification | — | One table-driven test per class (ROBUST / ASSUMPTION_SENSITIVE / INCONCLUSIVE) |
| Provenance in Rust | — | Every lead and timeline entry resolves to an evidence SHA-256, an extraction run ID and a source record ID; the returned run ID round-trips |
| Gaps in Rust | — | Parity tests against Python `_gaps` on the shared synthetic burglary case |
| Scenario compare | — | Baseline is unchanged after a scenario run; diff is deterministic |
| Frontend live path | 2 adapter unit tests | Render `CaseView` through a mocked live API; assert assumption badges, the export-gate banner, `inferred` provenance for tower-centroid events |
| Indian phone rules | generic tests | `98110 02233`, `09811002233`, `+91-98110-02233`, `919811002233` all → `+919811002233`; numbers in chat text normalized |
| PostgreSQL / Neo4j | none | Tests against throwaway DBs (testcontainers or the docker-compose stack); currently **unverified** |
| Python symlink tests | 5 fail on Windows | Re-run on Linux CI or in Windows Developer Mode |

---

## 7. Ranked USP opportunities

Scoring is 1–5 per criterion.

| Rank | USP | Novelty | PS-3 relevance | Readiness | Feasibility | Demo impact | Total |
|---:|---|:-:|:-:|:-:|:-:|:-:|:-:|
| 1 | **1 — Uncertainty-aware reconstruction (+ sensitivity)** | 5 | 5 | 4 | 4 | 5 | **23** |
| 2 | **2 — Evidence-to-claim traceability** | 3 | 5 | 3 | 4 | 4 | **19** |
| 3 | **3 — Evidence gap intelligence** | 4 | 4 | 3 (Python) | 4 | 4 | **19** |
| 4 | 4 — What-if simulator | 4 | 4 | 1 | 3 | 5 | **17** |
| 5 | 5 — India-focused | 2 | 3 | 1 | 3 | 3 | **12** |

**On uniqueness:** this audit did **not** survey competing hackathon entries or commercial tools (for example Cellebrite, Magnet AXIOM, Oxygen). So no USP is claimed to be unique. The strongest *defensible* claim is narrower and verifiable: contradiction leads are raised only when **no** instant inside the stated uncertainty windows allows the transit, and each lead states how much clock error would dissolve it. That is a precise, testable property, not a marketing word.

---

## 8. Recommended flagship and supporting USPs

### Flagship: USP 1 — Uncertainty-Aware Reconstruction with Contradiction Robustness

This builds on the most mature code, maps directly to Problem Statement 3 (contradiction detection and decision support), and demos well. **Fold USP 4 into it**, so the what-if becomes the UI for sensitivity rather than a separate product.

**Design (fits existing code):**

1. **Clock-offset tolerance per lead (closed form).** For a spatial lead, the existing metrics give `max_elapsed = B.end − A.start` (`spatial.rs:84`). The transit needs `required = distance_km × 3600 / threshold_kmh`. Then `margin_s = required − max_elapsed`, the smallest **relative** clock shift between the two sources that would make the transit feasible. For temporal-activity leads, use the same calculation with `duration_seconds` added (`contradictions.rs:48-61`).
2. **Shared-clock invariance.** If both events come from the same `source_evidence_id` with the same declared offset, a shared clock error cancels out. Mark the lead **clock-robust**. The data is already available (`ObservedLeadBasis.origin_evidence_id / destination_evidence_id`).
3. **Location tolerance.** If either endpoint has `COORDINATES_INFERRED_FROM_TOWER_ID`, recompute with `distance − (r_A + r_B)` using a configurable cell radius, and report whether the lead survives.
4. **Classification (new enum, deterministic, no probabilities):**
   - `ROBUST`: survives the configured drift bound (for example ±300 s) **and** the tower-radius bound, with no assumption on either endpoint.
   - `ASSUMPTION_SENSITIVE`: survives only while a labelled assumption holds; names the assumption(s) and the margin.
   - `INCONCLUSIVE`: dissolved by an error smaller than the precision of either source, or by the drift bound.
5. **Stateless scenario compare.** `POST /api/v1/cases/:id/reconstruction/scenarios` takes a baseline plus a per-evidence offset map. It runs `ForensicReconstructionService::reconstruct_case` in memory (it is already pure), writes **nothing to "latest"**, and returns a lead diff (appeared / dissolved / unchanged).
6. **UI.** Add a "Robustness" column and a margin slider to `ContradictionReview`. Show `assumptions` badges and the `export_safeguard` banner.

**Reusable:** `check_temporal_activity_feasibility`, `calculate_transit_metrics`, `ReconstructionAssumption`, `evaluate_export_safeguard`, and the pure `reconstruct_case`.
**Complexity:** MEDIUM. Roughly 300–500 lines of Rust plus tests, and about 200 lines of TS.

### Supporting USP A: USP 2 — Traceability
Port `FindingProvenance` (`findings.py:35-145`). Carry `source_record_id` and `extraction_run_id` through `RawTimelineEvent`. Return `reconstruction_run_id`. Write only the runs actually used into `reconstruction_run_inputs`. Put the run ID and evidence hashes in the report. Every robustness claim from the flagship then cites its evidence hashes.

### Supporting USP B: USP 3 — Evidence Gap Intelligence
Port `_gaps` and `build_limitations` to Rust, then add a **deterministic recommendation table** keyed by gap or assumption kind. Each recommendation gets an explanation, for example: "Lead L-3 is ASSUMPTION_SENSITIVE on tower centroid → request sector/azimuth and a tower dump for cell X between 21:40 and 22:20 IST." This turns the flagship's output into next investigative actions.

### Not worth implementing now (low differentiation or high cost)
- **Hinglish NLP / translation.** High effort, hard to validate on synthetic data, and generic LLM tools already do it. Do only Unicode normalization and Indian number normalization.
- **Persisted, versioned what-if scenarios with their own DB schema and history UI.** The stateless compare delivers the same demo value at about 20% of the cost.
- **A "real" tower database.** Do not imply a real database exists. Keep the mock clearly labelled synthetic.
- **Laya / ONNX decision-service work.** It supports none of the five USPs today.
- **Probability or confidence scores on contradictions.** They conflict with DEC-004 and AGENTS.md §4.5. Fixing the hard-coded `'HIGH'` severity is in scope; adding new scores is not.

---

## 9. Highest-priority improvements (top 3)

| # | Improvement | Files | Complexity |
|---|---|---|---|
| 1 | **Contradiction robustness:** clock-offset margin, tower-radius tolerance, ROBUST / ASSUMPTION_SENSITIVE / INCONCLUSIVE, plus a stateless scenario-compare endpoint | `backend-rust/src/reconstruction/{model,contradictions,service}.rs`, `api/routes/reconstruction.rs`, `app.rs`; new tests | MEDIUM |
| 2 | **Stop the frontend erasing uncertainty:** add `assumptions` and `export_safeguard` to the DTOs; derive provenance (`inferred` when an assumption exists) instead of `'observed'`; remove the dead `location_synthetic_centroid` check; call `/report`; list real cases; stop `normalizeCaseId` from making up IDs | `apps/web/src/services/reconstruction.ts:14-25, 300-353`, `features/workspace/data/repository.ts:49,78,110`, `pages/ReportsWorkspace.tsx:52-61` | LOW |
| 3 | **Port Python provenance and gaps to Rust** (`FindingProvenance`, `EvidenceGap`, `Limitation`) and return `reconstruction_run_id` | `backend-rust/src/reconstruction/*`, `persistence/reconstruction_repository.rs:99-127,170`; reference `backend/app/modules/reconstruction/findings.py`, `pipeline.py:1355` | MEDIUM |

Item 2 is the cheapest and most urgent. Today the UI shows a tower-centroid position as "observed", which goes against AGENTS.md §4.4.

---

## 10. Team-wise priorities

| Member | Priority work |
|---|---|
| **Member 4 — Forensic Intelligence** | Owns the flagship: robustness margin, classification, scenario-compare endpoint, Rust port of gaps and limitations, thicker report (run ID, hashes, per-lead robustness). |
| **Member 2 — Backend & Data** | Return `reconstruction_run_id`; write only the real run inputs; replace `let _` and `unwrap_or` with handled errors; remove hard-coded `'HIGH'` severity (store `NULL` or the robustness class, via an additive migration); keep events' declared tz and offset so stored events don't lose them; first tests against a real DB. |
| **Member 1 — AI/ML & Extraction** | Propagate `source_record_id` and precision; Indian mobile rules; normalize numbers in message text (`identifiers.rs:75`); WhatsApp `.txt` parser; CDR column-mapping profiles. All synthetic data only. |
| **Member 3 — Frontend** | Improvement #2 above; robustness column and margin slider in `ContradictionReview`; assumption badges in `EventReconstruction`; export-gate banner tied to real `export_safeguard`; live-path integration tests. |
| **Team** | Agree a shared-schema change for `ReconstructionAssumption`, `ExportSafeguard`, the robustness class and `FindingProvenance` before frontend DTOs are fixed (AGENTS.md §6). |

---

## 11. Suggested evaluator demonstration scenario (synthetic data only)

**"The 4-minute alibi"** uses the existing `data/synthetic/sample_cdr.csv` (Delhi `+91` numbers, `+05:30`) plus a synthetic CCTV exhibit uploaded with **no declared clock offset**.

1. **Ingest.** Upload the CDR and the CCTV export, and show the SHA-256 receipts and custody ledger. *(Works today.)*
2. **Reconstruct.** The timeline shows hatched uncertainty windows. One lead fires: the suspect's phone is on the Saket tower at 21:45, and the CCTV shows the vehicle near Noida Sec-62 four minutes later. *(Logic works today; UI display needs improvement #2.)*
3. **Honesty layer.** The lead is marked **ASSUMPTION_SENSITIVE**: the CCTV clock is uncalibrated (`CLOCK_OFFSET_NOT_DECLARED_ON_EVENT`), and the phone position is a tower centroid (`COORDINATES_INFERRED_FROM_TOWER_ID`). The export gate shows **BLOCKED** with reasons. *(Needs improvements #1 and #2.)*
4. **Sensitivity.** "This contradiction dissolves if the DVR clock is ≥ 11 min 20 s fast, or if the phone was ≥ 6.1 km from the tower centroid." *(The figures are illustrative; the real values come from the margin formula in §8. Needs #1.)*
5. **What-if.** Drag the CCTV offset to −12 min. The lead dissolves and the baseline is **unchanged**: a side-by-side diff shows "1 dissolved, 0 new". *(Needs #1.)*
6. **Gap recommendation.** "Request the DVR calibration certificate for exhibit CCTV-01; request sector/azimuth for TOWER-DELHI-SOUTH-04B." *(Needs supporting USP B.)*
7. **Traceability.** Click the lead → both evidence items → SHA-256 → extraction run → CDR row `cdr-rec-001`. *(Needs #3.)*

The demo shows that the system tells the investigator what the evidence supports, what it doesn't, and what would change the conclusion. It never claims identity or guilt.

---

*End of report. Read-only audit; no source files were modified.*
