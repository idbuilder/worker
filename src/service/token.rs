//! Token service for authentication.
//!
//! Implements two-tier authentication:
//! - Admin tokens: Full access to configuration APIs
//! - Key tokens: Limited to ID generation APIs
//!
//! Key tokens are associated with a key name and can be retrieved or reset
//! using the key name. Tokens are 64 characters of URL-safe base64.
//!
//! ## Global Token
//!
//! A special global key token (`__global__`) can be created to authenticate
//! against any config that has `key_token_enable: false` (the default).

use std::collections::HashMap;
use std::time::Duration;

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use rand::Rng;

use crate::config::AuthConfig;

/// Token length in characters (64 base64 chars = 48 bytes).
const TOKEN_BYTES: usize = 48;

/// Reserved key name for the global token.
///
/// The global token can authenticate against any config that has
/// `key_token_enable: false` (the default).
pub const GLOBAL_TOKEN_KEY: &str = "__global__";

/// Check if a key name is reserved (starts or ends with `__`).
///
/// Reserved key names cannot be used for user-defined configurations.
#[must_use]
pub fn is_reserved_key_name(name: &str) -> bool {
    name.starts_with("__") || name.ends_with("__")
}

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
    /// Token ID (the actual token string, 64 characters).
    pub token: String,
    /// Associated key name.
    pub key: String,
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
    #[must_use]
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    /// Check if the token is valid (not expired).
    #[must_use]
    pub fn is_valid(&self) -> bool {
        !self.is_expired()
    }
}

/// Token storage (in-memory for standalone mode).
struct TokenStore {
    /// Stored tokens indexed by token string.
    tokens: RwLock<HashMap<String, TokenInfo>>,
    /// Mapping from key name to token string for quick lookup.
    key_to_token: RwLock<HashMap<String, String>>,
}

impl TokenStore {
    fn new() -> Self {
        Self {
            tokens: RwLock::new(HashMap::new()),
            key_to_token: RwLock::new(HashMap::new()),
        }
    }

    fn insert(&self, token_info: TokenInfo) {
        let key = token_info.key.clone();
        let token = token_info.token.clone();
        self.tokens.write().insert(token.clone(), token_info);
        self.key_to_token.write().insert(key, token);
    }

    fn get(&self, token: &str) -> Option<TokenInfo> {
        let tokens = self.tokens.read();
        tokens.get(token).cloned()
    }

    fn get_by_key(&self, key: &str) -> Option<TokenInfo> {
        let token = {
            let key_to_token = self.key_to_token.read();
            key_to_token.get(key)?.clone()
        };
        let tokens = self.tokens.read();
        tokens.get(&token).cloned()
    }

    fn remove(&self, token: &str) -> bool {
        let mut tokens = self.tokens.write();
        let mut key_to_token = self.key_to_token.write();
        if let Some(info) = tokens.remove(token) {
            key_to_token.remove(&info.key);
            true
        } else {
            false
        }
    }

    fn remove_by_key(&self, key: &str) -> bool {
        let mut key_to_token = self.key_to_token.write();
        let mut tokens = self.tokens.write();
        key_to_token.remove(key).is_some_and(|token| {
            tokens.remove(&token);
            true
        })
    }

    fn cleanup_expired(&self) {
        let mut tokens = self.tokens.write();
        let expired_keys: Vec<String> = tokens
            .iter()
            .filter(|(_, info)| !info.is_valid())
            .map(|(_, info)| info.key.clone())
            .collect();
        for key in &expired_keys {
            self.key_to_token.write().remove(key);
        }
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
    #[must_use]
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
        if let Some(info) = self.store.get(token)
            && info.is_valid()
        {
            return Some(info.token_type);
        }

        None
    }

    /// Get token info if valid.
    pub fn get_token_info(&self, token: &str) -> Option<TokenInfo> {
        // Admin token has special handling
        if token == self.admin_token {
            return Some(TokenInfo {
                token: token.to_string(),
                key: String::new(),
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

    /// Get the key associated with a token.
    ///
    /// Returns `None` if the token is invalid, expired, or is the admin token.
    pub fn get_token_key(&self, token: &str) -> Option<String> {
        // Admin token doesn't have an associated key
        if token == self.admin_token {
            return None;
        }

        let info = self.store.get(token)?;
        if info.is_valid() {
            Some(info.key)
        } else {
            None
        }
    }

    /// Get token by key name. Returns None if not found or expired.
    pub fn get_token_by_key(&self, key: &str) -> Option<TokenInfo> {
        let info = self.store.get_by_key(key)?;
        if info.is_valid() { Some(info) } else { None }
    }

    /// Get or create a token for the given key.
    ///
    /// If a valid token exists for the key, returns it.
    /// Otherwise, creates a new token and returns it.
    pub fn get_or_create_token(&self, key: &str) -> TokenInfo {
        // Check if a valid token already exists
        if let Some(info) = self.get_token_by_key(key) {
            return info;
        }

        // Create a new token
        self.create_token_for_key(key)
    }

    /// Reset (regenerate) the token for a given key.
    ///
    /// Removes the existing token (if any) and creates a new one.
    pub fn reset_token(&self, key: &str) -> TokenInfo {
        // Remove existing token for this key
        self.store.remove_by_key(key);

        // Create a new token
        self.create_token_for_key(key)
    }

    /// Create a new token for a key (internal helper).
    fn create_token_for_key(&self, key: &str) -> TokenInfo {
        let token = generate_token();
        let now = Utc::now();
        let expires_at =
            now + chrono::Duration::from_std(self.key_token_expiration).unwrap_or_default();

        let info = TokenInfo {
            token,
            key: key.to_string(),
            token_type: TokenType::Key,
            description: format!("Token for key: {key}"),
            created_at: now,
            expires_at,
            permissions: vec![key.to_string()],
        };

        self.store.insert(info.clone());

        info
    }

    /// Generate a new key token (legacy method for backwards compatibility).
    ///
    /// # Arguments
    ///
    /// * `key` - The key name to associate with this token
    /// * `description` - Human-readable description
    /// * `expires_in` - Optional custom expiration (uses default if None)
    /// * `permissions` - Optional permissions for the token
    ///
    /// # Returns
    ///
    /// The generated token info.
    pub fn generate_key_token(
        &self,
        key: String,
        description: String,
        expires_in: Option<Duration>,
        permissions: Vec<String>,
    ) -> TokenInfo {
        // Remove any existing token for this key
        self.store.remove_by_key(&key);

        let token = generate_token();
        let now = Utc::now();
        let expiration = expires_in.unwrap_or(self.key_token_expiration);
        let expires_at = now + chrono::Duration::from_std(expiration).unwrap_or_default();

        let info = TokenInfo {
            token,
            key,
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

/// Generate a random 64-character URL-safe base64 token string.
fn generate_token() -> String {
    let mut rng = rand::rng();
    let mut bytes = [0u8; TOKEN_BYTES];
    rng.fill(&mut bytes);

    URL_SAFE_NO_PAD.encode(bytes)
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
            "order-id".to_string(),
            "Test token".to_string(),
            None,
            vec!["increment".to_string()],
        );

        // Token should be 64 characters of base64
        assert_eq!(info.token.len(), 64);
        assert_eq!(info.key, "order-id");
        assert_eq!(service.validate(&info.token), Some(TokenType::Key));
    }

    #[test]
    fn test_token_length() {
        let token = generate_token();
        assert_eq!(token.len(), 64);
        // Verify it's valid base64
        assert!(URL_SAFE_NO_PAD.decode(&token).is_ok());
    }

    #[test]
    fn test_get_or_create_token() {
        let service = create_test_service();

        // First call should create
        let info1 = service.get_or_create_token("my-key");
        assert_eq!(info1.key, "my-key");
        assert_eq!(info1.token.len(), 64);

        // Second call should return the same token
        let info2 = service.get_or_create_token("my-key");
        assert_eq!(info1.token, info2.token);

        // Different key should create different token
        let info3 = service.get_or_create_token("other-key");
        assert_ne!(info1.token, info3.token);
    }

    #[test]
    fn test_reset_token() {
        let service = create_test_service();

        // Create initial token
        let info1 = service.get_or_create_token("reset-key");
        let original_token = info1.token;

        // Reset should create a new token
        let info2 = service.reset_token("reset-key");
        assert_ne!(original_token, info2.token);
        assert_eq!(info2.key, "reset-key");

        // Old token should be invalid
        assert!(service.validate(&original_token).is_none());
        // New token should be valid
        assert_eq!(service.validate(&info2.token), Some(TokenType::Key));
    }

    #[test]
    fn test_get_token_by_key() {
        let service = create_test_service();

        // Should return None for non-existent key
        assert!(service.get_token_by_key("nonexistent").is_none());

        // Create a token
        let info = service.get_or_create_token("lookup-key");

        // Should find it by key
        let found = service.get_token_by_key("lookup-key").unwrap();
        assert_eq!(found.token, info.token);
    }

    #[test]
    fn test_revoke_token() {
        let service = create_test_service();

        let info =
            service.generate_key_token("revoke-key".to_string(), "Test".to_string(), None, vec![]);

        assert!(service.validate(&info.token).is_some());
        assert!(service.revoke(&info.token));
        assert!(service.validate(&info.token).is_none());
        // Key lookup should also fail
        assert!(service.get_token_by_key("revoke-key").is_none());
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
            "expired-key".to_string(),
            "Expiring".to_string(),
            Some(Duration::from_secs(0)),
            vec![],
        );

        // Should be expired immediately
        std::thread::sleep(Duration::from_millis(10));
        assert!(service.validate(&info.token).is_none());
        // get_token_by_key should also return None for expired tokens
        assert!(service.get_token_by_key("expired-key").is_none());
    }

    #[test]
    fn test_is_reserved_key_name() {
        // Reserved names (start or end with __)
        assert!(is_reserved_key_name("__global__"));
        assert!(is_reserved_key_name("__reserved"));
        assert!(is_reserved_key_name("reserved__"));
        assert!(is_reserved_key_name("__"));

        // Non-reserved names
        assert!(!is_reserved_key_name("normal-key"));
        assert!(!is_reserved_key_name("my_key"));
        assert!(!is_reserved_key_name("key_with_underscores"));
        assert!(!is_reserved_key_name("_single"));
        assert!(!is_reserved_key_name("single_"));
        assert!(!is_reserved_key_name(""));
    }

    #[test]
    fn test_get_token_key() {
        let service = create_test_service();

        // Admin token returns None
        assert!(service.get_token_key("test_admin_token").is_none());

        // Invalid token returns None
        assert!(service.get_token_key("invalid_token").is_none());

        // Create a key token
        let info = service.get_or_create_token("my-key");
        assert_eq!(
            service.get_token_key(&info.token),
            Some("my-key".to_string())
        );

        // Test with global token
        let global_info = service.get_or_create_token(GLOBAL_TOKEN_KEY);
        assert_eq!(
            service.get_token_key(&global_info.token),
            Some(GLOBAL_TOKEN_KEY.to_string())
        );
    }
}
