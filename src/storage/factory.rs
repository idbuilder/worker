//! Storage backend factory.
//!
//! Creates the appropriate storage backend based on configuration.

use std::sync::Arc;

use crate::config::{StorageBackend, StorageConfig};
use crate::error::AppError;
use crate::storage::file::FileStorage;
use crate::storage::traits::Storage;

/// Create a storage backend based on configuration.
///
/// # Arguments
///
/// * `config` - Storage configuration
///
/// # Returns
///
/// An `Arc<dyn Storage>` pointing to the configured storage backend.
///
/// # Errors
///
/// Returns an error if the storage backend cannot be initialized.
pub async fn create_storage(config: &StorageConfig) -> Result<Arc<dyn Storage>, AppError> {
    match config.backend {
        StorageBackend::File => {
            let storage = FileStorage::new(&config.file).map_err(AppError::Storage)?;

            // Verify storage is healthy
            storage.health_check().await.map_err(AppError::Storage)?;

            Ok(Arc::new(storage))
        }
        StorageBackend::Redis => {
            // Redis storage not yet implemented
            Err(AppError::Internal(
                "Redis storage backend not yet implemented".to_string(),
            ))
        }
        StorageBackend::MySQL => {
            // MySQL storage not yet implemented
            Err(AppError::Internal(
                "MySQL storage backend not yet implemented".to_string(),
            ))
        }
        StorageBackend::PostgreSQL => {
            // PostgreSQL storage not yet implemented
            Err(AppError::Internal(
                "PostgreSQL storage backend not yet implemented".to_string(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_create_file_storage() {
        let temp_dir = TempDir::new().unwrap();

        let config = StorageConfig {
            backend: StorageBackend::File,
            file: crate::config::FileStorageConfig {
                data_dir: temp_dir.path().to_path_buf(),
            },
            ..Default::default()
        };

        let storage = create_storage(&config).await.unwrap();
        assert_eq!(storage.backend_name(), "file");
    }

    #[tokio::test]
    async fn test_create_redis_storage_not_implemented() {
        let config = StorageConfig {
            backend: StorageBackend::Redis,
            ..Default::default()
        };

        let result = create_storage(&config).await;
        assert!(result.is_err());
    }
}
