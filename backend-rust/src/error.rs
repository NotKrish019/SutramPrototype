//! Structured Application Errors & HTTP Response Mapping.
//!
//! Enforces security invariants:
//! - Never leaks internal filesystem paths, stack traces, or credentials in responses.
//! - Emits structured audit logs with internal error details.
//! - Returns standardized RFC 7807-compliant JSON error envelopes.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};

/// Application-level error enumeration.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Resource not found: {0}")]
    NotFound(String),

    #[error("Invalid request payload or parameters: {0}")]
    BadRequest(String),

    #[error("Conflicting forensic resource or duplicate entry: {0}")]
    Conflict(String),

    #[error("Payload exceeds maximum allowable size: {0}")]
    PayloadTooLarge(String),

    #[error("Content validation rejected: {0}")]
    UnprocessableEntity(String),

    #[error("Unsupported evidence format or media type: {0}")]
    UnsupportedMediaType(String),

    #[error("Internal processing error")]
    Internal(#[from] anyhow::Error),
}

/// JSON envelope for structured API error responses.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ErrorEnvelope {
    pub error: ErrorDetail,
}

/// Error detail payload.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ErrorDetail {
    pub code: String,
    pub message: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message) = match &self {
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, "NOT_FOUND", msg.clone()),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, "BAD_REQUEST", msg.clone()),
            AppError::Conflict(msg) => (StatusCode::CONFLICT, "CONFLICT", msg.clone()),
            AppError::PayloadTooLarge(msg) => (
                StatusCode::PAYLOAD_TOO_LARGE,
                "PAYLOAD_TOO_LARGE",
                msg.clone(),
            ),
            AppError::UnprocessableEntity(msg) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "UNPROCESSABLE_ENTITY",
                msg.clone(),
            ),
            AppError::UnsupportedMediaType(msg) => (
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "UNSUPPORTED_MEDIA_TYPE",
                msg.clone(),
            ),
            AppError::Internal(err) => {
                // Log internal diagnostic securely on the server side
                tracing::error!(error = ?err, "Internal server error encountered");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "INTERNAL_SERVER_ERROR",
                    "An internal server error occurred while processing the request.".to_string(),
                )
            }
        };

        let envelope = ErrorEnvelope {
            error: ErrorDetail {
                code: code.to_string(),
                message,
            },
        };

        (status, Json(envelope)).into_response()
    }
}
