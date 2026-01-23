//! Storage layer module.
//!
//! This module provides trait-based storage abstraction allowing different backends
//! to be used without changing business logic.

pub mod error;
pub mod factory;
pub mod file;
pub mod traits;

pub use error::StorageError;
pub use factory::create_storage;
pub use traits::{ConfigStorage, DistributedLock, LockGuard, SequenceStorage, Storage};
