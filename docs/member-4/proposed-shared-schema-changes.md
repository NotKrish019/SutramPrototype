# Proposed `shared/schemas/README.md` changes (DEC-001 … DEC-007)

**Status:** `PROPOSED`, not approved. **Owner:** Member 4. **Date:** 2026-11-20 (Phase 3D.2)

These edits were applied to `shared/schemas/README.md` on 2026-11-19, based on decision text
copied from a task brief. That text is not a verifiable team sign-off
(see [`team-contract-approvals-2026-11-19.md`](team-contract-approvals-2026-11-19.md)). Under
AGENTS.md §6 (shared schema governance), the shared file was restored to its committed content
in Phase 3D.2, and the proposed wording is kept here unchanged.

To adopt it: obtain sign-off from Members 1, 2 and 3. Record it in the approvals file (who, when,
where), then apply the diff below to `shared/schemas/README.md`.

```diff
diff --git a/shared/schemas/README.md b/shared/schemas/README.md
index 57db7b6..b23c70d 100644
--- a/shared/schemas/README.md
+++ b/shared/schemas/README.md
@@ -33,7 +33,16 @@ Represents an individual physical or digital evidentiary item ingested into a ca
 | `sha256_hash` | `str` (64 hex) | Cryptographic SHA-256 integrity hash computed at ingestion |
 | `storage_uri` | `str` | Internal non-public storage pointer |
 | `ingested_at` | `ISO 8601 UTC` | Exact ingestion timestamp |
-| `metadata` | `dict` / `JSON` | Format-specific metadata (e.g. EXIF, carrier headers, codec) |
+| `metadata` | `dict` / `JSON` | Format-specific metadata (e.g. EXIF, carrier headers, codec). Standard keys per DEC-006 below |
+
+**Standard `Evidence.metadata` keys (DEC-006, approved 2026-11-19).** All optional. A key that is absent stays absent or null and is never defaulted into a "calibrated" value.
+
+| Key | Type | Description |
+| :--- | :--- | :--- |
+| `source_timezone` | `str` (IANA) | Timezone of the device clock, e.g. `Asia/Kolkata` |
+| `clock_offset_seconds` | `float` | Additive correction (DEC-001): `true_utc = device_time_localized_to_utc + clock_offset_seconds`. A clock running 5 minutes fast has `-300` |
+| `recording_start_timestamp` | `str` (ISO 8601) | Start instant of continuous media (CCTV, dashcam, audio) as shown by the device |
+| `device_coordinates` | `GeoPoint` (`lat`, `lon`) | WGS84 position of a fixed capture device |
 
 ---
 
@@ -60,8 +69,14 @@ Represents a discrete occurrence pinned to time and space, extracted from eviden
 | `event_id` | `UUID` / `str` | Unique event identifier |
 | `case_id` | `UUID` / `str` | Parent case reference |
 | `title` | `str` | Short description of occurrence |
-| `timestamp` | `ISO 8601 UTC` | Normalized UTC timestamp |
-| `timestamp_precision`| `TimePrecision` (Enum)| `EXACT`, `MINUTE`, `HOUR`, `APPROXIMATE` |
+| `timestamp` | `ISO 8601 UTC` | Corrected UTC instant (`timestamp_utc`). Produced **only** by the Member 4 temporal engine from `raw_timestamp` (DEC-007) |
+| `raw_timestamp` | `str` | Timestamp exactly as the source shows it, preserved unchanged by extraction (DEC-007) |
+| `timestamp_precision`| `TimePrecision` (Enum)| `EXACT_SECOND`, `MINUTE`, `HOUR`, `DAY`, `ESTIMATED` (DEC-002) |
+| `source_timezone` | `str` (IANA, optional) | Timezone used to localize a naive `raw_timestamp`; null when not declared (DEC-007) |
+| `clock_offset_seconds` | `float` (optional) | Clock correction applied once (DEC-001); null when not declared (DEC-007) |
+| `duration_seconds` | `float` (optional) | Elapsed duration of the activity, finite and >= 0; kept separate from uncertainty (DEC-005) |
+| `uncertainty_window_start` | `ISO 8601 UTC` (optional) | Inclusive start of the half-open UTC window `[start, end)` (DEC-002, DEC-005) |
+| `uncertainty_window_end` | `ISO 8601 UTC` (optional) | Exclusive end. Both bounds are present or both absent; `start < end`; `start <= timestamp < end` (DEC-005) |
 | `location_name` | `str` (optional) | Human-readable location description |
 | `coordinates` | `GeoPoint` (lat, lon) | Optional GPS coordinates |
 | `involved_entity_ids`| `list[str]` | Associated entities participating in this event |
@@ -77,8 +92,16 @@ Represents an edge in the investigative Knowledge Graph linking two entities, or
 | `relationship_id` | `UUID` / `str` | Unique relationship edge identifier |
 | `source_id` | `str` | Source Entity or Event ID |
 | `target_id` | `str` | Target Entity or Event ID |
-| `relation_type` | `RelationType` (Enum)| `CALL_MADE`, `PRESENT_AT`, `COMMUNICATED_WITH`, `ASSOCIATE_OF`, `OWNS` |
-| `weight` | `float` (0.0 - 1.0) | Strength or correlation confidence |
-| `is_observed` | `bool` | `true` if directly documented; `false` if inferred by algorithm |
+| `relation_type` | `RelationType` (Enum)| `CALL_MADE`, `PRESENT_AT`, `COMMUNICATED_WITH`, `ASSOCIATE_OF`, `OWNS`, `CO_OCCURRED_WITH`, `CONTRADICTS` (DEC-003) |
+| `weight` | `float` (0.0 - 1.0) or `null` | Strength or correlation confidence. `null` when no confidence is computed (e.g. `CONTRADICTS`); never replaced by `0.0` (DEC-004) |
+| `is_observed` | `bool` | `true` if directly documented; `false` if inferred by algorithm (DEC-003: e.g. `CONTRADICTS` is always `false`) |
 | `evidence_basis` | `list[str]` | Evidence IDs backing this link |
 | `metadata` | `dict` / `JSON` | Additional contextual data (duration, frequency, etc.) |
+
+---
+
+## Change log
+
+- **2026-11-19:** Applied the team-approved decisions DEC-001 … DEC-007
+  (`docs/member-4/team-contract-approvals-2026-11-19.md`): Event time fields, precision enum,
+  Relationship taxonomy and nullable weight, and standard Evidence.metadata keys.
```
