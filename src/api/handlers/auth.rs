//! Authentication handlers.

use axum::{
    Json,
    extract::{Query, State},
};
use serde::Deserialize;

use crate::api::state::AppState;
use crate::domain::{ApiResponse, TokenResponse};
use crate::error::{AppError, Result};

/// Query parameters for token endpoints.
#[derive(Debug, Deserialize)]
pub struct TokenQuery {
    /// The key name to get/create/reset token for.
    pub key: String,
}

/// Get or create a token for a key.
///
/// If a valid token exists for the key, returns it.
/// Otherwise, creates a new token and returns it.
///
/// # Errors
///
/// Returns an error if the key parameter is missing.
pub async fn get_token(
    State(state): State<AppState>,
    Query(query): Query<TokenQuery>,
) -> Result<Json<ApiResponse<TokenResponse>>> {
    if query.key.is_empty() {
        return Err(AppError::BadRequest(
            "key parameter is required".to_string(),
        ));
    }

    let token_info = state.token_service.get_or_create_token(&query.key);

    let response = TokenResponse {
        key: token_info.key,
        token: token_info.token,
        token_type: "key".to_string(),
        expires_at: token_info.expires_at.to_rfc3339(),
    };

    Ok(Json(ApiResponse::success(response)))
}

/// Reset (regenerate) a token for a key.
///
/// Creates a new token for the key, invalidating any existing token.
///
/// # Errors
///
/// Returns an error if the key parameter is missing.
pub async fn reset_token(
    State(state): State<AppState>,
    Query(query): Query<TokenQuery>,
) -> Result<Json<ApiResponse<TokenResponse>>> {
    if query.key.is_empty() {
        return Err(AppError::BadRequest(
            "key parameter is required".to_string(),
        ));
    }

    let token_info = state.token_service.reset_token(&query.key);

    let response = TokenResponse {
        key: token_info.key,
        token: token_info.token,
        token_type: "key".to_string(),
        expires_at: token_info.expires_at.to_rfc3339(),
    };

    Ok(Json(ApiResponse::success(response)))
}

/// Verify admin token validity.
///
/// Returns success if the admin token is valid.
/// The actual authentication is handled by the `require_admin` middleware,
/// so if this handler is reached, the token is valid.
pub async fn verify() -> Json<ApiResponse<()>> {
    Json(ApiResponse::ok())
}
