//! Case Repository: Database access for investigative cases.

use sqlx::PgPool;
use uuid::Uuid;

use super::model::CaseEntity;
use crate::error::AppError;

#[derive(Debug, Clone)]
pub struct CaseRepository {
    pool: PgPool,
}

impl CaseRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Registers a new case in PostgreSQL.
    pub async fn create_case(&self, case: &CaseEntity) -> Result<CaseEntity, AppError> {
        let row = sqlx::query_as::<_, CaseEntity>(
            r#"
            INSERT INTO cases (case_id, title, description, fir_number, status, investigator_id, metadata, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING *
            "#,
        )
        .bind(case.case_id)
        .bind(&case.title)
        .bind(&case.description)
        .bind(&case.fir_number)
        .bind(&case.status)
        .bind(&case.investigator_id)
        .bind(&case.metadata)
        .bind(case.created_at)
        .bind(case.updated_at)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to insert case record: {}", e)))?;

        Ok(row)
    }

    /// Retrieves a case by its UUID.
    pub async fn get_case(&self, case_id: Uuid) -> Result<Option<CaseEntity>, AppError> {
        let row = sqlx::query_as::<_, CaseEntity>(
            r#"
            SELECT * FROM cases WHERE case_id = $1
            "#,
        )
        .bind(case_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to fetch case record: {}", e)))?;

        Ok(row)
    }

    /// Lists all cases ordered by updated_at descending.
    pub async fn list_cases(&self) -> Result<Vec<CaseEntity>, AppError> {
        let rows = sqlx::query_as::<_, CaseEntity>(
            r#"
            SELECT * FROM cases ORDER BY updated_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to list cases: {}", e)))?;

        Ok(rows)
    }

    /// Ensures a case exists, auto-registering a placeholder if missing (e.g. during reconciliation).
    pub async fn ensure_case_exists(
        &self,
        case_id: Uuid,
        title: &str,
    ) -> Result<CaseEntity, AppError> {
        if let Some(existing) = self.get_case(case_id).await? {
            return Ok(existing);
        }

        let now = chrono::Utc::now();
        let new_case = CaseEntity {
            case_id,
            title: title.to_string(),
            description: Some("Auto-registered forensic case container".to_string()),
            fir_number: None,
            status: "ACTIVE".to_string(),
            investigator_id: None,
            metadata: serde_json::json!({ "auto_created": true }),
            created_at: now,
            updated_at: now,
        };

        self.create_case(&new_case).await
    }
}
