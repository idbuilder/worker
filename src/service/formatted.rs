//! Formatted ID service.
//!
//! Handles custom formatted string ID generation with pattern support.

use std::sync::Arc;

use crate::config::SequenceConfig;
use crate::domain::{FormattedConfig, IdConfig, IdType};
use crate::error::{AppError, Result};
use crate::service::cache::SequenceCache;
use crate::service::pattern::ParsedPattern;
use crate::storage::traits::Storage;

/// Service for formatted string ID generation.
pub struct FormattedService {
    /// Storage backend.
    storage: Arc<dyn Storage>,
    /// Sequence cache for performance.
    cache: SequenceCache,
    /// Default batch size for prefetch.
    batch_size: u32,
}

impl FormattedService {
    /// Create a new formatted service.
    pub fn new(storage: Arc<dyn Storage>, config: &SequenceConfig) -> Self {
        Self {
            storage,
            cache: SequenceCache::new(config.prefetch_threshold),
            batch_size: config.default_batch_size,
        }
    }

    /// Generate formatted IDs.
    ///
    /// # Arguments
    ///
    /// * `name` - Configuration name
    /// * `count` - Number of IDs to generate
    ///
    /// # Returns
    ///
    /// A vector of generated ID strings.
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration is not found, pattern is invalid, or storage fails.
    pub async fn generate(&self, name: &str, count: u32) -> Result<Vec<String>> {
        // Get configuration
        let config = self
            .storage
            .get_formatted_config(name)
            .await
            .map_err(AppError::Storage)?
            .ok_or_else(|| AppError::ConfigNotFound(name.to_string()))?;

        // Parse pattern
        let pattern = ParsedPattern::parse(&config.pattern).map_err(AppError::InvalidConfig)?;

        // If pattern has sequence, we need to get sequence numbers
        if pattern.has_sequence() {
            let sequence_key = pattern.sequence_key(name, config.sequence_reset);
            let sequences = self.get_sequences(&sequence_key, count).await?;

            let mut ids = Vec::with_capacity(count as usize);
            for seq in sequences {
                let id = pattern.generate(Some(seq)).map_err(AppError::Internal)?;
                ids.push(id);
            }

            Ok(ids)
        } else {
            // Pattern without sequence (UUID or random only)
            let mut ids = Vec::with_capacity(count as usize);
            for _ in 0..count {
                let id = pattern.generate(None).map_err(AppError::Internal)?;
                ids.push(id);
            }
            Ok(ids)
        }
    }

    /// Get sequence numbers for generation.
    async fn get_sequences(&self, sequence_key: &str, count: u32) -> Result<Vec<i64>> {
        // Try to get from cache first
        match self.cache.get(sequence_key, count) {
            Ok(values) => {
                // Check if we need to prefetch
                if self.cache.needs_prefetch(sequence_key) {
                    self.prefetch(sequence_key).await?;
                }
                Ok(values)
            }
            Err(missing) => {
                // Need to fetch from storage
                // First, ensure sequence is initialized
                let exists = self
                    .storage
                    .exists(sequence_key)
                    .await
                    .map_err(AppError::Storage)?;

                if !exists {
                    self.storage
                        .initialize(sequence_key, IdType::Formatted, 1)
                        .await
                        .map_err(AppError::Storage)?;
                }

                let batch_count = std::cmp::max(missing, self.batch_size);
                let range = self
                    .storage
                    .get_and_increment(sequence_key, batch_count, 1)
                    .await
                    .map_err(AppError::Storage)?;

                // Put into cache
                self.cache.put(sequence_key, range);

                // Now get from cache
                self.cache.get(sequence_key, count).map_err(|_| {
                    AppError::Internal("Cache inconsistency after prefetch".to_string())
                })
            }
        }
    }

    /// Prefetch sequence numbers.
    async fn prefetch(&self, sequence_key: &str) -> Result<()> {
        let range = self
            .storage
            .get_and_increment(sequence_key, self.batch_size, 1)
            .await
            .map_err(AppError::Storage)?;

        self.cache.put(sequence_key, range);
        Ok(())
    }

    /// Create a new formatted configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration is invalid, already exists, or storage fails.
    pub async fn create_config(&self, config: FormattedConfig) -> Result<()> {
        // Validate configuration
        config.validate().map_err(AppError::InvalidConfig)?;

        // Check if already exists
        if self
            .storage
            .get_formatted_config(&config.name)
            .await
            .map_err(AppError::Storage)?
            .is_some()
        {
            return Err(AppError::ConfigExists(config.name.clone()));
        }

        // Save configuration
        self.storage
            .save_formatted_config(&config)
            .await
            .map_err(AppError::Storage)?;

        Ok(())
    }

    /// Get a formatted configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration is not found or storage fails.
    pub async fn get_config(&self, name: &str) -> Result<FormattedConfig> {
        self.storage
            .get_formatted_config(name)
            .await
            .map_err(AppError::Storage)?
            .ok_or_else(|| AppError::ConfigNotFound(name.to_string()))
    }

    /// List all formatted configurations.
    ///
    /// # Errors
    ///
    /// Returns an error if storage fails.
    pub async fn list_configs(&self) -> Result<Vec<FormattedConfig>> {
        self.storage
            .list_formatted_configs()
            .await
            .map_err(AppError::Storage)
    }

    /// Delete a formatted configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if storage fails.
    pub async fn delete_config(&self, name: &str) -> Result<bool> {
        self.storage
            .delete_formatted_config(name)
            .await
            .map_err(AppError::Storage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::FileStorageConfig;
    use crate::domain::SequenceReset;
    use crate::storage::file::FileStorage;
    use tempfile::TempDir;

    async fn create_test_service() -> (FormattedService, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let storage_config = FileStorageConfig {
            data_dir: temp_dir.path().to_path_buf(),
        };
        let storage = Arc::new(FileStorage::new(&storage_config).unwrap());
        let seq_config = SequenceConfig {
            default_batch_size: 100,
            prefetch_threshold: 10,
        };
        let service = FormattedService::new(storage, &seq_config);
        (service, temp_dir)
    }

    #[tokio::test]
    async fn test_create_and_generate_with_sequence() {
        let (service, _temp) = create_test_service().await;

        let config = FormattedConfig {
            name: "invoices".to_string(),
            pattern: "INV-{SEQ:6}".to_string(),
            sequence_reset: SequenceReset::Never,
        };

        service.create_config(config).await.unwrap();

        let ids = service.generate("invoices", 3).await.unwrap();
        assert_eq!(ids.len(), 3);
        assert_eq!(ids[0], "INV-000001");
        assert_eq!(ids[1], "INV-000002");
        assert_eq!(ids[2], "INV-000003");
    }

    #[tokio::test]
    async fn test_generate_with_uuid() {
        let (service, _temp) = create_test_service().await;

        let config = FormattedConfig {
            name: "orders".to_string(),
            pattern: "ORD-{UUID}".to_string(),
            sequence_reset: SequenceReset::Never,
        };

        service.create_config(config).await.unwrap();

        let ids = service.generate("orders", 2).await.unwrap();
        assert_eq!(ids.len(), 2);
        assert!(ids[0].starts_with("ORD-"));
        assert!(ids[1].starts_with("ORD-"));
        assert_ne!(ids[0], ids[1]); // UUIDs should be unique
    }

    #[tokio::test]
    async fn test_generate_with_date() {
        let (service, _temp) = create_test_service().await;

        let config = FormattedConfig {
            name: "test".to_string(),
            pattern: "{YYYY}-{SEQ:4}".to_string(),
            sequence_reset: SequenceReset::Never,
        };

        service.create_config(config).await.unwrap();

        let ids = service.generate("test", 1).await.unwrap();
        let now = chrono::Utc::now();
        let expected_prefix = format!("{}-", now.format("%Y"));
        assert!(ids[0].starts_with(&expected_prefix));
    }
}
