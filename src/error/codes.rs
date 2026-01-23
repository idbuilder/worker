//! Error code constants.
//!
//! Error codes are organized by category:
//! - 1xxx: Configuration errors
//! - 2xxx: Authentication/Authorization errors
//! - 3xxx: Validation errors
//! - 4xxx: Resource errors
//! - 5xxx: Internal/System errors

/// Error code type with semantic categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ErrorCode(i32);

impl ErrorCode {
    // ===== Configuration Errors (1xxx) =====

    /// Configuration not found.
    pub const CONFIG_NOT_FOUND: Self = Self(1001);

    /// Configuration already exists.
    pub const CONFIG_EXISTS: Self = Self(1002);

    /// Invalid configuration parameters.
    pub const INVALID_CONFIG: Self = Self(1003);

    /// Sequence exhausted (reached limit).
    pub const SEQUENCE_EXHAUSTED: Self = Self(1004);

    // ===== Authentication/Authorization Errors (2xxx) =====

    /// Authentication required.
    pub const UNAUTHORIZED: Self = Self(2001);

    /// Insufficient permissions.
    pub const FORBIDDEN: Self = Self(2002);

    /// Invalid token.
    pub const INVALID_TOKEN: Self = Self(2003);

    /// Token expired.
    pub const TOKEN_EXPIRED: Self = Self(2004);

    // ===== Validation Errors (3xxx) =====

    /// Bad request / invalid parameters.
    pub const BAD_REQUEST: Self = Self(3001);

    /// Missing required parameter.
    pub const MISSING_PARAM: Self = Self(3002);

    /// Invalid parameter value.
    pub const INVALID_PARAM: Self = Self(3003);

    // ===== Resource Errors (4xxx) =====

    /// Resource not found.
    pub const NOT_FOUND: Self = Self(4001);

    /// Rate limit exceeded.
    pub const RATE_LIMITED: Self = Self(4002);

    // ===== Internal/System Errors (5xxx) =====

    /// Storage backend error.
    pub const STORAGE_ERROR: Self = Self(5001);

    /// Internal server error.
    pub const INTERNAL_ERROR: Self = Self(5002);

    /// Service unavailable.
    pub const SERVICE_UNAVAILABLE: Self = Self(5003);

    /// Get the error code as an i32.
    #[must_use]
    pub const fn as_i32(self) -> i32 {
        self.0
    }

    /// Get the error code as a u32.
    #[must_use]
    #[allow(clippy::cast_sign_loss)]
    pub const fn as_u32(self) -> u32 {
        self.0 as u32
    }

    /// Get the category of this error code.
    #[must_use]
    pub const fn category(&self) -> ErrorCategory {
        match self.0 {
            1000..=1999 => ErrorCategory::Configuration,
            2000..=2999 => ErrorCategory::Authentication,
            3000..=3999 => ErrorCategory::Validation,
            4000..=4999 => ErrorCategory::Resource,
            5000..=5999 => ErrorCategory::Internal,
            _ => ErrorCategory::Unknown,
        }
    }
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<ErrorCode> for i32 {
    fn from(code: ErrorCode) -> Self {
        code.0
    }
}

/// Error category based on error code range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCategory {
    /// Configuration-related errors (1xxx).
    Configuration,
    /// Authentication/authorization errors (2xxx).
    Authentication,
    /// Validation errors (3xxx).
    Validation,
    /// Resource errors (4xxx).
    Resource,
    /// Internal/system errors (5xxx).
    Internal,
    /// Unknown category.
    Unknown,
}

impl std::fmt::Display for ErrorCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Configuration => write!(f, "configuration"),
            Self::Authentication => write!(f, "authentication"),
            Self::Validation => write!(f, "validation"),
            Self::Resource => write!(f, "resource"),
            Self::Internal => write!(f, "internal"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_code_values() {
        assert_eq!(ErrorCode::CONFIG_NOT_FOUND.as_i32(), 1001);
        assert_eq!(ErrorCode::UNAUTHORIZED.as_i32(), 2001);
        assert_eq!(ErrorCode::BAD_REQUEST.as_i32(), 3001);
        assert_eq!(ErrorCode::NOT_FOUND.as_i32(), 4001);
        assert_eq!(ErrorCode::INTERNAL_ERROR.as_i32(), 5002);
    }

    #[test]
    fn test_error_categories() {
        assert_eq!(
            ErrorCode::CONFIG_NOT_FOUND.category(),
            ErrorCategory::Configuration
        );
        assert_eq!(
            ErrorCode::UNAUTHORIZED.category(),
            ErrorCategory::Authentication
        );
        assert_eq!(ErrorCode::BAD_REQUEST.category(), ErrorCategory::Validation);
        assert_eq!(ErrorCode::NOT_FOUND.category(), ErrorCategory::Resource);
        assert_eq!(
            ErrorCode::INTERNAL_ERROR.category(),
            ErrorCategory::Internal
        );
    }
}
