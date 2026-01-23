//! Configuration management handlers.

use axum::{
    Json,
    extract::{Query, State},
};
use serde::Deserialize;

use crate::api::state::AppState;
use crate::domain::{
    ApiResponse, ConfigSummary, FormattedConfig, IncrementConfig, ListConfigQuery,
    ListConfigResponse, SnowflakeConfig,
};
use crate::error::{AppError, Result};

/// Query parameters for getting a config.
#[derive(Debug, Deserialize)]
pub struct GetConfigQuery {
    /// Configuration name.
    pub name: String,
}

// ============== List Configs ==============

/// List all configurations with pagination.
///
/// # Errors
///
/// Returns an error if the query parameters are invalid or storage fails.
pub async fn list_configs(
    State(state): State<AppState>,
    Query(query): Query<ListConfigQuery>,
) -> Result<Json<ApiResponse<ListConfigResponse>>> {
    // Validate query parameters
    query.validate().map_err(AppError::BadRequest)?;

    // Collect all configs from all three services
    let mut items: Vec<ConfigSummary> = Vec::new();

    // Get increment configs
    let increment_configs = state.increment_service.list_configs().await?;
    for config in increment_configs {
        items.push(ConfigSummary {
            key: config.name,
            id_type: "increment".to_string(),
        });
    }

    // Get snowflake configs
    let snowflake_configs = state.snowflake_service.list_configs().await?;
    for config in snowflake_configs {
        items.push(ConfigSummary {
            key: config.name,
            id_type: "snowflake".to_string(),
        });
    }

    // Get formatted configs
    let formatted_configs = state.formatted_service.list_configs().await?;
    for config in formatted_configs {
        items.push(ConfigSummary {
            key: config.name,
            id_type: "formatted".to_string(),
        });
    }

    // Sort by key
    items.sort_by(|a, b| a.key.cmp(&b.key));

    // Apply key prefix filter if provided
    if let Some(ref key_prefix) = query.key {
        items.retain(|item| item.key.starts_with(key_prefix));
    }

    // Apply cursor-based pagination (from)
    if let Some(ref from_cursor) = query.from {
        // Find items after the cursor
        if let Some(pos) = items
            .iter()
            .position(|item| item.key.as_str() > from_cursor.as_str())
        {
            items = items.split_off(pos);
        } else {
            items.clear();
        }
    }

    // Apply size limit and determine if there are more records
    let has_more = items.len() > query.size as usize;
    items.truncate(query.size as usize);

    // Determine next cursor
    let next_cursor = if has_more {
        items.last().map(|item| item.key.clone())
    } else {
        None
    };

    let response = ListConfigResponse {
        items,
        next_cursor,
        has_more,
    };

    Ok(Json(ApiResponse::success(response)))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::state::AppState;
    use crate::config::{AppConfig, FileStorageConfig};
    use crate::domain::SequenceReset;
    use crate::storage::file::FileStorage;
    use std::sync::Arc;
    use tempfile::TempDir;

    fn create_test_app_state(temp_dir: &TempDir) -> AppState {
        let storage_config = FileStorageConfig {
            data_dir: temp_dir.path().to_path_buf(),
        };
        let storage = Arc::new(FileStorage::new(&storage_config).unwrap());

        let mut app_config = AppConfig::default();
        app_config.sequence.default_batch_size = 100;
        app_config.sequence.prefetch_threshold = 10;
        app_config.storage.file = storage_config;

        AppState::new(Arc::new(app_config), storage)
    }

    #[tokio::test]
    async fn test_list_configs_empty() {
        let temp_dir = TempDir::new().unwrap();
        let state = create_test_app_state(&temp_dir);

        let query = ListConfigQuery {
            key: None,
            from: None,
            size: 20,
        };

        let result = list_configs(axum::extract::State(state), axum::extract::Query(query))
            .await
            .unwrap();

        let response = result.0;
        assert_eq!(response.code, 0);
        assert!(response.data.is_some());

        let data = response.data.unwrap();
        assert!(data.items.is_empty());
        assert!(!data.has_more);
        assert!(data.next_cursor.is_none());
    }

    #[tokio::test]
    async fn test_list_configs_with_data() {
        let temp_dir = TempDir::new().unwrap();
        let state = create_test_app_state(&temp_dir);

        // Create some configs
        let inc_config = IncrementConfig {
            name: "orders".to_string(),
            start: 1000,
            step: 1,
            min: 1,
            max: i64::MAX,
        };
        state
            .increment_service
            .create_config(inc_config)
            .await
            .unwrap();

        let sf_config = SnowflakeConfig {
            name: "events".to_string(),
            epoch: 1704067200000,
            worker_bits: 10,
            sequence_bits: 12,
        };
        state
            .snowflake_service
            .create_config(sf_config)
            .await
            .unwrap();

        let fmt_config = FormattedConfig {
            name: "invoices".to_string(),
            pattern: "INV-{SEQ:6}".to_string(),
            sequence_reset: SequenceReset::Never,
        };
        state
            .formatted_service
            .create_config(fmt_config)
            .await
            .unwrap();

        let query = ListConfigQuery {
            key: None,
            from: None,
            size: 20,
        };

        let result = list_configs(axum::extract::State(state), axum::extract::Query(query))
            .await
            .unwrap();

        let data = result.0.data.unwrap();
        assert_eq!(data.items.len(), 3);
        assert!(!data.has_more);

        // Verify sorted by key
        assert_eq!(data.items[0].key, "events");
        assert_eq!(data.items[0].id_type, "snowflake");
        assert_eq!(data.items[1].key, "invoices");
        assert_eq!(data.items[1].id_type, "formatted");
        assert_eq!(data.items[2].key, "orders");
        assert_eq!(data.items[2].id_type, "increment");
    }

    #[tokio::test]
    async fn test_list_configs_with_key_filter() {
        let temp_dir = TempDir::new().unwrap();
        let state = create_test_app_state(&temp_dir);

        // Create configs with different prefixes
        let inc1 = IncrementConfig {
            name: "order_a".to_string(),
            ..Default::default()
        };
        state.increment_service.create_config(inc1).await.unwrap();

        let inc2 = IncrementConfig {
            name: "order_b".to_string(),
            ..Default::default()
        };
        state.increment_service.create_config(inc2).await.unwrap();

        let inc3 = IncrementConfig {
            name: "user_a".to_string(),
            ..Default::default()
        };
        state.increment_service.create_config(inc3).await.unwrap();

        // Filter by "order" prefix
        let query = ListConfigQuery {
            key: Some("order".to_string()),
            from: None,
            size: 20,
        };

        let result = list_configs(axum::extract::State(state), axum::extract::Query(query))
            .await
            .unwrap();

        let data = result.0.data.unwrap();
        assert_eq!(data.items.len(), 2);
        assert!(data.items.iter().all(|item| item.key.starts_with("order")));
    }

    #[tokio::test]
    async fn test_list_configs_pagination() {
        let temp_dir = TempDir::new().unwrap();
        let state = create_test_app_state(&temp_dir);

        // Create 5 configs
        for i in 1..=5 {
            let config = IncrementConfig {
                name: format!("config_{i:02}"),
                ..Default::default()
            };
            state.increment_service.create_config(config).await.unwrap();
        }

        // First page: size = 2
        let query = ListConfigQuery {
            key: None,
            from: None,
            size: 2,
        };

        let result = list_configs(
            axum::extract::State(state.clone()),
            axum::extract::Query(query),
        )
        .await
        .unwrap();

        let data = result.0.data.unwrap();
        assert_eq!(data.items.len(), 2);
        assert!(data.has_more);
        assert_eq!(data.next_cursor, Some("config_02".to_string()));
        assert_eq!(data.items[0].key, "config_01");
        assert_eq!(data.items[1].key, "config_02");

        // Second page: from = "config_02"
        let query = ListConfigQuery {
            key: None,
            from: Some("config_02".to_string()),
            size: 2,
        };

        let result = list_configs(
            axum::extract::State(state.clone()),
            axum::extract::Query(query),
        )
        .await
        .unwrap();

        let data = result.0.data.unwrap();
        assert_eq!(data.items.len(), 2);
        assert!(data.has_more);
        assert_eq!(data.items[0].key, "config_03");
        assert_eq!(data.items[1].key, "config_04");

        // Third page: from = "config_04"
        let query = ListConfigQuery {
            key: None,
            from: Some("config_04".to_string()),
            size: 2,
        };

        let result = list_configs(axum::extract::State(state), axum::extract::Query(query))
            .await
            .unwrap();

        let data = result.0.data.unwrap();
        assert_eq!(data.items.len(), 1);
        assert!(!data.has_more);
        assert!(data.next_cursor.is_none());
        assert_eq!(data.items[0].key, "config_05");
    }
}
