//! Health check handlers.

use axum::{Json, extract::State, http::StatusCode};
use serde_json::{Value, json};

use crate::api::state::AppState;

/// Liveness probe - always returns 200 if the service is running.
pub async fn health() -> Json<Value> {
    Json(json!({
        "code": 0,
        "message": "success",
        "data": {
            "status": "healthy",
            "version": env!("CARGO_PKG_VERSION")
        }
    }))
}

/// Readiness probe - checks if the service can serve requests.
pub async fn ready(State(state): State<AppState>) -> (StatusCode, Json<Value>) {
    // Check storage health
    let storage_ok = state.storage.health_check().await.is_ok();

    let status_code = if storage_ok {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    let response = Json(json!({
        "code": if storage_ok { 0 } else { 5003 },
        "message": if storage_ok { "success" } else { "service unavailable" },
        "data": {
            "ready": storage_ok,
            "components": {
                "storage": storage_ok
            }
        }
    }));

    (status_code, response)
}

/// Prometheus metrics endpoint.
pub async fn metrics() -> String {
    // TODO: Implement proper Prometheus metrics
    // For now, return basic metrics
    let mut output = String::new();

    output.push_str("# HELP idbuilder_up Whether the service is up\n");
    output.push_str("# TYPE idbuilder_up gauge\n");
    output.push_str("idbuilder_up 1\n");

    output
}
