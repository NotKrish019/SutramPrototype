# YUKTI — Phase 1J.2: SQLx Database Infrastructure, Migrations, and Repositories

## Architectural Record & Implementation Documentation

- **System:** YUKTI Digital Forensics Platform
- **Phase:** Phase 1J.2 (SQLx Database Foundation, Migrations & Repositories)
- **Author:** Lead Rust Backend & Database Architect
- **Status:** APPROVED & IMPLEMENTED
- **Date:** 2026-11-18

---

## 1. Executive Summary & Objective

Phase 1J.2 implements the database persistence foundation for the YUKTI Rust backend. Following the normalized relational schema designed in Phase 1J.1, this phase establishes:
1. **SQLx Integration**: Type-safe asynchronous database access using `sqlx 0.8` configured with `runtime-tokio`, `tls-rustls`, `postgres`, `uuid`, `chrono`, `json`, `migrate`, and `macros`.
2. **Deterministic Driver Selection**: Explicit selection of `tls-rustls` (pure-Rust TLS) ensuring zero dependency on host OpenSSL libraries or GCC C runtime shims, maintaining strict compatibility with the `x86_64-pc-windows-gnullvm` toolchain.
3. **Database Migrations**: Versioned, immutable SQL migration (`20261118000001_initial_forensic_schema.sql`) defining all 10 forensic tables, foreign key constraints, active run partial unique index, and performance indexes.
4. **Typed Repositories**: High-performance asynchronous repository layer encapsulating all SQL operations with strict Rust domain type conversion.
5. **Transactional Multi-Table Writes**: Atomic persistence of extraction runs with `SELECT FOR UPDATE` run concurrency control and atomic multi-table insertion of identifiers, entities, events, relationships, and validation issues.
6. **Cryptographic Chain of Custody**: Append-only custody ledger repository with deterministic SHA-256 hash chaining.
7. **Continuous Integration**: PostgreSQL 16 Alpine service container configuration in GitHub Actions.

---

## 2. Dependencies and Feature Flags

The Rust backend `Cargo.toml` has been augmented with `sqlx`:
```toml
sqlx = { version = "0.8", default-features = false, features = [
    "runtime-tokio",
    "tls-rustls",
    "postgres",
    "uuid",
    "chrono",
    "json",
    "migrate",
    "macros"
] }
```

### Justification of Configuration:
- `runtime-tokio`: Integrates seamlessly with Axum and the existing Tokio asynchronous runtime.
- `tls-rustls`: Eliminates external dynamic link dependencies (such as `libcrypto` or `libssl`) on Windows LLVM-MinGW and cross-compilation environments.
- `postgres`: Direct PostgreSQL protocol driver.
- `uuid`, `chrono`, `json`: Automatic mapping of SQL types (`UUID`, `TIMESTAMPTZ`, `JSONB`) to Rust types (`uuid::Uuid`, `chrono::DateTime<Utc>`, `serde_json::Value`).
- `migrate`: Embedded migration runner via `sqlx::migrate!()`.
- `macros`: Procedural derive macros including `#[derive(sqlx::FromRow)]`.
- **Runtime Query Validation**: Queries are formulated using runtime SQL constructs (`sqlx::query_as` and `sqlx::query`) rather than compile-time checked queries (`sqlx::query!`). This architecture allows compilation in environments without an active database daemon (e.g. initial build machines or offline CI check stages) while retaining full runtime type safety.

---

## 3. Database Connection & Pool Management (`persistence::database`)

The `Database` abstraction provides:
- Thread-safe `PgPool` wrapper.
- Configurable connection parameters (`DATABASE_URL`, pool max/min connections, connection timeout).
- Health checking (`ping()` method issuing `SELECT 1`).
- Migration execution (`run_migrations()` applying all migrations in `backend-rust/migrations/`).
- Graceful shutdown (`close()` terminating all idle pool connections).

```rust
pub struct Database {
    pool: PgPool,
}

impl Database {
    pub async fn connect(config: &DatabaseConfig) -> Result<Self, AppError>;
    pub async fn run_migrations(&self) -> Result<(), AppError>;
    pub async fn ping(&self) -> Result<(), AppError>;
    pub fn pool(&self) -> &PgPool;
    pub async fn close(&self);
}
```

---

## 4. Repository Architecture

### 4.1 CaseRepository
- `create_case`: Inserts new case with classification and strict metadata validation.
- `get_case`: Queries case by `case_id`.
- `list_cases`: Paginated or full list of cases.
- `ensure_case_exists`: Idempotent helper ensuring default or specified case exists before evidence ingestion.

### 4.2 EvidenceRepository
- `register_evidence`: Inserts new evidence item with file metadata, hashes (`sha256`, `sha3_256`, `md5`), and initial status `INGESTED`.
- `get_evidence`: Fetches evidence by `evidence_id`.
- `find_case_id_for_evidence`: Resolves the parent `case_id` for an evidence item.
- `list_evidence_for_case`: Returns all exhibits registered under a case.
- `update_status`: Atomic status transitions (`INGESTED` -> `EXTRACTING` -> `EXTRACTED` / `FAILED`).

### 4.3 ExtractionRunRepository
- `create_run`: Inserts a new run record with `status = 'RUNNING'` and `is_active = FALSE`.
- `get_run`: Retrieves an extraction run and its full JSONB snapshot.
- `list_runs_for_evidence`: Retrieves the versioned run history for an evidence item in reverse chronological order.
- `get_active_run_for_evidence`: Fetches the currently designated active run for an exhibit.
- `complete_run_transactional`: **Atomic Multi-Table Transaction**:
  1. Opens a database transaction (`pool.begin()`).
  2. Acquires row-level lock on `evidence` (`SELECT evidence_id FROM evidence WHERE evidence_id = $1 FOR UPDATE`).
  3. Updates all previous runs for this evidence to `is_active = FALSE`.
  4. Updates the run record to `status = 'COMPLETED'`, `is_active = TRUE`, writes summary counts, sets `completed_at = NOW()`, and stores the complete JSONB snapshot in `result_snapshot`.
  5. Inserts all normalized extracted identifiers into `extracted_identifiers`.
  6. Inserts all canonical resolved entities into `resolved_entities`.
  7. Inserts all chronologically ordered timeline events into `timeline_events`.
  8. Inserts all provisional cross-source relationships into `provisional_relationships`.
  9. Inserts all extraction anomalies and warnings into `validation_issues`.
  10. Commits the transaction atomically.

### 4.4 CustodyRepository
- Implements an append-only ledger for ISO/IEC 27037 forensic integrity.
- Each event cryptographically links to the previous event via SHA-256 hash chaining:
  $$\text{event\_hash} = \text{SHA256}(\text{action} \parallel \text{evidence\_id} \parallel \text{case\_id} \parallel \text{actor} \parallel \text{timestamp} \parallel \text{prev\_event\_hash} \parallel \text{details})$$
- The initial entry uses the 64-zero genesis hash.

---

## 5. Verification Matrix & Quality Gate

| Area | Requirement | Status | Notes |
| :--- | :--- | :--- | :--- |
| Driver Safety | Pure-Rust TLS without C OpenSSL | PASSED | `tls-rustls` configured |
| Migrations | 10 core tables, partial unique active index | PASSED | Migration `20261118000001` created |
| Repositories | Typed CRUD + Transactional Completion | PASSED | All 4 repositories implemented |
| Concurrency | Row lock + partial index enforcement | PASSED | `SELECT FOR UPDATE` + index |
| CI Pipeline | PostgreSQL 16 container service | PASSED | GitHub Actions updated |
| Error Handling | Transparent domain error mapping | PASSED | `AppError` integration |
