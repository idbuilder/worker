//! Router setup and configuration.

use axum::{
    Router, middleware,
    routing::{get, post},
};
use tower_http::trace::TraceLayer;

use crate::api::handlers::{auth, config, health, id};
use crate::api::middleware::auth::{require_admin, require_key};
use crate::api::state::AppState;

/// Create the main application router.
pub fn create_router(state: AppState) -> Router {
    // Health and metrics routes (no auth required)
    let health_routes = Router::new()
        .route("/health", get(health::health))
        .route("/ready", get(health::ready))
        .route("/metrics", get(health::metrics));

    // Configuration routes (admin auth required)
    let config_routes = Router::new()
        .route("/list", get(config::list_configs))
        .route("/increment", post(config::create_increment))
        .route("/increment", get(config::get_increment))
        .route("/snowflake", post(config::create_snowflake))
        .route("/snowflake", get(config::get_snowflake))
        .route("/formatted", post(config::create_formatted))
        .route("/formatted", get(config::get_formatted))
        .layer(middleware::from_fn_with_state(state.clone(), require_admin));

    // ID generation routes (key auth required)
    let id_routes = Router::new()
        .route("/increment", get(id::generate_increment))
        .route("/snowflake", get(id::get_snowflake))
        .route("/formatted", get(id::generate_formatted))
        .layer(middleware::from_fn_with_state(state.clone(), require_key));

    // Auth routes (admin auth required)
    let auth_routes = Router::new()
        .route("/token", get(auth::get_token))
        .route("/tokenreset", get(auth::reset_token))
        .route("/verify", get(auth::verify))
        .layer(middleware::from_fn_with_state(state.clone(), require_admin));

    // Combine all routes
    Router::new()
        .merge(health_routes)
        .nest("/v1/config", config_routes)
        .nest("/v1/id", id_routes)
        .nest("/v1/auth", auth_routes)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
