//! File-based sequence storage.

use std::path::PathBuf;

use async_trait::async_trait;
use fs2::FileExt;
use tokio::sync::Mutex;

use crate::domain::{IdType, SequenceRange, SequenceState};
use crate::error::{StorageError, StorageResult};
use crate::storage::traits::SequenceStorage;

/// File-based sequence storage implementation.
pub struct FileSequenceStorage {
    /// Directory for sequence files.
    sequences_dir: PathBuf,
    /// Mutex for coordinating file operations within this process.
    lock: Mutex<()>,
}

impl FileSequenceStorage {
    /// Create a new file sequence storage.
    pub fn new(sequences_dir: PathBuf) -> Self {
        Self {
            sequences_dir,
            lock: Mutex::new(()),
        }
    }

    /// Get the file path for a sequence.
    fn sequence_path(&self, name: &str) -> PathBuf {
        self.sequences_dir
            .join(format!("{}.json", sanitize_name(name)))
    }

    /// Read sequence state from file with exclusive lock.
    fn read_state_locked(&self, name: &str) -> StorageResult<Option<SequenceState>> {
        let path = self.sequence_path(name);

        if !path.exists() {
            return Ok(None);
        }

        let file = std::fs::File::open(&path)?;
        file.lock_exclusive()
            .map_err(|e| StorageError::LockFailed(e.to_string()))?;

        let state: SequenceState = serde_json::from_reader(&file)?;
        file.unlock()
            .map_err(|e| StorageError::LockFailed(e.to_string()))?;

        Ok(Some(state))
    }

    /// Write sequence state to file with exclusive lock.
    fn write_state_locked(&self, state: &SequenceState) -> StorageResult<()> {
        let path = self.sequence_path(&state.name);

        let file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)?;

        file.lock_exclusive()
            .map_err(|e| StorageError::LockFailed(e.to_string()))?;

        serde_json::to_writer_pretty(&file, state)?;
        file.sync_all()?;
        file.unlock()
            .map_err(|e| StorageError::LockFailed(e.to_string()))?;

        Ok(())
    }

    /// Atomically update sequence state.
    fn update_state<F>(&self, name: &str, update_fn: F) -> StorageResult<SequenceState>
    where
        F: FnOnce(&mut SequenceState) -> StorageResult<()>,
    {
        let path = self.sequence_path(name);

        if !path.exists() {
            return Err(StorageError::NotFound(format!(
                "Sequence '{}' not found",
                name
            )));
        }

        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)?;

        file.lock_exclusive()
            .map_err(|e| StorageError::LockFailed(e.to_string()))?;

        // Read current state
        let mut state: SequenceState = serde_json::from_reader(&file)?;

        // Apply update
        update_fn(&mut state)?;

        // Update metadata
        state.version += 1;
        state.updated_at = chrono::Utc::now().timestamp_millis();

        // Write back (need to seek to beginning and truncate)
        use std::io::{Seek, SeekFrom, Write};
        let mut file = file;
        file.seek(SeekFrom::Start(0))?;
        file.set_len(0)?;

        let json = serde_json::to_string_pretty(&state)?;
        file.write_all(json.as_bytes())?;
        file.sync_all()?;

        file.unlock()
            .map_err(|e| StorageError::LockFailed(e.to_string()))?;

        Ok(state)
    }
}

#[async_trait]
impl SequenceStorage for FileSequenceStorage {
    async fn get_and_increment(
        &self,
        name: &str,
        count: u32,
        step: i64,
    ) -> StorageResult<SequenceRange> {
        let _guard = self.lock.lock().await;

        let count_i64 = i64::from(count);
        let mut start_value = 0i64;
        let mut end_value = 0i64;

        self.update_state(name, |state| {
            start_value = state.current_value;

            // Calculate end value based on step and count
            let increment = step * (count_i64 - 1);
            end_value = start_value + increment;

            // Update current value to next available
            state.current_value = start_value + (step * count_i64);

            Ok(())
        })?;

        Ok(SequenceRange::new(start_value, end_value, step))
    }

    async fn get_current(&self, name: &str) -> StorageResult<i64> {
        let _guard = self.lock.lock().await;

        let state = self
            .read_state_locked(name)?
            .ok_or_else(|| StorageError::NotFound(format!("Sequence '{}' not found", name)))?;

        Ok(state.current_value)
    }

    async fn initialize(&self, name: &str, id_type: IdType, start_value: i64) -> StorageResult<()> {
        let _guard = self.lock.lock().await;

        let path = self.sequence_path(name);

        // Don't overwrite existing sequence
        if path.exists() {
            return Ok(());
        }

        let state = SequenceState::new(name.to_string(), id_type, start_value);
        self.write_state_locked(&state)?;

        Ok(())
    }

    async fn exists(&self, name: &str) -> StorageResult<bool> {
        let path = self.sequence_path(name);
        Ok(path.exists())
    }

    async fn get_state(&self, name: &str) -> StorageResult<Option<SequenceState>> {
        let _guard = self.lock.lock().await;
        self.read_state_locked(name)
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

    fn create_test_storage() -> (FileSequenceStorage, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let storage = FileSequenceStorage::new(temp_dir.path().to_path_buf());
        (storage, temp_dir)
    }

    #[tokio::test]
    async fn test_initialize_and_get() {
        let (storage, _temp) = create_test_storage();

        storage
            .initialize("test", IdType::Increment, 100)
            .await
            .unwrap();
        assert_eq!(storage.get_current("test").await.unwrap(), 100);
    }

    #[tokio::test]
    async fn test_get_and_increment() {
        let (storage, _temp) = create_test_storage();

        storage
            .initialize("test", IdType::Increment, 1)
            .await
            .unwrap();

        let range = storage.get_and_increment("test", 5, 1).await.unwrap();
        assert_eq!(range.start, 1);
        assert_eq!(range.end, 5);
        assert_eq!(range.step, 1);

        // Next batch should start at 6
        let range = storage.get_and_increment("test", 3, 1).await.unwrap();
        assert_eq!(range.start, 6);
        assert_eq!(range.end, 8);
    }

    #[tokio::test]
    async fn test_get_and_increment_with_step() {
        let (storage, _temp) = create_test_storage();

        storage
            .initialize("test", IdType::Increment, 0)
            .await
            .unwrap();

        let range = storage.get_and_increment("test", 3, 2).await.unwrap();
        assert_eq!(range.start, 0);
        assert_eq!(range.end, 4); // 0, 2, 4
        assert_eq!(range.step, 2);

        let values: Vec<i64> = range.iter().collect();
        assert_eq!(values, vec![0, 2, 4]);
    }

    #[tokio::test]
    async fn test_sanitize_name() {
        assert_eq!(sanitize_name("simple"), "simple");
        assert_eq!(sanitize_name("with-dash"), "with-dash");
        assert_eq!(sanitize_name("with_underscore"), "with_underscore");
        assert_eq!(sanitize_name("with/slash"), "with_slash");
        assert_eq!(sanitize_name("with space"), "with_space");
    }
}
