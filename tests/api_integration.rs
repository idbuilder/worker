//! Integration tests for IDBuilder Worker API.
//!
//! These tests spin up a real server instance and make HTTP requests to verify
//! the complete request/response cycle.

use std::net::SocketAddr;
use std::sync::Arc;

use reqwest::{Client, Response, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tempfile::TempDir;
use tokio::net::TcpListener;

use idbuilder_worker::api::{AppState, create_router};
use idbuilder_worker::config::{
    AdminConfig, AppConfig, AuthConfig, FileStorageConfig, ObservabilityConfig, SequenceConfig,
    ServerConfig, StorageBackend, StorageConfig,
};
use idbuilder_worker::storage::create_storage;

// ============================================================================
// Test Harness
// ============================================================================

/// Test server instance.
struct TestServer {
    addr: SocketAddr,
    client: Client,
    admin_token: String,
    _temp_dir: TempDir,
}

impl TestServer {
    async fn new() -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let admin_token = "test_admin_token_12345".to_string();

        let config = AppConfig {
            server: ServerConfig {
                host: "127.0.0.1".parse().unwrap(),
                port: 0,
                workers: 1,
            },
            storage: StorageConfig {
                backend: StorageBackend::File,
                file: FileStorageConfig {
                    data_dir: temp_dir.path().to_path_buf(),
                },
                ..Default::default()
            },
            controller: Default::default(),
            sequence: SequenceConfig {
                default_batch_size: 100,
                prefetch_threshold: 10,
            },
            auth: AuthConfig {
                admin_token: admin_token.clone(),
                key_token_expiration: 3600,
            },
            observability: ObservabilityConfig {
                log_level: "warn".to_string(),
                log_format: "text".to_string(),
                metrics_enabled: true,
                metrics_path: "/metrics".to_string(),
            },
            admin: AdminConfig {
                enabled: false,
                path: "./static".to_string(),
            },
        };

        let storage = create_storage(&config.storage)
            .await
            .expect("Failed to create storage");

        let state = AppState::new(Arc::new(config), storage);
        let app = create_router(state);

        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("Failed to bind");
        let addr = listener.local_addr().expect("Failed to get local addr");

        tokio::spawn(async move {
            axum::serve(listener, app).await.expect("Server failed");
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        Self {
            addr,
            client: Client::new(),
            admin_token,
            _temp_dir: temp_dir,
        }
    }

    fn base_url(&self) -> String {
        format!("http://{}", self.addr)
    }

    async fn get(&self, path: &str) -> Response {
        self.client
            .get(format!("{}{}", self.base_url(), path))
            .send()
            .await
            .expect("Request failed")
    }

    async fn get_admin(&self, path: &str) -> Response {
        self.client
            .get(format!("{}{}", self.base_url(), path))
            .header("Authorization", format!("Bearer {}", self.admin_token))
            .send()
            .await
            .expect("Request failed")
    }

    async fn get_with_token(&self, path: &str, token: &str) -> Response {
        self.client
            .get(format!("{}{}", self.base_url(), path))
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await
            .expect("Request failed")
    }

    async fn post_admin<T: Serialize>(&self, path: &str, body: &T) -> Response {
        self.client
            .post(format!("{}{}", self.base_url(), path))
            .header("Authorization", format!("Bearer {}", self.admin_token))
            .json(body)
            .send()
            .await
            .expect("Request failed")
    }
}

/// API response structure.
#[derive(Debug, Deserialize)]
struct ApiResponse<T> {
    code: i32,
    #[allow(dead_code)]
    message: String,
    data: Option<T>,
}

impl<T> ApiResponse<T> {
    fn is_success(&self) -> bool {
        self.code == 0
    }
}

// ============================================================================
// Health Endpoint Tests
// ============================================================================

#[derive(Debug, Deserialize)]
struct HealthData {
    status: String,
}

#[derive(Debug, Deserialize)]
struct ReadyData {
    ready: bool,
}

#[tokio::test]
async fn test_health_endpoint() {
    let server = TestServer::new().await;
    let response = server.get("/health").await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: ApiResponse<HealthData> = response.json().await.unwrap();
    assert!(body.is_success());
    assert_eq!(body.data.unwrap().status, "healthy");
}

#[tokio::test]
async fn test_ready_endpoint() {
    let server = TestServer::new().await;
    let response = server.get("/ready").await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: ApiResponse<ReadyData> = response.json().await.unwrap();
    assert!(body.is_success());
    assert!(body.data.unwrap().ready);
}

#[tokio::test]
async fn test_metrics_endpoint() {
    let server = TestServer::new().await;
    let response = server.get("/metrics").await;
    assert_eq!(response.status(), StatusCode::OK);

    let text = response.text().await.unwrap();
    assert!(text.contains("idbuilder_up"));
}

// ============================================================================
// Authentication Tests
// ============================================================================

#[tokio::test]
async fn test_unauthorized_access_to_config() {
    let server = TestServer::new().await;
    let response = server.get("/v1/config/increment?name=test").await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_unauthorized_access_to_id() {
    let server = TestServer::new().await;
    let response = server.get("/v1/id/increment?name=test").await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_invalid_token() {
    let server = TestServer::new().await;
    let response = server
        .get_with_token("/v1/config/increment?name=test", "invalid_token")
        .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_verify_with_valid_admin_token() {
    let server = TestServer::new().await;
    let response = server.get_admin("/v1/auth/verify").await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: ApiResponse<()> = response.json().await.unwrap();
    assert!(body.is_success());
}

#[tokio::test]
async fn test_verify_without_token() {
    let server = TestServer::new().await;
    let response = server.get("/v1/auth/verify").await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_verify_with_invalid_token() {
    let server = TestServer::new().await;
    let response = server
        .get_with_token("/v1/auth/verify", "invalid_token")
        .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_verify_with_key_token_forbidden() {
    let server = TestServer::new().await;

    // Get a key token first
    let response = server.get_admin("/v1/auth/token?key=verify_test").await;
    let body: ApiResponse<TokenData> = response.json().await.unwrap();
    let key_token = body.data.unwrap().token;

    // Key token should not be able to access verify endpoint (admin only)
    let response = server.get_with_token("/v1/auth/verify", &key_token).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[derive(Debug, Deserialize)]
struct TokenData {
    key: String,
    token: String,
}

#[tokio::test]
async fn test_create_and_use_key_token() {
    let server = TestServer::new().await;

    // Get (auto-create) a key token via GET
    let response = server.get_admin("/v1/auth/token?key=token_test").await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: ApiResponse<TokenData> = response.json().await.unwrap();
    assert!(body.is_success());
    let data = body.data.unwrap();
    assert_eq!(data.key, "token_test");
    let token = data.token;
    // Token should be 64 characters of base64
    assert_eq!(token.len(), 64);

    // Create a config with admin token
    server
        .post_admin(
            "/v1/config/increment",
            &json!({
                "name": "token_test",
                "start": 1,
                "step": 1,
                "min": 1,
                "max": 1000000
            }),
        )
        .await;

    // Use key token to generate IDs (should work)
    let response = server
        .get_with_token("/v1/id/increment?name=token_test&count=1", &token)
        .await;
    assert_eq!(response.status(), StatusCode::OK);

    // Key token cannot access config endpoints
    let response = server
        .get_with_token("/v1/config/increment?name=token_test", &token)
        .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

// ============================================================================
// Increment Config Tests
// ============================================================================

#[tokio::test]
async fn test_create_increment_config() {
    let server = TestServer::new().await;

    let response = server
        .post_admin(
            "/v1/config/increment",
            &json!({
                "name": "orders",
                "start": 1000,
                "step": 1,
                "min": 1,
                "max": 9999999
            }),
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: ApiResponse<()> = response.json().await.unwrap();
    assert!(body.is_success());
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct IncrementConfigData {
    name: String,
    start: i64,
    step: i64,
}

#[tokio::test]
async fn test_get_increment_config() {
    let server = TestServer::new().await;

    server
        .post_admin(
            "/v1/config/increment",
            &json!({
                "name": "invoices",
                "start": 100,
                "step": 1,
                "min": 1,
                "max": 999999
            }),
        )
        .await;

    let response = server.get_admin("/v1/config/increment?name=invoices").await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: ApiResponse<IncrementConfigData> = response.json().await.unwrap();
    assert!(body.is_success());
    let data = body.data.unwrap();
    assert_eq!(data.name, "invoices");
    assert_eq!(data.start, 100);
}

#[tokio::test]
async fn test_duplicate_increment_config() {
    let server = TestServer::new().await;

    let config = json!({
        "name": "duplicate_test",
        "start": 1,
        "step": 1,
        "min": 1,
        "max": 1000
    });

    let response = server.post_admin("/v1/config/increment", &config).await;
    assert_eq!(response.status(), StatusCode::OK);

    let response = server.post_admin("/v1/config/increment", &config).await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_invalid_increment_config() {
    let server = TestServer::new().await;

    let response = server
        .post_admin(
            "/v1/config/increment",
            &json!({
                "name": "invalid",
                "start": 1,
                "step": 0,
                "min": 1,
                "max": 1000
            }),
        )
        .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_get_nonexistent_config() {
    let server = TestServer::new().await;
    let response = server
        .get_admin("/v1/config/increment?name=nonexistent")
        .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// ============================================================================
// Snowflake Config Tests
// ============================================================================

#[tokio::test]
async fn test_create_snowflake_config() {
    let server = TestServer::new().await;

    let response = server
        .post_admin(
            "/v1/config/snowflake",
            &json!({
                "name": "events",
                "epoch": 1704067200000_i64,
                "worker_bits": 10,
                "sequence_bits": 12
            }),
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[derive(Debug, Deserialize)]
struct SnowflakeConfigData {
    name: String,
    worker_bits: u8,
    sequence_bits: u8,
}

#[tokio::test]
async fn test_get_snowflake_config() {
    let server = TestServer::new().await;

    server
        .post_admin(
            "/v1/config/snowflake",
            &json!({
                "name": "messages",
                "epoch": 1704067200000_i64,
                "worker_bits": 8,
                "sequence_bits": 14
            }),
        )
        .await;

    let response = server.get_admin("/v1/config/snowflake?name=messages").await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: ApiResponse<SnowflakeConfigData> = response.json().await.unwrap();
    let data = body.data.unwrap();
    assert_eq!(data.name, "messages");
    assert_eq!(data.worker_bits, 8);
    assert_eq!(data.sequence_bits, 14);
}

#[tokio::test]
async fn test_invalid_snowflake_config_bits() {
    let server = TestServer::new().await;

    let response = server
        .post_admin(
            "/v1/config/snowflake",
            &json!({
                "name": "invalid_sf",
                "epoch": 1704067200000_i64,
                "worker_bits": 15,
                "sequence_bits": 15
            }),
        )
        .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

// ============================================================================
// Formatted Config Tests
// ============================================================================

#[tokio::test]
async fn test_create_formatted_config() {
    let server = TestServer::new().await;

    let response = server
        .post_admin(
            "/v1/config/formatted",
            &json!({
                "name": "invoice_numbers",
                "pattern": "INV{YYYY}{MM}{DD}-{SEQ:6}",
                "sequence_reset": "daily"
            }),
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[derive(Debug, Deserialize)]
struct FormattedConfigData {
    name: String,
    pattern: String,
}

#[tokio::test]
async fn test_get_formatted_config() {
    let server = TestServer::new().await;

    server
        .post_admin(
            "/v1/config/formatted",
            &json!({
                "name": "order_codes",
                "pattern": "ORD-{UUID}",
                "sequence_reset": "never"
            }),
        )
        .await;

    let response = server
        .get_admin("/v1/config/formatted?name=order_codes")
        .await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: ApiResponse<FormattedConfigData> = response.json().await.unwrap();
    let data = body.data.unwrap();
    assert_eq!(data.name, "order_codes");
    assert_eq!(data.pattern, "ORD-{UUID}");
}

#[tokio::test]
async fn test_invalid_formatted_pattern() {
    let server = TestServer::new().await;

    let response = server
        .post_admin(
            "/v1/config/formatted",
            &json!({
                "name": "invalid_pattern",
                "pattern": "STATIC-PREFIX",
                "sequence_reset": "never"
            }),
        )
        .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

// ============================================================================
// ID Generation Tests - Increment
// ============================================================================

#[derive(Debug, Deserialize)]
struct IncrementIdData {
    ids: Vec<i64>,
}

#[tokio::test]
async fn test_generate_increment_ids() {
    let server = TestServer::new().await;

    server
        .post_admin(
            "/v1/config/increment",
            &json!({
                "name": "gen_test",
                "start": 1,
                "step": 1,
                "min": 1,
                "max": 1000000
            }),
        )
        .await;

    let response = server
        .get_admin("/v1/id/increment?name=gen_test&count=5")
        .await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: ApiResponse<IncrementIdData> = response.json().await.unwrap();
    assert!(body.is_success());
    let ids = body.data.unwrap().ids;
    assert_eq!(ids.len(), 5);
    assert_eq!(ids, vec![1, 2, 3, 4, 5]);
}

#[tokio::test]
async fn test_generate_increment_ids_sequential() {
    let server = TestServer::new().await;

    server
        .post_admin(
            "/v1/config/increment",
            &json!({
                "name": "seq_test",
                "start": 100,
                "step": 1,
                "min": 1,
                "max": 1000000
            }),
        )
        .await;

    let response = server
        .get_admin("/v1/id/increment?name=seq_test&count=3")
        .await;
    let body: ApiResponse<IncrementIdData> = response.json().await.unwrap();
    assert_eq!(body.data.unwrap().ids, vec![100, 101, 102]);

    let response = server
        .get_admin("/v1/id/increment?name=seq_test&count=3")
        .await;
    let body: ApiResponse<IncrementIdData> = response.json().await.unwrap();
    assert_eq!(body.data.unwrap().ids, vec![103, 104, 105]);
}

#[tokio::test]
async fn test_generate_increment_with_step() {
    let server = TestServer::new().await;

    server
        .post_admin(
            "/v1/config/increment",
            &json!({
                "name": "step_test",
                "start": 0,
                "step": 10,
                "min": 0,
                "max": 1000000
            }),
        )
        .await;

    let response = server
        .get_admin("/v1/id/increment?name=step_test&count=4")
        .await;
    let body: ApiResponse<IncrementIdData> = response.json().await.unwrap();
    assert_eq!(body.data.unwrap().ids, vec![0, 10, 20, 30]);
}

#[tokio::test]
async fn test_generate_increment_nonexistent() {
    let server = TestServer::new().await;
    let response = server
        .get_admin("/v1/id/increment?name=nonexistent&count=1")
        .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// ============================================================================
// ID Generation Tests - Snowflake
// ============================================================================

#[derive(Debug, Deserialize)]
struct SnowflakeIdData {
    worker_id: u32,
    epoch: i64,
    worker_bits: u8,
    sequence_bits: u8,
}

#[tokio::test]
async fn test_get_snowflake_with_worker_id() {
    let server = TestServer::new().await;

    server
        .post_admin(
            "/v1/config/snowflake",
            &json!({
                "name": "sf_gen",
                "epoch": 1704067200000_i64,
                "worker_bits": 10,
                "sequence_bits": 12
            }),
        )
        .await;

    let response = server.get_admin("/v1/id/snowflake?name=sf_gen").await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: ApiResponse<SnowflakeIdData> = response.json().await.unwrap();
    let data = body.data.unwrap();
    assert!(data.worker_id < 1024);
    assert_eq!(data.epoch, 1704067200000);
    assert_eq!(data.worker_bits, 10);
    assert_eq!(data.sequence_bits, 12);
}

// ============================================================================
// ID Generation Tests - Formatted
// ============================================================================

#[derive(Debug, Deserialize)]
struct FormattedIdData {
    ids: Vec<String>,
}

#[tokio::test]
async fn test_generate_formatted_with_sequence() {
    let server = TestServer::new().await;

    server
        .post_admin(
            "/v1/config/formatted",
            &json!({
                "name": "fmt_seq",
                "pattern": "ID-{SEQ:4}",
                "sequence_reset": "never"
            }),
        )
        .await;

    let response = server
        .get_admin("/v1/id/formatted?name=fmt_seq&count=3")
        .await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: ApiResponse<FormattedIdData> = response.json().await.unwrap();
    let ids = body.data.unwrap().ids;
    assert_eq!(ids.len(), 3);
    assert_eq!(ids[0], "ID-0001");
    assert_eq!(ids[1], "ID-0002");
    assert_eq!(ids[2], "ID-0003");
}

#[tokio::test]
async fn test_generate_formatted_with_uuid() {
    let server = TestServer::new().await;

    server
        .post_admin(
            "/v1/config/formatted",
            &json!({
                "name": "fmt_uuid",
                "pattern": "ORD-{UUID}",
                "sequence_reset": "never"
            }),
        )
        .await;

    let response = server
        .get_admin("/v1/id/formatted?name=fmt_uuid&count=2")
        .await;
    let body: ApiResponse<FormattedIdData> = response.json().await.unwrap();
    let ids = body.data.unwrap().ids;

    assert_eq!(ids.len(), 2);
    assert!(ids[0].starts_with("ORD-"));
    assert!(ids[1].starts_with("ORD-"));
    assert_ne!(ids[0], ids[1]);
}

#[tokio::test]
async fn test_generate_formatted_with_random() {
    let server = TestServer::new().await;

    server
        .post_admin(
            "/v1/config/formatted",
            &json!({
                "name": "fmt_rand",
                "pattern": "CODE-{RAND:8}",
                "sequence_reset": "never"
            }),
        )
        .await;

    let response = server
        .get_admin("/v1/id/formatted?name=fmt_rand&count=2")
        .await;
    let body: ApiResponse<FormattedIdData> = response.json().await.unwrap();
    let ids = body.data.unwrap().ids;

    assert_eq!(ids.len(), 2);
    assert!(ids[0].starts_with("CODE-"));
    assert_eq!(ids[0].len(), 13);
    assert_ne!(ids[0], ids[1]);
}

// ============================================================================
// Concurrency Tests
// ============================================================================

use std::collections::HashSet;
use std::sync::Arc as StdArc;
use tokio::sync::Barrier;

#[tokio::test]
async fn test_concurrent_increment_generation() {
    let server = StdArc::new(TestServer::new().await);

    server
        .post_admin(
            "/v1/config/increment",
            &json!({
                "name": "concurrent_test",
                "start": 1,
                "step": 1,
                "min": 1,
                "max": 1000000
            }),
        )
        .await;

    let num_tasks = 5;
    let ids_per_task = 20;
    let barrier = StdArc::new(Barrier::new(num_tasks));

    let mut handles = Vec::new();

    for _ in 0..num_tasks {
        let server = StdArc::clone(&server);
        let barrier = StdArc::clone(&barrier);

        let handle = tokio::spawn(async move {
            barrier.wait().await;

            let mut all_ids = Vec::new();
            for _ in 0..ids_per_task {
                let response = server
                    .get_admin("/v1/id/increment?name=concurrent_test&count=1")
                    .await;
                let body: ApiResponse<IncrementIdData> = response.json().await.unwrap();
                all_ids.extend(body.data.unwrap().ids);
            }
            all_ids
        });

        handles.push(handle);
    }

    let mut all_ids = Vec::new();
    for handle in handles {
        let ids = handle.await.expect("Task panicked");
        all_ids.extend(ids);
    }

    let unique_ids: HashSet<i64> = all_ids.iter().copied().collect();
    assert_eq!(unique_ids.len(), all_ids.len(), "All IDs should be unique");
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[tokio::test]
async fn test_empty_name_parameter() {
    let server = TestServer::new().await;
    let response = server.get_admin("/v1/id/increment?name=&count=1").await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_count_validation_zero() {
    let server = TestServer::new().await;

    server
        .post_admin(
            "/v1/config/increment",
            &json!({
                "name": "count_test",
                "start": 1,
                "step": 1,
                "min": 1,
                "max": 1000000
            }),
        )
        .await;

    let response = server
        .get_admin("/v1/id/increment?name=count_test&count=0")
        .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_count_validation_too_large() {
    let server = TestServer::new().await;

    server
        .post_admin(
            "/v1/config/increment",
            &json!({
                "name": "large_count",
                "start": 1,
                "step": 1,
                "min": 1,
                "max": 10000000
            }),
        )
        .await;

    let response = server
        .get_admin("/v1/id/increment?name=large_count&count=1001")
        .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_unknown_route() {
    let server = TestServer::new().await;
    let response = server.get("/unknown/route").await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
