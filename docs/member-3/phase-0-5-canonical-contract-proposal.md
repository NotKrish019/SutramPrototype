# YUKTI Phase 0.5 — Consolidated Canonical Contract Proposal

**Author:** Member 3 (Frontend & Visualization)
**Date:** 2026-11-18
**Baseline commit:** `179abe0` (`origin/main`)
**Status:** `PROPOSED` — for discussion with Members 1, 2 and 4. No shared contract file has been modified.

This document consolidates every contract discrepancy found across the five places the data model is
currently described, and proposes one canonical answer per item. It builds on, and is consistent with,
Member 4's [`phase-3c-contract-decisions.md`](../member-4/phase-3c-contract-decisions.md) (DEC-001 … DEC-006).

Sources compared:

| Code | Source |
| :--- | :--- |
| **S** | `shared/schemas/README.md` |
| **E** | `shared/enums/README.md` |
| **X** | `shared/examples/README.md` |
| **F** | `apps/web/src/types/index.ts` |
| **B** | `backend/app/schemas/` (`health.py`, `reconstruction.py`) |

> Note: **B** has no Pydantic models yet for Case, Evidence, Entity, Event or Relationship. Only the
> reconstruction DTOs (Member 4) and the health response exist. **F** defines only the three enums
> listed below, `HealthCheckResponse` and `CaseSummary`.

---

## 1. Enumerations

### 1.1 `CaseStatus`

| Value | S | E | F | B |
| :--- | :-: | :-: | :-: | :-: |
| `DRAFT` | – | ✓ | ✓ | – |
| `ACTIVE` | ✓ | ✓ | ✓ | – |
| `UNDER_REVIEW` | ✓ | ✓ | ✓ | – |
| `CLOSED` | ✓ | – | – | – |
| `ARCHIVED` | – | ✓ | ✓ | – |

**Proposal (P-01):** Five values — `DRAFT`, `ACTIVE`, `UNDER_REVIEW`, `CLOSED`, `ARCHIVED`.
`CLOSED` = investigation concluded; `ARCHIVED` = retained, read-only. The superset loses no meaning.
*Alternative:* four values per **E**, with `CLOSED` accepted on input as a legacy alias of `ARCHIVED`.

### 1.2 `EvidenceType`

| Value | S | E | F |
| :--- | :-: | :-: | :-: |
| `CCTV`, `CDR`, `CHAT_EXPORT`, `DOCUMENT_SCAN`, `DISK_IMAGE` | ✓ | ✓ | ✓ |
| `AUDIO_RECORDING`, `FINANCIAL_LEDGER` | "etc." | ✓ | ✓ |

**Proposal (P-02):** Adopt the seven values in **E**. Replace "etc." in **S** with the explicit list.
*Open question:* is an `OTHER` value needed for unclassified seized files?

### 1.3 `EntityType`

| Value | S | E | F |
| :--- | :-: | :-: | :-: |
| `PERSON`, `PHONE_NUMBER`, `VEHICLE`, `LOCATION`, `ORGANIZATION`, `CRYPTO_WALLET` | ✓ | ✓ | ✓ |
| `EMAIL` | ✓ | – | – |
| `DEVICE` | – | ✓ | ✓ |

**Proposal (P-03):** Eight values — the above plus both `EMAIL` and `DEVICE`. Both are real, distinct
identifier classes (e-mail addresses appear in chat exports and disk images; IMEI/MAC in CDRs).

### 1.4 `RelationType`

The repository name is **`RelationType`** (not `RelationshipType`). Keep that name everywhere.

| Value | S | E | X | F | B |
| :--- | :-: | :-: | :-: | :-: | :-: |
| `COMMUNICATED_WITH` | ✓ | ✓ | ✓ | – | – |
| `PRESENT_AT` | ✓ | ✓ | – | – | – |
| `OWNS` | ✓ | ✓ | – | – | – |
| `ASSOCIATE_OF` | ✓ | ✓ | – | – | – |
| `CALL_MADE` | ✓ | – | – | – | – |
| `CO_OCCURRED_WITH` | – | ✓ | – | – | – |
| `CONTRADICTS` | – | ✓ | – | – | ✓ (emitted) |

**Proposal (P-04):** Six canonical values — `COMMUNICATED_WITH`, `PRESENT_AT`, `CO_OCCURRED_WITH`,
`OWNS`, `ASSOCIATE_OF`, `CONTRADICTS`. Express calls as `COMMUNICATED_WITH` with
`metadata.channel = "PHONE_CALL"` (this is already exactly what **X** does for a CDR call), and accept
`CALL_MADE` on input as a legacy alias. *Alternative (DEC-003 option 1):* keep `CALL_MADE` as a seventh value.
For the UI, fewer edge types means a clearer graph legend, so Member 3 prefers the six-value form.

### 1.5 `TimePrecision`

| Value | S | E | X | F | B |
| :--- | :-: | :-: | :-: | :-: | :-: |
| `EXACT` | ✓ | – | – | – | alias → `EXACT_SECOND` |
| `EXACT_SECOND` | – | ✓ | ✓ | – | ✓ |
| `MINUTE`, `HOUR` | ✓ | ✓ | – | – | ✓ |
| `DAY` | – | ✓ | – | – | ✓ |
| `APPROXIMATE` | ✓ | – | – | – | alias → `ESTIMATED` |
| `ESTIMATED` | – | ✓ | – | – | ✓ |

**Proposal (P-05):** Endorse DEC-002 — `EXACT_SECOND`, `MINUTE`, `HOUR`, `DAY`, `ESTIMATED`.
`EXACT` / `APPROXIMATE` are accepted **only on input** (backend adapter already does this) and are
never emitted by any API. The frontend type will contain only the five canonical values.

---

## 2. Models

### 2.1 `Case`

| Field | S | X | F (`CaseSummary`) |
| :--- | :-: | :-: | :-: |
| `case_id`, `title`, `status`, `created_at` | ✓ | ✓ | ✓ |
| `description` | ✓ (required) | **missing** | ✓ |
| `fir_number` | optional | – | optional |
| `updated_at` | ✓ | **missing** | – |
| `investigator_id` | ✓ | **missing** | – |

**Proposal (P-06):** Keep **S** as-is. Frontend will add a full `Case` interface and keep `CaseSummary`
as an explicit `Pick<Case, …>` for list views. **X** should include all required fields.

### 2.2 `Evidence`

**F** has no `Evidence` type; **X** omits `storage_uri` and `metadata`.

**Proposal (P-07):** Keep **S** fields. Endorse DEC-006 standard `metadata` keys:
`source_timezone` (IANA, default `"UTC"`), `clock_offset_seconds` (additive correction, DEC-001),
`recording_start_timestamp`, `device_coordinates`.
*Open question for Member 2:* should `storage_uri` be returned to the browser at all? It is an internal
pointer; the UI only needs `evidence_id`, `original_filename`, `sha256_hash`, size and timestamps.

### 2.3 `Entity`

**F** has no `Entity` type.

**Proposal (P-08):** Keep **S** fields. Clarify that `confidence` is *extraction* confidence only and
must never be displayed or interpreted as proof of identity (Forensic Principle 5). UI will label it
"extraction confidence".

### 2.4 `Event`

| Field | S | X | B (reconstruction) |
| :--- | :-: | :-: | :-: |
| `timestamp`, `timestamp_precision`, `source_evidence_id`, `involved_entity_ids` | ✓ | ✓ | ✓ |
| `coordinates` | `GeoPoint (lat, lon)` | – | `GeoPoint {lat, lon}` (validated WGS84) |
| `duration_seconds` | – | – | ✓ optional, ≥ 0 |
| `uncertainty_window_start` / `_end` | – | – | ✓ optional |

**Proposal (P-09):** Endorse DEC-005 — add the three optional fields. Define `GeoPoint` explicitly as
`{ "lat": number, "lon": number }` (WGS84 decimal degrees) in **S**.
*Open questions:*
- Forensic Principle "preserve source timestamps and timezone details": should `Event` also carry the
  original raw timestamp string and `source_timezone`, or is a back-reference to `Evidence.metadata`
  sufficient for the UI to display "as recorded" vs "normalized UTC"?
- Can an `Event` itself be inferred (not observed)? If yes, `Event` needs an `is_observed` flag like `Relationship`.

### 2.5 `Relationship`

| Field | S | B (`DerivedContradictionRelationship`) |
| :--- | :-: | :-: |
| `weight` | `float` 0–1 (required) | `Optional[float] = None` |
| `relation_type` | enum | `str` (default `"CONTRADICTS"`) |
| other fields | ✓ | ✓ |

**Proposal (P-10):** Endorse DEC-004 — `weight: number | null`. UI rule: `null` is rendered as
"no score", **never** as 0 or 0 %. `CONTRADICTS` edges render as alert-styled dashed edges with the
metrics from `metadata`. `is_observed = false` edges render visually distinct (e.g. dashed) from observed
edges everywhere in the UI.
*Open question:* `source_id` / `target_id` may reference an Entity **or** an Event. Add
`source_kind` / `target_kind` (`"ENTITY" | "EVENT"`) so clients don't have to guess from ID prefixes?

---

## 3. Cross-cutting conventions (P-11)

1. Field names are `snake_case` on the wire and in TypeScript interfaces (no camelCase mapping layer).
2. All timestamps are ISO 8601 UTC strings ending in `Z`.
3. Enum values are `UPPER_SNAKE_CASE` strings. Frontend uses string-literal unions plus a runtime
   `as const` array per enum for validating API responses.
4. Error envelope: `{"error": {"code": string, "message": string, "details"?: unknown}}` (per
   `.agents/rules/project-standards.md`). Currently only the backend 500 handler emits this; FastAPI
   4xx/422 responses still return `{"detail": …}`. *Ask to Member 2:* apply the envelope to
   `HTTPException` and `RequestValidationError` too.

---

## 4. Proposed adoption order (after unanimous agreement)

1. Update `shared/enums/README.md` and `shared/schemas/README.md` together in **one** commit, and fix
   `shared/examples/README.md` in the same commit.
2. Member 2 adds Pydantic models / `StrEnum`s in `backend/app/schemas/`; Member 4 switches
   `DerivedContradictionRelationship.relation_type` to the shared enum.
3. Member 3 mirrors the result in `apps/web/src/types/` with tests.
4. *Optional, later:* move the canonical contract to machine-readable JSON Schema in `shared/schemas/`
   and generate both Pydantic and TypeScript types from it, eliminating drift.

## 5. Decision table

| ID | Topic | Proposal | Needs |
| :--- | :--- | :--- | :--- |
| P-01 | CaseStatus | 5 values incl. `CLOSED` + `ARCHIVED` | All |
| P-02 | EvidenceType | 7 values from **E** | M2, M4 |
| P-03 | EntityType | 8 values (add `EMAIL` to **E**) | M1, M2 |
| P-04 | RelationType | 6 values; `CALL_MADE` → `COMMUNICATED_WITH` + `metadata.channel` | All |
| P-05 | TimePrecision | = DEC-002 | All |
| P-06 | Case | full `Case` in frontend; fix example | M2, M3 |
| P-07 | Evidence | = DEC-006; decide `storage_uri` exposure | M2, M4 |
| P-08 | Entity | clarify `confidence` semantics | M1 |
| P-09 | Event | = DEC-005; define `GeoPoint`; raw timestamp question | All |
| P-10 | Relationship | = DEC-004; `source_kind`/`target_kind` question | All |
| P-11 | Conventions | snake_case, UTC `Z`, error envelope everywhere | M2 |
