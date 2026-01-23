//! # IDBuilder Worker
//!
//! A distributed ID generation service supporting multiple ID types:
//!
//! - **Auto-increment IDs**: Sequential numeric IDs with configurable start, step, and range
//! - **Snowflake IDs**: Twitter Snowflake-style distributed IDs (client-side generation)
//! - **Formatted IDs**: Custom string patterns like `INV20260123-0001`
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │                           Worker Service                             │
//! ├─────────────────────────────────────────────────────────────────────┤
//! │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌────────────┐ │
//! │  │   API Layer │  │   Service   │  │   Storage   │  │  Domain    │ │
//! │  │  (Axum)     │→ │   Layer     │→ │   Layer     │  │  Models    │ │
//! │  └─────────────┘  └─────────────┘  └─────────────┘  └────────────┘ │
//! └─────────────────────────────────────────────────────────────────────┘
//! ```

#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![forbid(unsafe_code)]

pub mod api;
pub mod config;
pub mod domain;
pub mod error;
pub mod service;
pub mod storage;

use std::net::SocketAddr;
use std::sync::Arc;

use tokio::net::TcpListener;
use tokio::signal;
use tracing::{info, warn};

use crate::api::create_router;
use crate::api::state::AppState;
use crate::config::AppConfig;
use crate::storage::create_storage;

/// Run the IDBuilder worker service.
///
/// This function:
/// 1. Loads configuration from files and environment
/// 2. Initializes the storage backend
/// 3. Creates all services
/// 4. Starts the HTTP server
/// 5. Handles graceful shutdown
///
/// # Errors
///
/// Returns an error if:
/// - Configuration cannot be loaded
/// - Storage backend fails to initialize
/// - HTTP server fails to bind
pub async fn run() -> anyhow::Result<()> {
    // Load configuration
    let config = AppConfig::load()?;

    // Initialize logging
    init_logging(&config);

    info!(
        version = env!("CARGO_PKG_VERSION"),
        "Starting IDBuilder Worker"
    );

    // Initialize storage
    let storage = create_storage(&config.storage).await?;
    info!(backend = ?config.storage.backend, "Storage initialized");

    // Create application state
    let state = AppState::new(Arc::new(config.clone()), storage);

    // Create router
    let app = create_router(state);

    // Bind to address
    let addr = SocketAddr::new(config.server.host, config.server.port);
    let listener = TcpListener::bind(addr).await?;
    info!(%addr, "HTTP server listening");

    // Start server with graceful shutdown
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!("Server shutdown complete");
    Ok(())
}

/// Initialize logging based on configuration.
fn init_logging(config: &AppConfig) {
    use tracing_subscriber::{EnvFilter, fmt, prelude::*};

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&config.observability.log_level));

    let subscriber = tracing_subscriber::registry().with(filter);

    if config.observability.log_format == "json" {
        subscriber.with(fmt::layer().json()).init();
    } else {
        subscriber.with(fmt::layer()).init();
    }
}

/// Wait for shutdown signal (Ctrl+C or SIGTERM).
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {
            warn!("Received Ctrl+C, initiating graceful shutdown");
        }
        () = terminate => {
            warn!("Received SIGTERM, initiating graceful shutdown");
        }
    }
}
