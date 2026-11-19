-- YUKTI Forensic Schema Migration: Enforce Strict Half-Open Temporal Uncertainty Window
-- Version: 20261119000001
-- Target: PostgreSQL 16+
-- Fixes FIND-2A-01: Ensures nominal_timestamp < uncertainty_window_end (DEC-002 half-open [start, end) semantics)

ALTER TABLE events DROP CONSTRAINT IF EXISTS chk_event_uncertainty_window;

ALTER TABLE events ADD CONSTRAINT chk_event_uncertainty_window CHECK (
    uncertainty_window_start IS NULL 
    OR uncertainty_window_end IS NULL 
    OR (uncertainty_window_start < uncertainty_window_end 
        AND uncertainty_window_start <= nominal_timestamp 
        AND nominal_timestamp < uncertainty_window_end)
);
