//! Sequence caching for performance optimization.
//!
//! Pre-allocates batches of sequence numbers to reduce storage I/O.

use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};

use parking_lot::RwLock;

use crate::domain::SequenceRange;

/// Cached sequence range for a single sequence.
pub struct CachedSequence {
    /// Current value (atomically incremented).
    current: AtomicI64,
    /// Maximum value in the cached range.
    max: i64,
    /// Step between values.
    step: i64,
}

impl CachedSequence {
    /// Create a new cached sequence from a range.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // AtomicI64::new is not const
    pub fn from_range(range: SequenceRange) -> Self {
        Self {
            current: AtomicI64::new(range.start),
            max: range.end,
            step: range.step,
        }
    }

    /// Try to get the next value from cache.
    ///
    /// Returns `None` if the cache is exhausted.
    pub fn next(&self) -> Option<i64> {
        loop {
            let current = self.current.load(Ordering::SeqCst);

            // Check if exhausted
            if self.step > 0 && current > self.max {
                return None;
            }
            if self.step < 0 && current < self.max {
                return None;
            }

            let next = current + self.step;

            if self
                .current
                .compare_exchange(current, next, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                return Some(current);
            }
            // CAS failed, retry
        }
    }

    /// Try to get multiple values from cache.
    ///
    /// Returns as many values as available, up to `count`.
    pub fn next_batch(&self, count: u32) -> Vec<i64> {
        let mut values = Vec::with_capacity(count as usize);

        for _ in 0..count {
            match self.next() {
                Some(v) => values.push(v),
                None => break,
            }
        }

        values
    }

    /// Get remaining count in cache.
    pub fn remaining(&self) -> u64 {
        let current = self.current.load(Ordering::SeqCst);

        #[allow(clippy::cast_sign_loss)]
        if self.step > 0 {
            if current > self.max {
                0
            } else {
                ((self.max - current) / self.step + 1) as u64
            }
        } else if current < self.max {
            0
        } else {
            ((current - self.max) / (-self.step) + 1) as u64
        }
    }

    /// Check if the cache needs refilling.
    pub fn needs_refill(&self, threshold: u32) -> bool {
        self.remaining() < u64::from(threshold)
    }
}

/// Sequence cache manager.
pub struct SequenceCache {
    /// Cached sequences by name.
    sequences: RwLock<HashMap<String, CachedSequence>>,
    /// Default prefetch threshold.
    prefetch_threshold: u32,
}

impl SequenceCache {
    /// Create a new sequence cache.
    #[must_use]
    pub fn new(prefetch_threshold: u32) -> Self {
        Self {
            sequences: RwLock::new(HashMap::new()),
            prefetch_threshold,
        }
    }

    /// Get values from cache, or return how many are missing.
    ///
    /// Returns `Ok(values)` if all requested values were cached.
    /// Returns `Err(missing_count)` if cache is insufficient.
    ///
    /// # Errors
    ///
    /// Returns `Err(missing_count)` if the cache doesn't have enough values.
    pub fn get(&self, name: &str, count: u32) -> Result<Vec<i64>, u32> {
        let sequences = self.sequences.read();

        sequences.get(name).map_or(Err(count), |cached| {
            let values = cached.next_batch(count);

            if values.len() == count as usize {
                Ok(values)
            } else {
                // Not enough in cache
                #[allow(clippy::cast_possible_truncation)]
                let missing = count - values.len() as u32;
                Err(missing)
            }
        })
    }

    /// Add a new range to the cache.
    pub fn put(&self, name: &str, range: SequenceRange) {
        let mut sequences = self.sequences.write();
        sequences.insert(name.to_string(), CachedSequence::from_range(range));
    }

    /// Check if a sequence needs prefetch.
    pub fn needs_prefetch(&self, name: &str) -> bool {
        let sequences = self.sequences.read();

        sequences
            .get(name)
            .is_none_or(|c| c.needs_refill(self.prefetch_threshold))
    }

    /// Remove a sequence from cache.
    pub fn remove(&self, name: &str) {
        let mut sequences = self.sequences.write();
        sequences.remove(name);
    }

    /// Clear all cached sequences.
    pub fn clear(&self) {
        let mut sequences = self.sequences.write();
        sequences.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cached_sequence_next() {
        let cached = CachedSequence::from_range(SequenceRange::new(1, 5, 1));

        assert_eq!(cached.next(), Some(1));
        assert_eq!(cached.next(), Some(2));
        assert_eq!(cached.next(), Some(3));
        assert_eq!(cached.next(), Some(4));
        assert_eq!(cached.next(), Some(5));
        assert_eq!(cached.next(), None);
    }

    #[test]
    fn test_cached_sequence_with_step() {
        let cached = CachedSequence::from_range(SequenceRange::new(0, 10, 2));

        assert_eq!(cached.next(), Some(0));
        assert_eq!(cached.next(), Some(2));
        assert_eq!(cached.next(), Some(4));
        assert_eq!(cached.next(), Some(6));
        assert_eq!(cached.next(), Some(8));
        assert_eq!(cached.next(), Some(10));
        assert_eq!(cached.next(), None);
    }

    #[test]
    fn test_cached_sequence_remaining() {
        let cached = CachedSequence::from_range(SequenceRange::new(1, 10, 1));

        assert_eq!(cached.remaining(), 10);
        cached.next();
        assert_eq!(cached.remaining(), 9);
    }

    #[test]
    fn test_sequence_cache_get() {
        let cache = SequenceCache::new(10);

        // No cache yet
        assert!(cache.get("test", 5).is_err());

        // Add to cache
        cache.put("test", SequenceRange::new(1, 100, 1));

        // Get from cache
        let values = cache.get("test", 5).unwrap();
        assert_eq!(values, vec![1, 2, 3, 4, 5]);

        // Get more
        let values = cache.get("test", 3).unwrap();
        assert_eq!(values, vec![6, 7, 8]);
    }

    #[test]
    fn test_sequence_cache_needs_prefetch() {
        let cache = SequenceCache::new(10);

        // No cache - needs prefetch
        assert!(cache.needs_prefetch("test"));

        // Add small range
        cache.put("test", SequenceRange::new(1, 5, 1));
        assert!(cache.needs_prefetch("test")); // 5 < 10 threshold

        // Add larger range
        cache.put("test", SequenceRange::new(1, 100, 1));
        assert!(!cache.needs_prefetch("test")); // 100 > 10 threshold
    }
}
