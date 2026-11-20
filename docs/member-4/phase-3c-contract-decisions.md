# YUKTI Phase 3C — Contract Decision Review Package

**Author:** Member 4 (Forensic Intelligence & System Integration)  
**Date:** 2026-11-17  
**Monorepo Target:** YUKTI Core Contracts (SUTRAM 2026)  
**Status:** `PROPOSED`. Approval is NOT verified. An "approved" status was recorded on 2026-11-19 from a copied task brief; Phase 3D.2 withdrew it (see [`team-contract-approvals-2026-11-19.md`](team-contract-approvals-2026-11-19.md)). DEC-007 (raw timestamp preservation) has been added as a proposal. If approved, it would reverse the assumption that `Event.timestamp` arrives already normalized. DEC-005 proposes a strictly half-open window (`start <= nominal < end`).

---

## 1. Executive Summary & Context

During the Phase 3A integration audit of Member 4's reconstruction modules (`temporal.py`, `spatial.py`, `contradictions.py`, `video.py`) and their DTOs in `reconstruction.py`, several contract discrepancies were uncovered across:
- `shared/schemas/README.md` (canonical data model documentation)
- `shared/enums/README.md` (canonical taxonomy documentation)
- `shared/examples/README.md` (JSON payload examples)
- `apps/web/src/types/index.ts` (frontend TypeScript interfaces)
- `backend/app/schemas/reconstruction.py` (backend reconstruction DTOs)

In accordance with **Section 6 of AGENTS.md** (*"No member or agent may unilaterally modify shared/schemas/. Shared contract adjustments require team alignment"*), this document formally details the evidence-based contract decisions required to enable end-to-end integration across all four team roles without unilateral schema alterations or breaking changes.

---

## 2. Evidence-Based Decision Analysis

### Decision 1: Clock Correction Convention

#### Current Documented Behavior vs. Implementation
- **Phase 3B Proposal Text:** Stated $\text{True UTC} = \text{Device Time} - \text{Offset}$, which defines offset as "clock error" or "clock drift" ($\text{Error} = \text{Device} - \text{True}$).
- **Existing Implementation in `temporal.py`:**
  ```python
  def parse_and_normalize_timestamp(
      raw_timestamp: str | datetime,
      source_timezone: str = "UTC",
      clock_offset_seconds: float = 0.0,
  ) -> datetime:
      local_dt = _parse_iso_to_localized_datetime(raw_timestamp, source_timezone=source_timezone)
      utc_dt = local_dt.astimezone(timezone.utc)
      return utc_dt + timedelta(seconds=clock_offset_seconds)
  ```
- **Existing Tests in `test_temporal.py`:**
  - Fast clock (+120s fast): `clock_offset_seconds = -120.0`
  - Slow clock (45.5s slow): `clock_offset_seconds = +45.5`

#### Conflict Discovered
An ambiguity exists between **Clock Error** ($\Delta t_{\text{error}} = t_{\text{device}} - t_{\text{true}}$) and **Clock Correction** ($\Delta t_{\text{correction}} = t_{\text{true}} - t_{\text{device}}$). Subtracting a positive error produces the same numerical result as adding a signed correction, but if the convention is undocumented, users or upstream services might invert the sign (e.g. entering `+120` for a fast clock, erroneously pushing the true time even further into the future).

#### Recommended Canonical Decision
Formally standardize `clock_offset_seconds` as the **Additive Clock Correction**:
$$\mathbf{t}_{\text{UTC}} = \mathbf{t}_{\text{device\_localized\_to\_UTC}} + \text{clock\_offset\_seconds}$$
- **If device is FAST by $\Delta t$ seconds:** $\text{clock\_offset\_seconds} = -\Delta t$ (negative value pulls time back).
- **If device is SLOW by $\Delta t$ seconds:** $\text{clock\_offset\_seconds} = +\Delta t$ (positive value advances time forward).

#### Numerical Example
- **Scenario:** CCTV DVR installed in New Delhi running 120.0 seconds fast relative to Indian Standard Time (IST, `Asia/Kolkata`, UTC+05:30).
  - Recorded Device Timestamp: `2026-11-10T21:47:00` (naive local DVR display)
  - Configured Timezone: `Asia/Kolkata`
  - Configured Clock Offset: `-120.0` seconds
  - Localized Instant: `2026-11-10T21:47:00+05:30`
  - Converted to UTC: `2026-11-10T16:17:00Z`
  - Applying Additive Offset: $16:17:00\text{Z} + (-120.0\text{s}) = \mathbf{2026-11-10T16:15:00Z}$
- **Scenario 2 (Slow Device):** Mobile device in UTC running 45.5 seconds slow.
  - Recorded Device Timestamp: `2026-11-10T10:00:00Z`
  - Configured Clock Offset: `+45.5` seconds
  - Applying Additive Offset: $10:00:00\text{Z} + (+45.5\text{s}) = \mathbf{2026-11-10T10:00:45.500000Z}$

#### Implementation & Test Impact
- **Existing Implementation:** 100% compliant. No code changes needed in `temporal.py` or `video.py`.
- **Existing Tests:** 100% compliant. All 41 temporal and 50 video tests pass without modification.
- **Action Required:** Document this exact definition in `shared/schemas/README.md` under `Evidence.metadata`.

---

### Decision 2: TimePrecision Semantics

#### Current Documented Behavior vs. Implementation
- `shared/schemas/README.md` (Table 4): Lists `EXACT`, `MINUTE`, `HOUR`, `APPROXIMATE`.
- `shared/enums/README.md` (Section 5): Defines `EXACT_SECOND`, `MINUTE`, `HOUR`, `DAY`, `ESTIMATED`.
- `apps/web/src/types/index.ts`: `TimePrecision` is entirely missing from TypeScript interfaces.
- `backend/app/schemas/reconstruction.py`: Implements `CanonicalTimePrecision` (`EXACT_SECOND`, `MINUTE`, `HOUR`, `DAY`, `ESTIMATED`) with legacy alias translation.

#### Semantic Clarification: Precision vs. Accuracy
- **Precision:** Granularity/resolution of the timestamp record as emitted by the source modality (e.g. seconds for CCTV PTS, minute for SMS header, date for bank statement, coarse estimate for witness).
- **Accuracy:** Truth value or fidelity relative to UTC reference, governed independently by `clock_offset_seconds`.
- A CCTV timestamp can have `EXACT_SECOND` precision while having low accuracy (e.g. 5 minutes clock drift). Conversely, an NTP-synchronized daily audit has high accuracy but `DAY` precision.

#### Recommended Canonical Decision
1. **Canonical Enum Values:** Adopt `shared/enums/README.md` as sole source of truth:
   - `EXACT_SECOND` (uncertainty window half-width: $[t, t+1\text{s})$)
   - `MINUTE` (floored to local minute: $[HH:MM:00, HH:MM+1:00)$)
   - `HOUR` (floored to local hour: $[HH:00:00, HH+1:00:00)$)
   - `DAY` (floored to local calendar day: $[00:00:00, 24:00:00)$)
   - `ESTIMATED` (nominal instant $\pm \text{half\_width}$, default $\pm 900$s)
2. **Local Timezone Construction Before UTC Conversion:**
   Calendar units (`MINUTE`, `HOUR`, `DAY`) **must** be floored and advanced in the source timezone *before* conversion to UTC.
   - *Forensic Justification:* A calendar day across Daylight Saving Time transitions has 23 hours (spring-forward) or 25 hours (fall-back) in local clock time. Converting to UTC first and adding 24 hours creates non-existent local times or clips real local events.
3. **Half-Open Intervals:** All intervals must be mathematically half-open $[start, end)$ where $start \le t < end$.
4. **Legacy Alias Translation:** The backend and frontend must tolerate legacy inputs:
   - `"EXACT"` $\rightarrow$ `EXACT_SECOND`
   - `"APPROXIMATE"` $\rightarrow$ `ESTIMATED`

---

### Decision 3: RelationType Completeness

#### Current Documented Behavior Across Repositories
| Source | Documented / Implemented Values |
| :--- | :--- |
| `shared/schemas/README.md` (Table 5) | `CALL_MADE`, `PRESENT_AT`, `COMMUNICATED_WITH`, `ASSOCIATE_OF`, `OWNS` |
| `shared/enums/README.md` (Section 4) | `COMMUNICATED_WITH`, `PRESENT_AT`, `CO_OCCURRED_WITH`, `OWNS`, `ASSOCIATE_OF`, `CONTRADICTS` |
| `shared/examples/README.md` (Line 49) | Uses `COMMUNICATED_WITH` (with `"call_duration_seconds": 184` in metadata) |
| `backend/app/schemas/reconstruction.py` | Produces `CONTRADICTS` with `is_observed=False` |

#### Conflicts & Lost Values Identified
1. `CALL_MADE` appears in `shared/schemas/README.md`, but is absent from `shared/enums/README.md`.
2. `CO_OCCURRED_WITH` appears in `shared/enums/README.md`, but is omitted from `shared/schemas/README.md`.
3. `CONTRADICTS` appears in `shared/enums/README.md`, but is omitted from `shared/schemas/README.md`.
4. If `shared/schemas/README.md` is strictly enforced without amendment, Member 4's contradiction engine (`CONTRADICTS`) and co-occurrence correlation (`CO_OCCURRED_WITH`) would be rejected by schema validators!

#### Recommended Canonical Decision
Adopt the superset of 7 well-defined relation types:
1. `COMMUNICATED_WITH`: Telephony, chat, SMS, or email exchange.
2. `CALL_MADE`: Retained as a specialized sub-relation of telephony communication (or aliased to `COMMUNICATED_WITH` with `metadata.channel = "PHONE_CALL"`).
3. `PRESENT_AT`: Co-location of an entity at a specific event or coordinate.
4. `CO_OCCURRED_WITH`: Observed spatio-temporal co-presence of two entities without verified communication.
5. `OWNS`: Property, vehicle, device, phone number, or account ownership.
6. `ASSOCIATE_OF`: Known social, familial, or criminal syndicate relationship.
7. `CONTRADICTS`: Algorithmically detected physical, temporal, or spatial incompatibility between two claims or events.

---

### Decision 4: `Relationship.weight` Nullability & Rendering

#### Current Documented Behavior vs. Implementation
- `shared/schemas/README.md`: Declares `weight: float (0.0 - 1.0)`.
- `backend/app/schemas/reconstruction.py`: Declares `weight: Optional[float] = None` for `CONTRADICTS`.

#### Forensic Rationale
Forensic Integrity Principle #5 prohibits identity and confidence fabrications. A contradiction detected between two events (e.g. suspect observed 35 km away 4 minutes later, requiring 427 km/h travel) is a verified physical incompatibility—it does not possess a "confidence weight" of 0.85 or 0.95. Fabricating a floating-point score on a contradiction lead misrepresents an analytical fact as a statistical heuristic.

#### Recommended Canonical Decision
1. **Contract Definition:** Update `Relationship.weight` to `Optional[float] = None` in Python and `weight: number | null` in TypeScript.
2. **Frontend UI Rendering Rules (Member 3):**
   - **Weighted Relationships (`weight != null`):** Render link stroke width proportional to confidence, or display confidence badge (e.g. `95%`).
   - **Unweighted Relationships (`weight == null`):**
     - For `relation_type === "CONTRADICTS"`: Render as an alert-styled edge (e.g. crimson dashed line with an investigative alert marker ⚠️). Tooltip must display physical metrics from `metadata` (e.g., `"Required speed: 427 km/h, Nominal elapsed: 295s"`).
     - **Strict Prohibition:** The frontend must **never** default `null` to `0.0` or render it as "0% confidence".

---

### Decision 5: Event Duration & Uncertainty Representation

#### Current Documented Behavior vs. Implementation
- `shared/schemas/README.md`: Defines `timestamp: ISO 8601 UTC` and `timestamp_precision: TimePrecision`. Lacks fields for duration and explicit interval bounds.
- `backend/app/schemas/reconstruction.py`: Supports `duration_seconds: Optional[float]`, `uncertainty_window_start: Optional[datetime]`, `uncertainty_window_end: Optional[datetime]`.

#### Semantic Clarification: Duration vs. Uncertainty Window
- **`duration_seconds`:** The physical elapsed runtime of the event itself (e.g., a phone call lasting 180s, or lock tampering lasting 30s).
- **`uncertainty_window_start / end`:** The epistemic interval during which the event is known to have occurred (e.g., witness sighting between 14:00 and 14:30).
- An event with duration $D = 300\text{s}$ occurring within an uncertainty window $[14:00, 14:30)$ has 30 minutes of temporal uncertainty, but only 5 minutes of physical activity duration.

#### Recommended Canonical Decision
1. Extend `Event` schema in `shared/schemas/README.md` with:
   - `duration_seconds: Optional[float] = Field(default=None, ge=0.0)`
   - `uncertainty_window_start: Optional[str] = Field(default=None)` (ISO 8601 UTC string)
   - `uncertainty_window_end: Optional[str] = Field(default=None)` (ISO 8601 UTC string)
2. **Validation Rules:**
   - If explicit uncertainty bounds are provided, validation must enforce: $\text{window\_start\_utc} < \text{window\_end\_utc}$ and $\text{window\_start\_utc} \le \text{nominal\_utc} \le \text{window\_end\_utc}$.
   - Duration must be non-negative: $\text{duration\_seconds} \ge 0.0$.
3. **Data Representation:**
   - Internal Python models use timezone-aware `datetime.datetime` with `timezone.utc`.
   - API transport serialization uses standard ISO 8601 strings ending in `Z` (e.g. `2026-11-17T02:00:01Z`).

---

### Decision 6: Evidence Metadata Standards

#### Current Documented Behavior vs. Implementation
- `shared/schemas/README.md`: Defines `metadata: dict / JSON` without standardized key names.
- `backend/app/modules/reconstruction/temporal.py` and `video.py`: Rely on `source_timezone`, `clock_offset_seconds`, `recording_start_timestamp`.

#### Recommended Canonical Decision
Standardize the following four top-level keys within `Evidence.metadata`:

| Key Name | Type | Optional / Required | Default | Description |
| :--- | :--- | :--- | :--- | :--- |
| `source_timezone` | `str` | Optional | `"UTC"` | Valid IANA timezone name (e.g. `"Asia/Kolkata"`, `"America/New_York"`). |
| `clock_offset_seconds` | `float` | Optional | `0.0` | Signed additive clock correction in seconds: $\text{True UTC} = \text{Device Time} + \text{Offset}$. |
| `recording_start_timestamp` | `str` (ISO 8601) | Optional | `None` | UTC or localized start instant for continuous media (CCTV, dashcam, audio wiretap). |
| `device_coordinates` | `dict` (`lat`, `lon`) | Optional | `None` | WGS84 coordinates (`-90 <= lat <= 90`, `-180 <= lon <= 180`) of fixed capture device. |

#### Component Ownership of Validation
- **Member 2 (Ingestion Service):** Validates format, syntax, and boundaries upon upload (e.g. `ZoneInfo(source_timezone)` validation, float checks, coordinate ranges).
- **Member 4 (Reconstruction Engine):** Performs forensic validation at execution time (cryptographic SHA-256 integrity verification, safe path sandboxing, DST ambiguity detection).

---

## 3. Master Decision Table

| Decision ID | Topic | Proposing Role | Affected Roles | Proposed Canonical Value | Status |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **DEC-001** | Clock Offset Convention | Member 4 | Member 2 (Ingestion), Member 4 | Additive Correction: $\text{True UTC} = \text{Device Time} + \text{Offset}$ | `PROPOSED` |
| **DEC-002** | TimePrecision Enums | Member 4 | All Members (1, 2, 3, 4) | `EXACT_SECOND`, `MINUTE`, `HOUR`, `DAY`, `ESTIMATED` with local-first flooring | `PROPOSED` |
| **DEC-003** | RelationType Taxonomy | Member 4 | Member 1 (ML), Member 2 (DB), Member 3 (UI) | Include `CONTRADICTS` and `CO_OCCURRED_WITH`; generalize `CALL_MADE` | `PROPOSED` |
| **DEC-004** | Relationship.weight Nullability | Member 4 | Member 2 (DB), Member 3 (UI) | `weight: Optional[float] = None` (nullable; never default null to 0.0) | `PROPOSED` |
| **DEC-005** | Event Duration & Uncertainty | Member 4 | Member 1 (ML), Member 2 (DB) | Add `duration_seconds`, `uncertainty_window_start`, `uncertainty_window_end` | `PROPOSED` |
| **DEC-006** | Evidence Metadata Schema | Member 4 | Member 2 (Ingestion), Member 4 | Standardize `source_timezone`, `clock_offset_seconds`, `recording_start_timestamp` | `PROPOSED` |
| **DEC-007** | Raw Timestamp Preservation | Member 4 (from Phase 3E brief) | Member 1 (extraction), Member 2 (DB), Member 4 | Extraction keeps `raw_timestamp` unchanged; only the Member 4 temporal engine produces `timestamp_utc`, applying localization and offset exactly once | `PROPOSED` |

*Note: All seven decisions (DEC-001 … DEC-007) remain `PROPOSED` until a verified team sign-off is recorded. The code fails closed until then.*

---

## 4. Required Approvals & Next Steps

1. **Member 1 (AI/ML & Evidence Intelligence):**
   - Review `DEC-002` (Precision) and `DEC-005` (Duration/Uncertainty) for compatibility with OCR and entity extraction outputs.
   - Review `DEC-003` for relationship extraction models.
2. **Member 2 (Backend & Data Engineering):**
   - Review `DEC-001` and `DEC-006` for Evidence ingestion schema and validation ownership.
   - Review `DEC-004` for PostgreSQL / Neo4j schema definitions (`weight` column nullability).
3. **Member 3 (Frontend & Visualization):**
   - Review `DEC-002` for TypeScript enum alignment in `apps/web/src/types/index.ts`.
   - Review `DEC-004` for Knowledge Graph UI edge styling of unweighted contradiction links.

---

## 5. Verification Status

All 143 existing backend tests were executed prior to publication of this decision package. No implementation code or shared contracts were modified.

```text
======================== 143 passed, 1 warning in 2.74s ========================
```
