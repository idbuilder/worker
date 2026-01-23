//! File-based storage backend.
//!
//! This backend stores data as JSON files with file locking for atomic operations.
//! Suitable for development and single-node deployments.
//!
//! Directory structure:
//! ```text
//! data/
//! ├── sequences/
//! │   └── {name}.json
//! ├── configs/
//! │   ├── increment/
//! │   │   └── {name}.json
//! │   ├── snowflake/
//! │   │   └── {name}.json
//! │   └── formatted/
//! │       └── {name}.json
//! └── locks/
//!     └── {key}.lock
//! ```

mod config;
mod lock;
mod sequence;

use std::path::PathBuf;

use async_trait::async_trait;

use crate::config::FileStorageConfig;
use crate::domain::{
    FormattedConfig, IdType, IncrementConfig, SequenceRange, SequenceState, SnowflakeConfig,
};
use crate::error::{StorageError, StorageResult};
use crate::storage::traits::{ConfigStorage, DistributedLock, LockGuard, SequenceStorage, Storage};

pub use config::FileConfigStorage;
pub use lock::FileLock;
pub use sequence::FileSequenceStorage;

/// File-based storage implementation.
pub struct FileStorage {
    /// Base data directory.
    base_dir: PathBuf,
    /// Sequence storage.
    sequence_storage: FileSequenceStorage,
    /// Config storage.
    config_storage: FileConfigStorage,
    /// Lock manager.
    lock_manager: FileLock,
}

impl FileStorage {
    /// Create a new file storage instance.
    ///
    /// # Arguments
    ///
    /// * `config` - File storage configuration
    ///
    /// # Errors
    ///
    /// Returns an error if the data directories cannot be created.
    pub fn new(config: &FileStorageConfig) -> StorageResult<Self> {
        let base_dir = config.data_dir.clone();

        // Create directory structure
        Self::ensure_directories(&base_dir)?;

        Ok(Self {
            sequence_storage: FileSequenceStorage::new(base_dir.join("sequences")),
            config_storage: FileConfigStorage::new(base_dir.join("configs")),
            lock_manager: FileLock::new(base_dir.join("locks")),
            base_dir,
        })
    }

    /// Ensure all required directories exist.
    fn ensure_directories(base_dir: &PathBuf) -> StorageResult<()> {
        let dirs = [
            base_dir.clone(),
            base_dir.join("sequences"),
            base_dir.join("configs"),
            base_dir.join("configs/increment"),
            base_dir.join("configs/snowflake"),
            base_dir.join("configs/formatted"),
            base_dir.join("locks"),
        ];

        for dir in &dirs {
            std::fs::create_dir_all(dir).map_err(|e| {
                StorageError::FileIO(format!("Failed to create directory {:?}: {}", dir, e))
            })?;
        }

        Ok(())
    }
}

#[async_trait]
impl SequenceStorage for FileStorage {
    async fn get_and_increment(
        &self,
        name: &str,
        count: u32,
        step: i64,
    ) -> StorageResult<SequenceRange> {
        self.sequence_storage
            .get_and_increment(name, count, step)
            .await
    }

    async fn get_current(&self, name: &str) -> StorageResult<i64> {
        self.sequence_storage.get_current(name).await
    }

    async fn initialize(&self, name: &str, id_type: IdType, start_value: i64) -> StorageResult<()> {
        self.sequence_storage
            .initialize(name, id_type, start_value)
            .await
    }

    async fn exists(&self, name: &str) -> StorageResult<bool> {
        self.sequence_storage.exists(name).await
    }

    async fn get_state(&self, name: &str) -> StorageResult<Option<SequenceState>> {
        self.sequence_storage.get_state(name).await
    }
}

#[async_trait]
impl ConfigStorage for FileStorage {
    async fn save_increment_config(&self, config: &IncrementConfig) -> StorageResult<()> {
        self.config_storage.save_increment_config(config).await
    }

    async fn get_increment_config(&self, name: &str) -> StorageResult<Option<IncrementConfig>> {
        self.config_storage.get_increment_config(name).await
    }

    async fn list_increment_configs(&self) -> StorageResult<Vec<IncrementConfig>> {
        self.config_storage.list_increment_configs().await
    }

    async fn delete_increment_config(&self, name: &str) -> StorageResult<bool> {
        self.config_storage.delete_increment_config(name).await
    }

    async fn save_snowflake_config(&self, config: &SnowflakeConfig) -> StorageResult<()> {
        self.config_storage.save_snowflake_config(config).await
    }

    async fn get_snowflake_config(&self, name: &str) -> StorageResult<Option<SnowflakeConfig>> {
        self.config_storage.get_snowflake_config(name).await
    }

    async fn list_snowflake_configs(&self) -> StorageResult<Vec<SnowflakeConfig>> {
        self.config_storage.list_snowflake_configs().await
    }

    async fn delete_snowflake_config(&self, name: &str) -> StorageResult<bool> {
        self.config_storage.delete_snowflake_config(name).await
    }

    async fn save_formatted_config(&self, config: &FormattedConfig) -> StorageResult<()> {
        self.config_storage.save_formatted_config(config).await
    }

    async fn get_formatted_config(&self, name: &str) -> StorageResult<Option<FormattedConfig>> {
        self.config_storage.get_formatted_config(name).await
    }

    async fn list_formatted_configs(&self) -> StorageResult<Vec<FormattedConfig>> {
        self.config_storage.list_formatted_configs().await
    }

    async fn delete_formatted_config(&self, name: &str) -> StorageResult<bool> {
        self.config_storage.delete_formatted_config(name).await
    }
}

#[async_trait]
impl DistributedLock for FileStorage {
    async fn acquire(&self, key: &str, ttl: std::time::Duration) -> StorageResult<LockGuard> {
        self.lock_manager.acquire(key, ttl).await
    }

    async fn try_acquire(
        &self,
        key: &str,
        ttl: std::time::Duration,
    ) -> StorageResult<Option<LockGuard>> {
        self.lock_manager.try_acquire(key, ttl).await
    }

    async fn is_locked(&self, key: &str) -> StorageResult<bool> {
        self.lock_manager.is_locked(key).await
    }
}

#[async_trait]
impl Storage for FileStorage {
    async fn health_check(&self) -> StorageResult<()> {
        // Check if base directory is accessible
        if !self.base_dir.exists() {
            return Err(StorageError::Unavailable);
        }

        // Try to create a test file
        let test_file = self.base_dir.join(".health_check");
        tokio::fs::write(&test_file, b"ok")
            .await
            .map_err(|e| StorageError::FileIO(format!("Health check failed: {}", e)))?;
        tokio::fs::remove_file(&test_file)
            .await
            .map_err(|e| StorageError::FileIO(format!("Health check cleanup failed: {}", e)))?;

        Ok(())
    }

    fn backend_name(&self) -> &'static str {
        "file"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_storage() -> (FileStorage, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let config = FileStorageConfig {
            data_dir: temp_dir.path().to_path_buf(),
        };
        let storage = FileStorage::new(&config).unwrap();
        (storage, temp_dir)
    }

    #[tokio::test]
    async fn test_health_check() {
        let (storage, _temp) = create_test_storage();
        assert!(storage.health_check().await.is_ok());
    }

    #[tokio::test]
    async fn test_sequence_operations() {
        let (storage, _temp) = create_test_storage();

        // Initialize sequence
        storage
            .initialize("test_seq", IdType::Increment, 1)
            .await
            .unwrap();

        // Check exists
        assert!(storage.exists("test_seq").await.unwrap());
        assert!(!storage.exists("nonexistent").await.unwrap());

        // Get current
        assert_eq!(storage.get_current("test_seq").await.unwrap(), 1);

        // Get and increment
        let range = storage.get_and_increment("test_seq", 5, 1).await.unwrap();
        assert_eq!(range.start, 1);
        assert_eq!(range.end, 5);

        // Verify current updated
        assert_eq!(storage.get_current("test_seq").await.unwrap(), 6);
    }

    #[tokio::test]
    async fn test_config_operations() {
        let (storage, _temp) = create_test_storage();

        let config = IncrementConfig {
            name: "test_config".to_string(),
            start: 1,
            step: 1,
            min: 1,
            max: 1000,
        };

        // Save
        storage.save_increment_config(&config).await.unwrap();

        // Get
        let loaded = storage
            .get_increment_config("test_config")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(loaded.name, "test_config");
        assert_eq!(loaded.start, 1);

        // List
        let configs = storage.list_increment_configs().await.unwrap();
        assert_eq!(configs.len(), 1);

        // Delete
        assert!(
            storage
                .delete_increment_config("test_config")
                .await
                .unwrap()
        );
        assert!(
            storage
                .get_increment_config("test_config")
                .await
                .unwrap()
                .is_none()
        );
    }
}
