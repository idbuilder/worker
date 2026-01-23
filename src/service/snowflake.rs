//! Snowflake ID service.
//!
//! Manages Snowflake configurations and worker ID allocation.
//! Note: Actual ID generation happens client-side; this service provides
//! configuration and worker ID assignment.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use parking_lot::RwLock;

use crate::domain::{IdConfig, SnowflakeConfig, SnowflakeIdResponse};
use crate::error::{AppError, Result};
use crate::storage::traits::Storage;

/// Worker ID allocation info.
struct WorkerIdAllocation {
    /// Allocated worker ID.
    worker_id: u32,
    /// When the allocation was made.
    allocated_at: Instant,
    /// Lease duration.
    lease_duration: Duration,
}

impl WorkerIdAllocation {
    /// Check if the allocation is still valid.
    fn is_valid(&self) -> bool {
        self.allocated_at.elapsed() < self.lease_duration
    }
}

/// Worker ID allocator for Snowflake IDs.
pub struct WorkerIdAllocator {
    /// Next worker ID to allocate per config.
    next_id: RwLock<HashMap<String, AtomicU32>>,
    /// Active allocations per config.
    allocations: RwLock<HashMap<String, Vec<WorkerIdAllocation>>>,
    /// Default lease duration.
    lease_duration: Duration,
}

impl WorkerIdAllocator {
    /// Create a new worker ID allocator.
    pub fn new(lease_duration: Duration) -> Self {
        Self {
            next_id: RwLock::new(HashMap::new()),
            allocations: RwLock::new(HashMap::new()),
            lease_duration,
        }
    }

    /// Allocate a worker ID for a configuration.
    pub fn allocate(&self, config_name: &str, max_worker_id: u32) -> Option<u32> {
        // Clean up expired allocations first
        self.cleanup_expired(config_name);

        let mut next_ids = self.next_id.write();
        let counter = next_ids
            .entry(config_name.to_string())
            .or_insert_with(|| AtomicU32::new(0));

        // Try to find an available worker ID
        let start_id = counter.load(Ordering::SeqCst);
        let mut current_id = start_id;

        loop {
            if current_id > max_worker_id {
                current_id = 0;
            }

            if !self.is_allocated(config_name, current_id) {
                // Found an available ID
                counter.store((current_id + 1) % (max_worker_id + 1), Ordering::SeqCst);

                // Record allocation
                let mut allocations = self.allocations.write();
                let config_allocations = allocations
                    .entry(config_name.to_string())
                    .or_insert_with(Vec::new);

                config_allocations.push(WorkerIdAllocation {
                    worker_id: current_id,
                    allocated_at: Instant::now(),
                    lease_duration: self.lease_duration,
                });

                return Some(current_id);
            }

            current_id += 1;
            if current_id > max_worker_id {
                current_id = 0;
            }

            // Wrapped around completely
            if current_id == start_id {
                return None;
            }
        }
    }

    /// Check if a worker ID is currently allocated.
    fn is_allocated(&self, config_name: &str, worker_id: u32) -> bool {
        let allocations = self.allocations.read();

        if let Some(config_allocations) = allocations.get(config_name) {
            config_allocations
                .iter()
                .any(|a| a.worker_id == worker_id && a.is_valid())
        } else {
            false
        }
    }

    /// Clean up expired allocations for a config.
    fn cleanup_expired(&self, config_name: &str) {
        let mut allocations = self.allocations.write();

        if let Some(config_allocations) = allocations.get_mut(config_name) {
            config_allocations.retain(|a| a.is_valid());
        }
    }
}

impl Default for WorkerIdAllocator {
    fn default() -> Self {
        Self::new(Duration::from_secs(60)) // 1 minute default lease
    }
}

/// Service for Snowflake ID configuration management.
pub struct SnowflakeService {
    /// Storage backend.
    storage: Arc<dyn Storage>,
    /// Worker ID allocator.
    worker_id_allocator: WorkerIdAllocator,
}

impl SnowflakeService {
    /// Create a new Snowflake service.
    pub fn new(storage: Arc<dyn Storage>) -> Self {
        Self {
            storage,
            worker_id_allocator: WorkerIdAllocator::default(),
        }
    }

    /// Create with custom worker ID lease duration.
    pub fn with_lease_duration(storage: Arc<dyn Storage>, lease_duration: Duration) -> Self {
        Self {
            storage,
            worker_id_allocator: WorkerIdAllocator::new(lease_duration),
        }
    }

    /// Get Snowflake configuration with an allocated worker ID.
    ///
    /// This is the main endpoint for clients requesting Snowflake ID generation.
    /// Clients receive all parameters needed for local ID generation.
    pub async fn get_config_with_worker_id(&self, name: &str) -> Result<SnowflakeIdResponse> {
        let config = self
            .storage
            .get_snowflake_config(name)
            .await
            .map_err(AppError::Storage)?
            .ok_or_else(|| AppError::ConfigNotFound(name.to_string()))?;

        // Calculate max worker ID based on bits
        let max_worker_id = (1u32 << config.worker_bits) - 1;

        // Allocate a worker ID
        let worker_id = self
            .worker_id_allocator
            .allocate(name, max_worker_id)
            .ok_or_else(|| {
                AppError::Internal(format!(
                    "No available worker IDs for config '{}' (max: {})",
                    name, max_worker_id
                ))
            })?;

        Ok(SnowflakeIdResponse {
            worker_id,
            epoch: config.epoch,
            worker_bits: config.worker_bits,
            sequence_bits: config.sequence_bits,
        })
    }

    /// Create a new Snowflake configuration.
    pub async fn create_config(&self, config: SnowflakeConfig) -> Result<()> {
        // Validate configuration
        config.validate().map_err(|e| AppError::InvalidConfig(e))?;

        // Check if already exists
        if self
            .storage
            .get_snowflake_config(&config.name)
            .await
            .map_err(AppError::Storage)?
            .is_some()
        {
            return Err(AppError::ConfigExists(config.name.clone()));
        }

        // Save configuration
        self.storage
            .save_snowflake_config(&config)
            .await
            .map_err(AppError::Storage)?;

        Ok(())
    }

    /// Get a Snowflake configuration.
    pub async fn get_config(&self, name: &str) -> Result<SnowflakeConfig> {
        self.storage
            .get_snowflake_config(name)
            .await
            .map_err(AppError::Storage)?
            .ok_or_else(|| AppError::ConfigNotFound(name.to_string()))
    }

    /// List all Snowflake configurations.
    pub async fn list_configs(&self) -> Result<Vec<SnowflakeConfig>> {
        self.storage
            .list_snowflake_configs()
            .await
            .map_err(AppError::Storage)
    }

    /// Delete a Snowflake configuration.
    pub async fn delete_config(&self, name: &str) -> Result<bool> {
        self.storage
            .delete_snowflake_config(name)
            .await
            .map_err(AppError::Storage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::FileStorageConfig;
    use crate::storage::file::FileStorage;
    use tempfile::TempDir;

    async fn create_test_service() -> (SnowflakeService, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let storage_config = FileStorageConfig {
            data_dir: temp_dir.path().to_path_buf(),
        };
        let storage = Arc::new(FileStorage::new(&storage_config).unwrap());
        let service = SnowflakeService::new(storage);
        (service, temp_dir)
    }

    #[test]
    fn test_worker_id_allocator() {
        let allocator = WorkerIdAllocator::new(Duration::from_secs(60));

        // Allocate worker IDs
        let id1 = allocator.allocate("test", 3).unwrap();
        let id2 = allocator.allocate("test", 3).unwrap();
        let id3 = allocator.allocate("test", 3).unwrap();
        let id4 = allocator.allocate("test", 3).unwrap();

        // All 4 IDs (0-3) should be different
        let ids = vec![id1, id2, id3, id4];
        let mut sorted = ids.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), 4);

        // No more IDs available
        assert!(allocator.allocate("test", 3).is_none());
    }

    #[tokio::test]
    async fn test_create_and_get_config() {
        let (service, _temp) = create_test_service().await;

        let config = SnowflakeConfig {
            name: "events".to_string(),
            epoch: 1704067200000,
            worker_bits: 10,
            sequence_bits: 12,
        };

        service.create_config(config).await.unwrap();

        let loaded = service.get_config("events").await.unwrap();
        assert_eq!(loaded.name, "events");
        assert_eq!(loaded.epoch, 1704067200000);
    }

    #[tokio::test]
    async fn test_get_config_with_worker_id() {
        let (service, _temp) = create_test_service().await;

        let config = SnowflakeConfig {
            name: "test".to_string(),
            epoch: 1704067200000,
            worker_bits: 5, // max worker_id = 31
            sequence_bits: 12,
        };

        service.create_config(config).await.unwrap();

        let response = service.get_config_with_worker_id("test").await.unwrap();
        assert!(response.worker_id <= 31);
        assert_eq!(response.epoch, 1704067200000);
        assert_eq!(response.worker_bits, 5);
        assert_eq!(response.sequence_bits, 12);
    }
}
