//! Custody Repository: Append-only cryptographic ledger tracking evidence chain of custody.

use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use super::model::CustodyEventEntity;
use crate::error::AppError;

const GENESIS_PREV_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";

#[derive(Debug, Clone)]
pub struct CustodyRepository {
    pool: PgPool,
}

impl CustodyRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Appends a new immutable custody event to the chain-of-custody ledger.
    pub async fn append_event(
        &self,
        evidence_id: Uuid,
        case_id: Uuid,
        action: &str,
        actor: &str,
        payload_sha256: Option<&str>,
        details: serde_json::Value,
    ) -> Result<CustodyEventEntity, AppError> {
        // Fetch previous event hash for this evidence item (or genesis hash)
        let prev_hash = sqlx::query_scalar::<_, String>(
            r#"
            SELECT event_hash FROM custody_events 
            WHERE evidence_id = $1 
            ORDER BY custody_id DESC 
            LIMIT 1
            "#,
        )
        .bind(evidence_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            AppError::Internal(anyhow::anyhow!(
                "Failed to query latest custody hash: {}",
                e
            ))
        })?
        .unwrap_or_else(|| GENESIS_PREV_HASH.to_string());

        let event_id_uuid = Uuid::new_v4();
        let now = chrono::Utc::now();
        let now_str = now.to_rfc3339();

        // Compute deterministic cryptographic SHA-256 event hash
        let mut hasher = Sha256::new();
        hasher.update(action.as_bytes());
        hasher.update(evidence_id.to_string().as_bytes());
        hasher.update(case_id.to_string().as_bytes());
        hasher.update(actor.as_bytes());
        hasher.update(now_str.as_bytes());
        hasher.update(prev_hash.as_bytes());
        hasher.update(
            serde_json::to_string(&details)
                .unwrap_or_default()
                .as_bytes(),
        );
        let event_hash = hex::encode(hasher.finalize());

        let row = sqlx::query_as::<_, CustodyEventEntity>(
            r#"
            INSERT INTO custody_events (
                event_id_uuid, evidence_id, case_id, action, actor,
                payload_sha256, details, prev_event_hash, event_hash, timestamp
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING *
            "#,
        )
        .bind(event_id_uuid)
        .bind(evidence_id)
        .bind(case_id)
        .bind(action)
        .bind(actor)
        .bind(payload_sha256)
        .bind(&details)
        .bind(&prev_hash)
        .bind(&event_hash)
        .bind(now)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            AppError::Internal(anyhow::anyhow!("Failed to append custody event: {}", e))
        })?;

        Ok(row)
    }

    /// Lists all custody history records for an evidence exhibit in strictly chronological order.
    pub async fn list_custody_for_evidence(
        &self,
        evidence_id: Uuid,
    ) -> Result<Vec<CustodyEventEntity>, AppError> {
        let rows = sqlx::query_as::<_, CustodyEventEntity>(
            r#"
            SELECT * FROM custody_events 
            WHERE evidence_id = $1 
            ORDER BY custody_id ASC
            "#,
        )
        .bind(evidence_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to list custody events: {}", e)))?;

        Ok(rows)
    }
}
