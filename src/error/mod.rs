//! Error handling module.
//!
//! This module provides unified error handling with proper HTTP status code mapping
//! and standardized API error responses.

pub mod codes;

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

pub use codes::ErrorCode;

/// Application-level error type.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// Configuration not found.
    #[error("Configuration not found: {0}")]
    ConfigNotFound(String),

    /// Configuration already exists.
    #[error("Configuration already exists: {0}")]
    ConfigExists(String),

    /// Invalid configuration.
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    /// Sequence exhausted (reached max value).
    #[error("Sequence exhausted for: {0}")]
    SequenceExhausted(String),

    /// Authentication failed.
    #[error("Authentication failed")]
    Unauthorized,

    /// Insufficient permissions.
    #[error("Insufficient permissions")]
    Forbidden,

    /// Invalid request parameters.
    #[error("Invalid request: {0}")]
    BadRequest(String),

    /// Resource not found.
    #[error("Resource not found: {0}")]
    NotFound(String),

    /// Rate limit exceeded.
    #[error("Rate limit exceeded")]
    RateLimited,

    /// Storage backend error.
    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),

    /// Internal server error.
    #[error("Internal error: {0}")]
    Internal(String),
}

impl AppError {
    /// Get the error code for this error.
    #[must_use]
    pub const fn error_code(&self) -> ErrorCode {
        match self {
            Self::ConfigNotFound(_) => ErrorCode::CONFIG_NOT_FOUND,
            Self::ConfigExists(_) => ErrorCode::CONFIG_EXISTS,
            Self::InvalidConfig(_) => ErrorCode::INVALID_CONFIG,
            Self::SequenceExhausted(_) => ErrorCode::SEQUENCE_EXHAUSTED,
            Self::Unauthorized => ErrorCode::UNAUTHORIZED,
            Self::Forbidden => ErrorCode::FORBIDDEN,
            Self::BadRequest(_) => ErrorCode::BAD_REQUEST,
            Self::NotFound(_) => ErrorCode::NOT_FOUND,
            Self::RateLimited => ErrorCode::RATE_LIMITED,
            Self::Storage(_) => ErrorCode::STORAGE_ERROR,
            Self::Internal(_) => ErrorCode::INTERNAL_ERROR,
        }
    }

    /// Get the HTTP status code for this error.
    #[must_use]
    pub const fn status_code(&self) -> StatusCode {
        match self {
            Self::ConfigNotFound(_) | Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::ConfigExists(_) => StatusCode::CONFLICT,
            Self::InvalidConfig(_) | Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::SequenceExhausted(_) => StatusCode::SERVICE_UNAVAILABLE,
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::RateLimited => StatusCode::TOO_MANY_REQUESTS,
            Self::Storage(_) | Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let code = self.error_code().as_i32();
        let message = self.to_string();

        tracing::error!(
            error_code = code,
            status = %status,
            message = %message,
            "Request failed"
        );

        let body = Json(json!({
            "code": code,
            "message": message,
            "data": null
        }));

        (status, body).into_response()
    }
}

/// Storage-specific error type.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    /// Connection error.
    #[error("Connection failed: {0}")]
    Connection(String),

    /// Query execution error.
    #[error("Query failed: {0}")]
    Query(String),

    /// Serialization/deserialization error.
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Optimistic lock conflict.
    #[error("Concurrent modification detected")]
    ConcurrentModification,

    /// Lock acquisition failed.
    #[error("Failed to acquire lock: {0}")]
    LockFailed(String),

    /// Lock timeout.
    #[error("Lock timeout: {0}")]
    LockTimeout(String),

    /// File I/O error.
    #[error("File I/O error: {0}")]
    FileIO(String),

    /// Data not found.
    #[error("Data not found: {0}")]
    NotFound(String),

    /// Backend not available.
    #[error("Storage backend unavailable")]
    Unavailable,
}

impl From<std::io::Error> for StorageError {
    fn from(err: std::io::Error) -> Self {
        Self::FileIO(err.to_string())
    }
}

impl From<serde_json::Error> for StorageError {
    fn from(err: serde_json::Error) -> Self {
        Self::Serialization(err.to_string())
    }
}

/// Result type alias using `AppError`.
pub type Result<T> = std::result::Result<T, AppError>;

/// Result type alias using `StorageError`.
pub type StorageResult<T> = std::result::Result<T, StorageError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_codes() {
        assert_eq!(
            AppError::ConfigNotFound("test".to_string()).error_code(),
            ErrorCode::CONFIG_NOT_FOUND
        );
        assert_eq!(AppError::Unauthorized.error_code(), ErrorCode::UNAUTHORIZED);
        assert_eq!(
            AppError::Internal("test".to_string()).error_code(),
            ErrorCode::INTERNAL_ERROR
        );
    }

    #[test]
    fn test_status_codes() {
        assert_eq!(
            AppError::ConfigNotFound("test".to_string()).status_code(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            AppError::Unauthorized.status_code(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            AppError::RateLimited.status_code(),
            StatusCode::TOO_MANY_REQUESTS
        );
    }
}
