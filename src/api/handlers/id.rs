//! ID generation handlers.

use axum::{
    Json,
    extract::{Query, State},
};
use serde::Deserialize;

use crate::api::state::AppState;
use crate::domain::{ApiResponse, FormattedIdResponse, IncrementIdResponse, SnowflakeIdResponse};
use crate::error::{AppError, Result};

/// Query parameters for ID generation.
#[derive(Debug, Deserialize)]
pub struct GenerateQuery {
    /// Configuration name.
    pub name: String,

    /// Number of IDs to generate (default: 1, max: 1000).
    #[serde(default = "default_count")]
    pub count: u32,
}

fn default_count() -> u32 {
    1
}

impl GenerateQuery {
    /// Validate the query parameters.
    fn validate(&self) -> Result<()> {
        if self.name.is_empty() {
            return Err(AppError::BadRequest("name is required".to_string()));
        }
        if self.count == 0 {
            return Err(AppError::BadRequest("count must be at least 1".to_string()));
        }
        if self.count > 1000 {
            return Err(AppError::BadRequest("count cannot exceed 1000".to_string()));
        }
        Ok(())
    }
}

/// Generate auto-increment IDs.
pub async fn generate_increment(
    State(state): State<AppState>,
    Query(query): Query<GenerateQuery>,
) -> Result<Json<ApiResponse<IncrementIdResponse>>> {
    query.validate()?;

    let ids = state
        .increment_service
        .generate(&query.name, query.count)
        .await?;

    Ok(Json(ApiResponse::success(IncrementIdResponse::new(ids))))
}

/// Get snowflake configuration with worker ID.
pub async fn get_snowflake(
    State(state): State<AppState>,
    Query(query): Query<GenerateQuery>,
) -> Result<Json<ApiResponse<SnowflakeIdResponse>>> {
    if query.name.is_empty() {
        return Err(AppError::BadRequest("name is required".to_string()));
    }

    let response = state
        .snowflake_service
        .get_config_with_worker_id(&query.name)
        .await?;

    Ok(Json(ApiResponse::success(response)))
}

/// Generate formatted string IDs.
pub async fn generate_formatted(
    State(state): State<AppState>,
    Query(query): Query<GenerateQuery>,
) -> Result<Json<ApiResponse<FormattedIdResponse>>> {
    query.validate()?;

    let ids = state
        .formatted_service
        .generate(&query.name, query.count)
        .await?;

    Ok(Json(ApiResponse::success(FormattedIdResponse::new(ids))))
}
