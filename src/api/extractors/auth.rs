//! Authentication context extractor.

use axum::{extract::FromRequestParts, http::request::Parts};
use std::future::Future;

use crate::error::AppError;
use crate::service::{GLOBAL_TOKEN_KEY, TokenType};

/// Authentication context extracted from request.
#[derive(Debug, Clone)]
pub struct AuthContext {
    /// Token type (Admin or Key).
    pub token_type: TokenType,
    /// The token string.
    pub token: String,
    /// The key this token is authorized for (empty for admin tokens).
    pub token_key: String,
}

impl AuthContext {
    /// Create a new auth context.
    #[must_use]
    pub const fn new(token_type: TokenType, token: String, token_key: String) -> Self {
        Self {
            token_type,
            token,
            token_key,
        }
    }

    /// Check if this is an admin token.
    #[must_use]
    pub fn is_admin(&self) -> bool {
        self.token_type == TokenType::Admin
    }

    /// Check if this is a key token.
    #[must_use]
    pub fn is_key(&self) -> bool {
        self.token_type == TokenType::Key
    }

    /// Check if this token can access the given key.
    ///
    /// # Arguments
    ///
    /// * `key` - The key name to check access for.
    /// * `key_token_enable` - Whether the config requires per-key token authentication.
    ///
    /// # Returns
    ///
    /// `true` if the token can access the key, `false` otherwise.
    #[must_use]
    pub fn can_access_key(&self, key: &str, key_token_enable: bool) -> bool {
        // Admin token can access anything
        if self.is_admin() {
            return true;
        }

        // If key_token_enable is false, global token is accepted
        if !key_token_enable && self.token_key == GLOBAL_TOKEN_KEY {
            return true;
        }

        // Otherwise, token must be for this specific key
        self.token_key == key
    }
}

impl<S> FromRequestParts<S> for AuthContext
where
    S: Send + Sync,
{
    type Rejection = AppError;

    fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        // Get auth context from extensions (set by auth middleware)
        let result = parts
            .extensions
            .get::<Self>()
            .cloned()
            .ok_or(AppError::Unauthorized);
        std::future::ready(result)
    }
}
