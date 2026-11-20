//! Forensic Reconstruction Repository via SQLx.
//!
//! Provides relational persistence for reconstruction runs, contradiction leads,
//! spatial trajectories, and transactional graph outbox notification.

use chrono::Utc;
use sqlx::PgPool;
use tracing::{debug, info};
use uuid::Uuid;

use super::model::{deterministic_uuid, ContradictionEntity, EventEntity, ReconstructionRunEntity};
use crate::error::AppError;
use crate::graph::GraphOutboxRepository;
use crate::reconstruction::service::FullCaseReconstructionResult;

/// Typed repository for forensic reconstruction persistence.
#[derive(Clone)]
pub struct ReconstructionRunRepository {
    pool: PgPool,
}

impl ReconstructionRunRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Fetches all extracted events for a case, chronologically ordered by nominal timestamp.
    pub async fn get_events_for_case(&self, case_id: Uuid) -> Result<Vec<EventEntity>, AppError> {
        let events = sqlx::query_as::<_, EventEntity>(
            r#"
            SELECT event_id, extraction_run_id, evidence_id, case_id, source_record_id,
                   event_type, nominal_timestamp, timestamp_precision, duration_seconds,
                   uncertainty_window_start, uncertainty_window_end, latitude, longitude,
                   involved_entity_ids, payload, is_observed, created_at
            FROM events
            WHERE case_id = $1
            ORDER BY nominal_timestamp ASC
            "#,
        )
        .bind(case_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            AppError::Internal(anyhow::anyhow!("Failed to query events for case: {}", e))
        })?;

        Ok(events)
    }

    /// Atomically persists a full reconstruction run, its contradiction leads, spatial trajectories,
    /// and enqueues a `RECONSTRUCTION_COMPLETED` outbox event for Neo4j projection.
    pub async fn save_full_reconstruction_atomic(
        &self,
        case_id: Uuid,
        run_id: Uuid,
        contributing_extraction_run_ids: &[Uuid],
        reconstruction_type: &str,
        parameters: &serde_json::Value,
        result: &FullCaseReconstructionResult,
    ) -> Result<(), AppError> {
        let mut tx = self.pool.begin().await.map_err(|e| {
            AppError::Internal(anyhow::anyhow!(
                "Failed to begin transaction for reconstruction run: {}",
                e
            ))
        })?;

        let now = Utc::now();
        let snapshot_json = serde_json::to_value(result).map_err(|e| {
            AppError::Internal(anyhow::anyhow!(
                "Failed to serialize reconstruction snapshot: {}",
                e
            ))
        })?;

        let primary_extraction_run_id = contributing_extraction_run_ids.first().copied();

        // 1. Insert reconstruction_runs row
        sqlx::query(
            r#"
            INSERT INTO reconstruction_runs (
                reconstruction_run_id, extraction_run_id, case_id, reconstruction_type,
                status, parameters, started_at, completed_at, result_snapshot, error_message, created_at
            )
            VALUES ($1, $2, $3, $4, 'SUCCESS', $5, $6, $6, $7, NULL, $6)
            "#,
        )
        .bind(run_id)
        .bind(primary_extraction_run_id)
        .bind(case_id)
        .bind(reconstruction_type)
        .bind(parameters)
        .bind(now)
        .bind(&snapshot_json)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to insert reconstruction_run: {}", e)))?;

        // 1b. Preserve multi-input provenance links in reconstruction_run_inputs
        for ext_id in contributing_extraction_run_ids {
            sqlx::query(
                r#"
                INSERT INTO reconstruction_run_inputs (reconstruction_run_id, extraction_run_id, created_at)
                VALUES ($1, $2, $3)
                ON CONFLICT (reconstruction_run_id, extraction_run_id) DO NOTHING
                "#,
            )
            .bind(run_id)
            .bind(ext_id)
            .bind(now)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                AppError::Internal(anyhow::anyhow!(
                    "Failed to insert reconstruction_run_inputs for {}: {}",
                    ext_id,
                    e
                ))
            })?;
        }

        // Also associate all distinct extraction runs found from the case's events
        sqlx::query(
            r#"
            INSERT INTO reconstruction_run_inputs (reconstruction_run_id, extraction_run_id, created_at)
            SELECT DISTINCT $1, extraction_run_id, $2
            FROM events
            WHERE case_id = $3 AND extraction_run_id IS NOT NULL
            ON CONFLICT (reconstruction_run_id, extraction_run_id) DO NOTHING
            "#,
        )
        .bind(run_id)
        .bind(now)
        .bind(case_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            AppError::Internal(anyhow::anyhow!(
                "Failed to insert reconstruction_run_inputs from events: {}",
                e
            ))
        })?;

        // 2. Insert detected contradiction leads
        for lead in &result.contradictions.leads {
            let contradiction_id = Uuid::new_v4();
            let ev1_uuid = Uuid::parse_str(&lead.event_a_id)
                .unwrap_or_else(|_| deterministic_uuid(&case_id, lead.event_a_id.as_bytes()));
            let ev2_uuid = Uuid::parse_str(&lead.event_b_id)
                .unwrap_or_else(|_| deterministic_uuid(&case_id, lead.event_b_id.as_bytes()));

            // Verify both events exist in the events table before inserting to prevent FK violation
            let exists_query = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM events WHERE event_id IN ($1, $2) AND case_id = $3",
            )
            .bind(ev1_uuid)
            .bind(ev2_uuid)
            .bind(case_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| {
                AppError::Internal(anyhow::anyhow!(
                    "Failed to verify contradiction event foreign keys: {}",
                    e
                ))
            })?;

            if exists_query == 2 {
                let metrics_json = serde_json::json!({
                    "derived_inference": lead.derived_inference,
                    "observed_basis": lead.observed_basis,
                    "limitations": lead.limitations
                });

                let lead_type_str = match lead.lead_type {
                    crate::reconstruction::ContradictionLeadType::SpatialTransitImpossible => {
                        "SPATIAL_TRANSIT_IMPOSSIBLE"
                    }
                    crate::reconstruction::ContradictionLeadType::TemporalActivityInfeasible => {
                        "TEMPORAL_ACTIVITY_INFEASIBLE"
                    }
                };

                sqlx::query(
                    r#"
                    INSERT INTO contradictions (
                        contradiction_id, reconstruction_run_id, case_id, event_id_1, event_id_2,
                        contradiction_type, severity, explanation, metrics, created_at
                    )
                    VALUES ($1, $2, $3, $4, $5, $6, 'HIGH', $7, $8, $9)
                    "#,
                )
                .bind(contradiction_id)
                .bind(run_id)
                .bind(case_id)
                .bind(ev1_uuid)
                .bind(ev2_uuid)
                .bind(lead_type_str)
                .bind(&lead.neutral_explanation)
                .bind(&metrics_json)
                .bind(now)
                .execute(&mut *tx)
                .await
                .map_err(|e| {
                    AppError::Internal(anyhow::anyhow!("Failed to insert contradiction row: {}", e))
                })?;
            } else {
                debug!(
                    lead_id = %lead.lead_id,
                    ev1 = %lead.event_a_id,
                    ev2 = %lead.event_b_id,
                    "Skipping relational contradiction row: referenced events not found in relational table"
                );
            }
        }

        // 3. Insert spatial trajectories
        for traj in &result.spatial.trajectories {
            let trajectory_id = Uuid::new_v4();
            let waypoints_json = serde_json::to_value(&traj.waypoints).unwrap_or_default();

            sqlx::query(
                r#"
                INSERT INTO spatial_trajectories (
                    trajectory_id, reconstruction_run_id, case_id, entity_id,
                    waypoints, total_distance_km, average_speed_kmh, max_speed_kmh, created_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, NULL, $7, $8)
                "#,
            )
            .bind(trajectory_id)
            .bind(run_id)
            .bind(case_id)
            .bind(&traj.entity_id)
            .bind(&waypoints_json)
            .bind(traj.total_distance_km)
            .bind(traj.max_min_required_speed_kmh)
            .bind(now)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                AppError::Internal(anyhow::anyhow!("Failed to insert spatial_trajectory row: {}", e))
            })?;
        }

        // 4. Enqueue RECONSTRUCTION_COMPLETED event into graph outbox
        let outbox_payload = serde_json::json!({
            "reconstruction_type": reconstruction_type,
            "status": "SUCCESS",
            "leads": result.contradictions.leads,
            "trajectory_count": result.spatial.trajectories.len(),
            "event_count": result.timeline.total_events
        });

        GraphOutboxRepository::enqueue_transactional(
            &mut tx,
            case_id,
            "RECONSTRUCTION_RUN",
            run_id,
            1,
            "RECONSTRUCTION_COMPLETED",
            &outbox_payload,
        )
        .await?;

        tx.commit().await.map_err(|e| {
            AppError::Internal(anyhow::anyhow!(
                "Failed to commit reconstruction run transaction: {}",
                e
            ))
        })?;

        info!(
            case_id = %case_id,
            run_id = %run_id,
            leads = result.contradictions.leads.len(),
            trajectories = result.spatial.trajectories.len(),
            "Successfully persisted forensic reconstruction run and enqueued graph outbox projection"
        );

        Ok(())
    }

    /// Retrieves the most recent successful reconstruction result for a case.
    pub async fn get_latest_reconstruction(
        &self,
        case_id: Uuid,
    ) -> Result<Option<FullCaseReconstructionResult>, AppError> {
        let row = sqlx::query_as::<_, (Uuid, Option<serde_json::Value>)>(
            r#"
            SELECT reconstruction_run_id, result_snapshot
            FROM reconstruction_runs
            WHERE case_id = $1 AND status = 'SUCCESS' AND result_snapshot IS NOT NULL
            ORDER BY started_at DESC
            LIMIT 1
            "#,
        )
        .bind(case_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            AppError::Internal(anyhow::anyhow!(
                "Failed to query latest reconstruction: {}",
                e
            ))
        })?;

        if let Some((run_id, Some(snapshot))) = row {
            let mut res =
                serde_json::from_value::<FullCaseReconstructionResult>(snapshot).map_err(|e| {
                    AppError::Internal(anyhow::anyhow!(
                        "Failed to parse reconstruction snapshot: {}",
                        e
                    ))
                })?;
            if res.reconstruction_run_id.is_none() {
                res.reconstruction_run_id = Some(run_id.to_string());
            }
            if res.run_id.is_none() {
                res.run_id = Some(run_id.to_string());
            }
            Ok(Some(res))
        } else {
            Ok(None)
        }
    }

    /// Lists all historical reconstruction runs for a case.
    pub async fn list_reconstruction_runs(
        &self,
        case_id: Uuid,
    ) -> Result<Vec<ReconstructionRunEntity>, AppError> {
        let runs = sqlx::query_as::<_, ReconstructionRunEntity>(
            r#"
            SELECT reconstruction_run_id, extraction_run_id, case_id, reconstruction_type,
                   status, parameters, started_at, completed_at, result_snapshot, error_message, created_at
            FROM reconstruction_runs
            WHERE case_id = $1
            ORDER BY started_at DESC
            "#,
        )
        .bind(case_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to list reconstruction runs: {}", e)))?;

        Ok(runs)
    }

    /// Lists all detected contradictions for a case.
    pub async fn get_contradictions_by_case(
        &self,
        case_id: Uuid,
    ) -> Result<Vec<ContradictionEntity>, AppError> {
        let rows = sqlx::query_as::<_, ContradictionEntity>(
            r#"
            SELECT contradiction_id, reconstruction_run_id, case_id, event_id_1, event_id_2,
                   contradiction_type, severity, explanation, metrics, created_at
            FROM contradictions
            WHERE case_id = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind(case_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            AppError::Internal(anyhow::anyhow!("Failed to query contradictions: {}", e))
        })?;

        Ok(rows)
    }

    /// Retrieves all input extraction run IDs for a given reconstruction run.
    pub async fn get_reconstruction_inputs(
        &self,
        reconstruction_run_id: Uuid,
    ) -> Result<Vec<Uuid>, AppError> {
        let rows = sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT extraction_run_id
            FROM reconstruction_run_inputs
            WHERE reconstruction_run_id = $1
            ORDER BY created_at ASC
            "#,
        )
        .bind(reconstruction_run_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            AppError::Internal(anyhow::anyhow!(
                "Failed to query reconstruction inputs: {}",
                e
            ))
        })?;

        Ok(rows)
    }
}
