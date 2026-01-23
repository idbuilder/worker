//! IDBuilder Worker Service Entry Point
//!
//! This is the main entry point for the IDBuilder worker service.
//! It initializes configuration, storage, services, and starts the HTTP server.

use idbuilder_worker::run;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    run().await
}
