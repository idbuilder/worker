//! Pattern parser for formatted IDs.
//!
//! Parses pattern strings like "INV{YYYY}{MM}{DD}-{SEQ:4}" and generates IDs.

use chrono::{Datelike, Timelike, Utc};
use rand::Rng;

use crate::domain::SequenceReset;

/// Parsed placeholder in a pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Placeholder {
    /// Literal text.
    Literal(String),
    /// 4-digit year.
    Year4,
    /// 2-digit year.
    Year2,
    /// 2-digit month.
    Month,
    /// 2-digit day.
    Day,
    /// 2-digit hour (24h).
    Hour,
    /// 2-digit minute.
    Minute,
    /// 2-digit second.
    Second,
    /// Zero-padded sequence number.
    Sequence(u8),
    /// Random alphanumeric characters.
    Random(u8),
    /// UUID v4.
    Uuid,
}

/// Parsed pattern.
#[derive(Debug, Clone)]
pub struct ParsedPattern {
    /// Pattern parts.
    parts: Vec<Placeholder>,
    /// Whether this pattern has a sequence placeholder.
    has_sequence: bool,
}

impl ParsedPattern {
    /// Parse a pattern string.
    pub fn parse(pattern: &str) -> Result<Self, String> {
        let mut parts = Vec::new();
        let mut chars = pattern.chars().peekable();
        let mut literal = String::new();
        let mut has_sequence = false;

        while let Some(c) = chars.next() {
            if c == '{' {
                // Save any accumulated literal
                if !literal.is_empty() {
                    parts.push(Placeholder::Literal(std::mem::take(&mut literal)));
                }

                // Parse placeholder
                let mut placeholder = String::new();
                let mut found_close = false;

                for inner in chars.by_ref() {
                    if inner == '}' {
                        found_close = true;
                        break;
                    }
                    placeholder.push(inner);
                }

                if !found_close {
                    return Err("unclosed placeholder".to_string());
                }

                let part = parse_placeholder(&placeholder)?;
                if matches!(part, Placeholder::Sequence(_)) {
                    has_sequence = true;
                }
                parts.push(part);
            } else {
                literal.push(c);
            }
        }

        // Save any remaining literal
        if !literal.is_empty() {
            parts.push(Placeholder::Literal(literal));
        }

        Ok(Self {
            parts,
            has_sequence,
        })
    }

    /// Check if this pattern has a sequence placeholder.
    pub fn has_sequence(&self) -> bool {
        self.has_sequence
    }

    /// Generate an ID from this pattern.
    ///
    /// # Arguments
    ///
    /// * `sequence` - Sequence number to use (if pattern has {SEQ:N})
    pub fn generate(&self, sequence: Option<i64>) -> Result<String, String> {
        let now = Utc::now();
        let mut result = String::new();

        for part in &self.parts {
            match part {
                Placeholder::Literal(s) => result.push_str(s),
                Placeholder::Year4 => result.push_str(&format!("{:04}", now.year())),
                Placeholder::Year2 => result.push_str(&format!("{:02}", now.year() % 100)),
                Placeholder::Month => result.push_str(&format!("{:02}", now.month())),
                Placeholder::Day => result.push_str(&format!("{:02}", now.day())),
                Placeholder::Hour => result.push_str(&format!("{:02}", now.hour())),
                Placeholder::Minute => result.push_str(&format!("{:02}", now.minute())),
                Placeholder::Second => result.push_str(&format!("{:02}", now.second())),
                Placeholder::Sequence(width) => {
                    let seq = sequence.ok_or("sequence required but not provided")?;
                    result.push_str(&format!("{:0width$}", seq, width = *width as usize));
                }
                Placeholder::Random(len) => {
                    result.push_str(&generate_random(*len as usize));
                }
                Placeholder::Uuid => {
                    result.push_str(&uuid::Uuid::new_v4().to_string());
                }
            }
        }

        Ok(result)
    }

    /// Get the sequence key for a given reset mode.
    ///
    /// This generates a key that changes based on the reset period,
    /// allowing sequence counters to reset at the appropriate interval.
    pub fn sequence_key(&self, base_name: &str, reset: SequenceReset) -> String {
        let now = Utc::now();

        match reset {
            SequenceReset::Never => base_name.to_string(),
            SequenceReset::Daily => {
                format!(
                    "{}:{:04}{:02}{:02}",
                    base_name,
                    now.year(),
                    now.month(),
                    now.day()
                )
            }
            SequenceReset::Monthly => {
                format!("{}:{:04}{:02}", base_name, now.year(), now.month())
            }
            SequenceReset::Yearly => {
                format!("{}:{:04}", base_name, now.year())
            }
        }
    }
}

/// Parse a placeholder string.
fn parse_placeholder(placeholder: &str) -> Result<Placeholder, String> {
    match placeholder {
        "YYYY" => Ok(Placeholder::Year4),
        "YY" => Ok(Placeholder::Year2),
        "MM" => Ok(Placeholder::Month),
        "DD" => Ok(Placeholder::Day),
        "HH" => Ok(Placeholder::Hour),
        "mm" => Ok(Placeholder::Minute),
        "ss" => Ok(Placeholder::Second),
        "UUID" => Ok(Placeholder::Uuid),
        _ => {
            if let Some(n_str) = placeholder.strip_prefix("SEQ:") {
                let n: u8 = n_str
                    .parse()
                    .map_err(|_| format!("invalid sequence width: {}", n_str))?;
                if n == 0 || n > 20 {
                    return Err(format!("sequence width must be 1-20, got {}", n));
                }
                Ok(Placeholder::Sequence(n))
            } else if let Some(n_str) = placeholder.strip_prefix("RAND:") {
                let n: u8 = n_str
                    .parse()
                    .map_err(|_| format!("invalid random length: {}", n_str))?;
                if n == 0 || n > 32 {
                    return Err(format!("random length must be 1-32, got {}", n));
                }
                Ok(Placeholder::Random(n))
            } else {
                Err(format!("unknown placeholder: {{{}}}", placeholder))
            }
        }
    }
}

/// Generate random alphanumeric characters.
fn generate_random(len: usize) -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut rng = rand::rng();

    (0..len)
        .map(|_| {
            let idx = rng.random_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_pattern() {
        let pattern = ParsedPattern::parse("INV{YYYY}{MM}{DD}-{SEQ:4}").unwrap();
        assert!(pattern.has_sequence());
        assert_eq!(pattern.parts.len(), 6);
    }

    #[test]
    fn test_parse_uuid_pattern() {
        let pattern = ParsedPattern::parse("PREFIX-{UUID}").unwrap();
        assert!(!pattern.has_sequence());
    }

    #[test]
    fn test_generate_with_sequence() {
        let pattern = ParsedPattern::parse("ID-{SEQ:4}").unwrap();
        let id = pattern.generate(Some(42)).unwrap();
        assert_eq!(id, "ID-0042");
    }

    #[test]
    fn test_generate_with_random() {
        let pattern = ParsedPattern::parse("CODE-{RAND:8}").unwrap();
        let id = pattern.generate(None).unwrap();
        assert!(id.starts_with("CODE-"));
        assert_eq!(id.len(), 13); // "CODE-" + 8 random
    }

    #[test]
    fn test_generate_with_uuid() {
        let pattern = ParsedPattern::parse("{UUID}").unwrap();
        let id = pattern.generate(None).unwrap();
        assert_eq!(id.len(), 36); // UUID format
    }

    #[test]
    fn test_sequence_key() {
        let pattern = ParsedPattern::parse("{SEQ:4}").unwrap();

        let key_never = pattern.sequence_key("test", SequenceReset::Never);
        assert_eq!(key_never, "test");

        let key_daily = pattern.sequence_key("test", SequenceReset::Daily);
        assert!(key_daily.starts_with("test:"));
        assert_eq!(key_daily.len(), 13); // "test:" + 8 chars (YYYYMMDD)
    }

    #[test]
    fn test_invalid_patterns() {
        assert!(ParsedPattern::parse("{INVALID}").is_err());
        assert!(ParsedPattern::parse("{SEQ:0}").is_err());
        assert!(ParsedPattern::parse("{SEQ:21}").is_err());
        assert!(ParsedPattern::parse("{RAND:0}").is_err());
        assert!(ParsedPattern::parse("{RAND:33}").is_err());
        assert!(ParsedPattern::parse("{UNCLOSED").is_err());
    }
}
