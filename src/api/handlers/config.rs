//! Configuration management handlers.

use axum::{
    Json,
    extract::{Query, State},
};
use serde::Deserialize;

use crate::api::state::AppState;
use crate::domain::{ApiResponse, FormattedConfig, IncrementConfig, SnowflakeConfig};
use crate::error::Result;

/// Query parameters for getting a config.
#[derive(Debug, Deserialize)]
pub struct GetConfigQuery {
    /// Configuration name.
    pub name: String,
}

// ============== Increment Config ==============

/// Create a new increment configuration.
///
/// # Errors
///
/// Returns an error if the configuration is invalid or already exists.
pub async fn create_increment(
    State(state): State<AppState>,
    Json(config): Json<IncrementConfig>,
) -> Result<Json<ApiResponse<()>>> {
    state.increment_service.create_config(config).await?;
    Ok(Json(ApiResponse::ok()))
}

/// Get an increment configuration.
///
/// # Errors
///
/// Returns an error if the configuration is not found.
pub async fn get_increment(
    State(state): State<AppState>,
    Query(query): Query<GetConfigQuery>,
) -> Result<Json<ApiResponse<IncrementConfig>>> {
    let config = state.increment_service.get_config(&query.name).await?;
    Ok(Json(ApiResponse::success(config)))
}

// ============== Snowflake Config ==============

/// Create a new snowflake configuration.
///
/// # Errors
///
/// Returns an error if the configuration is invalid or already exists.
pub async fn create_snowflake(
    State(state): State<AppState>,
    Json(config): Json<SnowflakeConfig>,
) -> Result<Json<ApiResponse<()>>> {
    state.snowflake_service.create_config(config).await?;
    Ok(Json(ApiResponse::ok()))
}

/// Get a snowflake configuration.
///
/// # Errors
///
/// Returns an error if the configuration is not found.
pub async fn get_snowflake(
    State(state): State<AppState>,
    Query(query): Query<GetConfigQuery>,
) -> Result<Json<ApiResponse<SnowflakeConfig>>> {
    let config = state.snowflake_service.get_config(&query.name).await?;
    Ok(Json(ApiResponse::success(config)))
}

// ============== Formatted Config ==============

/// Create a new formatted configuration.
///
/// # Errors
///
/// Returns an error if the configuration is invalid or already exists.
pub async fn create_formatted(
    State(state): State<AppState>,
    Json(config): Json<FormattedConfig>,
) -> Result<Json<ApiResponse<()>>> {
    state.formatted_service.create_config(config).await?;
    Ok(Json(ApiResponse::ok()))
}

/// Get a formatted configuration.
///
/// # Errors
///
/// Returns an error if the configuration is not found.
pub async fn get_formatted(
    State(state): State<AppState>,
    Query(query): Query<GetConfigQuery>,
) -> Result<Json<ApiResponse<FormattedConfig>>> {
    let config = state.formatted_service.get_config(&query.name).await?;
    Ok(Json(ApiResponse::success(config)))
}
