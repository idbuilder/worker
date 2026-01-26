//! ID configuration types.
//!
//! These types represent the configuration for different ID generation strategies.

use serde::{Deserialize, Serialize};

use crate::service::is_reserved_key_name;

/// Type of ID generation strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IdType {
    /// Auto-increment sequential IDs.
    Increment,
    /// Snowflake-style distributed IDs.
    Snowflake,
    /// Custom formatted string IDs.
    Formatted,
}

impl std::fmt::Display for IdType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Increment => write!(f, "increment"),
            Self::Snowflake => write!(f, "snowflake"),
            Self::Formatted => write!(f, "formatted"),
        }
    }
}

/// Trait for ID configuration types.
pub trait IdConfig: Send + Sync + Clone + Serialize + for<'de> Deserialize<'de> {
    /// Get the configuration name.
    fn name(&self) -> &str;

    /// Get the ID type.
    fn id_type() -> IdType;

    /// Validate the configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration is invalid.
    fn validate(&self) -> Result<(), String>;
}

/// Configuration for auto-increment ID generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncrementConfig {
    /// Unique name for this configuration.
    pub name: String,

    /// Starting value for the sequence.
    #[serde(default = "default_start")]
    pub start: i64,

    /// Increment step (can be negative for decrementing).
    #[serde(default = "default_step")]
    pub step: i64,

    /// Minimum allowed value.
    #[serde(default = "default_min")]
    pub min: i64,

    /// Maximum allowed value.
    #[serde(default = "default_max")]
    pub max: i64,

    /// Whether to require per-key token authentication.
    /// If false (default), global token authentication is accepted.
    #[serde(default = "default_key_token_enable")]
    pub key_token_enable: bool,
}

const fn default_start() -> i64 {
    1
}
const fn default_step() -> i64 {
    1
}
const fn default_min() -> i64 {
    1
}
const fn default_max() -> i64 {
    i64::MAX
}

/// Default for `key_token_enable` - false means global token is accepted.
const fn default_key_token_enable() -> bool {
    false
}

impl IdConfig for IncrementConfig {
    fn name(&self) -> &str {
        &self.name
    }

    fn id_type() -> IdType {
        IdType::Increment
    }

    fn validate(&self) -> Result<(), String> {
        if self.name.is_empty() {
            return Err("name cannot be empty".to_string());
        }
        if self.name.len() > 255 {
            return Err("name cannot exceed 255 characters".to_string());
        }
        if is_reserved_key_name(&self.name) {
            return Err("name cannot start or end with '__' (reserved)".to_string());
        }
        if self.step == 0 {
            return Err("step cannot be zero".to_string());
        }
        if self.min > self.max {
            return Err("min cannot be greater than max".to_string());
        }
        if self.start < self.min || self.start > self.max {
            return Err("start must be between min and max".to_string());
        }
        Ok(())
    }
}

impl Default for IncrementConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            start: 1,
            step: 1,
            min: 1,
            max: i64::MAX,
            key_token_enable: false,
        }
    }
}

/// Configuration for Snowflake ID generation.
///
/// Snowflake IDs are 64-bit integers composed of:
/// - Timestamp (milliseconds since epoch)
/// - Worker ID
/// - Sequence number
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnowflakeConfig {
    /// Unique name for this configuration.
    pub name: String,

    /// Custom epoch timestamp in milliseconds.
    /// Default is 2024-01-01 00:00:00 UTC (1704067200000).
    #[serde(default = "default_epoch")]
    pub epoch: i64,

    /// Number of bits allocated for worker ID.
    #[serde(default = "default_worker_bits")]
    pub worker_bits: u8,

    /// Number of bits allocated for sequence number.
    #[serde(default = "default_sequence_bits")]
    pub sequence_bits: u8,

    /// Whether to require per-key token authentication.
    /// If false (default), global token authentication is accepted.
    #[serde(default = "default_key_token_enable")]
    pub key_token_enable: bool,
}

const fn default_epoch() -> i64 {
    1_704_067_200_000 // 2024-01-01 00:00:00 UTC
}
const fn default_worker_bits() -> u8 {
    10
}
const fn default_sequence_bits() -> u8 {
    12
}

impl IdConfig for SnowflakeConfig {
    fn name(&self) -> &str {
        &self.name
    }

    fn id_type() -> IdType {
        IdType::Snowflake
    }

    fn validate(&self) -> Result<(), String> {
        if self.name.is_empty() {
            return Err("name cannot be empty".to_string());
        }
        if self.name.len() > 255 {
            return Err("name cannot exceed 255 characters".to_string());
        }
        if is_reserved_key_name(&self.name) {
            return Err("name cannot start or end with '__' (reserved)".to_string());
        }
        if self.epoch <= 0 {
            return Err("epoch must be positive".to_string());
        }
        // Total bits for worker + sequence must leave room for timestamp
        // 64 bits total - 1 sign bit - 41 timestamp bits = 22 bits available
        let total_bits = u16::from(self.worker_bits) + u16::from(self.sequence_bits);
        if total_bits > 22 {
            return Err(format!(
                "worker_bits + sequence_bits must be <= 22, got {total_bits}"
            ));
        }
        if self.worker_bits == 0 {
            return Err("worker_bits must be at least 1".to_string());
        }
        if self.sequence_bits == 0 {
            return Err("sequence_bits must be at least 1".to_string());
        }
        Ok(())
    }
}

impl Default for SnowflakeConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            epoch: default_epoch(),
            worker_bits: 10,
            sequence_bits: 12,
            key_token_enable: false,
        }
    }
}

/// Sequence reset mode for formatted IDs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SequenceReset {
    /// Never reset the sequence.
    #[default]
    Never,
    /// Reset sequence daily at midnight.
    Daily,
    /// Reset sequence monthly on the 1st.
    Monthly,
    /// Reset sequence yearly on January 1st.
    Yearly,
}

/// Configuration for formatted string ID generation.
///
/// Pattern syntax:
/// - `{YYYY}` - 4-digit year
/// - `{YY}` - 2-digit year
/// - `{MM}` - 2-digit month
/// - `{DD}` - 2-digit day
/// - `{HH}` - 2-digit hour (24h)
/// - `{mm}` - 2-digit minute
/// - `{ss}` - 2-digit second
/// - `{SEQ:N}` - N-digit zero-padded sequence
/// - `{RAND:N}` - N random alphanumeric characters
/// - `{UUID}` - UUID v4
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormattedConfig {
    /// Unique name for this configuration.
    pub name: String,

    /// Pattern string for ID generation.
    /// Example: "INV{YYYY}{MM}{DD}-{SEQ:4}"
    pub pattern: String,

    /// When to reset the sequence counter.
    #[serde(default)]
    pub sequence_reset: SequenceReset,

    /// Whether to require per-key token authentication.
    /// If false (default), global token authentication is accepted.
    #[serde(default = "default_key_token_enable")]
    pub key_token_enable: bool,
}

impl IdConfig for FormattedConfig {
    fn name(&self) -> &str {
        &self.name
    }

    fn id_type() -> IdType {
        IdType::Formatted
    }

    fn validate(&self) -> Result<(), String> {
        if self.name.is_empty() {
            return Err("name cannot be empty".to_string());
        }
        if self.name.len() > 255 {
            return Err("name cannot exceed 255 characters".to_string());
        }
        if is_reserved_key_name(&self.name) {
            return Err("name cannot start or end with '__' (reserved)".to_string());
        }
        if self.pattern.is_empty() {
            return Err("pattern cannot be empty".to_string());
        }

        // Validate pattern syntax
        validate_pattern(&self.pattern)?;

        Ok(())
    }
}

impl Default for FormattedConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            pattern: String::new(),
            sequence_reset: SequenceReset::Never,
            key_token_enable: false,
        }
    }
}

/// Validate a pattern string.
fn validate_pattern(pattern: &str) -> Result<(), String> {
    let mut chars = pattern.chars();
    let mut has_sequence = false;

    while let Some(c) = chars.next() {
        if c == '{' {
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
                return Err("unclosed placeholder in pattern".to_string());
            }

            // Validate placeholder
            if !is_valid_placeholder(&placeholder) {
                return Err(format!("invalid placeholder: {{{placeholder}}}"));
            }

            if placeholder.starts_with("SEQ:") {
                has_sequence = true;
            }
        }
    }

    if !has_sequence && !pattern.contains("{UUID}") && !pattern.contains("{RAND:") {
        return Err(
            "pattern must contain at least one of: {{SEQ:N}}, {{UUID}}, or {{RAND:N}}".to_string(),
        );
    }

    Ok(())
}

/// Check if a placeholder is valid.
fn is_valid_placeholder(placeholder: &str) -> bool {
    match placeholder {
        "YYYY" | "YY" | "MM" | "DD" | "HH" | "mm" | "ss" | "UUID" => true,
        _ => {
            // Check for SEQ:N or RAND:N format
            placeholder
                .strip_prefix("SEQ:")
                .map(|n_str| n_str.parse::<u8>().is_ok_and(|n| n > 0 && n <= 20))
                .or_else(|| {
                    placeholder
                        .strip_prefix("RAND:")
                        .map(|n_str| n_str.parse::<u8>().is_ok_and(|n| n > 0 && n <= 32))
                })
                .unwrap_or(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_increment_config_validation() {
        let mut config = IncrementConfig {
            name: "test".to_string(),
            start: 1,
            step: 1,
            min: 1,
            max: 100,
            key_token_enable: false,
        };
        assert!(config.validate().is_ok());

        config.step = 0;
        assert!(config.validate().is_err());

        config.step = 1;
        config.min = 100;
        config.max = 1;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_snowflake_config_validation() {
        let mut config = SnowflakeConfig {
            name: "test".to_string(),
            epoch: 1_704_067_200_000,
            worker_bits: 10,
            sequence_bits: 12,
            key_token_enable: false,
        };
        assert!(config.validate().is_ok());

        // Exceeds 22 bits
        config.worker_bits = 15;
        config.sequence_bits = 10;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_formatted_config_validation() {
        let config = FormattedConfig {
            name: "invoice".to_string(),
            pattern: "INV{YYYY}{MM}{DD}-{SEQ:4}".to_string(),
            sequence_reset: SequenceReset::Daily,
            key_token_enable: false,
        };
        assert!(config.validate().is_ok());

        let config = FormattedConfig {
            name: "test".to_string(),
            pattern: "PREFIX-{UUID}".to_string(),
            sequence_reset: SequenceReset::Never,
            key_token_enable: false,
        };
        assert!(config.validate().is_ok());

        let config = FormattedConfig {
            name: "test".to_string(),
            pattern: "NO-UNIQUE-PART".to_string(),
            sequence_reset: SequenceReset::Never,
            key_token_enable: false,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_reserved_name_validation() {
        // IncrementConfig with reserved name
        let config = IncrementConfig {
            name: "__global__".to_string(),
            ..Default::default()
        };
        let err = config.validate().unwrap_err();
        assert!(err.contains("reserved"));

        let config = IncrementConfig {
            name: "__reserved".to_string(),
            ..Default::default()
        };
        assert!(config.validate().is_err());

        let config = IncrementConfig {
            name: "reserved__".to_string(),
            ..Default::default()
        };
        assert!(config.validate().is_err());

        // SnowflakeConfig with reserved name
        let config = SnowflakeConfig {
            name: "__test__".to_string(),
            ..Default::default()
        };
        assert!(config.validate().is_err());

        // FormattedConfig with reserved name
        let config = FormattedConfig {
            name: "__test__".to_string(),
            pattern: "{UUID}".to_string(),
            ..Default::default()
        };
        assert!(config.validate().is_err());

        // Valid names with underscores (but not double underscore prefix/suffix)
        let config = IncrementConfig {
            name: "my_key".to_string(),
            ..Default::default()
        };
        assert!(config.validate().is_ok());

        let config = IncrementConfig {
            name: "_single".to_string(),
            ..Default::default()
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_pattern_validation() {
        assert!(validate_pattern("INV{YYYY}{MM}{DD}-{SEQ:4}").is_ok());
        assert!(validate_pattern("{UUID}").is_ok());
        assert!(validate_pattern("ID-{RAND:8}").is_ok());
        assert!(validate_pattern("{SEQ:1}").is_ok());
        assert!(validate_pattern("{INVALID}").is_err());
        assert!(validate_pattern("{SEQ:0}").is_err());
        assert!(validate_pattern("{SEQ:}").is_err());
        assert!(validate_pattern("no-placeholder").is_err());
        assert!(validate_pattern("{UNCLOSED").is_err());
    }
}
