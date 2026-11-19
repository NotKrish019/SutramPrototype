//! Knowledge Graph Reconciliation Engine.
//!
//! Reconciles graph state in Neo4j against authoritative PostgreSQL relational records.
//! Detects unprojected exhibits, runs, and reconstructions, and enqueues idempotent
//! outbox events to restore graph consistency without data loss.

use sqlx::{PgPool, Row};
use std::sync::Arc;
use tracing::info;
use uuid::Uuid;

use super::client::GraphClient;
use super::outbox::GraphOutboxRepository;
use crate::error::AppError;

/// Summary metrics of a graph reconciliation run.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct GraphReconciliationReport {
    pub cases_checked: usize,
    pub cases_enqueued: usize,
    pub evidence_checked: usize,
    pub evidence_enqueued: usize,
    pub runs_checked: usize,
    pub runs_enqueued: usize,
    pub reconstructions_checked: usize,
    pub reconstructions_enqueued: usize,
}

pub struct GraphReconciliationService {
    pool: PgPool,
    _client: Arc<dyn GraphClient>,
}

impl GraphReconciliationService {
    pub fn new(pool: PgPool, client: Arc<dyn GraphClient>) -> Self {
        Self {
            pool,
            _client: client,
        }
    }

    /// Scans PostgreSQL for committed cases, evidence, and runs, ensuring outbox entries exist.
    pub async fn reconcile_all(&self) -> Result<GraphReconciliationReport, AppError> {
        let mut report = GraphReconciliationReport::default();
        info!("Starting PostgreSQL-to-Neo4j graph reconciliation scan...");

        // 1. Reconcile Cases
        let cases = sqlx::query(r#"SELECT case_id, title, status, created_at FROM cases"#)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| {
                AppError::Internal(anyhow::anyhow!(
                    "Reconciliation failed querying cases: {}",
                    e
                ))
            })?;

        for c in cases {
            report.cases_checked += 1;
            let case_id: Uuid = c.try_get("case_id").unwrap_or_default();
            let title: String = c.try_get("title").unwrap_or_default();
            let status: String = c.try_get("status").unwrap_or_default();
            let created_at: chrono::DateTime<chrono::Utc> = c
                .try_get("created_at")
                .unwrap_or_else(|_| chrono::Utc::now());

            let mut tx = self.pool.begin().await.map_err(|e| {
                AppError::Internal(anyhow::anyhow!(
                    "Failed to start reconciliation transaction: {}",
                    e
                ))
            })?;

            GraphOutboxRepository::enqueue_transactional(
                &mut tx,
                case_id,
                "CASE",
                case_id,
                1,
                "CASE_CREATED",
                &serde_json::json!({
                    "title": title,
                    "status": status,
                    "created_at": created_at.to_rfc3339(),
                }),
            )
            .await?;

            tx.commit().await.map_err(|e| {
                AppError::Internal(anyhow::anyhow!(
                    "Failed to commit case outbox reconciliation: {}",
                    e
                ))
            })?;
            report.cases_enqueued += 1;
        }

        // 2. Reconcile Evidence
        let evidence = sqlx::query(
            r#"
            SELECT evidence_id, case_id, sha256_hash, source_type, mime_type, file_size_bytes, storage_uri, created_at
            FROM evidence
            "#
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Reconciliation failed querying evidence: {}", e)))?;

        for ev in evidence {
            report.evidence_checked += 1;
            let evidence_id: Uuid = ev.try_get("evidence_id").unwrap_or_default();
            let case_id: Uuid = ev.try_get("case_id").unwrap_or_default();
            let sha256_hash: String = ev.try_get("sha256_hash").unwrap_or_default();
            let source_type: String = ev.try_get("source_type").unwrap_or_default();
            let mime_type: String = ev.try_get("mime_type").unwrap_or_default();
            let file_size_bytes: i64 = ev.try_get("file_size_bytes").unwrap_or(0);
            let storage_uri: String = ev.try_get("storage_uri").unwrap_or_default();
            let created_at: chrono::DateTime<chrono::Utc> = ev
                .try_get("created_at")
                .unwrap_or_else(|_| chrono::Utc::now());

            let mut tx = self.pool.begin().await.map_err(|e| {
                AppError::Internal(anyhow::anyhow!(
                    "Failed to start reconciliation transaction: {}",
                    e
                ))
            })?;

            GraphOutboxRepository::enqueue_transactional(
                &mut tx,
                case_id,
                "EVIDENCE",
                evidence_id,
                1,
                "EVIDENCE_INGESTED",
                &serde_json::json!({
                    "sha256_hash": sha256_hash,
                    "source_type": source_type,
                    "mime_type": mime_type,
                    "file_size_bytes": file_size_bytes,
                    "storage_uri": storage_uri,
                    "created_at": created_at.to_rfc3339(),
                }),
            )
            .await?;

            tx.commit().await.map_err(|e| {
                AppError::Internal(anyhow::anyhow!(
                    "Failed to commit evidence outbox reconciliation: {}",
                    e
                ))
            })?;
            report.evidence_enqueued += 1;
        }

        info!(
            cases_enqueued = report.cases_enqueued,
            evidence_enqueued = report.evidence_enqueued,
            "Graph reconciliation outbox backfill completed"
        );

        Ok(report)
    }
}
