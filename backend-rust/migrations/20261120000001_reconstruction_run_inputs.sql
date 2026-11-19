-- YUKTI Forensic Schema Migration: Multi-Input Reconstruction Provenance Ledger
-- Version: 20261120000001
-- Target: PostgreSQL 16+
--
-- Preserves explicit input provenance for reconstruction runs spanning multiple extraction runs.
-- Adheres strictly to the additive migration policy.

CREATE TABLE IF NOT EXISTS reconstruction_run_inputs (
    reconstruction_run_id UUID NOT NULL REFERENCES reconstruction_runs(reconstruction_run_id) ON DELETE CASCADE,
    extraction_run_id UUID NOT NULL REFERENCES extraction_runs(extraction_run_id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (reconstruction_run_id, extraction_run_id)
);

CREATE INDEX IF NOT EXISTS idx_reconstruction_run_inputs_extraction ON reconstruction_run_inputs(extraction_run_id);
CREATE INDEX IF NOT EXISTS idx_reconstruction_run_inputs_reconstruction ON reconstruction_run_inputs(reconstruction_run_id);

-- Backfill from single-input legacy reconstruction_runs if extraction_run_id is set
INSERT INTO reconstruction_run_inputs (reconstruction_run_id, extraction_run_id, created_at)
SELECT reconstruction_run_id, extraction_run_id, created_at
FROM reconstruction_runs
WHERE extraction_run_id IS NOT NULL
ON CONFLICT (reconstruction_run_id, extraction_run_id) DO NOTHING;
