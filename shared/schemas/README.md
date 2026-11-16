# Canonical Data Contracts & Schemas

This directory defines the 5 primary shared data models that form the backbone of the SUTRAM platform. Backend Pydantic models and Frontend TypeScript types must strictly align with these specifications.

---

## 1. Case
Represents the top-level investigative container.

| Field | Type | Description |
| :--- | :--- | :--- |
| `case_id` | `UUID` / `str` | Unique case identifier |
| `title` | `str` | Human-readable case title |
| `fir_number` | `str` (optional) | First Information Report / incident reference |
| `description` | `str` | Detailed case synopsis |
| `created_at` | `ISO 8601 UTC` | Case creation timestamp |
| `updated_at` | `ISO 8601 UTC` | Last modification timestamp |
| `status` | `CaseStatus` (Enum) | Current state (`ACTIVE`, `UNDER_REVIEW`, `CLOSED`) |
| `investigator_id` | `str` | ID of the lead investigator |

---

## 2. Evidence
Represents an individual physical or digital evidentiary item ingested into a case.

| Field | Type | Description |
| :--- | :--- | :--- |
| `evidence_id` | `UUID` / `str` | Unique evidence identifier |
| `case_id` | `UUID` / `str` | Foreign reference to associated Case |
| `source_type` | `EvidenceType` (Enum) | Modality (`CCTV`, `CDR`, `CHAT_EXPORT`, `DOCUMENT_SCAN`, `DISK_IMAGE`, etc.) |
| `original_filename`| `str` | Original filename at time of seizure |
| `file_size_bytes` | `int` | Exact file size |
| `sha256_hash` | `str` (64 hex) | Cryptographic SHA-256 integrity hash computed at ingestion |
| `storage_uri` | `str` | Internal non-public storage pointer |
| `ingested_at` | `ISO 8601 UTC` | Exact ingestion timestamp |
| `metadata` | `dict` / `JSON` | Format-specific metadata (e.g. EXIF, carrier headers, codec) |

---

## 3. Entity
Represents an identified real-world object, person, device, or digital identity extracted from evidence.

| Field | Type | Description |
| :--- | :--- | :--- |
| `entity_id` | `UUID` / `str` | Unique entity identifier |
| `case_id` | `UUID` / `str` | Parent case reference |
| `entity_type` | `EntityType` (Enum) | Class (`PERSON`, `PHONE_NUMBER`, `VEHICLE`, `LOCATION`, `ORGANIZATION`, `CRYPTO_WALLET`, `EMAIL`) |
| `canonical_name` | `str` | Normalized identifier (e.g., "+919876543210", "John Doe") |
| `aliases` | `list[str]` | Known aliases or alternate spellings |
| `source_evidence_ids`| `list[str]` | Traceability links to evidence sources |
| `confidence` | `float` (0.0 - 1.0) | Extraction confidence score |

---

## 4. Event
Represents a discrete occurrence pinned to time and space, extracted from evidence.

| Field | Type | Description |
| :--- | :--- | :--- |
| `event_id` | `UUID` / `str` | Unique event identifier |
| `case_id` | `UUID` / `str` | Parent case reference |
| `title` | `str` | Short description of occurrence |
| `timestamp` | `ISO 8601 UTC` | Normalized UTC timestamp |
| `timestamp_precision`| `TimePrecision` (Enum)| `EXACT`, `MINUTE`, `HOUR`, `APPROXIMATE` |
| `location_name` | `str` (optional) | Human-readable location description |
| `coordinates` | `GeoPoint` (lat, lon) | Optional GPS coordinates |
| `involved_entity_ids`| `list[str]` | Associated entities participating in this event |
| `source_evidence_id` | `str` | Direct reference to origin evidence |

---

## 5. Relationship
Represents an edge in the investigative Knowledge Graph linking two entities, or linking an entity to an event.

| Field | Type | Description |
| :--- | :--- | :--- |
| `relationship_id` | `UUID` / `str` | Unique relationship edge identifier |
| `source_id` | `str` | Source Entity or Event ID |
| `target_id` | `str` | Target Entity or Event ID |
| `relation_type` | `RelationType` (Enum)| `CALL_MADE`, `PRESENT_AT`, `COMMUNICATED_WITH`, `ASSOCIATE_OF`, `OWNS` |
| `weight` | `float` (0.0 - 1.0) | Strength or correlation confidence |
| `is_observed` | `bool` | `true` if directly documented; `false` if inferred by algorithm |
| `evidence_basis` | `list[str]` | Evidence IDs backing this link |
| `metadata` | `dict` / `JSON` | Additional contextual data (duration, frequency, etc.) |
