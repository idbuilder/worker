//! File-based distributed locking.
//!
//! Uses file locks (flock) for coordination between processes.
//! Note: File locks may not work correctly on all network filesystems.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use fs2::FileExt;
use parking_lot::Mutex;
use tokio::time::sleep;

use crate::error::{StorageError, StorageResult};
use crate::storage::traits::{DistributedLock, LockGuard};

/// File-based lock manager.
pub struct FileLock {
    /// Directory for lock files.
    locks_dir: PathBuf,
    /// Track of active locks for cleanup.
    active_locks: Arc<Mutex<HashMap<String, LockInfo>>>,
}

#[allow(dead_code)]
struct LockInfo {
    file: std::fs::File,
    acquired_at: Instant,
    ttl: Duration,
}

impl FileLock {
    /// Create a new file lock manager.
    #[must_use]
    pub fn new(locks_dir: PathBuf) -> Self {
        Self {
            locks_dir,
            active_locks: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Get the lock file path for a key.
    fn lock_path(&self, key: &str) -> PathBuf {
        self.locks_dir.join(format!("{}.lock", sanitize_name(key)))
    }

    /// Internal method to acquire lock.
    fn acquire_internal(&self, key: &str, ttl: Duration, blocking: bool) -> StorageResult<bool> {
        let path = self.lock_path(key);

        // Ensure locks directory exists
        std::fs::create_dir_all(&self.locks_dir)?;

        let file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)?;

        let result = if blocking {
            file.lock_exclusive()
        } else {
            file.try_lock_exclusive()
        };

        match result {
            Ok(()) => {
                // Write lock metadata
                use std::io::Write;
                let mut file_ref = &file;
                let pid = std::process::id();
                let timestamp = chrono::Utc::now().timestamp_millis();
                let ttl_ms = ttl.as_millis();
                writeln!(
                    file_ref,
                    "{{\"pid\":{pid},\"acquired_at\":{timestamp},\"ttl_ms\":{ttl_ms}}}"
                )
                .ok();

                self.active_locks.lock().insert(
                    key.to_string(),
                    LockInfo {
                        file,
                        acquired_at: Instant::now(),
                        ttl,
                    },
                );

                Ok(true)
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(false),
            Err(e) => Err(StorageError::LockFailed(e.to_string())),
        }
    }

    /// Release a lock.
    #[allow(dead_code)]
    fn release_internal(&self, key: &str) {
        let lock_info = self.active_locks.lock().remove(key);
        if let Some(info) = lock_info {
            // Unlock and close file
            let _ = info.file.unlock();
            // File is automatically closed when dropped
        }

        // Try to remove the lock file
        let path = self.lock_path(key);
        let _ = std::fs::remove_file(path);
    }
}

#[async_trait]
impl DistributedLock for FileLock {
    async fn acquire(&self, key: &str, ttl: Duration) -> StorageResult<LockGuard> {
        let key_owned = key.to_string();
        let active_locks = self.active_locks.clone();
        let locks_dir = self.locks_dir.clone();

        // Try to acquire with retries
        let max_attempts = 100;
        let retry_delay = Duration::from_millis(50);

        for attempt in 0..max_attempts {
            if self.acquire_internal(key, ttl, false)? {
                let key_for_release = key_owned.clone();

                return Ok(LockGuard::new(key_owned, move || async move {
                    let lock_info = active_locks.lock().remove(&key_for_release);
                    if let Some(info) = lock_info {
                        let _ = info.file.unlock();
                    }

                    let path = locks_dir.join(format!("{}.lock", sanitize_name(&key_for_release)));
                    let _ = std::fs::remove_file(path);
                }));
            }

            if attempt < max_attempts - 1 {
                sleep(retry_delay).await;
            }
        }

        Err(StorageError::LockTimeout(format!(
            "Failed to acquire lock '{key}' after {max_attempts} attempts"
        )))
    }

    async fn try_acquire(&self, key: &str, ttl: Duration) -> StorageResult<Option<LockGuard>> {
        let key_owned = key.to_string();
        let active_locks = self.active_locks.clone();
        let locks_dir = self.locks_dir.clone();

        if self.acquire_internal(key, ttl, false)? {
            let key_for_release = key_owned.clone();

            Ok(Some(LockGuard::new(key_owned, move || async move {
                let lock_info = active_locks.lock().remove(&key_for_release);
                if let Some(info) = lock_info {
                    let _ = info.file.unlock();
                }

                let path = locks_dir.join(format!("{}.lock", sanitize_name(&key_for_release)));
                let _ = std::fs::remove_file(path);
            })))
        } else {
            Ok(None)
        }
    }

    async fn is_locked(&self, key: &str) -> StorageResult<bool> {
        let path = self.lock_path(key);

        if !path.exists() {
            return Ok(false);
        }

        // Try to acquire lock non-blocking
        let Ok(file) = std::fs::OpenOptions::new().read(true).open(&path) else {
            return Ok(false);
        };

        match file.try_lock_exclusive() {
            Ok(()) => {
                // We got the lock, so it wasn't locked
                let _ = file.unlock();
                Ok(false)
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // Lock is held by someone else
                Ok(true)
            }
            Err(_) => Ok(false),
        }
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
    use tempfile::TempDir;

    fn create_test_lock_manager() -> (FileLock, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let lock_manager = FileLock::new(temp_dir.path().to_path_buf());
        (lock_manager, temp_dir)
    }

    #[tokio::test]
    async fn test_acquire_and_release() {
        let (lock_manager, _temp) = create_test_lock_manager();

        let guard = lock_manager
            .acquire("test_lock", Duration::from_secs(10))
            .await
            .unwrap();

        assert!(lock_manager.is_locked("test_lock").await.unwrap());

        guard.release().await;

        // Give a moment for async release
        tokio::time::sleep(Duration::from_millis(50)).await;

        assert!(!lock_manager.is_locked("test_lock").await.unwrap());
    }

    #[tokio::test]
    async fn test_try_acquire_fails_when_locked() {
        let (lock_manager, _temp) = create_test_lock_manager();

        let _guard1 = lock_manager
            .acquire("test_lock", Duration::from_secs(10))
            .await
            .unwrap();

        // try_acquire should return None when lock is held
        let result = lock_manager
            .try_acquire("test_lock", Duration::from_secs(10))
            .await
            .unwrap();

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_try_acquire_succeeds_when_unlocked() {
        let (lock_manager, _temp) = create_test_lock_manager();

        let result = lock_manager
            .try_acquire("new_lock", Duration::from_secs(10))
            .await
            .unwrap();

        assert!(result.is_some());
    }
}
