//! Server configuration.

use std::net::IpAddr;

use serde::Deserialize;

/// HTTP server configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    /// Host address to bind to.
    #[serde(default = "default_host")]
    pub host: IpAddr,

    /// Port to listen on.
    #[serde(default = "default_port")]
    pub port: u16,

    /// Number of worker threads (0 = number of CPU cores).
    #[serde(default)]
    pub workers: usize,
}

fn default_host() -> IpAddr {
    "0.0.0.0".parse().unwrap()
}

fn default_port() -> u16 {
    8080
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: 8080,
            workers: 0,
        }
    }
}
