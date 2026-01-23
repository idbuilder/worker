//! Application state for Axum handlers.

use std::sync::Arc;

use crate::config::AppConfig;
use crate::service::{FormattedService, IncrementService, SnowflakeService, TokenService};
use crate::storage::traits::Storage;

/// Shared application state.
#[derive(Clone)]
pub struct AppState {
    /// Application configuration.
    pub config: Arc<AppConfig>,
    /// Storage backend.
    pub storage: Arc<dyn Storage>,
    /// Increment ID service.
    pub increment_service: Arc<IncrementService>,
    /// Snowflake ID service.
    pub snowflake_service: Arc<SnowflakeService>,
    /// Formatted ID service.
    pub formatted_service: Arc<FormattedService>,
    /// Token service.
    pub token_service: Arc<TokenService>,
}

impl AppState {
    /// Create a new application state.
    pub fn new(config: Arc<AppConfig>, storage: Arc<dyn Storage>) -> Self {
        let increment_service = Arc::new(IncrementService::new(
            Arc::clone(&storage),
            &config.sequence,
        ));

        let snowflake_service = Arc::new(SnowflakeService::new(Arc::clone(&storage)));

        let formatted_service = Arc::new(FormattedService::new(
            Arc::clone(&storage),
            &config.sequence,
        ));

        let token_service = Arc::new(TokenService::new(&config.auth));

        Self {
            config,
            storage,
            increment_service,
            snowflake_service,
            formatted_service,
            token_service,
        }
    }
}
