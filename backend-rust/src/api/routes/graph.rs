//! Graph API Route Handlers.
//!
//! Provides typed REST endpoints for knowledge graph exploration:
//! - `GET /api/v1/cases/:case_id/graph/summary`
//! - `GET /api/v1/cases/:case_id/graph/entities/:entity_id/neighbors`
//! - `GET /api/v1/cases/:case_id/graph/path`

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::app::AppState;
use crate::error::AppError;
use crate::graph::GraphSummaryResponse;

#[derive(Debug, Deserialize)]
pub struct NeighborQuery {
    pub depth: Option<u32>,
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct PathQuery {
    pub from: String,
    pub to: String,
    pub max_hops: Option<u32>,
}

/// Handler for `GET /api/v1/cases/:case_id/graph/summary`.
pub async fn get_case_graph_summary(
    State(state): State<AppState>,
    Path(case_id_str): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let case_id = Uuid::parse_str(&case_id_str)
        .map_err(|e| AppError::BadRequest(format!("Invalid case_id UUID: {}", e)))?;

    if let Some(query_service) = &state.graph_query_service {
        let summary = query_service.get_case_summary(case_id).await?;
        Ok((StatusCode::OK, Json(summary)))
    } else {
        // Fallback when graph service is unconfigured
        Ok((
            StatusCode::OK,
            Json(GraphSummaryResponse {
                case_id,
                evidence_count: 0,
                extraction_run_count: 0,
                entity_candidate_count: 0,
                event_candidate_count: 0,
                relationship_assertion_count: 0,
                reconstruction_run_count: 0,
                contradiction_lead_count: 0,
                graph_synchronization_status: "GRAPH_DISABLED".to_string(),
                last_projection_time: None,
                identifier_privacy: "unconfigured".to_string(),
            }),
        ))
    }
}

/// Handler for `GET /api/v1/cases/:case_id/graph/entities/:entity_id/neighbors`.
pub async fn get_entity_neighbors(
    State(state): State<AppState>,
    Path((case_id_str, entity_id)): Path<(String, String)>,
    Query(params): Query<NeighborQuery>,
) -> Result<impl IntoResponse, AppError> {
    let case_id = Uuid::parse_str(&case_id_str)
        .map_err(|e| AppError::BadRequest(format!("Invalid case_id UUID: {}", e)))?;

    let depth = params.depth.unwrap_or(1);
    let limit = params.limit.unwrap_or(50);

    if let Some(query_service) = &state.graph_query_service {
        let neighbors = query_service
            .get_entity_neighbors(case_id, &entity_id, depth, limit)
            .await?;
        Ok((StatusCode::OK, Json(neighbors)))
    } else {
        Err(AppError::BadRequest(
            "Graph subsystem is not enabled or unconfigured on this node.".to_string(),
        ))
    }
}

/// Handler for `GET /api/v1/cases/:case_id/graph/path`.
pub async fn find_entity_path(
    State(state): State<AppState>,
    Path(case_id_str): Path<String>,
    Query(params): Query<PathQuery>,
) -> Result<impl IntoResponse, AppError> {
    let case_id = Uuid::parse_str(&case_id_str)
        .map_err(|e| AppError::BadRequest(format!("Invalid case_id UUID: {}", e)))?;

    let max_hops = params.max_hops.unwrap_or(3);

    if let Some(query_service) = &state.graph_query_service {
        let path = query_service
            .find_path(case_id, &params.from, &params.to, max_hops)
            .await?;
        Ok((StatusCode::OK, Json(path)))
    } else {
        Err(AppError::BadRequest(
            "Graph subsystem is not enabled or unconfigured on this node.".to_string(),
        ))
    }
}
