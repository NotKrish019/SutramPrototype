//! Case Management API Route Handlers.
//!
//! Provides endpoints:
//! - `POST /api/v1/cases`: Case registration and metadata ingestion.
//! - `GET  /api/v1/cases`: Listing of registered cases.
//! - `GET  /api/v1/cases/:case_id`: Case retrieval by ID.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::app::AppState;
use crate::error::AppError;
use crate::ingestion::storage::validate_safe_id;
use crate::persistence::CaseEntity;

#[derive(Debug, Deserialize)]
pub struct CreateCaseRequest {
    pub case_id: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub fir_number: Option<String>,
    pub investigator_id: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseDto {
    pub case_id: String,
    pub title: String,
    pub description: Option<String>,
    pub fir_number: Option<String>,
    pub status: String,
    pub investigator_id: Option<String>,
    pub metadata: serde_json::Value,
    pub created_at: String,
    pub updated_at: String,
}

impl From<CaseEntity> for CaseDto {
    fn from(c: CaseEntity) -> Self {
        Self {
            case_id: c.case_id.to_string(),
            title: c.title,
            description: c.description,
            fir_number: c.fir_number,
            status: c.status,
            investigator_id: c.investigator_id,
            metadata: c.metadata,
            created_at: c
                .created_at
                .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            updated_at: c
                .updated_at
                .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        }
    }
}

/// Handler for `POST /api/v1/cases`.
pub async fn create_case(
    State(state): State<AppState>,
    Json(payload): Json<CreateCaseRequest>,
) -> Result<impl IntoResponse, AppError> {
    if payload.title.trim().is_empty() {
        return Err(AppError::BadRequest(
            "Field 'title' cannot be empty".to_string(),
        ));
    }

    let case_uuid = match payload.case_id {
        Some(ref cid) => {
            validate_safe_id(cid, "case_id")?;
            Uuid::parse_str(cid).map_err(|e| {
                AppError::BadRequest(format!(
                    "Provided case_id '{}' is not a valid UUID: {}",
                    cid, e
                ))
            })?
        }
        None => Uuid::new_v4(),
    };

    let now = chrono::Utc::now();
    let entity = CaseEntity {
        case_id: case_uuid,
        title: payload.title.trim().to_string(),
        description: payload.description,
        fir_number: payload.fir_number,
        status: "ACTIVE".to_string(),
        investigator_id: payload.investigator_id,
        metadata: payload.metadata.unwrap_or_else(|| serde_json::json!({})),
        created_at: now,
        updated_at: now,
    };

    // If PostgreSQL repository is connected, persist
    if let Some(ref case_repo) = state.case_repo {
        let saved = case_repo.create_case(&entity).await?;
        return Ok((StatusCode::CREATED, Json(CaseDto::from(saved))));
    }

    // In filesystem mode, ensure case directory exists and persist case.json
    let storage = state.ingestion_service.storage();
    let case_dir = storage.root_dir().join(case_uuid.to_string());
    tokio::fs::create_dir_all(&case_dir).await.map_err(|e| {
        AppError::Internal(anyhow::anyhow!(
            "Failed to create case storage directory: {}",
            e
        ))
    })?;

    let case_file = case_dir.join("case.json");
    if let Ok(content) = serde_json::to_vec_pretty(&entity) {
        let _ = tokio::fs::write(&case_file, content).await;
    }

    Ok((StatusCode::CREATED, Json(CaseDto::from(entity))))
}

/// Handler for `GET /api/v1/cases`.
pub async fn list_cases(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    if let Some(ref case_repo) = state.case_repo {
        if let Ok(cases) = case_repo.list_cases().await {
            let dtos: Vec<CaseDto> = cases.into_iter().map(CaseDto::from).collect();
            return Ok((StatusCode::OK, Json(dtos)));
        }
    }

    // Filesystem listing fallback
    let storage = state.ingestion_service.storage();
    let mut dtos = Vec::new();
    if storage.root_dir().exists() {
        if let Ok(mut entries) = tokio::fs::read_dir(storage.root_dir()).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                if let Ok(ft) = entry.file_type().await {
                    if ft.is_dir() {
                        let name = entry.file_name().to_string_lossy().to_string();
                        if !name.starts_with('.') {
                            let case_file = entry.path().join("case.json");
                            if case_file.exists() {
                                if let Ok(bytes) = tokio::fs::read(&case_file).await {
                                    if let Ok(entity) = serde_json::from_slice::<CaseEntity>(&bytes)
                                    {
                                        dtos.push(CaseDto::from(entity));
                                        continue;
                                    }
                                }
                            }

                            dtos.push(CaseDto {
                                case_id: name.clone(),
                                title: format!("Case {}", name),
                                description: Some("Filesystem evidence case container".to_string()),
                                fir_number: None,
                                status: "ACTIVE".to_string(),
                                investigator_id: None,
                                metadata: serde_json::json!({ "storage": "filesystem" }),
                                created_at: chrono::Utc::now()
                                    .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                                updated_at: chrono::Utc::now()
                                    .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                            });
                        }
                    }
                }
            }
        }
    }

    dtos.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok((StatusCode::OK, Json(dtos)))
}

/// Handler for `GET /api/v1/cases/:case_id`.
pub async fn get_case(
    State(state): State<AppState>,
    Path(case_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    validate_safe_id(&case_id, "case_id")?;

    if let (Some(ref case_repo), Ok(case_uuid)) = (&state.case_repo, Uuid::parse_str(&case_id)) {
        if let Some(case_entity) = case_repo.get_case(case_uuid).await? {
            return Ok((StatusCode::OK, Json(CaseDto::from(case_entity))));
        }
    }

    // Filesystem fallback: check if directory exists
    let storage = state.ingestion_service.storage();
    let case_dir = storage.root_dir().join(&case_id);
    if case_dir.exists() {
        let case_file = case_dir.join("case.json");
        if case_file.exists() {
            if let Ok(bytes) = tokio::fs::read(&case_file).await {
                if let Ok(entity) = serde_json::from_slice::<CaseEntity>(&bytes) {
                    return Ok((StatusCode::OK, Json(CaseDto::from(entity))));
                }
            }
        }

        return Ok((
            StatusCode::OK,
            Json(CaseDto {
                case_id: case_id.clone(),
                title: format!("Case {}", case_id),
                description: Some("Filesystem evidence case container".to_string()),
                fir_number: None,
                status: "ACTIVE".to_string(),
                investigator_id: None,
                metadata: serde_json::json!({ "storage": "filesystem" }),
                created_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                updated_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            }),
        ));
    }

    Err(AppError::NotFound(format!(
        "Case not found for id '{}'",
        case_id
    )))
}
