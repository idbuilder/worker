//! Increment ID service.
//!
//! Handles auto-increment ID generation with batch pre-allocation.

use std::sync::Arc;

use crate::config::SequenceConfig;
use crate::domain::{IdConfig, IdType, IncrementConfig};
use crate::error::{AppError, Result};
use crate::service::cache::SequenceCache;
use crate::storage::traits::Storage;

/// Service for auto-increment ID generation.
pub struct IncrementService {
    /// Storage backend.
    storage: Arc<dyn Storage>,
    /// Sequence cache for performance.
    cache: SequenceCache,
    /// Default batch size for prefetch.
    batch_size: u32,
}

impl IncrementService {
    /// Create a new increment service.
    pub fn new(storage: Arc<dyn Storage>, config: &SequenceConfig) -> Self {
        Self {
            storage,
            cache: SequenceCache::new(config.prefetch_threshold),
            batch_size: config.default_batch_size,
        }
    }

    /// Generate auto-increment IDs.
    ///
    /// # Arguments
    ///
    /// * `name` - Configuration name
    /// * `count` - Number of IDs to generate
    ///
    /// # Returns
    ///
    /// A vector of generated IDs.
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration is not found, storage fails, or the sequence is exhausted.
    pub async fn generate(&self, name: &str, count: u32) -> Result<Vec<i64>> {
        // Get configuration
        let config = self
            .storage
            .get_increment_config(name)
            .await
            .map_err(AppError::Storage)?
            .ok_or_else(|| AppError::ConfigNotFound(name.to_string()))?;

        // Try to get from cache first
        match self.cache.get(name, count) {
            Ok(values) => {
                // Check if we need to prefetch
                if self.cache.needs_prefetch(name) {
                    self.prefetch(name, &config).await?;
                }
                Ok(values)
            }
            Err(missing) => {
                // Need to fetch from storage
                let batch_count = std::cmp::max(missing, self.batch_size);
                let range = self
                    .storage
                    .get_and_increment(name, batch_count, config.step)
                    .await
                    .map_err(AppError::Storage)?;

                // Validate range is within bounds
                if config.step > 0 && range.end > config.max {
                    return Err(AppError::SequenceExhausted(name.to_string()));
                }
                if config.step < 0 && range.end < config.min {
                    return Err(AppError::SequenceExhausted(name.to_string()));
                }

                // Put into cache
                self.cache.put(name, range);

                // Now get from cache
                self.cache.get(name, count).map_err(|_| {
                    AppError::Internal("Cache inconsistency after prefetch".to_string())
                })
            }
        }
    }

    /// Create a new increment configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration is invalid, already exists, or storage fails.
    pub async fn create_config(&self, config: IncrementConfig) -> Result<()> {
        // Validate configuration
        config.validate().map_err(AppError::InvalidConfig)?;

        // Check if already exists
        if self
            .storage
            .get_increment_config(&config.name)
            .await
            .map_err(AppError::Storage)?
            .is_some()
        {
            return Err(AppError::ConfigExists(config.name.clone()));
        }

        // Initialize sequence
        self.storage
            .initialize(&config.name, IdType::Increment, config.start)
            .await
            .map_err(AppError::Storage)?;

        // Save configuration
        self.storage
            .save_increment_config(&config)
            .await
            .map_err(AppError::Storage)?;

        Ok(())
    }

    /// Get an increment configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration is not found or storage fails.
    pub async fn get_config(&self, name: &str) -> Result<IncrementConfig> {
        self.storage
            .get_increment_config(name)
            .await
            .map_err(AppError::Storage)?
            .ok_or_else(|| AppError::ConfigNotFound(name.to_string()))
    }

    /// List all increment configurations.
    ///
    /// # Errors
    ///
    /// Returns an error if storage fails.
    pub async fn list_configs(&self) -> Result<Vec<IncrementConfig>> {
        self.storage
            .list_increment_configs()
            .await
            .map_err(AppError::Storage)
    }

    /// Delete an increment configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if storage fails.
    pub async fn delete_config(&self, name: &str) -> Result<bool> {
        // Clear cache for this sequence
        self.cache.remove(name);

        self.storage
            .delete_increment_config(name)
            .await
            .map_err(AppError::Storage)
    }

    /// Prefetch a batch of IDs into cache.
    async fn prefetch(&self, name: &str, config: &IncrementConfig) -> Result<()> {
        let range = self
            .storage
            .get_and_increment(name, self.batch_size, config.step)
            .await
            .map_err(AppError::Storage)?;

        self.cache.put(name, range);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::FileStorageConfig;
    use crate::storage::file::FileStorage;
    use tempfile::TempDir;

    async fn create_test_service() -> (IncrementService, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let storage_config = FileStorageConfig {
            data_dir: temp_dir.path().to_path_buf(),
        };
        let storage = Arc::new(FileStorage::new(&storage_config).unwrap());
        let seq_config = SequenceConfig {
            default_batch_size: 100,
            prefetch_threshold: 10,
        };
        let service = IncrementService::new(storage, &seq_config);
        (service, temp_dir)
    }

    #[tokio::test]
    async fn test_create_and_generate() {
        let (service, _temp) = create_test_service().await;

        // Create config
        let config = IncrementConfig {
            name: "orders".to_string(),
            start: 1000,
            step: 1,
            min: 1,
            max: i64::MAX,
            key_token_enable: false,
        };
        service.create_config(config).await.unwrap();

        // Generate IDs
        let ids = service.generate("orders", 5).await.unwrap();
        assert_eq!(ids.len(), 5);
        assert_eq!(ids[0], 1000);
        assert_eq!(ids[4], 1004);

        // Generate more
        let ids = service.generate("orders", 3).await.unwrap();
        assert_eq!(ids[0], 1005);
    }

    #[tokio::test]
    async fn test_duplicate_config_error() {
        let (service, _temp) = create_test_service().await;

        let config = IncrementConfig {
            name: "test".to_string(),
            ..Default::default()
        };

        service.create_config(config.clone()).await.unwrap();

        let result = service.create_config(config).await;
        assert!(matches!(result, Err(AppError::ConfigExists(_))));
    }

    #[tokio::test]
    async fn test_config_not_found() {
        let (service, _temp) = create_test_service().await;

        let result = service.generate("nonexistent", 1).await;
        assert!(matches!(result, Err(AppError::ConfigNotFound(_))));
    }
}
