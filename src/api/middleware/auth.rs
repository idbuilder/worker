//! Authentication middleware.

use axum::{
    Json,
    body::Body,
    extract::State,
    http::{Request, StatusCode, Uri, header::AUTHORIZATION},
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde_json::json;

use crate::api::extractors::AuthContext;
use crate::api::state::AppState;
use crate::error::ErrorCode;
use crate::service::{GLOBAL_TOKEN_KEY, TokenType};

/// Extract bearer token from Authorization header.
fn extract_bearer_token(req: &Request<Body>) -> Option<String> {
    let auth_header = req.headers().get(AUTHORIZATION)?.to_str().ok()?;

    auth_header
        .strip_prefix("Bearer ")
        .or_else(|| auth_header.strip_prefix("bearer "))
        .map(ToString::to_string)
}

/// Extract 'name' query parameter from URI.
fn extract_name_param(uri: &Uri) -> Option<String> {
    uri.query().and_then(|q| {
        url::form_urlencoded::parse(q.as_bytes())
            .find(|(k, _)| k == "name")
            .map(|(_, v)| v.into_owned())
    })
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
    let Some(token) = extract_bearer_token(&req) else {
        return unauthorized_response("Missing or invalid Authorization header");
    };

    let Some(token_type) = state.token_service.validate(&token) else {
        return unauthorized_response("Invalid or expired token");
    };

    // Get the token's associated key
    let token_key = if token_type == TokenType::Admin {
        String::new()
    } else {
        state
            .token_service
            .get_token_key(&token)
            .unwrap_or_default()
    };

    // Add auth context to request extensions
    req.extensions_mut()
        .insert(AuthContext::new(token_type, token, token_key));

    next.run(req).await
}

/// Middleware that requires admin authentication.
pub async fn require_admin(
    State(state): State<AppState>,
    mut req: Request<Body>,
    next: Next,
) -> Response {
    let Some(token) = extract_bearer_token(&req) else {
        return unauthorized_response("Missing or invalid Authorization header");
    };

    let Some(token_type) = state.token_service.validate(&token) else {
        return unauthorized_response("Invalid or expired token");
    };

    if token_type != TokenType::Admin {
        return forbidden_response("Admin token required");
    }

    // Add auth context to request extensions (admin tokens have empty token_key)
    req.extensions_mut()
        .insert(AuthContext::new(token_type, token, String::new()));

    next.run(req).await
}

/// Middleware that requires key authentication (or admin) with token-to-key validation.
///
/// This middleware verifies that:
/// 1. The token is valid (admin or key token)
/// 2. For key tokens accessing a specific key (via `name` query param):
///    - If the config has `key_token_enable: true`, the token must be for that specific key
///    - If the config has `key_token_enable: false` (default), both global and per-key tokens are accepted
pub async fn require_key(
    State(state): State<AppState>,
    mut req: Request<Body>,
    next: Next,
) -> Response {
    let Some(token) = extract_bearer_token(&req) else {
        return unauthorized_response("Missing or invalid Authorization header");
    };

    let Some(token_type) = state.token_service.validate(&token) else {
        return unauthorized_response("Invalid or expired token");
    };

    // Get the token's associated key
    let token_key = if token_type == TokenType::Admin {
        String::new()
    } else {
        state
            .token_service
            .get_token_key(&token)
            .unwrap_or_default()
    };

    // Extract requested key name from query params
    let requested_key = extract_name_param(req.uri());

    // If we have a requested key, validate authorization
    if let Some(ref key) = requested_key {
        // Admin tokens bypass key-specific checks
        if token_type != TokenType::Admin {
            // Fetch config to check key_token_enable
            let key_token_enable = get_key_token_enable(&state, key).await;

            // Validate token authorization
            if key_token_enable {
                // Requires per-key token
                if token_key != *key {
                    return forbidden_response(&format!("Token not authorized for key: {key}"));
                }
            } else {
                // Accepts global token OR per-key token
                if token_key != *key && token_key != GLOBAL_TOKEN_KEY {
                    return forbidden_response(&format!("Token not authorized for key: {key}"));
                }
            }
        }
    }

    // Add auth context to request extensions
    req.extensions_mut()
        .insert(AuthContext::new(token_type, token, token_key));

    next.run(req).await
}

/// Get `key_token_enable` setting for a key (checks all config types).
///
/// Returns `false` (accept global token) if the config is not found.
async fn get_key_token_enable(state: &AppState, key: &str) -> bool {
    // Try increment config
    if let Ok(Some(config)) = state.storage.get_increment_config(key).await {
        return config.key_token_enable;
    }
    // Try snowflake config
    if let Ok(Some(config)) = state.storage.get_snowflake_config(key).await {
        return config.key_token_enable;
    }
    // Try formatted config
    if let Ok(Some(config)) = state.storage.get_formatted_config(key).await {
        return config.key_token_enable;
    }
    // Default: accept global token
    false
}
