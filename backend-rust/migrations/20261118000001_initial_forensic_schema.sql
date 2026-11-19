-- YUKTI Forensic Schema Migration: Initial Relational Foundation
-- Version: 20261118000001
-- Target: PostgreSQL 16+

-- 1. Cases Table
CREATE TABLE IF NOT EXISTS cases (
    case_id UUID PRIMARY KEY,
    title VARCHAR(255) NOT NULL,
    description TEXT,
    fir_number VARCHAR(100),
    status VARCHAR(50) NOT NULL DEFAULT 'DRAFT',
    investigator_id VARCHAR(100),
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_case_status CHECK (status IN ('DRAFT', 'ACTIVE', 'UNDER_REVIEW', 'CLOSED', 'ARCHIVED'))
);

CREATE INDEX IF NOT EXISTS idx_cases_status ON cases(status);
CREATE INDEX IF NOT EXISTS idx_cases_fir_number ON cases(fir_number);

-- 2. Evidence Table
-- NOTE: No uniqueness constraint on (case_id, sha256_hash) to permit distinct exhibits with identical content.
CREATE TABLE IF NOT EXISTS evidence (
    evidence_id UUID PRIMARY KEY,
    case_id UUID NOT NULL REFERENCES cases(case_id) ON DELETE RESTRICT,
    original_filename VARCHAR(255) NOT NULL,
    source_type VARCHAR(50) NOT NULL,
    mime_type VARCHAR(100) NOT NULL,
    file_size_bytes BIGINT NOT NULL CHECK (file_size_bytes > 0),
    sha256_hash VARCHAR(64) NOT NULL CHECK (length(sha256_hash) = 64),
    storage_uri VARCHAR(255) NOT NULL,
    source_timezone VARCHAR(100) NOT NULL DEFAULT 'UTC',
    clock_offset_seconds DOUBLE PRECISION NOT NULL DEFAULT 0.0,
    ingestion_status VARCHAR(50) NOT NULL DEFAULT 'INGESTED',
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_evidence_ingestion_status CHECK (
        ingestion_status IN ('INGESTED', 'EXTRACTION_PENDING', 'EXTRACTED', 'EXTRACTION_FAILED', 'CORRUPTED')
    )
);

CREATE INDEX IF NOT EXISTS idx_evidence_case_id ON evidence(case_id);
CREATE INDEX IF NOT EXISTS idx_evidence_sha256 ON evidence(sha256_hash);
CREATE INDEX IF NOT EXISTS idx_evidence_created_at ON evidence(created_at);

-- 3. Extraction Runs Table
CREATE TABLE IF NOT EXISTS extraction_runs (
    extraction_run_id UUID PRIMARY KEY,
    evidence_id UUID NOT NULL REFERENCES evidence(evidence_id) ON DELETE RESTRICT,
    case_id UUID NOT NULL REFERENCES cases(case_id) ON DELETE RESTRICT,
    source_sha256 VARCHAR(64) NOT NULL,
    extractor_name VARCHAR(100) NOT NULL,
    extractor_version VARCHAR(50) NOT NULL,
    configuration_hash VARCHAR(64) NOT NULL,
    configuration JSONB NOT NULL,
    status VARCHAR(50) NOT NULL,
    is_active BOOLEAN NOT NULL DEFAULT FALSE,
    started_at TIMESTAMPTZ NOT NULL,
    completed_at TIMESTAMPTZ,
    duration_ms DOUBLE PRECISION,
    records_processed INTEGER NOT NULL DEFAULT 0,
    valid_records_count INTEGER NOT NULL DEFAULT 0,
    rejected_records_count INTEGER NOT NULL DEFAULT 0,
    validation_issue_count INTEGER NOT NULL DEFAULT 0,
    result_sha256 VARCHAR(64),
    result_snapshot JSONB,
    error_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_extraction_run_status CHECK (
        status IN ('PENDING', 'RUNNING', 'SUCCESS', 'PARTIAL_SUCCESS', 'FAILED', 'INTERRUPTED')
    )
);

CREATE INDEX IF NOT EXISTS idx_extraction_runs_evidence_started ON extraction_runs(evidence_id, started_at);
CREATE INDEX IF NOT EXISTS idx_extraction_runs_config_hash ON extraction_runs(configuration_hash);

-- Active Run Partial Unique Index: Exactly 0 or 1 active run per evidence item.
CREATE UNIQUE INDEX IF NOT EXISTS idx_extraction_runs_active_per_evidence 
ON extraction_runs (evidence_id) 
WHERE is_active = TRUE;

-- 4. Extracted Identifiers Table
CREATE TABLE IF NOT EXISTS extracted_identifiers (
    identifier_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    extraction_run_id UUID NOT NULL REFERENCES extraction_runs(extraction_run_id) ON DELETE RESTRICT,
    evidence_id UUID NOT NULL REFERENCES evidence(evidence_id) ON DELETE RESTRICT,
    case_id UUID NOT NULL REFERENCES cases(case_id) ON DELETE RESTRICT,
    source_record_id VARCHAR(100) NOT NULL,
    identifier_type VARCHAR(50) NOT NULL,
    raw_value TEXT NOT NULL,
    normalized_value TEXT,
    is_ambiguous BOOLEAN NOT NULL DEFAULT FALSE,
    location_reference VARCHAR(100),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_identifiers_lookup ON extracted_identifiers(identifier_type, normalized_value);
CREATE INDEX IF NOT EXISTS idx_identifiers_run ON extracted_identifiers(extraction_run_id);

-- 5. Resolved Entities Table
CREATE TABLE IF NOT EXISTS resolved_entities (
    entity_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    extraction_run_id UUID NOT NULL REFERENCES extraction_runs(extraction_run_id) ON DELETE RESTRICT,
    case_id UUID NOT NULL REFERENCES cases(case_id) ON DELETE RESTRICT,
    entity_type VARCHAR(50) NOT NULL,
    canonical_value TEXT NOT NULL,
    observed_identifiers JSONB NOT NULL DEFAULT '[]'::jsonb,
    confidence DOUBLE PRECISION NOT NULL CHECK (confidence >= 0.0 AND confidence <= 1.0),
    evidence_references JSONB NOT NULL DEFAULT '[]'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_entities_case_canonical ON resolved_entities(case_id, canonical_value);
CREATE INDEX IF NOT EXISTS idx_entities_run ON resolved_entities(extraction_run_id);

-- 6. Events Table
CREATE TABLE IF NOT EXISTS events (
    event_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    extraction_run_id UUID NOT NULL REFERENCES extraction_runs(extraction_run_id) ON DELETE RESTRICT,
    evidence_id UUID NOT NULL REFERENCES evidence(evidence_id) ON DELETE RESTRICT,
    case_id UUID NOT NULL REFERENCES cases(case_id) ON DELETE RESTRICT,
    source_record_id VARCHAR(100) NOT NULL,
    event_type VARCHAR(100) NOT NULL,
    nominal_timestamp TIMESTAMPTZ NOT NULL,
    timestamp_precision VARCHAR(50) NOT NULL,
    duration_seconds DOUBLE PRECISION CHECK (duration_seconds IS NULL OR duration_seconds >= 0.0),
    uncertainty_window_start TIMESTAMPTZ,
    uncertainty_window_end TIMESTAMPTZ,
    latitude DOUBLE PRECISION CHECK (latitude IS NULL OR (latitude >= -90.0 AND latitude <= 90.0)),
    longitude DOUBLE PRECISION CHECK (longitude IS NULL OR (longitude >= -180.0 AND longitude <= 180.0)),
    involved_entity_ids JSONB NOT NULL DEFAULT '[]'::jsonb,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    is_observed BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_event_uncertainty_window CHECK (
        uncertainty_window_start IS NULL 
        OR uncertainty_window_end IS NULL 
        OR (uncertainty_window_start < uncertainty_window_end 
            AND uncertainty_window_start <= nominal_timestamp 
            AND nominal_timestamp <= uncertainty_window_end)
    )
);

CREATE INDEX IF NOT EXISTS idx_events_timeline ON events(case_id, nominal_timestamp);
CREATE INDEX IF NOT EXISTS idx_events_run ON events(extraction_run_id);

-- 7. Relationships Table
CREATE TABLE IF NOT EXISTS relationships (
    relationship_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    extraction_run_id UUID NOT NULL REFERENCES extraction_runs(extraction_run_id) ON DELETE RESTRICT,
    case_id UUID NOT NULL REFERENCES cases(case_id) ON DELETE RESTRICT,
    source_id VARCHAR(100) NOT NULL,
    source_kind VARCHAR(50) NOT NULL CHECK (source_kind IN ('ENTITY', 'EVENT')),
    target_id VARCHAR(100) NOT NULL,
    target_kind VARCHAR(50) NOT NULL CHECK (target_kind IN ('ENTITY', 'EVENT')),
    relation_type VARCHAR(50) NOT NULL,
    weight DOUBLE PRECISION CHECK (weight IS NULL OR (weight >= 0.0 AND weight <= 1.0)),
    is_observed BOOLEAN NOT NULL DEFAULT TRUE,
    evidence_references JSONB NOT NULL DEFAULT '[]'::jsonb,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_relationships_pair ON relationships(source_id, target_id);
CREATE INDEX IF NOT EXISTS idx_relationships_run ON relationships(extraction_run_id);

-- 8. Validation Issues Table
CREATE TABLE IF NOT EXISTS validation_issues (
    issue_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    extraction_run_id UUID NOT NULL REFERENCES extraction_runs(extraction_run_id) ON DELETE RESTRICT,
    evidence_id UUID NOT NULL REFERENCES evidence(evidence_id) ON DELETE RESTRICT,
    case_id UUID NOT NULL REFERENCES cases(case_id) ON DELETE RESTRICT,
    record_id VARCHAR(100),
    field_name VARCHAR(100) NOT NULL,
    issue_type VARCHAR(100) NOT NULL,
    raw_value TEXT,
    message TEXT NOT NULL,
    row_index BIGINT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_validation_issues_run ON validation_issues(extraction_run_id);

-- 9. Custody Events Table (Append-Only Ledger)
CREATE TABLE IF NOT EXISTS custody_events (
    custody_id BIGSERIAL PRIMARY KEY,
    event_id_uuid UUID NOT NULL DEFAULT gen_random_uuid(),
    evidence_id UUID NOT NULL REFERENCES evidence(evidence_id) ON DELETE RESTRICT,
    case_id UUID NOT NULL REFERENCES cases(case_id) ON DELETE RESTRICT,
    action VARCHAR(100) NOT NULL,
    actor VARCHAR(100) NOT NULL,
    payload_sha256 VARCHAR(64),
    details JSONB NOT NULL DEFAULT '{}'::jsonb,
    prev_event_hash VARCHAR(64),
    event_hash VARCHAR(64) NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_custody_evidence ON custody_events(evidence_id, timestamp);

-- 10. Reconstruction Runs & Artifacts (Phase 1K Preparation)
CREATE TABLE IF NOT EXISTS reconstruction_runs (
    reconstruction_run_id UUID PRIMARY KEY,
    extraction_run_id UUID NOT NULL REFERENCES extraction_runs(extraction_run_id) ON DELETE RESTRICT,
    case_id UUID NOT NULL REFERENCES cases(case_id) ON DELETE RESTRICT,
    reconstruction_type VARCHAR(50) NOT NULL,
    status VARCHAR(50) NOT NULL,
    parameters JSONB NOT NULL DEFAULT '{}'::jsonb,
    started_at TIMESTAMPTZ NOT NULL,
    completed_at TIMESTAMPTZ,
    error_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS contradictions (
    contradiction_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    reconstruction_run_id UUID NOT NULL REFERENCES reconstruction_runs(reconstruction_run_id) ON DELETE RESTRICT,
    case_id UUID NOT NULL REFERENCES cases(case_id) ON DELETE RESTRICT,
    event_id_1 UUID NOT NULL REFERENCES events(event_id) ON DELETE RESTRICT,
    event_id_2 UUID NOT NULL REFERENCES events(event_id) ON DELETE RESTRICT,
    contradiction_type VARCHAR(50) NOT NULL,
    severity VARCHAR(50) NOT NULL,
    explanation TEXT NOT NULL,
    metrics JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS spatial_trajectories (
    trajectory_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    reconstruction_run_id UUID NOT NULL REFERENCES reconstruction_runs(reconstruction_run_id) ON DELETE RESTRICT,
    case_id UUID NOT NULL REFERENCES cases(case_id) ON DELETE RESTRICT,
    entity_id VARCHAR(100) NOT NULL,
    waypoints JSONB NOT NULL DEFAULT '[]'::jsonb,
    total_distance_km DOUBLE PRECISION,
    average_speed_kmh DOUBLE PRECISION,
    max_speed_kmh DOUBLE PRECISION,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS video_artifacts (
    artifact_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    reconstruction_run_id UUID NOT NULL REFERENCES reconstruction_runs(reconstruction_run_id) ON DELETE RESTRICT,
    evidence_id UUID NOT NULL REFERENCES evidence(evidence_id) ON DELETE RESTRICT,
    case_id UUID NOT NULL REFERENCES cases(case_id) ON DELETE RESTRICT,
    frame_number BIGINT NOT NULL,
    pts_timestamp_seconds DOUBLE PRECISION,
    frame_sha256 VARCHAR(64) NOT NULL,
    frame_storage_uri VARCHAR(255) NOT NULL,
    telemetry JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
