//! File-based configuration storage.

use std::path::PathBuf;

use async_trait::async_trait;
use fs2::FileExt;
use tokio::sync::Mutex;

use crate::domain::{FormattedConfig, IncrementConfig, SnowflakeConfig};
use crate::error::{StorageError, StorageResult};
use crate::storage::traits::ConfigStorage;

/// File-based configuration storage implementation.
pub struct FileConfigStorage {
    /// Base directory for configs.
    configs_dir: PathBuf,
    /// Mutex for coordinating file operations.
    lock: Mutex<()>,
}

impl FileConfigStorage {
    /// Create a new file config storage.
    #[must_use]
    pub fn new(configs_dir: PathBuf) -> Self {
        Self {
            configs_dir,
            lock: Mutex::new(()),
        }
    }

    /// Get the file path for a config.
    fn config_path(&self, id_type: &str, name: &str) -> PathBuf {
        self.configs_dir
            .join(id_type)
            .join(format!("{}.json", sanitize_name(name)))
    }

    /// Save a config to file.
    fn save_config<T: serde::Serialize>(
        &self,
        id_type: &str,
        name: &str,
        config: &T,
    ) -> StorageResult<()> {
        let path = self.config_path(id_type, name);

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)?;

        file.lock_exclusive()
            .map_err(|e| StorageError::LockFailed(e.to_string()))?;

        serde_json::to_writer_pretty(&file, config)?;
        file.sync_all()?;
        file.unlock()
            .map_err(|e| StorageError::LockFailed(e.to_string()))?;

        Ok(())
    }

    /// Load a config from file.
    fn load_config<T: serde::de::DeserializeOwned>(
        &self,
        id_type: &str,
        name: &str,
    ) -> StorageResult<Option<T>> {
        let path = self.config_path(id_type, name);

        if !path.exists() {
            return Ok(None);
        }

        let file = std::fs::File::open(&path)?;
        file.lock_shared()
            .map_err(|e| StorageError::LockFailed(e.to_string()))?;

        let config: T = serde_json::from_reader(&file)?;
        file.unlock()
            .map_err(|e| StorageError::LockFailed(e.to_string()))?;

        Ok(Some(config))
    }

    /// List all configs of a type.
    fn list_configs<T: serde::de::DeserializeOwned>(&self, id_type: &str) -> StorageResult<Vec<T>> {
        let dir = self.configs_dir.join(id_type);

        if !dir.exists() {
            return Ok(Vec::new());
        }

        let mut configs = Vec::new();

        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                let file = std::fs::File::open(&path)?;
                file.lock_shared()
                    .map_err(|e| StorageError::LockFailed(e.to_string()))?;

                match serde_json::from_reader(&file) {
                    Ok(config) => configs.push(config),
                    Err(e) => {
                        tracing::warn!(path = ?path, error = %e, "Failed to parse config file");
                    }
                }

                file.unlock()
                    .map_err(|e| StorageError::LockFailed(e.to_string()))?;
            }
        }

        Ok(configs)
    }

    /// Delete a config file.
    fn delete_config(&self, id_type: &str, name: &str) -> StorageResult<bool> {
        let path = self.config_path(id_type, name);

        if !path.exists() {
            return Ok(false);
        }

        std::fs::remove_file(&path)?;
        Ok(true)
    }
}

#[async_trait]
impl ConfigStorage for FileConfigStorage {
    async fn save_increment_config(&self, config: &IncrementConfig) -> StorageResult<()> {
        let _guard = self.lock.lock().await;
        self.save_config("increment", &config.name, config)
    }

    async fn get_increment_config(&self, name: &str) -> StorageResult<Option<IncrementConfig>> {
        let _guard = self.lock.lock().await;
        self.load_config("increment", name)
    }

    async fn list_increment_configs(&self) -> StorageResult<Vec<IncrementConfig>> {
        let _guard = self.lock.lock().await;
        self.list_configs("increment")
    }

    async fn delete_increment_config(&self, name: &str) -> StorageResult<bool> {
        let _guard = self.lock.lock().await;
        self.delete_config("increment", name)
    }

    async fn save_snowflake_config(&self, config: &SnowflakeConfig) -> StorageResult<()> {
        let _guard = self.lock.lock().await;
        self.save_config("snowflake", &config.name, config)
    }

    async fn get_snowflake_config(&self, name: &str) -> StorageResult<Option<SnowflakeConfig>> {
        let _guard = self.lock.lock().await;
        self.load_config("snowflake", name)
    }

    async fn list_snowflake_configs(&self) -> StorageResult<Vec<SnowflakeConfig>> {
        let _guard = self.lock.lock().await;
        self.list_configs("snowflake")
    }

    async fn delete_snowflake_config(&self, name: &str) -> StorageResult<bool> {
        let _guard = self.lock.lock().await;
        self.delete_config("snowflake", name)
    }

    async fn save_formatted_config(&self, config: &FormattedConfig) -> StorageResult<()> {
        let _guard = self.lock.lock().await;
        self.save_config("formatted", &config.name, config)
    }

    async fn get_formatted_config(&self, name: &str) -> StorageResult<Option<FormattedConfig>> {
        let _guard = self.lock.lock().await;
        self.load_config("formatted", name)
    }

    async fn list_formatted_configs(&self) -> StorageResult<Vec<FormattedConfig>> {
        let _guard = self.lock.lock().await;
        self.list_configs("formatted")
    }

    async fn delete_formatted_config(&self, name: &str) -> StorageResult<bool> {
        let _guard = self.lock.lock().await;
        self.delete_config("formatted", name)
    }
}

/// Sanitize a name for use as a filename.
fn sanitize_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::SequenceReset;
    use tempfile::TempDir;

    fn create_test_storage() -> (FileConfigStorage, TempDir) {
        let temp_dir = TempDir::new().unwrap();

        // Create subdirectories
        std::fs::create_dir_all(temp_dir.path().join("increment")).unwrap();
        std::fs::create_dir_all(temp_dir.path().join("snowflake")).unwrap();
        std::fs::create_dir_all(temp_dir.path().join("formatted")).unwrap();

        let storage = FileConfigStorage::new(temp_dir.path().to_path_buf());
        (storage, temp_dir)
    }

    #[tokio::test]
    async fn test_increment_config_crud() {
        let (storage, _temp) = create_test_storage();

        let config = IncrementConfig {
            name: "orders".to_string(),
            start: 1000,
            step: 1,
            min: 1,
            max: i64::MAX,
        };

        // Create
        storage.save_increment_config(&config).await.unwrap();

        // Read
        let loaded = storage
            .get_increment_config("orders")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(loaded.name, "orders");
        assert_eq!(loaded.start, 1000);

        // List
        let all = storage.list_increment_configs().await.unwrap();
        assert_eq!(all.len(), 1);

        // Delete
        assert!(storage.delete_increment_config("orders").await.unwrap());
        assert!(
            storage
                .get_increment_config("orders")
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn test_snowflake_config_crud() {
        let (storage, _temp) = create_test_storage();

        let config = SnowflakeConfig {
            name: "events".to_string(),
            epoch: 1704067200000,
            worker_bits: 10,
            sequence_bits: 12,
        };

        storage.save_snowflake_config(&config).await.unwrap();

        let loaded = storage
            .get_snowflake_config("events")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(loaded.name, "events");
        assert_eq!(loaded.epoch, 1704067200000);
    }

    #[tokio::test]
    async fn test_formatted_config_crud() {
        let (storage, _temp) = create_test_storage();

        let config = FormattedConfig {
            name: "invoices".to_string(),
            pattern: "INV{YYYY}{MM}{DD}-{SEQ:4}".to_string(),
            sequence_reset: SequenceReset::Daily,
        };

        storage.save_formatted_config(&config).await.unwrap();

        let loaded = storage
            .get_formatted_config("invoices")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(loaded.name, "invoices");
        assert_eq!(loaded.pattern, "INV{YYYY}{MM}{DD}-{SEQ:4}");
        assert_eq!(loaded.sequence_reset, SequenceReset::Daily);
    }
}
