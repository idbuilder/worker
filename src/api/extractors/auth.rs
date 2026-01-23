//! Authentication context extractor.

use axum::{extract::FromRequestParts, http::request::Parts};
use std::future::Future;

use crate::error::AppError;
use crate::service::TokenType;

/// Authentication context extracted from request.
#[derive(Debug, Clone)]
pub struct AuthContext {
    /// Token type (Admin or Key).
    pub token_type: TokenType,
    /// The token string.
    pub token: String,
}

impl AuthContext {
    /// Create a new auth context.
    #[must_use]
    pub const fn new(token_type: TokenType, token: String) -> Self {
        Self { token_type, token }
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
