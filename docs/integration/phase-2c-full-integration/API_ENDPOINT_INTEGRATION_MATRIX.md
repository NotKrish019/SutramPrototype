# YUKTI — API Endpoint Integration Matrix

**Project:** YUKTI (SUTRAM 2026)  
**Document ID:** API-P2C-003  
**Version:** 1.0.0  
**Date:** 2026-11-20  

---

## 1. Rust Production API Endpoints (`backend-rust`)

The canonical backend runs on port 8080 (configurable via `PORT` environment variable). All routes are prefixed with `/api/v1`.

| HTTP Method | Route Path | Purpose / Description | Auth / Scope | Implementation Status | Test Coverage |
|---|---|---|---|---|---|
| `GET` | `/health` | Server health check & component diagnostics | Public | **IMPLEMENTED** | `tests/health.rs`, `integration_runs.rs` |
| `POST` | `/api/v1/cases` | Create a new forensic investigation case | Investigator | **IMPLEMENTED** | `tests/integration_runs.rs` (`test_30`) |
| `GET` | `/api/v1/cases` | List all investigation cases | Investigator | **IMPLEMENTED** | `tests/integration_runs.rs` (`test_30`) |
| `GET` | `/api/v1/cases/:case_id` | Retrieve case metadata and summary by ID | Investigator | **IMPLEMENTED** | `tests/integration_runs.rs` (`test_30`) |
| `GET` | `/api/v1/cases/:case_id/evidence` | List all evidence exhibits scoped to a case | Investigator | **IMPLEMENTED** | `tests/integration_runs.rs` (`test_31`) |
| `POST` | `/api/v1/evidence/upload` | Multipart evidence upload & streaming SHA-256 | Investigator | **IMPLEMENTED** | `tests/ingestion.rs`, `integration_runs.rs` |
| `POST` | `/api/v1/evidence/upload?auto_extract=true` | Upload evidence and synchronously extract | Investigator | **IMPLEMENTED** | `tests/integration_runs.rs` (`test_01`, `02`) |
| `GET` | `/api/v1/evidence/:id` | Get evidence item metadata and manifest | Investigator | **IMPLEMENTED** | `tests/ingestion.rs`, `integration_runs.rs` |
| `POST` | `/api/v1/evidence/:id/extract` | Trigger extraction for an existing evidence item | Investigator | **IMPLEMENTED** | `tests/extraction.rs`, `integration_runs.rs` |
| `GET` | `/api/v1/evidence/:id/extraction` | Get active extraction result for an exhibit | Investigator | **IMPLEMENTED** | `tests/extraction.rs`, `integration_runs.rs` |
| `GET` | `/api/v1/evidence/:id/extraction/runs`| Get historical extraction runs for an exhibit | Investigator | **IMPLEMENTED** | `tests/integration_runs.rs` (`test_08`) |
| `POST` | `/api/v1/evidence/:id/reprocess` | Reprocess exhibit with new timezone/clock offset | Investigator | **IMPLEMENTED** | `tests/integration_runs.rs` (`test_11`-`14`) |
| `POST` | `/api/v1/reconstruction/run` | Execute temporal & spatial reconstruction | Investigator | **IMPLEMENTED** | `tests/reconstruction_tests.rs` |
| `GET` | `/api/v1/reconstruction/:case_id`| Retrieve versioned reconstruction result | Investigator | **IMPLEMENTED** | `tests/reconstruction_tests.rs` |
| `GET` | `/api/v1/cases/:case_id/graph` | Retrieve full knowledge graph for case | Investigator | **IMPLEMENTED** | `tests/graph_tests.rs` |
| `GET` | `/api/v1/cases/:case_id/graph/neighborhood/:entity_id` | Retrieve 1-hop / 2-hop entity neighborhood | Investigator | **IMPLEMENTED** | `tests/graph_tests.rs` |
| `GET` | `/api/v1/cases/:case_id/graph/paths` | Trace shortest or all paths between two entities | Investigator | **IMPLEMENTED** | `tests/graph_tests.rs` |
| `POST` | `/api/v1/cases/:case_id/graph/project` | Project extraction run into Neo4j graph | Investigator | **IMPLEMENTED** | `tests/graph_tests.rs` |

---

## 2. API Contract Payloads & DTO Specifications

### 2.1 Case Creation (`POST /api/v1/cases`)
**Request:**
```json
{
  "title": "Operation Cyber Track",
  "description": "Multi-exhibit digital forensics correlation",
  "case_number": "CASE-2026-DEL-0091",
  "lead_investigator": "Insp. R. Sharma"
}
```
**Response (201 Created):**
```json
{
  "case_id": "c7a8e234-90ab-4c12-89cd-ef1234567890",
  "title": "Operation Cyber Track",
  "description": "Multi-exhibit digital forensics correlation",
  "case_number": "CASE-2026-DEL-0091",
  "lead_investigator": "Insp. R. Sharma",
  "status": "OPEN",
  "created_at": "2026-11-20T02:40:00Z"
}
```

### 2.2 Evidence Upload (`POST /api/v1/evidence/upload?auto_extract=true`)
**Request:**
Multipart form-data:
- `file`: Raw exhibit binary / CSV / JSON
- `case_id` (optional query or form parameter): Scope exhibit to case

**Response (200 OK or 201 Created):**
```json
{
  "receipt": {
    "evidence_id": "ev-01934abc-7def-8901-2345-6789abcdef01",
    "case_id": "c7a8e234-90ab-4c12-89cd-ef1234567890",
    "original_filename": "cdr_tower_south.csv",
    "mime_type": "text/csv",
    "byte_size": 624,
    "sha256_hash": "4a7138...e831",
    "ingestion_status": "VERIFIED",
    "ingested_at": "2026-11-20T02:45:12Z"
  },
  "extraction": {
    "run_id": "run-01934abc-90ef-8901-2345-6789abcdef02",
    "status": "SUCCESS",
    "entity_count": 4,
    "event_count": 5,
    "relationship_count": 4,
    "validation_issues": []
  }
}
```

### 2.3 Case-Scoped Evidence Listing (`GET /api/v1/cases/:case_id/evidence`)
**Response (200 OK):**
```json
[
  {
    "evidence_id": "ev-01934abc-7def-8901-2345-6789abcdef01",
    "case_id": "c7a8e234-90ab-4c12-89cd-ef1234567890",
    "original_filename": "cdr_tower_south.csv",
    "mime_type": "text/csv",
    "byte_size": 624,
    "sha256_hash": "4a7138...e831",
    "ingestion_status": "VERIFIED",
    "ingested_at": "2026-11-20T02:45:12Z"
  }
]
```

---

## 3. Python Reference Backend Comparison Matrix

| Feature / Domain | Python Reference Route | Rust Production Route | Parity Status |
|---|---|---|---|
| Health Check | `GET /health` | `GET /health` | Full Parity |
| Ingestion Upload | `POST /api/v1/evidence/upload` | `POST /api/v1/evidence/upload` | Full Parity (+ streaming) |
| Evidence Metadata | `GET /api/v1/evidence/{id}` | `GET /api/v1/evidence/:id` | Full Parity |
| Extraction Trigger | `POST /api/v1/evidence/{id}/extract`| `POST /api/v1/evidence/:id/extract` | Full Parity |
| Extraction Result | `GET /api/v1/evidence/{id}/extraction` | `GET /api/v1/evidence/:id/extraction` | Full Parity |
| Historical Runs | `GET /api/v1/evidence/{id}/extraction/runs` | `GET /api/v1/evidence/:id/extraction/runs` | Full Parity |
| Evidence Reprocess | `POST /api/v1/evidence/{id}/reprocess` | `POST /api/v1/evidence/:id/reprocess` | Full Parity |
| Case Creation | N/A (Python relied on implicit case ids) | `POST /api/v1/cases` | Exceeds Python (Full CRUD) |
| Case Evidence Scoping | N/A (unscoped in Python) | `GET /api/v1/cases/:case_id/evidence` | Exceeds Python |
| Graph Projection | N/A (Python had experimental script) | `POST /api/v1/cases/:case_id/graph/project` | Exceeds Python (Native Neo4j Engine) |
| Graph Query / Paths | N/A | `GET /api/v1/cases/:case_id/graph/paths` | Exceeds Python |
| Reconstruction | `POST /api/v1/reconstruction/timeline` | `POST /api/v1/reconstruction/run` | Full Parity + DEC-001..006 |
