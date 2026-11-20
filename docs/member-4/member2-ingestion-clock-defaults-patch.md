# Proposed Member 2 patch: no undeclared clock defaults, plus Evidence contract alignment

**Status:** `PROPOSED` for Member 2. Not applied (Member 4 does not modify Member 2 files).
**Raised by:** Member 4 (Phase 3D.2, 2026-11-20)

## 1. Undeclared `source_timezone` / `clock_offset_seconds` defaults

Today, every Evidence record gets `source_timezone="UTC"` and `clock_offset_seconds=0.0` in
`metadata`, whether or not anyone declared them. A consumer cannot tell "the uploader said the
device runs on UTC with no drift" from "nobody said anything". A naive timestamp from an IST
device is then normalized as if it were UTC, which is 5 h 30 min wrong, and is reported as a
declared calibration. This conflicts with DEC-006 as proposed (absent keys stay absent).

### Proposed diff

```diff
--- a/backend/app/modules/ingestion/service.py
+++ b/backend/app/modules/ingestion/service.py
@@ async def ingest_evidence(
         source_type: Optional[str] = None,
-        source_timezone: str = "UTC",
-        clock_offset_seconds: float = 0.0,
+        source_timezone: Optional[str] = None,
+        clock_offset_seconds: Optional[float] = None,
         metadata: Optional[dict[str, Any]] = None,
@@
         # 5. Build metadata payload
         merged_metadata = dict(metadata or {})
-        merged_metadata.update({
-            "source_timezone": source_timezone,
-            "clock_offset_seconds": clock_offset_seconds,
-            "content_detection_reason": reason,
-        })
+        # Write clock keys only when actually declared. Absent stays absent (DEC-006).
+        if source_timezone is not None:
+            merged_metadata["source_timezone"] = source_timezone
+        if clock_offset_seconds is not None:
+            merged_metadata["clock_offset_seconds"] = clock_offset_seconds
+        merged_metadata["content_detection_reason"] = reason
@@ async def execute_extraction_run(
         *,
-        source_timezone: str = "UTC",
-        clock_offset_seconds: float = 0.0,
+        source_timezone: Optional[str] = None,
+        clock_offset_seconds: Optional[float] = None,
         custom_params: Optional[dict[str, Any]] = None,
--- a/backend/app/api/routes/evidence.py
+++ b/backend/app/api/routes/evidence.py
@@ async def upload_evidence(
-    source_timezone: str = Form(
-        "UTC",
+    source_timezone: Optional[str] = Form(
+        None,
         description="IANA timezone (e.g. 'Asia/Kolkata') for device clock normalization per DEC-006.",
     ),
-    clock_offset_seconds: float = Form(
-        0.0,
+    clock_offset_seconds: Optional[float] = Form(
+        None,
         description="Signed additive clock correction in seconds per DEC-001 (nominal = device + offset).",
     ),
@@ async def reprocess_evidence(
     evidence_id: str,
-    source_timezone: str = Form("UTC"),
-    clock_offset_seconds: float = Form(0.0),
+    source_timezone: Optional[str] = Form(None),
+    clock_offset_seconds: Optional[float] = Form(None),
 ) -> ExtractionRunDTO:
```

Member 2 should also check that `EvidenceExtractionRequest` (Member 1) already accepts `None`
for both fields. It does: both are `Optional[...] = None`.

### Member 4 interim safeguard (implemented)

Until the patch lands, on records that carry Member 2's ingestion marker
(`metadata.content_detection_reason`), Member 4 treats `source_timezone == "UTC"` and
`clock_offset_seconds == 0.0` as **undeclared** (`evidence_adapter.MEMBER2_UNDECLARED_TIME_DEFAULTS`).
It emits the WARNING diagnostic `CLOCK_METADATA_INGESTION_DEFAULT_IGNORED`. Naive timestamps
from such evidence are rejected (`SOURCE_TIMEZONE_UNKNOWN`) instead of being read as UTC.
The cost: a genuine "UTC, zero drift" declaration made through Member 2 is also treated as
unknown until the patch lands. That is the safe direction. Remove the safeguard together with
the patch.

Tests: `tests/integration/reconstruction/test_real_producers.py`
(`test_undeclared_member2_defaults_do_not_normalize_ist_device_time_as_utc`,
`test_explicit_member2_timezone_and_offset_are_preserved_and_applied_once`) and
`tests/reconstruction/adapters/test_evidence_adapter.py::test_member2_ingestion_defaults_treated_as_unknown`.

## 2. Evidence record contract gaps (real `EvidenceRecord` → Member 4)

`test_member2_real_evidence_record_is_blocked_by_storage_contract` feeds the real record from
`EvidenceIngestionService.ingest_evidence` to the pipeline. The evidence is excluded. Gaps, in
the order the validator reports them:

| # | Member 2 `EvidenceRecord` | Shared `Evidence` (`shared/schemas/README.md`) / Member 4 | Needed |
| :--- | :--- | :--- | :--- |
| 1 | `source_type = "CALL_DETAIL_RECORD"` (inferred by sniffing) | `EvidenceType` enum: `CDR`, `CHAT_EXPORT`, … | Member 2 emits the committed enum values (or the team agrees a mapping) |
| 2 | no `storage_uri`; `storage_path` = absolute host path | `storage_uri` = canonical key or `urn:yukti:evidence:{id}:storage` | Member 2 exposes `storage_uri` |
| 3 | bytes at `{base}/{case}/{evidence}/{sanitized_filename}` | bytes at `data/evidence/raw/{case}/{evidence}/{evidence}.raw` under the storage root | Agree one storage layout (or a Member 2 staging step) |
| 4 | `created_at` | `ingested_at` | Rename or alias |

Member 4 does not bridge these by trusting absolute host paths or by guessing an enum mapping.
