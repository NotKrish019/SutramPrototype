-- YUKTI Forensic Schema Migration: Transactional Outbox for Graph Projection
-- Version: 20261119000002
-- Target: PostgreSQL 16+
-- Implements Phase 2A.3 Transactional Outbox Pattern for Idempotent Neo4j Graph Projection

CREATE TABLE IF NOT EXISTS graph_projection_outbox (
    outbox_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    case_id UUID NOT NULL,
    aggregate_type VARCHAR(50) NOT NULL,
    aggregate_id UUID NOT NULL,
    aggregate_version INTEGER NOT NULL DEFAULT 1,
    event_type VARCHAR(100) NOT NULL,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    status VARCHAR(50) NOT NULL DEFAULT 'PENDING',
    attempt_count INTEGER NOT NULL DEFAULT 0,
    max_attempts INTEGER NOT NULL DEFAULT 5,
    next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    processed_at TIMESTAMPTZ,
    CONSTRAINT chk_outbox_status CHECK (
        status IN ('PENDING', 'PROCESSING', 'SUCCEEDED', 'FAILED', 'RETRY_WAIT', 'DEAD_LETTER')
    ),
    CONSTRAINT uq_outbox_aggregate_event UNIQUE (aggregate_type, aggregate_id, event_type)
);

CREATE INDEX IF NOT EXISTS idx_outbox_queue ON graph_projection_outbox(status, next_attempt_at);
CREATE INDEX IF NOT EXISTS idx_outbox_case_id ON graph_projection_outbox(case_id);
