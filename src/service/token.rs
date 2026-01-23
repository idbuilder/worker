//! Token service for authentication.
//!
//! Implements two-tier authentication:
//! - Admin tokens: Full access to configuration APIs
//! - Key tokens: Limited to ID generation APIs

use std::collections::HashMap;
use std::time::Duration;

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use rand::Rng;

use crate::config::AuthConfig;

/// Token type for access control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenType {
    /// Admin token with full configuration access.
    Admin,
    /// Key token for ID generation only.
    Key,
}

impl std::fmt::Display for TokenType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Admin => write!(f, "admin"),
            Self::Key => write!(f, "key"),
        }
    }
}

/// Token metadata.
#[derive(Debug, Clone)]
pub struct TokenInfo {
    /// Token ID (the actual token string).
    pub token: String,
    /// Token type.
    pub token_type: TokenType,
    /// Description/label for the token.
    pub description: String,
    /// When the token was created.
    pub created_at: DateTime<Utc>,
    /// When the token expires.
    pub expires_at: DateTime<Utc>,
    /// Permissions granted to this token.
    pub permissions: Vec<String>,
}

impl TokenInfo {
    /// Check if the token is expired.
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    /// Check if the token is valid (not expired).
    pub fn is_valid(&self) -> bool {
        !self.is_expired()
    }
}

/// Token storage (in-memory for standalone mode).
struct TokenStore {
    /// Stored tokens indexed by token string.
    tokens: RwLock<HashMap<String, TokenInfo>>,
}

impl TokenStore {
    fn new() -> Self {
        Self {
            tokens: RwLock::new(HashMap::new()),
        }
    }

    fn insert(&self, token_info: TokenInfo) {
        let mut tokens = self.tokens.write();
        tokens.insert(token_info.token.clone(), token_info);
    }

    fn get(&self, token: &str) -> Option<TokenInfo> {
        let tokens = self.tokens.read();
        tokens.get(token).cloned()
    }

    fn remove(&self, token: &str) -> bool {
        let mut tokens = self.tokens.write();
        tokens.remove(token).is_some()
    }

    fn cleanup_expired(&self) {
        let mut tokens = self.tokens.write();
        tokens.retain(|_, info| info.is_valid());
    }
}

/// Token service for authentication and authorization.
pub struct TokenService {
    /// Admin token (from configuration).
    admin_token: String,
    /// Default expiration for key tokens.
    key_token_expiration: Duration,
    /// Token storage.
    store: TokenStore,
}

impl TokenService {
    /// Create a new token service.
    pub fn new(config: &AuthConfig) -> Self {
        Self {
            admin_token: config.admin_token.clone(),
            key_token_expiration: Duration::from_secs(config.key_token_expiration),
            store: TokenStore::new(),
        }
    }

    /// Validate a token and return its type if valid.
    ///
    /// # Arguments
    ///
    /// * `token` - The token string to validate
    ///
    /// # Returns
    ///
    /// `Some(TokenType)` if valid, `None` if invalid or expired.
    pub fn validate(&self, token: &str) -> Option<TokenType> {
        // Check if it's the admin token
        if token == self.admin_token {
            return Some(TokenType::Admin);
        }

        // Check if it's a generated key token
        if let Some(info) = self.store.get(token) {
            if info.is_valid() {
                return Some(info.token_type);
            }
        }

        None
    }

    /// Get token info if valid.
    pub fn get_token_info(&self, token: &str) -> Option<TokenInfo> {
        // Admin token has special handling
        if token == self.admin_token {
            return Some(TokenInfo {
                token: token.to_string(),
                token_type: TokenType::Admin,
                description: "Admin token".to_string(),
                created_at: DateTime::from_timestamp(0, 0).unwrap_or_else(Utc::now),
                expires_at: DateTime::from_timestamp(i64::MAX / 2, 0).unwrap_or_else(Utc::now),
                permissions: vec!["*".to_string()],
            });
        }

        let info = self.store.get(token)?;
        if info.is_valid() { Some(info) } else { None }
    }

    /// Generate a new key token.
    ///
    /// # Arguments
    ///
    /// * `description` - Human-readable description
    /// * `expires_in` - Optional custom expiration (uses default if None)
    /// * `permissions` - Optional permissions for the token
    ///
    /// # Returns
    ///
    /// The generated token info.
    pub fn generate_key_token(
        &self,
        description: String,
        expires_in: Option<Duration>,
        permissions: Vec<String>,
    ) -> TokenInfo {
        let token = generate_token("key");
        let now = Utc::now();
        let expiration = expires_in.unwrap_or(self.key_token_expiration);
        let expires_at = now + chrono::Duration::from_std(expiration).unwrap_or_default();

        let info = TokenInfo {
            token: token.clone(),
            token_type: TokenType::Key,
            description,
            created_at: now,
            expires_at,
            permissions,
        };

        self.store.insert(info.clone());

        info
    }

    /// Revoke a token.
    ///
    /// # Arguments
    ///
    /// * `token` - The token to revoke
    ///
    /// # Returns
    ///
    /// `true` if the token was revoked, `false` if it didn't exist.
    pub fn revoke(&self, token: &str) -> bool {
        // Cannot revoke the admin token
        if token == self.admin_token {
            return false;
        }

        self.store.remove(token)
    }

    /// Clean up expired tokens.
    pub fn cleanup(&self) {
        self.store.cleanup_expired();
    }
}

/// Generate a random token string.
fn generate_token(prefix: &str) -> String {
    let mut rng = rand::rng();
    let mut bytes = [0u8; 24];
    rng.fill(&mut bytes);

    let encoded = URL_SAFE_NO_PAD.encode(bytes);
    format!("{}_{}", prefix, encoded)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_service() -> TokenService {
        let config = AuthConfig {
            admin_token: "test_admin_token".to_string(),
            key_token_expiration: 3600, // 1 hour
        };
        TokenService::new(&config)
    }

    #[test]
    fn test_validate_admin_token() {
        let service = create_test_service();

        assert_eq!(service.validate("test_admin_token"), Some(TokenType::Admin));
    }

    #[test]
    fn test_validate_invalid_token() {
        let service = create_test_service();

        assert_eq!(service.validate("invalid_token"), None);
    }

    #[test]
    fn test_generate_and_validate_key_token() {
        let service = create_test_service();

        let info = service.generate_key_token(
            "Test token".to_string(),
            None,
            vec!["increment".to_string()],
        );

        assert!(info.token.starts_with("key_"));
        assert_eq!(service.validate(&info.token), Some(TokenType::Key));
    }

    #[test]
    fn test_revoke_token() {
        let service = create_test_service();

        let info = service.generate_key_token("Test".to_string(), None, vec![]);

        assert!(service.validate(&info.token).is_some());
        assert!(service.revoke(&info.token));
        assert!(service.validate(&info.token).is_none());
    }

    #[test]
    fn test_cannot_revoke_admin_token() {
        let service = create_test_service();

        assert!(!service.revoke("test_admin_token"));
        assert!(service.validate("test_admin_token").is_some());
    }

    #[test]
    fn test_expired_token() {
        let service = create_test_service();

        // Generate token with very short expiration
        let info = service.generate_key_token(
            "Expiring".to_string(),
            Some(Duration::from_secs(0)),
            vec![],
        );

        // Should be expired immediately
        std::thread::sleep(Duration::from_millis(10));
        assert!(service.validate(&info.token).is_none());
    }
}
