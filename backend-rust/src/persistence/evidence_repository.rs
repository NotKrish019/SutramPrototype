//! Evidence Repository: Database access for evidence exhibit metadata.

use sqlx::PgPool;
use uuid::Uuid;

use super::model::EvidenceEntity;
use crate::error::AppError;

#[derive(Debug, Clone)]
pub struct EvidenceRepository {
    pool: PgPool,
}

impl EvidenceRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Registers a newly ingested evidence exhibit in PostgreSQL.
    pub async fn register_evidence(
        &self,
        evidence: &EvidenceEntity,
    ) -> Result<EvidenceEntity, AppError> {
        let row = sqlx::query_as::<_, EvidenceEntity>(
            r#"
            INSERT INTO evidence (
                evidence_id, case_id, original_filename, source_type, mime_type,
                file_size_bytes, sha256_hash, storage_uri, source_timezone,
                clock_offset_seconds, ingestion_status, metadata, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            RETURNING *
            "#,
        )
        .bind(evidence.evidence_id)
        .bind(evidence.case_id)
        .bind(&evidence.original_filename)
        .bind(&evidence.source_type)
        .bind(&evidence.mime_type)
        .bind(evidence.file_size_bytes)
        .bind(&evidence.sha256_hash)
        .bind(&evidence.storage_uri)
        .bind(&evidence.source_timezone)
        .bind(evidence.clock_offset_seconds)
        .bind(&evidence.ingestion_status)
        .bind(&evidence.metadata)
        .bind(evidence.created_at)
        .bind(evidence.updated_at)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            AppError::Internal(anyhow::anyhow!("Failed to register evidence record: {}", e))
        })?;

        Ok(row)
    }

    /// Retrieves an evidence exhibit by its UUID.
    pub async fn get_evidence(
        &self,
        evidence_id: Uuid,
    ) -> Result<Option<EvidenceEntity>, AppError> {
        let row = sqlx::query_as::<_, EvidenceEntity>(
            r#"
            SELECT * FROM evidence WHERE evidence_id = $1
            "#,
        )
        .bind(evidence_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            AppError::Internal(anyhow::anyhow!("Failed to fetch evidence record: {}", e))
        })?;

        Ok(row)
    }

    /// Finds the associated case_id for a given evidence exhibit.
    pub async fn find_case_id_for_evidence(
        &self,
        evidence_id: Uuid,
    ) -> Result<Option<Uuid>, AppError> {
        let row = sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT case_id FROM evidence WHERE evidence_id = $1
            "#,
        )
        .bind(evidence_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            AppError::Internal(anyhow::anyhow!(
                "Failed to find case_id for evidence: {}",
                e
            ))
        })?;

        Ok(row)
    }

    /// Lists all evidence exhibits associated with a specific case.
    pub async fn list_evidence_for_case(
        &self,
        case_id: Uuid,
    ) -> Result<Vec<EvidenceEntity>, AppError> {
        let rows = sqlx::query_as::<_, EvidenceEntity>(
            r#"
            SELECT * FROM evidence WHERE case_id = $1 ORDER BY created_at ASC
            "#,
        )
        .bind(case_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            AppError::Internal(anyhow::anyhow!("Failed to list evidence for case: {}", e))
        })?;

        Ok(rows)
    }

    /// Updates the ingestion lifecycle status of an evidence exhibit.
    pub async fn update_status(&self, evidence_id: Uuid, status: &str) -> Result<(), AppError> {
        sqlx::query(
            r#"
            UPDATE evidence 
            SET ingestion_status = $2, updated_at = NOW() 
            WHERE evidence_id = $1
            "#,
        )
        .bind(evidence_id)
        .bind(status)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            AppError::Internal(anyhow::anyhow!("Failed to update evidence status: {}", e))
        })?;

        Ok(())
    }
}
