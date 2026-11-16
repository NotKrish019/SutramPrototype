# Canonical JSON Schema Examples

This directory provides representative JSON payloads demonstrating how the 5 primary entities interact during analysis.

---

### Example: Integrated Evidence & Extracted Fact

```json
{
  "case": {
    "case_id": "c7f4e912-9843-41a6-9810-74e1e32d8491",
    "title": "Operation CyberTrail - Syndicate Investigation",
    "status": "ACTIVE",
    "created_at": "2026-11-16T10:00:00Z"
  },
  "evidence": {
    "evidence_id": "ev-001",
    "case_id": "c7f4e912-9843-41a6-9810-74e1e32d8491",
    "source_type": "CDR",
    "original_filename": "cdr_tower_delhi_south_20261110.csv",
    "file_size_bytes": 1048576,
    "sha256_hash": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    "ingested_at": "2026-11-16T10:15:00Z"
  },
  "entity": {
    "entity_id": "ent-101",
    "case_id": "c7f4e912-9843-41a6-9810-74e1e32d8491",
    "entity_type": "PHONE_NUMBER",
    "canonical_name": "+919811002233",
    "aliases": ["Target Bravo Phone"],
    "source_evidence_ids": ["ev-001"],
    "confidence": 1.0
  },
  "event": {
    "event_id": "evt-501",
    "case_id": "c7f4e912-9843-41a6-9810-74e1e32d8491",
    "title": "Outbound Call to +919899887766",
    "timestamp": "2026-11-10T21:45:12Z",
    "timestamp_precision": "EXACT_SECOND",
    "location_name": "South Extension Tower 4B",
    "involved_entity_ids": ["ent-101"],
    "source_evidence_id": "ev-001"
  },
  "relationship": {
    "relationship_id": "rel-901",
    "source_id": "ent-101",
    "target_id": "ent-102",
    "relation_type": "COMMUNICATED_WITH",
    "weight": 0.95,
    "is_observed": true,
    "evidence_basis": ["ev-001"],
    "metadata": {
      "call_duration_seconds": 184
    }
  }
}
```
