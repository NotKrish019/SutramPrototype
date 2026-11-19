-- YUKTI Forensic Schema Migration: Reconstruction Subsystem Refinement
-- Version: 20261119000003
-- Target: PostgreSQL 16+

-- 1. Allow case-level reconstructions spanning multiple exhibits (relax extraction_run_id NOT NULL)
ALTER TABLE reconstruction_runs ALTER COLUMN extraction_run_id DROP NOT NULL;

-- 2. Add result snapshot storage to reconstruction_runs
ALTER TABLE reconstruction_runs ADD COLUMN IF NOT EXISTS result_snapshot JSONB;

-- 3. Optimization indexes for forensic queries
CREATE INDEX IF NOT EXISTS idx_reconstruction_runs_case ON reconstruction_runs(case_id, started_at DESC);
CREATE INDEX IF NOT EXISTS idx_contradictions_run ON contradictions(reconstruction_run_id);
CREATE INDEX IF NOT EXISTS idx_contradictions_case ON contradictions(case_id);
CREATE INDEX IF NOT EXISTS idx_spatial_trajectories_run ON spatial_trajectories(reconstruction_run_id);
CREATE INDEX IF NOT EXISTS idx_spatial_trajectories_case ON spatial_trajectories(case_id);
