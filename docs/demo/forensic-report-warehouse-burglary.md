# Reconstruction report — case case-wh-burglary-001

> **Disclaimer.** DECISION SUPPORT FOR INVESTIGATORS. This report is generated automatically from a reconstruction run over the evidence and extraction inputs listed in it. It organises those inputs and the analytical inferences derived from them for review by a qualified investigator. It does not establish the identity of any person, nor guilt, intent, deception or the occurrence of any offence. Contradictions are analytical leads to be checked against the original evidence, not findings of fact. Entity identifiers are registry references, not identifications. Every finding must be verified against the cited source evidence before it is relied upon.

## 1. Case summary and run metadata

| Field | Value |
| --- | --- |
| Case id | `case-wh-burglary-001` |
| Run id | `run-d270ccd5108009dd06f3370df6b7aa9f` |
| Run status | `COMPLETED_WITH_WARNINGS`: The run completed. Warnings were recorded; see Limitations and the diagnostics list. |
| Algorithm version | `yukti-m4-reconstruction/3F.2` |
| Configuration version | `cfg-sha256-c28c593bd11e484064ea5fe9c8e6086c` |
| Result SHA-256 | `c4150a4141038409d52329fd2fb9cdde5a583c1a0b691b1888213e1aaa2b5b92` |
| Report generated (UTC) | `2026-11-20T06:38:53Z` |
| Report generator | `yukti-m4-report/3I.1` |
| Relationship export | `EXPORTED` |
| Contract decisions enabled for this run | `DEC-001`, `DEC-002`, `DEC-003`, `DEC-004`, `DEC-005`, `DEC-006`, `DEC-007` |
| Enabled without verified team sign-off (proposal only) | `DEC-001`, `DEC-002`, `DEC-003`, `DEC-004`, `DEC-005`, `DEC-006`, `DEC-007` |
| Withheld contract decisions | None |
| Reference speed threshold | 180 km/h |
| Report content SHA-256 | `a61125bd653409a460e19df8b6c99c2904765ae2239c24c8591ef65104067fcb` |

**Counts.** evidence items: 3, evidence verified: 3, evidence excluded: 0, timeline events: 5, spatial observations: 4, movement segments: 1, contradiction leads: 1, relationships: 1, excluded inputs: 0, evidence gaps: 3, limitations: 6, diagnostics error: 0, diagnostics warning: 2, diagnostics info: 2

**Timeline span (UTC).** `[2026-11-10T16:10:00Z, 2026-11-10T16:25:01Z)`

**Analytical leads for review (inferred, not findings of fact):**

- **L1** Analytical lead (SPEED\_THRESHOLD\_EXCEEDANCE\_LEAD) for entity reference 'ent-person-ravi': events 'evt-01-shutter-forced' and 'evt-03-location-shared' are 25.33 km apart; reconciling them needs at least 304.0 km/h against a configured reference of 180 km/h. Not a finding of fact. Sources: `ev-cctv-dock` (SHA-256 `e77a6607dd66…8890`; records `osd:2026-11-10T21:42:00`); `ev-chat-export` (SHA-256 `b162a2806d85…7d81`; records `line:1`)

*Reconstruction output separates observed source facts from derived analytical inferences. Contradiction leads are investigative prompts for human review; they do not establish identity, deception, guilt or intent. Declared clock offsets are applied as supplied and are not independently verified.*

## 2. Evidence inventory

| Evidence | Type | SHA-256 | Integrity | Used | Clock calibration | Events |
| --- | --- | --- | --- | --- | --- | --- |
| `ev-cctv-dock` | CCTV | `e77a6607dd66c74a6f1bca53e372ce4ba9d88b77466d3ca479ca7fefee338890` | `VERIFIED` SHA-256 and size recomputed from the stored bytes during the run and matched the ingestion record. | Yes | `DECLARED_NOT_INDEPENDENTLY_VERIFIED` | 2 |
| `ev-cdr-tower` | CDR | `a4448e5eb36dd93bce2696defbb9e3550bca98150b0802602a7ef90f066f9972` | `VERIFIED` SHA-256 and size recomputed from the stored bytes during the run and matched the ingestion record. | Yes | `NOT_DECLARED_UNCALIBRATED` | 1 |
| `ev-chat-export` | CHAT\_EXPORT | `b162a2806d85cfa2f0c374f7186111129d4ab24ba36d8bf0ffa05e2605747d81` | `VERIFIED` SHA-256 and size recomputed from the stored bytes during the run and matched the ingestion record. | Yes | `DECLARED_NOT_INDEPENDENTLY_VERIFIED` | 2 |

## 3. Timeline of events

*Frame references are temporal alignment only: the frame was captured during the event's uncertainty window from the same source. They do not show or confirm the event. No semantic recognition was performed on any frame.*

### E1. FORCED\_ENTRY: Rear shutter forced open at loading dock

- Basis: **OBSERVED**. Event extracted from the source by Member 1; corrected UTC and window derived once by Member 4 from raw\_timestamp.
- Event id: `evt-01-shutter-forced`
- Raw timestamp: `2026-11-10T21:42:00` (precision `EXACT_SECOND`)
- Corrected UTC: `2026-11-10T16:10:00Z` (precision `EXACT_SECOND`)
- Uncertainty window: `[2026-11-10T16:10:00Z, 2026-11-10T16:10:01Z)`
- Time basis: `SOURCE_TIMEZONE_METADATA`; source timezone `Asia/Kolkata`; clock offset `-120 s`; calibration `DECLARED_NOT_INDEPENDENTLY_VERIFIED`
- Ordering: `STRICTLY_ORDERED`
- Entity references: `ent-person-ravi`
- Coordinates: `28.53550, 77.27100`
- Frames captured during this window: None
- Sources: `ev-cctv-dock` (SHA-256 `e77a6607dd66…8890`; records `osd:2026-11-10T21:42:00`)

### E2. PHONE\_CALL: Outbound call from handset near warehouse

- Basis: **OBSERVED**. Event extracted from the source by Member 1; corrected UTC and window derived once by Member 4 from raw\_timestamp.
- Event id: `evt-02-outbound-call`
- Raw timestamp: `2026-11-10T16:12:00Z` (precision `MINUTE`)
- Corrected UTC: `2026-11-10T16:12:00Z` (precision `MINUTE`)
- Uncertainty window: `[2026-11-10T16:12:00Z, 2026-11-10T16:13:00Z)`
- Time basis: `TIMESTAMP_UTC_OFFSET`; source timezone `Not provided`; clock offset `Not provided`; calibration `NOT_DECLARED_UNCALIBRATED`
- Ordering: `STRICTLY_ORDERED`
- Entity references: `ent-phone-01`
- Coordinates: `28.53600, 77.27000`
- Frames captured during this window: None
- Sources: `ev-cdr-tower` (SHA-256 `a4448e5eb36d…9972`; records `row:2`)

### E3. LOCATION\_SHARE: Live location shared in chat

- Basis: **OBSERVED**. Event extracted from the source by Member 1; corrected UTC and window derived once by Member 4 from raw\_timestamp.
- Event id: `evt-03-location-shared`
- Raw timestamp: `2026-11-10T21:44:00+05:30` (precision `MINUTE`)
- Corrected UTC: `2026-11-10T16:14:00Z` (precision `MINUTE`)
- Uncertainty window: `[2026-11-10T16:14:00Z, 2026-11-10T16:15:00Z)`
- Time basis: `TIMESTAMP_UTC_OFFSET`; source timezone `Asia/Kolkata`; clock offset `+0 s`; calibration `DECLARED_NOT_INDEPENDENTLY_VERIFIED`
- Ordering: `STRICTLY_ORDERED`
- Entity references: `ent-person-ravi`
- Coordinates: `28.45950, 77.02660`
- Frames captured during this window: None
- Sources: `ev-chat-export` (SHA-256 `b162a2806d85…7d81`; records `line:1`)

### E4. MESSAGE: Chat message mentioning pickup vehicle

- Basis: **OBSERVED**. Event extracted from the source by Member 1; corrected UTC and window derived once by Member 4 from raw\_timestamp.
- Event id: `evt-04-vehicle-message`
- Raw timestamp: `2026-11-10T21:50:00` (precision `MINUTE`)
- Corrected UTC: `2026-11-10T16:20:00Z` (precision `MINUTE`)
- Uncertainty window: `[2026-11-10T16:20:00Z, 2026-11-10T16:21:00Z)`
- Time basis: `SOURCE_TIMEZONE_METADATA`; source timezone `Asia/Kolkata`; clock offset `+0 s`; calibration `DECLARED_NOT_INDEPENDENTLY_VERIFIED`
- Ordering: `STRICTLY_ORDERED`
- Entity references: `ent-person-ravi`; unresolved `ent-unknown-77`
- Coordinates: `Not provided`
- Frames captured during this window: None
- Sources: `ev-chat-export` (SHA-256 `b162a2806d85…7d81`; records `line:2`)

### E5. VEHICLE\_DEPARTURE: Van leaves warehouse premises

- Basis: **OBSERVED**. Event extracted from the source by Member 1; corrected UTC and window derived once by Member 4 from raw\_timestamp.
- Event id: `evt-05-van-departs`
- Raw timestamp: `2026-11-10T21:57:00` (precision `EXACT_SECOND`)
- Corrected UTC: `2026-11-10T16:25:00Z` (precision `EXACT_SECOND`)
- Uncertainty window: `[2026-11-10T16:25:00Z, 2026-11-10T16:25:01Z)`
- Time basis: `SOURCE_TIMEZONE_METADATA`; source timezone `Asia/Kolkata`; clock offset `-120 s`; calibration `DECLARED_NOT_INDEPENDENTLY_VERIFIED`
- Ordering: `STRICTLY_ORDERED`
- Entity references: `ent-vehicle-van`
- Coordinates: `28.53550, 77.27100`
- Frames captured during this window: None
- Sources: `ev-cctv-dock` (SHA-256 `e77a6607dd66…8890`; records `osd:2026-11-10T21:57:00`)

## 4. Movement and spatial analysis

### Spatial observations (observed)

| Ref | Entity reference | Event | Coordinates | Window (UTC) | Sources |
| --- | --- | --- | --- | --- | --- |
| S1 | `ent-person-ravi` | `evt-01-shutter-forced` | `28.53550, 77.27100` | `[2026-11-10T16:10:00Z, 2026-11-10T16:10:01Z)` | `ev-cctv-dock` (SHA-256 `e77a6607dd66…8890`; records `osd:2026-11-10T21:42:00`) |
| S2 | `ent-person-ravi` | `evt-03-location-shared` | `28.45950, 77.02660` | `[2026-11-10T16:14:00Z, 2026-11-10T16:15:00Z)` | `ev-chat-export` (SHA-256 `b162a2806d85…7d81`; records `line:1`) |
| S3 | `ent-phone-01` | `evt-02-outbound-call` | `28.53600, 77.27000` | `[2026-11-10T16:12:00Z, 2026-11-10T16:13:00Z)` | `ev-cdr-tower` (SHA-256 `a4448e5eb36d…9972`; records `row:2`) |
| S4 | `ent-vehicle-van` | `evt-05-van-departs` | `28.53550, 77.27100` | `[2026-11-10T16:25:00Z, 2026-11-10T16:25:01Z)` | `ev-cctv-dock` (SHA-256 `e77a6607dd66…8890`; records `osd:2026-11-10T21:57:00`) |

### Movement segments and travel feasibility (inferred)

#### M1. `evt-01-shutter-forced` → `evt-03-location-shared` (entity reference `ent-person-ravi`)

- Basis: **INFERRED**
- Straight-line distance: 25.33 km
- Elapsed time: nominal 240 s, possible range 239 s to 300 s
- Minimum required speed: 304.0 km/h (reference 180 km/h) → `EXCEEDS_CONFIGURED_THRESHOLD`
- Feasibility: Inferred: covering the straight-line distance in the longest possible elapsed time needs at least 304.0 km/h, above the configured reference of 180 km/h. Analytical indicator only; see the contradiction leads.
- Assumption: configured\_speed\_threshold\_kmh is a user-configurable reference assumption for analytical triage and does not establish a physical transit limit or statistical likelihood.
- Sources: `ev-cctv-dock` (SHA-256 `e77a6607dd66…8890`; records `osd:2026-11-10T21:42:00`); `ev-chat-export` (SHA-256 `b162a2806d85…7d81`; records `line:1`)

## 5. Contradictions and inconsistencies (analytical leads)

> Each item below is an ANALYTICAL LEAD produced by a configured rule, not a proven fact. It indicates that two observations are hard to reconcile under the stated assumptions. Possible explanations include clock error, location error, extraction error, shared devices or accounts, and wrong entity linkage. Verify against the cited evidence.

### L1. ANALYTICAL LEAD (inferred, not proven): `SPEED_THRESHOLD_EXCEEDANCE_LEAD`

- Lead id: `lead-31c75fbcff2a7e22b3d15a3be23a3290ebbb0c1b4dcbf14c8f9c8d8149cfe25d`
- Entity reference: `ent-person-ravi`
- Events: `evt-01-shutter-forced` `2026-11-10T16:10:00Z 2026-11-10T16:10:01Z` → `evt-03-location-shared` `2026-11-10T16:14:00Z 2026-11-10T16:15:00Z`
- Derived: distance 25.33 km, longest possible elapsed 300 s, minimum required speed 304.0 km/h vs reference 180 km/h (`EXCEEDS_CONFIGURED_THRESHOLD`)
- Explanation: Observed event 'evt-01-shutter-forced' (evidence 'ev-cctv-dock') at (28.5355, 77.2710) (\[2026-11-10T16:10:00+00:00, 2026-11-10T16:10:01+00:00)) and subsequent strictly ordered event 'evt-03-location-shared' (evidence 'ev-chat-export') at (28.4595, 77.0266) (\[2026-11-10T16:14:00+00:00, 2026-11-10T16:15:00+00:00)) for entity 'ent-person-ravi' are separated by 25.33 km over a conservative maximum elapsed window of 300.0s, requiring at least 304.01 km/h compared against the configured reference threshold of 180.00 km/h.
- Exported relationship: `rel-contra-6d7b1756a205fd04a7d537f3869d2351`
- Lead limitation: The configured speed threshold is a user-adjustable reference benchmark for investigative triage and does not establish a physical upper bound on transit.
- Lead limitation: This indicator is a derived analytical lead for human investigator review and does not establish factual inaccuracy, actor state of mind, or statistical likelihood.
- Lead limitation: Alternative explanations include uncalibrated device clock offsets, shared or transferred device possession, high-speed transport modes, or coarse cell-tower location centroids.
- Sources: `ev-cctv-dock` (SHA-256 `e77a6607dd66…8890`; records `osd:2026-11-10T21:42:00`); `ev-chat-export` (SHA-256 `b162a2806d85…7d81`; records `line:1`)

## 6. Relationships (observed vs inferred)

| Ref | Type | Source → target | Basis | Weight | Sources |
| --- | --- | --- | --- | --- | --- |
| R1 | `CONTRADICTS` | `evt-01-shutter-forced` → `evt-03-location-shared` | **INFERRED** (is_observed false) | Not provided | `ev-cctv-dock` (SHA-256 `e77a6607dd66…8890`; records `osd:2026-11-10T21:42:00`); `ev-chat-export` (SHA-256 `b162a2806d85…7d81`; records `line:1`) |

## 7. Limitations, evidence gaps and pending items

### Limitations

- `REPORT_SCOPE` This report covers only the inputs supplied to this run, processed by the stated algorithm and configuration versions. Evidence not supplied to the run is not reflected, and the absence of a finding does not mean the absence of an event.
- `REFERENCE_SPEED_THRESHOLD_ASSUMPTION` Travel feasibility uses a configured reference speed of 180 km/h and straight-line (haversine) distance. Both are analytical assumptions, not measurements of the actual route or vehicle. Applies to: `seg-ent-person-ravi-evt-01-shutter-forced-to-evt-03-location-shared`.
- `UNCALIBRATED_CLOCKS` No clock offset was declared for these sources. Their timestamps are used as recorded; the clocks are not known to be accurate. Applies to: `ev-cdr-tower`.
- `DECLARED_CLOCK_OFFSETS_NOT_VERIFIED` Declared clock offsets were applied once, as supplied at ingestion. Member 4 does not independently verify them. Applies to: `ev-cctv-dock`, `ev-chat-export`.
- `UNRESOLVED_ENTITY_REFERENCES` Entity references without an exact registration were withheld from trajectories and contradiction analysis. Applies to: `evt-04-vehicle-message`.
- `FRAMES_NOT_SEMANTICALLY_ANALYZED` Video was processed by frame extraction with provenance only. No semantic event recognition was performed; frame links to events are temporal alignment only.

### Evidence gaps

- `EVENT_WITHOUT_COORDINATES` `evt-04-vehicle-message`: Event has no coordinates; it cannot place entities spatially.
- `UNRESOLVED_ENTITY_REFERENCE` `evt-04-vehicle-message`: Entity reference 'ent-unknown-77' has no exact registration.
- `VIDEO_WITHOUT_FRAMES` `ev-cctv-dock`: No frame artifacts were extracted or supplied for this video evidence.

### Clock calibrations (none independently verified)

- `ev-cctv-dock`: `DECLARED_NOT_INDEPENDENTLY_VERIFIED`, offset `-120 s`, timezone `Asia/Kolkata`. Declared offset applied once as supplied at ingestion; not independently verified.
- `ev-cdr-tower`: `NOT_DECLARED_UNCALIBRATED`, offset `Not provided`, timezone `Not provided`. No offset declared; timestamps used as recorded. The clock is not known to be accurate.
- `ev-chat-export`: `DECLARED_NOT_INDEPENDENTLY_VERIFIED`, offset `+0 s`, timezone `Asia/Kolkata`. Declared offset applied once as supplied at ingestion; not independently verified.

### Pending items

- `CONTRACT_DECISION_NOT_TEAM_APPROVED` `DEC-001`: This decision was enabled for the run as a proposal only; no verified team sign-off exists. Findings that depend on it are conditional on its approval.
- `CONTRACT_DECISION_NOT_TEAM_APPROVED` `DEC-002`: This decision was enabled for the run as a proposal only; no verified team sign-off exists. Findings that depend on it are conditional on its approval.
- `CONTRACT_DECISION_NOT_TEAM_APPROVED` `DEC-003`: This decision was enabled for the run as a proposal only; no verified team sign-off exists. Findings that depend on it are conditional on its approval.
- `CONTRACT_DECISION_NOT_TEAM_APPROVED` `DEC-004`: This decision was enabled for the run as a proposal only; no verified team sign-off exists. Findings that depend on it are conditional on its approval.
- `CONTRACT_DECISION_NOT_TEAM_APPROVED` `DEC-005`: This decision was enabled for the run as a proposal only; no verified team sign-off exists. Findings that depend on it are conditional on its approval.
- `CONTRACT_DECISION_NOT_TEAM_APPROVED` `DEC-006`: This decision was enabled for the run as a proposal only; no verified team sign-off exists. Findings that depend on it are conditional on its approval.
- `CONTRACT_DECISION_NOT_TEAM_APPROVED` `DEC-007`: This decision was enabled for the run as a proposal only; no verified team sign-off exists. Findings that depend on it are conditional on its approval.
- `VIDEO_FRAMES_NOT_EXTRACTED` `ev-cctv-dock`: Frame extraction was skipped (no derived-artifact store configured).

### Excluded inputs

- None.

### Errors and warnings

- `WARNING` `VIDEO_STAGE_SKIPPED` `ev-cctv-dock`: No artifact\_root configured; frames were not extracted.
- `WARNING` `ENTITY_REFERENCE_UNRESOLVED` `evt-04-vehicle-message`: Event 'evt-04-vehicle-message' reference 'ent-unknown-77': No registered entity\_id or exact alias matches this reference; it is preserved but excluded from spatial reconstruction. No similarity matching is attempted.

## Appendix A. Chain of custody and provenance

Original evidence is read through a read-only, no-follow descriptor and never modified. Hashes below are those recorded at ingestion and recomputed in the run. Derived frames are written to new directories and never overwrite earlier artifacts.

### Evidence

| Evidence | SHA-256 | Storage reference | Integrity | Video before / after processing |
| --- | --- | --- | --- | --- |
| `ev-cctv-dock` | `e77a6607dd66c74a6f1bca53e372ce4ba9d88b77466d3ca479ca7fefee338890` | `urn:yukti:evidence:ev-cctv-dock:storage` | `VERIFIED` | n/a |
| `ev-cdr-tower` | `a4448e5eb36dd93bce2696defbb9e3550bca98150b0802602a7ef90f066f9972` | `urn:yukti:evidence:ev-cdr-tower:storage` | `VERIFIED` | n/a |
| `ev-chat-export` | `b162a2806d85cfa2f0c374f7186111129d4ab24ba36d8bf0ffa05e2605747d81` | `urn:yukti:evidence:ev-chat-export:storage` | `VERIFIED` | n/a |

### Video processing

- `ev-cctv-dock`: `SKIPPED_NO_ARTIFACT_ROOT`, `FRAME_EXTRACTION_WITH_PROVENANCE`, handoff `Not provided`, 0 frame(s). Frame extraction with provenance only (FFmpeg sampling bound to the source SHA-256). No semantic event recognition was performed and no events were derived from frames.

### Frame artifacts

- None.

### Finding provenance

| Finding | Type | Observed | Evidence (SHA-256) | Source records |
| --- | --- | --- | --- | --- |
| `evt-01-shutter-forced` | `TIMELINE_EVENT` | observed | `ev-cctv-dock` `e77a6607dd66…8890` | `osd:2026-11-10T21:42:00` |
| `evt-02-outbound-call` | `TIMELINE_EVENT` | observed | `ev-cdr-tower` `a4448e5eb36d…9972` | `row:2` |
| `evt-03-location-shared` | `TIMELINE_EVENT` | observed | `ev-chat-export` `b162a2806d85…7d81` | `line:1` |
| `evt-04-vehicle-message` | `TIMELINE_EVENT` | observed | `ev-chat-export` `b162a2806d85…7d81` | `line:2` |
| `evt-05-van-departs` | `TIMELINE_EVENT` | observed | `ev-cctv-dock` `e77a6607dd66…8890` | `osd:2026-11-10T21:57:00` |
| `obs:ent-person-ravi:evt-01-shutter-forced` | `SPATIAL_OBSERVATION` | observed | `ev-cctv-dock` `e77a6607dd66…8890` | `osd:2026-11-10T21:42:00` |
| `obs:ent-person-ravi:evt-03-location-shared` | `SPATIAL_OBSERVATION` | observed | `ev-chat-export` `b162a2806d85…7d81` | `line:1` |
| `obs:ent-phone-01:evt-02-outbound-call` | `SPATIAL_OBSERVATION` | observed | `ev-cdr-tower` `a4448e5eb36d…9972` | `row:2` |
| `obs:ent-vehicle-van:evt-05-van-departs` | `SPATIAL_OBSERVATION` | observed | `ev-cctv-dock` `e77a6607dd66…8890` | `osd:2026-11-10T21:57:00` |
| `seg-ent-person-ravi-evt-01-shutter-forced-to-evt-03-location-shared` | `MOVEMENT_SEGMENT` | inferred | `ev-cctv-dock` `e77a6607dd66…8890`, `ev-chat-export` `b162a2806d85…7d81` | `line:1`, `osd:2026-11-10T21:42:00` |
| `lead-31c75fbcff2a7e22b3d15a3be23a3290ebbb0c1b4dcbf14c8f9c8d8149cfe25d` | `CONTRADICTION_LEAD` | inferred | `ev-cctv-dock` `e77a6607dd66…8890`, `ev-chat-export` `b162a2806d85…7d81` | `line:1`, `osd:2026-11-10T21:42:00` |
| `rel-contra-6d7b1756a205fd04a7d537f3869d2351` | `RELATIONSHIP` | inferred | `ev-cctv-dock` `e77a6607dd66…8890`, `ev-chat-export` `b162a2806d85…7d81` | `line:1`, `osd:2026-11-10T21:42:00` |

*Reconstruction output separates observed source facts from derived analytical inferences. Contradiction leads are investigative prompts for human review; they do not establish identity, deception, guilt or intent. Declared clock offsets are applied as supplied and are not independently verified.*

---

> DECISION SUPPORT FOR INVESTIGATORS. This report is generated automatically from a reconstruction run over the evidence and extraction inputs listed in it. It organises those inputs and the analytical inferences derived from them for review by a qualified investigator. It does not establish the identity of any person, nor guilt, intent, deception or the occurrence of any offence. Contradictions are analytical leads to be checked against the original evidence, not findings of fact. Entity identifiers are registry references, not identifications. Every finding must be verified against the cited source evidence before it is relied upon.
