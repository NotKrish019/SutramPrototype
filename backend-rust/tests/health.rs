//! Integration tests for Health Check endpoint.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use sutram_backend_rust::{api::routes::health::HealthResponse, create_router, AppConfig};
use tower::ServiceExt;

#[tokio::test]
async fn test_health_endpoint_returns_ok_and_service_name() {
    let config = AppConfig::default();
    let app = create_router(config);

    let request = Request::builder()
        .uri("/health")
        .method("GET")
        .body(Body::empty())
        .expect("Failed to build request");

    let response = app
        .oneshot(request)
        .await
        .expect("Failed to execute request");

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = response
        .into_body()
        .collect()
        .await
        .expect("Failed to read response body")
        .to_bytes();

    let health_resp: HealthResponse =
        serde_json::from_slice(&body_bytes).expect("Failed to deserialize JSON response");

    assert_eq!(health_resp.status, "ok");
    assert_eq!(health_resp.service, "sutram-backend-rust");
}

#[tokio::test]
async fn test_unknown_route_returns_404() {
    let config = AppConfig::default();
    let app = create_router(config);

    let request = Request::builder()
        .uri("/non-existent-route")
        .method("GET")
        .body(Body::empty())
        .expect("Failed to build request");

    let response = app
        .oneshot(request)
        .await
        .expect("Failed to execute request");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_readiness_endpoint_unconfigured_mode() {
    let config = AppConfig::default();
    let app = create_router(config);

    let request = Request::builder()
        .uri("/health/ready")
        .method("GET")
        .body(Body::empty())
        .expect("Failed to build request");

    let response = app
        .oneshot(request)
        .await
        .expect("Failed to execute request");

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = response
        .into_body()
        .collect()
        .await
        .expect("Failed to read response body")
        .to_bytes();

    let json: serde_json::Value =
        serde_json::from_slice(&body_bytes).expect("Failed to parse json");

    assert_eq!(json["status"], "degraded");
    assert_eq!(json["service"], "sutram-backend-rust");
    assert_eq!(json["database"], "unconfigured_filesystem_mode");
    assert_eq!(json["graph"], "disabled");
}
