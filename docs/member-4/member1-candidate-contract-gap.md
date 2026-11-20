# Member 1 → Member 4 event candidate contract gap

**Status:** `BLOCKED`. Member 1 contract change required. **Raised by:** Member 4 (Phase 3D.2, 2026-11-20)
**Producer:** `backend/app/schemas/extraction.py` → `ExtractedEventCandidate`
(emitted by `app/modules/extraction/cdr.py` and `chat.py` via `extract_evidence`)
**Consumer:** `backend/app/modules/reconstruction/adapters/event_adapter.py` → `CANDIDATE_FIELDS`

## Field-by-field comparison

| Member 4 candidate field | Member 1 `ExtractedEventCandidate` | Status |
| :--- | :--- | :--- |
| `candidate_id` | `candidate_id: str` | Match |
| `source_evidence_id` | `source_evidence_id: str` | Match |
| `source_record_id` | `source_record_id: str` | Match |
| `raw_timestamp` | `raw_timestamp: str` (kept verbatim, DEC-007 compatible) | Match |
| `event_type` | `event_type: str` | Match |
| `involved_entity_raw_ids` | `involved_entity_raw_ids: list[str]` | Match |
| `duration_seconds` | `duration_seconds: Optional[float]` (ge 0) | Match |
| **`timestamp_precision`** | absent | **Gap: required, no safe derivation** |
| **`description`** | absent | **Gap: required, no safe derivation** |
| `coordinates` | absent | OK: nullable in Member 4 contract |
| `extraction_confidence` | absent | OK: nullable in Member 4 contract |
| (none) | `location_name: Optional[str]` | Preserved verbatim (`source_fields_preserved`) |
| (none) | `metadata: dict[str, Any]` | Preserved verbatim (`source_fields_preserved`) |

## Why Member 4 does not map the two missing fields

- **`timestamp_precision`**: the only possible source is the formatting of `raw_timestamp`
  (for example, "has seconds, so `EXACT_SECOND`"). Formatting does not tell you precision.
  Carriers and chat exports pad or round values, and a wrong precision changes the uncertainty
  window and with it the contradiction findings. Inferring it would fabricate an observed fact.
- **`description`**: building one from `event_type` (for example, "TELEPHONY_CALL") would be
  synthesized text presented as extracted content.

## Current behaviour (tested)

`tests/integration/reconstruction/test_real_producers.py` runs the real `extract_evidence` on a
synthetic CDR and passes the real `UnifiedExtractionResult` object to the pipeline:

- every candidate is excluded with `PRODUCER_CONTRACT_GAP`, and no timeline event is produced;
- the diagnostic names the missing fields and the producer fields it did not process
  (`location_name`, `metadata`). Nothing is dropped silently.

When the two fields are supplied, `location_name` and `metadata` are carried verbatim on the
reconstructed event (`source_fields_preserved`) and survive the snapshot round trip.

## Required Member 1 change (proposal; needs team agreement)

Add to `ExtractedEventCandidate`, populated by each parser from what the source actually states:

```python
timestamp_precision: str = Field(
    description="DEC-002 (proposed): EXACT_SECOND | MINUTE | HOUR | DAY | ESTIMATED, "
    "as stated or documented for the source format."
)
description: str = Field(description="Short human-readable summary of the source record.")
```

Note that `timestamp_precision` values depend on DEC-002, which is still `PROPOSED`.
