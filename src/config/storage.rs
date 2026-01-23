//! Storage configuration.

use std::path::PathBuf;

use config::ConfigError;
use serde::Deserialize;

/// Storage backend type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StorageBackend {
    /// File-based storage (development/single-node).
    #[default]
    File,
    /// Redis storage (high-performance distributed).
    Redis,
    /// `MySQL` storage (strong consistency).
    #[serde(rename = "mysql")]
    MySQL,
    /// `PostgreSQL` storage (strong consistency).
    #[serde(rename = "postgresql")]
    PostgreSQL,
}

impl std::fmt::Display for StorageBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::File => write!(f, "file"),
            Self::Redis => write!(f, "redis"),
            Self::MySQL => write!(f, "mysql"),
            Self::PostgreSQL => write!(f, "postgresql"),
        }
    }
}

/// Storage configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct StorageConfig {
    /// Storage backend type.
    #[serde(default)]
    pub backend: StorageBackend,

    /// File storage configuration.
    #[serde(default)]
    pub file: FileStorageConfig,

    /// Redis storage configuration.
    #[serde(default)]
    pub redis: RedisStorageConfig,

    /// `MySQL` storage configuration.
    #[serde(default)]
    pub mysql: MySqlStorageConfig,

    /// `PostgreSQL` storage configuration.
    #[serde(default)]
    pub postgresql: PostgresStorageConfig,
}

impl StorageConfig {
    /// Validate the storage configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if required configuration fields are missing for the selected backend.
    pub fn validate(&self) -> Result<(), ConfigError> {
        match self.backend {
            StorageBackend::File => {
                // File storage doesn't require much validation
                Ok(())
            }
            StorageBackend::Redis => {
                if self.redis.urls.is_empty() {
                    return Err(ConfigError::Message(
                        "storage.redis.urls cannot be empty".to_string(),
                    ));
                }
                Ok(())
            }
            StorageBackend::MySQL => {
                if self.mysql.url.is_empty() {
                    return Err(ConfigError::Message(
                        "storage.mysql.url cannot be empty".to_string(),
                    ));
                }
                Ok(())
            }
            StorageBackend::PostgreSQL => {
                if self.postgresql.url.is_empty() {
                    return Err(ConfigError::Message(
                        "storage.postgresql.url cannot be empty".to_string(),
                    ));
                }
                Ok(())
            }
        }
    }
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            backend: StorageBackend::File,
            file: FileStorageConfig::default(),
            redis: RedisStorageConfig::default(),
            mysql: MySqlStorageConfig::default(),
            postgresql: PostgresStorageConfig::default(),
        }
    }
}

/// File storage configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct FileStorageConfig {
    /// Directory for storing data files.
    #[serde(default = "default_data_dir")]
    pub data_dir: PathBuf,
}

fn default_data_dir() -> PathBuf {
    PathBuf::from("./data")
}

impl Default for FileStorageConfig {
    fn default() -> Self {
        Self {
            data_dir: default_data_dir(),
        }
    }
}

/// Redis storage configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct RedisStorageConfig {
    /// Redis URL(s) - single node or cluster.
    #[serde(default = "default_redis_urls")]
    pub urls: Vec<String>,

    /// Connection pool size.
    #[serde(default = "default_pool_size")]
    pub pool_size: u32,

    /// Connection timeout in seconds.
    #[serde(default = "default_connect_timeout")]
    pub connect_timeout: u64,
}

fn default_redis_urls() -> Vec<String> {
    vec!["redis://127.0.0.1:6379".to_string()]
}

const fn default_pool_size() -> u32 {
    10
}

const fn default_connect_timeout() -> u64 {
    5
}

impl Default for RedisStorageConfig {
    fn default() -> Self {
        Self {
            urls: default_redis_urls(),
            pool_size: 10,
            connect_timeout: 5,
        }
    }
}

/// `MySQL` storage configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct MySqlStorageConfig {
    /// `MySQL` connection URL.
    #[serde(default)]
    pub url: String,

    /// Connection pool minimum size.
    #[serde(default = "default_min_connections")]
    pub min_connections: u32,

    /// Connection pool maximum size.
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,

    /// Connection timeout in seconds.
    #[serde(default = "default_connect_timeout")]
    pub connect_timeout: u64,
}

const fn default_min_connections() -> u32 {
    5
}

const fn default_max_connections() -> u32 {
    20
}

impl Default for MySqlStorageConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            min_connections: 5,
            max_connections: 20,
            connect_timeout: 5,
        }
    }
}

/// `PostgreSQL` storage configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct PostgresStorageConfig {
    /// `PostgreSQL` connection URL.
    #[serde(default)]
    pub url: String,

    /// Connection pool minimum size.
    #[serde(default = "default_min_connections")]
    pub min_connections: u32,

    /// Connection pool maximum size.
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,

    /// Connection timeout in seconds.
    #[serde(default = "default_connect_timeout")]
    pub connect_timeout: u64,
}

impl Default for PostgresStorageConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            min_connections: 5,
            max_connections: 20,
            connect_timeout: 5,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_backend_display() {
        assert_eq!(StorageBackend::File.to_string(), "file");
        assert_eq!(StorageBackend::Redis.to_string(), "redis");
        assert_eq!(StorageBackend::MySQL.to_string(), "mysql");
        assert_eq!(StorageBackend::PostgreSQL.to_string(), "postgresql");
    }

    #[test]
    fn test_storage_config_validation() {
        let config = StorageConfig::default();
        assert!(config.validate().is_ok());

        let mut config = StorageConfig::default();
        config.backend = StorageBackend::Redis;
        config.redis.urls = vec![];
        assert!(config.validate().is_err());
    }
}
