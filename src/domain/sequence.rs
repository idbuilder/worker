//! Sequence state and range types.

use serde::{Deserialize, Serialize};

use super::IdType;

/// A range of sequence values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SequenceRange {
    /// First value in the range (inclusive).
    pub start: i64,
    /// Last value in the range (inclusive).
    pub end: i64,
    /// Step between values.
    pub step: i64,
}

impl SequenceRange {
    /// Create a new sequence range.
    #[must_use]
    pub const fn new(start: i64, end: i64, step: i64) -> Self {
        Self { start, end, step }
    }

    /// Get the number of IDs in this range.
    #[must_use]
    pub const fn count(&self) -> u64 {
        if self.step == 0 {
            return 0;
        }
        let diff = if self.step > 0 {
            self.end - self.start
        } else {
            self.start - self.end
        };
        if diff < 0 {
            0
        } else {
            (diff / self.step.abs()) as u64 + 1
        }
    }

    /// Check if the range is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.count() == 0
    }

    /// Iterate over values in the range.
    pub fn iter(&self) -> SequenceRangeIterator {
        SequenceRangeIterator {
            current: self.start,
            end: self.end,
            step: self.step,
            exhausted: false,
        }
    }
}

impl IntoIterator for SequenceRange {
    type Item = i64;
    type IntoIter = SequenceRangeIterator;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Iterator over a sequence range.
pub struct SequenceRangeIterator {
    current: i64,
    end: i64,
    step: i64,
    exhausted: bool,
}

impl Iterator for SequenceRangeIterator {
    type Item = i64;

    fn next(&mut self) -> Option<Self::Item> {
        if self.exhausted || self.step == 0 {
            return None;
        }

        let in_range = if self.step > 0 {
            self.current <= self.end
        } else {
            self.current >= self.end
        };

        if in_range {
            let value = self.current;
            match self.current.checked_add(self.step) {
                Some(next) => self.current = next,
                None => self.exhausted = true,
            }
            Some(value)
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.exhausted || self.step == 0 {
            return (0, Some(0));
        }

        let remaining = if self.step > 0 {
            if self.current > self.end {
                0
            } else {
                ((self.end - self.current) / self.step + 1) as usize
            }
        } else if self.current < self.end {
            0
        } else {
            ((self.current - self.end) / (-self.step) + 1) as usize
        };

        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for SequenceRangeIterator {}

/// Persistent state of a sequence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequenceState {
    /// Sequence name.
    pub name: String,

    /// Type of ID this sequence is used for.
    pub id_type: IdType,

    /// Current value (next value to be allocated).
    pub current_value: i64,

    /// Version for optimistic locking.
    pub version: u64,

    /// Last update timestamp (milliseconds since epoch).
    pub updated_at: i64,
}

impl SequenceState {
    /// Create a new sequence state.
    #[must_use]
    pub fn new(name: String, id_type: IdType, start_value: i64) -> Self {
        Self {
            name,
            id_type,
            current_value: start_value,
            version: 0,
            updated_at: chrono::Utc::now().timestamp_millis(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sequence_range_count() {
        let range = SequenceRange::new(1, 10, 1);
        assert_eq!(range.count(), 10);

        let range = SequenceRange::new(1, 10, 2);
        assert_eq!(range.count(), 5);

        let range = SequenceRange::new(10, 1, -1);
        assert_eq!(range.count(), 10);

        let range = SequenceRange::new(1, 1, 1);
        assert_eq!(range.count(), 1);
    }

    #[test]
    fn test_sequence_range_iter() {
        let range = SequenceRange::new(1, 5, 1);
        let values: Vec<i64> = range.iter().collect();
        assert_eq!(values, vec![1, 2, 3, 4, 5]);

        let range = SequenceRange::new(0, 10, 2);
        let values: Vec<i64> = range.iter().collect();
        assert_eq!(values, vec![0, 2, 4, 6, 8, 10]);

        let range = SequenceRange::new(5, 1, -1);
        let values: Vec<i64> = range.iter().collect();
        assert_eq!(values, vec![5, 4, 3, 2, 1]);
    }

    #[test]
    fn test_sequence_range_exact_size() {
        let range = SequenceRange::new(1, 100, 1);
        let iter = range.iter();
        assert_eq!(iter.len(), 100);
    }
}
