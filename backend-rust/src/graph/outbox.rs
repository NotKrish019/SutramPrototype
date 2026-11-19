//! Graph Projection Transactional Outbox Repository.
//!
//! Provides durable, transactional outbox operations in PostgreSQL.
//! Uses `SELECT ... FOR UPDATE SKIP LOCKED` for concurrent worker safety
//! and exponential backoff on retries.

use sqlx::{PgPool, Postgres, Row, Transaction};
use tracing::{debug, warn};
use uuid::Uuid;

use super::model::{OutboxEvent, OutboxMetrics};
use crate::error::AppError;

pub struct GraphOutboxRepository {
    pool: PgPool,
}

impl GraphOutboxRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Enqueues an outbox event within an existing PostgreSQL transaction.
    ///
    /// Idempotent: If an event with the same `(aggregate_type, aggregate_id, event_type)` exists,
    /// it resets the status to `PENDING` and updates the payload.
    pub async fn enqueue_transactional(
        tx: &mut Transaction<'_, Postgres>,
        case_id: Uuid,
        aggregate_type: &str,
        aggregate_id: Uuid,
        aggregate_version: i32,
        event_type: &str,
        payload: &serde_json::Value,
    ) -> Result<Uuid, AppError> {
        let outbox_id = Uuid::new_v4();

        let row = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO graph_projection_outbox (
                outbox_id, case_id, aggregate_type, aggregate_id, aggregate_version,
                event_type, payload, status, attempt_count, max_attempts, next_attempt_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, 'PENDING', 0, 5, NOW())
            ON CONFLICT (aggregate_type, aggregate_id, event_type)
            DO UPDATE SET
                payload = EXCLUDED.payload,
                aggregate_version = EXCLUDED.aggregate_version,
                status = 'PENDING',
                next_attempt_at = NOW(),
                attempt_count = 0,
                last_error = NULL
            RETURNING outbox_id
            "#,
        )
        .bind(outbox_id)
        .bind(case_id)
        .bind(aggregate_type)
        .bind(aggregate_id)
        .bind(aggregate_version)
        .bind(event_type)
        .bind(payload)
        .fetch_one(&mut **tx)
        .await
        .map_err(|e| {
            AppError::Internal(anyhow::anyhow!("Failed to enqueue outbox event: {}", e))
        })?;

        debug!(outbox_id = %row, event_type = %event_type, "Enqueued graph projection outbox event");
        Ok(row)
    }

    /// Atomically claims pending outbox events using `FOR UPDATE SKIP LOCKED`.
    pub async fn claim_pending_events(&self, limit: i64) -> Result<Vec<OutboxEvent>, AppError> {
        let mut tx = self.pool.begin().await.map_err(|e| {
            AppError::Internal(anyhow::anyhow!("Failed to begin outbox transaction: {}", e))
        })?;

        let events = sqlx::query_as::<_, OutboxEvent>(
            r#"
            UPDATE graph_projection_outbox
            SET status = 'PROCESSING'
            WHERE outbox_id IN (
                SELECT outbox_id FROM graph_projection_outbox
                WHERE status IN ('PENDING', 'RETRY_WAIT') AND next_attempt_at <= NOW()
                ORDER BY created_at ASC
                LIMIT $1
                FOR UPDATE SKIP LOCKED
            )
            RETURNING *
            "#,
        )
        .bind(limit)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| {
            AppError::Internal(anyhow::anyhow!(
                "Failed to claim pending outbox events: {}",
                e
            ))
        })?;

        tx.commit().await.map_err(|e| {
            AppError::Internal(anyhow::anyhow!("Failed to commit outbox claim: {}", e))
        })?;

        Ok(events)
    }

    /// Marks an outbox event as successfully projected into the graph.
    pub async fn mark_succeeded(&self, outbox_id: Uuid) -> Result<(), AppError> {
        sqlx::query(
            r#"
            UPDATE graph_projection_outbox
            SET status = 'SUCCEEDED',
                processed_at = NOW(),
                last_error = NULL
            WHERE outbox_id = $1
            "#,
        )
        .bind(outbox_id)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            AppError::Internal(anyhow::anyhow!(
                "Failed to mark outbox event succeeded: {}",
                e
            ))
        })?;

        Ok(())
    }

    /// Marks an outbox event as failed with exponential backoff or dead-letter transition.
    pub async fn mark_failed(&self, outbox_id: Uuid, error_message: &str) -> Result<(), AppError> {
        let sanitized_err = if error_message.len() > 1000 {
            &error_message[..1000]
        } else {
            error_message
        };

        sqlx::query(
            r#"
            UPDATE graph_projection_outbox
            SET attempt_count = attempt_count + 1,
                last_error = $2,
                status = CASE 
                    WHEN attempt_count + 1 >= max_attempts THEN 'DEAD_LETTER' 
                    ELSE 'RETRY_WAIT' 
                END,
                next_attempt_at = NOW() + (POWER(2, LEAST(attempt_count, 10)) * INTERVAL '1 second')
            WHERE outbox_id = $1
            "#,
        )
        .bind(outbox_id)
        .bind(sanitized_err)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            AppError::Internal(anyhow::anyhow!(
                "Failed to update outbox event failure: {}",
                e
            ))
        })?;

        warn!(outbox_id = %outbox_id, "Outbox event marked as failed/retry");
        Ok(())
    }

    /// Retrieves aggregate counts across all outbox statuses for a case or globally.
    pub async fn get_metrics(&self, case_id: Option<Uuid>) -> Result<OutboxMetrics, AppError> {
        let rows = if let Some(cid) = case_id {
            sqlx::query(
                r#"
                SELECT status, COUNT(*) as count 
                FROM graph_projection_outbox 
                WHERE case_id = $1 
                GROUP BY status
                "#,
            )
            .bind(cid)
            .fetch_all(&self.pool)
            .await
        } else {
            sqlx::query(
                r#"
                SELECT status, COUNT(*) as count 
                FROM graph_projection_outbox 
                GROUP BY status
                "#,
            )
            .fetch_all(&self.pool)
            .await
        }
        .map_err(|e| {
            AppError::Internal(anyhow::anyhow!("Failed to fetch outbox metrics: {}", e))
        })?;

        let mut metrics = OutboxMetrics::default();
        for r in rows {
            let status: String = r.try_get("status").unwrap_or_default();
            let count: i64 = r.try_get("count").unwrap_or(0);
            match status.as_str() {
                "PENDING" => metrics.pending = count,
                "PROCESSING" => metrics.processing = count,
                "SUCCEEDED" => metrics.succeeded = count,
                "FAILED" => metrics.failed = count,
                "RETRY_WAIT" => metrics.retry_wait = count,
                "DEAD_LETTER" => metrics.dead_letter = count,
                _ => {}
            }
        }

        Ok(metrics)
    }
}
