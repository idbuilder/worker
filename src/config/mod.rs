//! Configuration management module.
//!
//! Supports loading configuration from:
//! - TOML files (config/default.toml, config/{profile}.toml)
//! - Environment variables with `IDBUILDER_WORKER__<SECTION>__<KEY>` pattern

mod server;
mod storage;

use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;

pub use server::ServerConfig;
pub use storage::{
    FileStorageConfig, MySqlStorageConfig, PostgresStorageConfig, RedisStorageConfig,
    StorageBackend, StorageConfig,
};

/// Application configuration.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct AppConfig {
    /// HTTP server configuration.
    pub server: ServerConfig,

    /// Storage backend configuration.
    pub storage: StorageConfig,

    /// Controller service configuration.
    pub controller: ControllerConfig,

    /// Sequence generation configuration.
    pub sequence: SequenceConfig,

    /// Authentication configuration.
    pub auth: AuthConfig,

    /// Observability configuration.
    pub observability: ObservabilityConfig,
}

impl AppConfig {
    /// Load configuration from files and environment.
    ///
    /// Configuration is loaded in the following order (later sources override earlier):
    /// 1. `config/default.toml`
    /// 2. `config/{IDBUILDER_PROFILE}.toml` (if `IDBUILDER_PROFILE` is set)
    /// 3. Environment variables with `IDBUILDER_WORKER__` prefix
    ///
    /// # Errors
    ///
    /// Returns an error if configuration cannot be loaded or is invalid.
    pub fn load() -> Result<Self, ConfigError> {
        // Determine profile
        let profile =
            std::env::var("IDBUILDER_PROFILE").unwrap_or_else(|_| "development".to_string());

        // Build configuration
        let config = Config::builder()
            // Load default configuration
            .add_source(File::with_name("config/default").required(false))
            // Load profile-specific configuration
            .add_source(File::with_name(&format!("config/{profile}")).required(false))
            // Override with environment variables
            // IDBUILDER_WORKER__SERVER__PORT=8080 -> server.port = 8080
            .add_source(
                Environment::with_prefix("IDBUILDER_WORKER")
                    .separator("__")
                    .try_parsing(true),
            )
            .build()?;

        // Deserialize and validate
        let app_config: Self = config.try_deserialize()?;
        app_config.validate()?;

        Ok(app_config)
    }

    /// Validate the configuration.
    fn validate(&self) -> Result<(), ConfigError> {
        // Validate server config
        if self.server.port == 0 {
            return Err(ConfigError::Message("server.port cannot be 0".to_string()));
        }

        // Validate storage config
        self.storage.validate()?;

        // Validate sequence config
        if self.sequence.default_batch_size == 0 {
            return Err(ConfigError::Message(
                "sequence.default_batch_size cannot be 0".to_string(),
            ));
        }

        Ok(())
    }
}

/// Controller service configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct ControllerConfig {
    /// Controller endpoint URL.
    #[serde(default)]
    pub endpoint: String,

    /// Heartbeat interval in seconds.
    #[serde(default = "default_heartbeat_interval")]
    pub heartbeat_interval: u64,
}

const fn default_heartbeat_interval() -> u64 {
    30
}

impl Default for ControllerConfig {
    fn default() -> Self {
        Self {
            endpoint: String::new(),
            heartbeat_interval: 30,
        }
    }
}

/// Sequence generation configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct SequenceConfig {
    /// Default batch size for pre-allocating sequence numbers.
    #[serde(default = "default_batch_size")]
    pub default_batch_size: u32,

    /// Prefetch threshold (prefetch when remaining < threshold).
    #[serde(default = "default_prefetch_threshold")]
    pub prefetch_threshold: u32,
}

const fn default_batch_size() -> u32 {
    1000
}

const fn default_prefetch_threshold() -> u32 {
    100
}

impl Default for SequenceConfig {
    fn default() -> Self {
        Self {
            default_batch_size: 1000,
            prefetch_threshold: 100,
        }
    }
}

/// Authentication configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct AuthConfig {
    /// Admin token for configuration APIs.
    #[serde(default = "default_admin_token")]
    pub admin_token: String,

    /// Key token expiration in seconds.
    #[serde(default = "default_key_token_expiration")]
    pub key_token_expiration: u64,
}

fn default_admin_token() -> String {
    "admin_change_me_in_production".to_string()
}

const fn default_key_token_expiration() -> u64 {
    30 * 24 * 60 * 60 // 30 days
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            admin_token: default_admin_token(),
            key_token_expiration: default_key_token_expiration(),
        }
    }
}

/// Observability configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct ObservabilityConfig {
    /// Log level.
    #[serde(default = "default_log_level")]
    pub log_level: String,

    /// Log format: "text" or "json".
    #[serde(default = "default_log_format")]
    pub log_format: String,

    /// Enable Prometheus metrics endpoint.
    #[serde(default = "default_metrics_enabled")]
    pub metrics_enabled: bool,

    /// Metrics endpoint path.
    #[serde(default = "default_metrics_path")]
    pub metrics_path: String,
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_log_format() -> String {
    "text".to_string()
}

const fn default_metrics_enabled() -> bool {
    true
}

fn default_metrics_path() -> String {
    "/metrics".to_string()
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            log_level: default_log_level(),
            log_format: default_log_format(),
            metrics_enabled: true,
            metrics_path: default_metrics_path(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AppConfig::default();
        assert_eq!(config.server.port, 8080);
        assert_eq!(config.storage.backend, StorageBackend::File);
        assert_eq!(config.sequence.default_batch_size, 1000);
    }
}
