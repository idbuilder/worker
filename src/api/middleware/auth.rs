//! Authentication middleware.

use axum::{
    Json,
    body::Body,
    extract::State,
    http::{Request, StatusCode, header::AUTHORIZATION},
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde_json::json;

use crate::api::extractors::AuthContext;
use crate::api::state::AppState;
use crate::error::ErrorCode;
use crate::service::TokenType;

/// Extract bearer token from Authorization header.
fn extract_bearer_token(req: &Request<Body>) -> Option<String> {
    let auth_header = req.headers().get(AUTHORIZATION)?.to_str().ok()?;

    if let Some(token) = auth_header.strip_prefix("Bearer ") {
        Some(token.to_string())
    } else if let Some(token) = auth_header.strip_prefix("bearer ") {
        Some(token.to_string())
    } else {
        None
    }
}

/// Create an unauthorized response.
fn unauthorized_response(message: &str) -> Response {
    let body = Json(json!({
        "code": ErrorCode::UNAUTHORIZED.as_i32(),
        "message": message,
        "data": null
    }));

    (StatusCode::UNAUTHORIZED, body).into_response()
}

/// Create a forbidden response.
fn forbidden_response(message: &str) -> Response {
    let body = Json(json!({
        "code": ErrorCode::FORBIDDEN.as_i32(),
        "message": message,
        "data": null
    }));

    (StatusCode::FORBIDDEN, body).into_response()
}

/// Middleware that requires any valid authentication (admin or key).
pub async fn require_auth(
    State(state): State<AppState>,
    mut req: Request<Body>,
    next: Next,
) -> Response {
    let token = match extract_bearer_token(&req) {
        Some(t) => t,
        None => return unauthorized_response("Missing or invalid Authorization header"),
    };

    let token_type = match state.token_service.validate(&token) {
        Some(t) => t,
        None => return unauthorized_response("Invalid or expired token"),
    };

    // Add auth context to request extensions
    req.extensions_mut()
        .insert(AuthContext::new(token_type, token));

    next.run(req).await
}

/// Middleware that requires admin authentication.
pub async fn require_admin(
    State(state): State<AppState>,
    mut req: Request<Body>,
    next: Next,
) -> Response {
    let token = match extract_bearer_token(&req) {
        Some(t) => t,
        None => return unauthorized_response("Missing or invalid Authorization header"),
    };

    let token_type = match state.token_service.validate(&token) {
        Some(t) => t,
        None => return unauthorized_response("Invalid or expired token"),
    };

    if token_type != TokenType::Admin {
        return forbidden_response("Admin token required");
    }

    // Add auth context to request extensions
    req.extensions_mut()
        .insert(AuthContext::new(token_type, token));

    next.run(req).await
}

/// Middleware that requires key authentication (or admin).
pub async fn require_key(
    State(state): State<AppState>,
    mut req: Request<Body>,
    next: Next,
) -> Response {
    let token = match extract_bearer_token(&req) {
        Some(t) => t,
        None => return unauthorized_response("Missing or invalid Authorization header"),
    };

    let token_type = match state.token_service.validate(&token) {
        Some(t) => t,
        None => return unauthorized_response("Invalid or expired token"),
    };

    // Both admin and key tokens can access key routes
    req.extensions_mut()
        .insert(AuthContext::new(token_type, token));

    next.run(req).await
}
