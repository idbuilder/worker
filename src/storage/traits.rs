//! Storage trait definitions.
//!
//! These traits define the interface for storage backends, enabling swapping
//! between different implementations without changing business logic.

use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use async_trait::async_trait;

use crate::domain::{
    FormattedConfig, IdType, IncrementConfig, SequenceRange, SequenceState, SnowflakeConfig,
};
use crate::error::StorageResult;

/// Sequence storage operations.
///
/// Provides atomic operations for managing sequence counters.
#[async_trait]
pub trait SequenceStorage: Send + Sync {
    /// Get the next batch of sequence values and atomically increment the counter.
    ///
    /// # Arguments
    ///
    /// * `name` - Sequence name
    /// * `count` - Number of IDs to allocate
    /// * `step` - Increment step (can be negative)
    ///
    /// # Returns
    ///
    /// A `SequenceRange` containing the allocated values.
    async fn get_and_increment(
        &self,
        name: &str,
        count: u32,
        step: i64,
    ) -> StorageResult<SequenceRange>;

    /// Get the current sequence value without incrementing.
    async fn get_current(&self, name: &str) -> StorageResult<i64>;

    /// Initialize a new sequence with the given starting value.
    ///
    /// This is idempotent - if the sequence already exists, it will not be modified.
    async fn initialize(&self, name: &str, id_type: IdType, start_value: i64) -> StorageResult<()>;

    /// Check if a sequence exists.
    async fn exists(&self, name: &str) -> StorageResult<bool>;

    /// Get the full sequence state.
    async fn get_state(&self, name: &str) -> StorageResult<Option<SequenceState>>;
}

/// Configuration storage operations.
///
/// Provides CRUD operations for ID configurations.
#[async_trait]
pub trait ConfigStorage: Send + Sync {
    /// Save an increment configuration.
    async fn save_increment_config(&self, config: &IncrementConfig) -> StorageResult<()>;

    /// Get an increment configuration by name.
    async fn get_increment_config(&self, name: &str) -> StorageResult<Option<IncrementConfig>>;

    /// List all increment configurations.
    async fn list_increment_configs(&self) -> StorageResult<Vec<IncrementConfig>>;

    /// Delete an increment configuration.
    async fn delete_increment_config(&self, name: &str) -> StorageResult<bool>;

    /// Save a snowflake configuration.
    async fn save_snowflake_config(&self, config: &SnowflakeConfig) -> StorageResult<()>;

    /// Get a snowflake configuration by name.
    async fn get_snowflake_config(&self, name: &str) -> StorageResult<Option<SnowflakeConfig>>;

    /// List all snowflake configurations.
    async fn list_snowflake_configs(&self) -> StorageResult<Vec<SnowflakeConfig>>;

    /// Delete a snowflake configuration.
    async fn delete_snowflake_config(&self, name: &str) -> StorageResult<bool>;

    /// Save a formatted configuration.
    async fn save_formatted_config(&self, config: &FormattedConfig) -> StorageResult<()>;

    /// Get a formatted configuration by name.
    async fn get_formatted_config(&self, name: &str) -> StorageResult<Option<FormattedConfig>>;

    /// List all formatted configurations.
    async fn list_formatted_configs(&self) -> StorageResult<Vec<FormattedConfig>>;

    /// Delete a formatted configuration.
    async fn delete_formatted_config(&self, name: &str) -> StorageResult<bool>;
}

/// Distributed lock operations.
///
/// Provides distributed locking for coordination across multiple workers.
#[async_trait]
pub trait DistributedLock: Send + Sync {
    /// Acquire a distributed lock.
    ///
    /// # Arguments
    ///
    /// * `key` - Lock key/name
    /// * `ttl` - Time-to-live for the lock (auto-release after this duration)
    ///
    /// # Returns
    ///
    /// A `LockGuard` that releases the lock when dropped.
    async fn acquire(&self, key: &str, ttl: Duration) -> StorageResult<LockGuard>;

    /// Try to acquire a lock without waiting.
    ///
    /// Returns `None` if the lock is already held by another process.
    async fn try_acquire(&self, key: &str, ttl: Duration) -> StorageResult<Option<LockGuard>>;

    /// Check if a lock is currently held.
    async fn is_locked(&self, key: &str) -> StorageResult<bool>;
}

/// RAII guard for distributed locks.
///
/// The lock is automatically released when the guard is dropped.
pub struct LockGuard {
    key: String,
    release_fn: Option<Box<dyn FnOnce() -> Pin<Box<dyn Future<Output = ()> + Send>> + Send>>,
}

impl LockGuard {
    /// Create a new lock guard.
    pub fn new<F, Fut>(key: String, release_fn: F) -> Self
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        Self {
            key,
            release_fn: Some(Box::new(move || Box::pin(release_fn()))),
        }
    }

    /// Get the lock key.
    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Manually release the lock.
    pub async fn release(mut self) {
        if let Some(release_fn) = self.release_fn.take() {
            release_fn().await;
        }
    }
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        if let Some(release_fn) = self.release_fn.take() {
            // Spawn a task to release the lock asynchronously
            tokio::spawn(async move {
                release_fn().await;
            });
        }
    }
}

/// Combined storage trait for all storage operations.
///
/// This trait combines sequence storage, config storage, and distributed locking
/// into a single interface.
#[async_trait]
pub trait Storage: SequenceStorage + ConfigStorage + DistributedLock {
    /// Check if the storage backend is healthy and reachable.
    async fn health_check(&self) -> StorageResult<()>;

    /// Get the storage backend name.
    fn backend_name(&self) -> &'static str;
}

/// Trait object alias for Storage.
pub type DynStorage = dyn Storage;
