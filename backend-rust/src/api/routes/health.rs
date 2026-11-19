//! Health Check Route Handlers.
//!
//! Provides liveness (`GET /health`) and database readiness (`GET /health/ready`) probes.

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};

use crate::app::AppState;

/// Payload returned by the liveness check endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HealthResponse {
    pub status: String,
    pub service: String,
}

/// Payload returned by the readiness probe endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReadinessResponse {
    pub status: String,
    pub service: String,
    pub database: String,
    pub graph: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Handler for `GET /health` (Liveness probe).
pub async fn health_check() -> impl IntoResponse {
    Json(HealthResponse {
        status: "ok".to_string(),
        service: "sutram-backend-rust".to_string(),
    })
}

/// Handler for `GET /health/ready` (Readiness probe).
pub async fn readiness_check(State(state): State<AppState>) -> impl IntoResponse {
    let mut db_status = "unconfigured_filesystem_mode".to_string();
    let mut graph_status = "disabled".to_string();
    let mut is_db_healthy = true;
    let mut is_graph_healthy = true;
    let mut err_msg = None;

    if let Some(db) = &state.database {
        match db.ping().await {
            Ok(()) => {
                db_status = "connected".to_string();
            }
            Err(e) => {
                db_status = "disconnected".to_string();
                is_db_healthy = false;
                err_msg = Some(format!("Database error: {}", e));
            }
        }
    } else if state.config.database_url.is_some() {
        db_status = "failed_to_initialize".to_string();
        is_db_healthy = false;
        err_msg = Some(
            "Database configured in environment but connection was not established at startup"
                .to_string(),
        );
    }

    if let Some(graph) = &state.graph_client {
        match graph.ping().await {
            Ok(()) => {
                graph_status = "connected".to_string();
            }
            Err(e) => {
                graph_status = "disconnected".to_string();
                is_graph_healthy = false;
                if err_msg.is_none() {
                    err_msg = Some(format!("Graph error: {}", e));
                }
            }
        }
    }

    if !is_db_healthy || !is_graph_healthy {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ReadinessResponse {
                status: "not_ready".to_string(),
                service: "sutram-backend-rust".to_string(),
                database: db_status,
                graph: graph_status,
                error: err_msg,
            }),
        )
    } else if db_status == "unconfigured_filesystem_mode" || graph_status == "disabled" {
        (
            StatusCode::OK,
            Json(ReadinessResponse {
                status: "degraded".to_string(),
                service: "sutram-backend-rust".to_string(),
                database: db_status,
                graph: graph_status,
                error: None,
            }),
        )
    } else {
        (
            StatusCode::OK,
            Json(ReadinessResponse {
                status: "ready".to_string(),
                service: "sutram-backend-rust".to_string(),
                database: db_status,
                graph: graph_status,
                error: None,
            }),
        )
    }
}
