//! Data Transfer Objects for API requests and responses.

use serde::{Deserialize, Serialize};

/// Standard API response wrapper.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    /// Response code (0 = success, non-zero = error).
    pub code: i32,

    /// Human-readable message.
    pub message: String,

    /// Response data (null on error).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
}

impl<T> ApiResponse<T> {
    /// Create a success response.
    pub fn success(data: T) -> Self {
        Self {
            code: 0,
            message: "success".to_string(),
            data: Some(data),
        }
    }

    /// Create an error response.
    pub fn error(code: i32, message: impl Into<String>) -> ApiResponse<()> {
        ApiResponse {
            code,
            message: message.into(),
            data: None,
        }
    }
}

impl ApiResponse<()> {
    /// Create a success response with no data.
    #[must_use]
    pub fn ok() -> Self {
        Self {
            code: 0,
            message: "success".to_string(),
            data: None,
        }
    }
}

/// Request to generate IDs.
#[derive(Debug, Clone, Deserialize)]
pub struct GenerateRequest {
    /// Configuration name.
    pub name: String,

    /// Number of IDs to generate (default: 1, max: 1000).
    #[serde(default = "default_count")]
    pub count: u32,
}

fn default_count() -> u32 {
    1
}

impl GenerateRequest {
    /// Validate the request.
    pub fn validate(&self) -> Result<(), String> {
        if self.name.is_empty() {
            return Err("name is required".to_string());
        }
        if self.count == 0 {
            return Err("count must be at least 1".to_string());
        }
        if self.count > 1000 {
            return Err("count cannot exceed 1000".to_string());
        }
        Ok(())
    }
}

/// Generic ID response with a list of generated IDs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdResponse<T> {
    /// List of generated IDs.
    pub ids: Vec<T>,
}

impl<T> IdResponse<T> {
    /// Create a new ID response.
    pub fn new(ids: Vec<T>) -> Self {
        Self { ids }
    }
}

/// Response for increment ID generation.
pub type IncrementIdResponse = IdResponse<i64>;

/// Response for formatted ID generation.
pub type FormattedIdResponse = IdResponse<String>;

/// Response for snowflake configuration request.
///
/// Contains all parameters needed for client-side ID generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnowflakeIdResponse {
    /// Allocated worker ID for this client.
    pub worker_id: u32,

    /// Custom epoch timestamp in milliseconds.
    pub epoch: i64,

    /// Number of bits for worker ID.
    pub worker_bits: u8,

    /// Number of bits for sequence number.
    pub sequence_bits: u8,
}

/// Response containing configuration details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigResponse<T> {
    /// The configuration.
    pub config: T,

    /// When the configuration was created.
    pub created_at: String,
}

impl<T> ConfigResponse<T> {
    /// Create a new config response.
    pub fn new(config: T, created_at: String) -> Self {
        Self { config, created_at }
    }
}

/// Request to create a new token.
#[derive(Debug, Clone, Deserialize)]
pub struct TokenRequest {
    /// Token description/label.
    #[serde(default)]
    pub description: String,

    /// Expiration time in seconds (default: 30 days).
    #[serde(default = "default_expiration")]
    pub expires_in: u64,

    /// Permissions for this token.
    #[serde(default)]
    pub permissions: Vec<String>,
}

fn default_expiration() -> u64 {
    30 * 24 * 60 * 60 // 30 days
}

/// Response containing token information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenResponse {
    /// The generated token.
    pub token: String,

    /// Token type (always "key" for generated tokens).
    pub token_type: String,

    /// Token expiration timestamp (ISO 8601).
    pub expires_at: String,
}

/// Health check response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    /// Service status.
    pub status: String,

    /// Service version.
    pub version: String,
}

/// Readiness check response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadyResponse {
    /// Overall readiness status.
    pub ready: bool,

    /// Individual component statuses.
    pub components: ReadyComponents,
}

/// Component readiness statuses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadyComponents {
    /// Storage backend status.
    pub storage: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_response_success() {
        let response = ApiResponse::success(vec![1, 2, 3]);
        assert_eq!(response.code, 0);
        assert_eq!(response.message, "success");
        assert_eq!(response.data, Some(vec![1, 2, 3]));
    }

    #[test]
    fn test_api_response_error() {
        let response = ApiResponse::<()>::error(1001, "config not found");
        assert_eq!(response.code, 1001);
        assert_eq!(response.message, "config not found");
        assert!(response.data.is_none());
    }

    #[test]
    fn test_generate_request_validation() {
        let req = GenerateRequest {
            name: "test".to_string(),
            count: 10,
        };
        assert!(req.validate().is_ok());

        let req = GenerateRequest {
            name: "".to_string(),
            count: 10,
        };
        assert!(req.validate().is_err());

        let req = GenerateRequest {
            name: "test".to_string(),
            count: 0,
        };
        assert!(req.validate().is_err());

        let req = GenerateRequest {
            name: "test".to_string(),
            count: 1001,
        };
        assert!(req.validate().is_err());
    }
}
