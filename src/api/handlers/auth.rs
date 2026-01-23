//! Authentication handlers.

use std::time::Duration;

use axum::{
    Json,
    extract::{Path, State},
};
use serde::Deserialize;

use crate::api::state::AppState;
use crate::domain::{ApiResponse, TokenResponse};
use crate::error::{AppError, Result};

/// Request to create a new token.
#[derive(Debug, Deserialize)]
pub struct CreateTokenRequest {
    /// Token description.
    #[serde(default)]
    pub description: String,

    /// Expiration time in seconds.
    #[serde(default)]
    pub expires_in: Option<u64>,

    /// Permissions for this token.
    #[serde(default)]
    pub permissions: Vec<String>,
}

/// Create a new key token.
pub async fn create_token(
    State(state): State<AppState>,
    Json(request): Json<CreateTokenRequest>,
) -> Result<Json<ApiResponse<TokenResponse>>> {
    let expires_in = request.expires_in.map(Duration::from_secs);

    let token_info = state.token_service.generate_key_token(
        request.description,
        expires_in,
        request.permissions,
    );

    let response = TokenResponse {
        token: token_info.token,
        token_type: "key".to_string(),
        expires_at: token_info.expires_at.to_rfc3339(),
    };

    Ok(Json(ApiResponse::success(response)))
}

/// Revoke a token.
pub async fn revoke_token(
    State(state): State<AppState>,
    Path(token_id): Path<String>,
) -> Result<Json<ApiResponse<()>>> {
    if state.token_service.revoke(&token_id) {
        Ok(Json(ApiResponse::ok()))
    } else {
        Err(AppError::NotFound("Token not found".to_string()))
    }
}
