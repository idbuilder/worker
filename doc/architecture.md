# IDBuilder Worker Architecture

| Metadata       | Value                                              |
|----------------|---------------------------------------------------|
| Author(s)      | IDBuilder Team                                    |
| Status         | Draft                                             |
| Created        | 2025-01-23                                        |
| Last Updated   | 2025-01-23                                        |

---

## Table of Contents

1. [Overview](#overview)
2. [Technology Stack](#technology-stack)
3. [System Architecture](#system-architecture)
4. [Module Design](#module-design)
5. [Persistence Layer](#persistence-layer)
6. [Database Schema](#database-schema)
7. [Distributed Deployment](#distributed-deployment)
8. [Database Initialization](#database-initialization)
9. [Configuration](#configuration)
10. [API Design](#api-design)
11. [Error Handling](#error-handling)
12. [Observability](#observability)

---

## Overview

The Worker is the core component of IDBuilder responsible for:

- Processing ID generation requests from clients
- Ensuring ID uniqueness across distributed instances
- Managing ID sequences and state persistence
- Syncing configuration with Controller

### Responsibilities

| Responsibility | Description |
|---------------|-------------|
| ID Generation | Generate auto-increment, snowflake config, and formatted string IDs |
| State Management | Persist and manage ID sequence states |
| Token Validation | Validate key tokens for ID generation requests |
| Worker Registration | Register with Controller and obtain worker ID |
| Health Reporting | Report health status to Controller |

---

## Technology Stack

### Core Framework

| Component | Choice | Rationale |
|-----------|--------|-----------|
| Language | Rust | Memory safety, high performance, zero-cost abstractions |
| Web Framework | **Axum** | Tokio-native, modular, excellent ergonomics, strong ecosystem |
| Async Runtime | Tokio | Industry standard, battle-tested, full-featured |
| Serialization | Serde | De facto standard for Rust serialization |

### Why Axum?

1. **Performance**: Built on Tokio and Hyper, minimal overhead
2. **Type Safety**: Compile-time route validation, extractor pattern
3. **Modularity**: Tower middleware ecosystem, composable services
4. **Ecosystem**: Strong integration with SQLx, Redis clients, tracing
5. **Ergonomics**: Clean API, good documentation, active maintenance

### Key Dependencies

| Category | Crate | Version | Purpose |
|----------|-------|---------|---------|
| Web | axum | 0.7 | HTTP framework |
| Async | tokio | 1.x | Async runtime |
| Middleware | tower, tower-http | 0.4, 0.5 | Middleware stack |
| Serialization | serde, serde_json | 1.x | JSON serialization |
| Database | sqlx | 0.7 | Async SQL (MySQL, PostgreSQL) |
| Cache | redis | 0.24 | Redis client with cluster support |
| Config | config, dotenvy | 0.14, 0.15 | Configuration management |
| Logging | tracing, tracing-subscriber | 0.1, 0.3 | Structured logging |
| Metrics | metrics, metrics-exporter-prometheus | 0.22, 0.14 | Prometheus metrics |
| Time | chrono | 0.4 | Date/time handling |
| Error | thiserror, anyhow | 1.x | Error handling |

---

## System Architecture

### High-Level Architecture

```
                                    ┌─────────────────────────────────────────────────────────┐
                                    │                        Worker                           │
                                    │  ┌─────────────────────────────────────────────────┐   │
                                    │  │                   API Layer                      │   │
                                    │  │  ┌─────────┐ ┌─────────┐ ┌─────────────────┐   │   │
    ┌──────────┐   HTTP Request     │  │  │Increment│ │Snowflake│ │    Formatted    │   │   │
    │  Client  │ ─────────────────▶ │  │  │  API    │ │   API   │ │       API       │   │   │
    └──────────┘                    │  │  └────┬────┘ └────┬────┘ └────────┬────────┘   │   │
                                    │  └───────┼──────────┼────────────────┼────────────┘   │
                                    │          │          │                │                │
                                    │  ┌───────▼──────────▼────────────────▼────────────┐   │
                                    │  │                Service Layer                    │   │
                                    │  │  ┌─────────────┐  ┌─────────────┐  ┌─────────┐ │   │
                                    │  │  │  ID Gen     │  │   Config    │  │  Auth   │ │   │
                                    │  │  │  Service    │  │   Service   │  │ Service │ │   │
                                    │  │  └──────┬──────┘  └──────┬──────┘  └────┬────┘ │   │
                                    │  └─────────┼────────────────┼──────────────┼──────┘   │
                                    │            │                │              │          │
                                    │  ┌─────────▼────────────────▼──────────────▼──────┐   │
                                    │  │              Persistence Layer                  │   │
                                    │  │  ┌────────────────────────────────────────────┐│   │
                                    │  │  │           Storage Abstraction (Trait)      ││   │
                                    │  │  └────────────────────────────────────────────┘│   │
                                    │  │       │           │          │          │      │   │
                                    │  │  ┌────▼───┐ ┌─────▼────┐ ┌───▼───┐ ┌────▼────┐│   │
                                    │  │  │  File  │ │  Redis   │ │ MySQL │ │PostgreSQL││   │
                                    │  │  │ Storage│ │  Storage │ │Storage│ │ Storage ││   │
                                    │  │  └────────┘ └──────────┘ └───────┘ └─────────┘│   │
                                    │  └────────────────────────────────────────────────┘   │
                                    └─────────────────────────────────────────────────────────┘
                                                              │
                                                    gRPC/HTTP │
                                                              ▼
                                                     ┌────────────────┐
                                                     │   Controller   │
                                                     └────────────────┘
```

### Component Interaction

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                              Worker Instance                                  │
│                                                                              │
│  ┌────────────────┐    ┌─────────────────┐    ┌─────────────────────────┐   │
│  │  HTTP Server   │───▶│  Request Router │───▶│    Middleware Chain     │   │
│  │    (Axum)      │    │                 │    │  - Auth                 │   │
│  └────────────────┘    └─────────────────┘    │  - Rate Limit           │   │
│                                               │  - Tracing              │   │
│                                               │  - Metrics              │   │
│                                               └───────────┬─────────────┘   │
│                                                           │                 │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                         Service Layer                                │   │
│  │  ┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐  │   │
│  │  │ IncrementService │  │ SnowflakeService │  │ FormattedService │  │   │
│  │  │ - get_next_id()  │  │ - get_config()   │  │ - generate()     │  │   │
│  │  │ - get_batch()    │  │ - assign_worker()│  │ - parse_parts()  │  │   │
│  │  └────────┬─────────┘  └────────┬─────────┘  └────────┬─────────┘  │   │
│  │           └─────────────────────┼─────────────────────┘            │   │
│  │                    ┌────────────▼────────────┐                      │   │
│  │                    │    SequenceManager      │                      │   │
│  │                    │  - Atomic operations    │                      │   │
│  │                    │  - Batch allocation     │                      │   │
│  │                    │  - Overflow handling    │                      │   │
│  │                    └────────────┬────────────┘                      │   │
│  └─────────────────────────────────┼────────────────────────────────────┘   │
│  ┌─────────────────────────────────▼────────────────────────────────────┐   │
│  │                      Persistence Layer (Storage Trait)               │   │
│  └───────────────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────────────┘
```

---

## Module Design

### Project Structure

```
worker/
├── Cargo.toml
├── src/
│   ├── main.rs                    # Entry point
│   ├── lib.rs                     # Library root
│   ├── config/                    # Configuration module
│   ├── api/                       # HTTP API layer
│   │   ├── router.rs              # Route definitions
│   │   ├── handlers/              # Request handlers
│   │   ├── middleware/            # Auth, rate limit, tracing
│   │   └── extractors/            # Custom extractors
│   ├── service/                   # Business logic
│   │   ├── increment.rs           # Auto-increment service
│   │   ├── snowflake.rs           # Snowflake service
│   │   ├── formatted.rs           # Formatted ID service
│   │   └── sequence.rs            # Sequence manager
│   ├── storage/                   # Persistence layer
│   │   ├── traits.rs              # Storage trait definitions
│   │   ├── file/                  # File-based storage
│   │   ├── redis/                 # Redis storage
│   │   ├── mysql/                 # MySQL storage
│   │   ├── postgres/              # PostgreSQL storage
│   │   └── migration/             # Schema migration
│   ├── domain/                    # Domain models
│   ├── controller/                # Controller client
│   └── error/                     # Error handling
├── migrations/                    # SQL migrations
└── config/                        # Configuration files
```

### Core Storage Trait

The `Storage` trait defines the interface all backends must implement:

| Method | Description |
|--------|-------------|
| `get_sequence(key)` | Get current sequence value for a key |
| `increment_sequence(key, delta)` | Atomically increment and return new value |
| `reserve_range(key, size, delta)` | Batch increment - reserve a range of IDs |
| `get_config(key)` | Get configuration for a key |
| `set_config(key, config)` | Store configuration |
| `acquire_lock(lock_key, ttl)` | Acquire distributed lock |
| `init_schema()` | Initialize storage schema (tables, indexes) |
| `health_check()` | Health check |

---

## Persistence Layer

### Storage Backend Comparison

| Backend | Use Case | Consistency | Lock Mechanism |
|---------|----------|-------------|----------------|
| File | Development, single-node | Single-node only | flock() |
| Redis | High-performance distributed | Eventual (Redlock) | Redlock algorithm |
| MySQL | Strong consistency required | Strong (InnoDB) | Row-level / Optimistic locking |
| PostgreSQL | Strong consistency required | Strong (MVCC) | Advisory locks / Row-level locks |

### File Storage

Data stored as JSON files:
- `{base_path}/sequences/{key}.json` - Sequence state
- `{base_path}/configs/{key}.json` - ID configuration
- `{base_path}/locks/{key}.lock` - File-based locks (flock)

### Redis Storage

Key patterns:
- `{prefix}:seq:{key}` - Sequence counter (uses INCRBY)
- `{prefix}:cfg:{key}` - Configuration (JSON)
- `{prefix}:lock:{key}` - Distributed lock (Redlock)

Features:
- Atomic increments via Redis INCRBY
- Batch operations via Lua scripts
- Distributed locking via Redlock algorithm

### MySQL/PostgreSQL Storage

Uses optimistic locking with version field for atomic updates.

---

## Database Schema

### Table: `id_sequences`

Stores the current state of ID sequences.

| Column | MySQL Type | PostgreSQL Type | Nullable | Default | Description |
|--------|------------|-----------------|----------|---------|-------------|
| `id` | BIGINT AUTO_INCREMENT | BIGSERIAL | NO | - | Primary key |
| `key_name` | VARCHAR(255) | VARCHAR(255) | NO | - | Unique identifier for the sequence |
| `current_value` | BIGINT | BIGINT | NO | 0 | Current sequence value |
| `version` | BIGINT | BIGINT | NO | 0 | Optimistic lock version |
| `created_at` | TIMESTAMP | TIMESTAMPTZ | NO | CURRENT_TIMESTAMP / NOW() | Record creation time |
| `updated_at` | TIMESTAMP | TIMESTAMPTZ | NO | CURRENT_TIMESTAMP / NOW() | Last update time |

**Indexes:**
- PRIMARY KEY (`id`)
- UNIQUE INDEX `idx_key_name` (`key_name`)

**Notes:**
- MySQL: `updated_at` uses `ON UPDATE CURRENT_TIMESTAMP`
- Used for auto-increment and formatted ID generation

---

### Table: `id_configs`

Stores ID generation configurations.

| Column | MySQL Type | PostgreSQL Type | Nullable | Default | Description |
|--------|------------|-----------------|----------|---------|-------------|
| `id` | BIGINT AUTO_INCREMENT | BIGSERIAL | NO | - | Primary key |
| `key_name` | VARCHAR(255) | VARCHAR(255) | NO | - | Unique identifier for the configuration |
| `id_type` | VARCHAR(50) | VARCHAR(50) | NO | - | Type: `increment`, `snowflake`, `formatted` |
| `config_json` | JSON | JSONB | NO | - | Configuration details in JSON format |
| `created_at` | TIMESTAMP | TIMESTAMPTZ | NO | CURRENT_TIMESTAMP / NOW() | Record creation time |
| `updated_at` | TIMESTAMP | TIMESTAMPTZ | NO | CURRENT_TIMESTAMP / NOW() | Last update time |

**Indexes:**
- PRIMARY KEY (`id`)
- UNIQUE INDEX `idx_configs_key` (`key_name`)

**`config_json` Structure by Type:**

For `increment`:
```json
{
  "base": 1000,
  "delta": 1,
  "max_request_delta": 100,
  "rand_delta": false
}
```

For `snowflake`:
```json
{
  "skip_size": 1,
  "base_ts": 1673606841000,
  "ts_size": 41,
  "worker_id_size": 10,
  "seq_size": 12
}
```

For `formatted`:
```json
{
  "parts": [
    {"type": "fixed-chars", "value": "INV"},
    {"type": "date-format", "format": "yyyyMMdd"},
    {"type": "auto-increment", "length": 4, "reset_scope": "date"}
  ]
}
```

---

### Table: `distributed_locks`

Manages distributed locks for coordination.

| Column | MySQL Type | PostgreSQL Type | Nullable | Default | Description |
|--------|------------|-----------------|----------|---------|-------------|
| `lock_key` | VARCHAR(255) | VARCHAR(255) | NO | - | Primary key, lock identifier |
| `owner_id` | VARCHAR(255) | VARCHAR(255) | NO | - | ID of the lock owner (worker instance) |
| `expires_at` | TIMESTAMP | TIMESTAMPTZ | NO | - | Lock expiration time |
| `created_at` | TIMESTAMP | TIMESTAMPTZ | NO | CURRENT_TIMESTAMP / NOW() | Lock acquisition time |

**Indexes:**
- PRIMARY KEY (`lock_key`)

**Notes:**
- Used for schema initialization coordination
- Expired locks can be acquired by other workers
- PostgreSQL can also use advisory locks as an alternative

---

### Table: `schema_versions`

Tracks database schema migration state.

| Column | MySQL Type | PostgreSQL Type | Nullable | Default | Description |
|--------|------------|-----------------|----------|---------|-------------|
| `id` | INT AUTO_INCREMENT | SERIAL | NO | - | Primary key |
| `version` | INT | INT | NO | - | Schema version number |
| `applied_at` | TIMESTAMP | TIMESTAMPTZ | NO | CURRENT_TIMESTAMP / NOW() | When migration was applied |
| `description` | VARCHAR(500) | VARCHAR(500) | YES | NULL | Migration description |

**Indexes:**
- PRIMARY KEY (`id`)
- UNIQUE INDEX `idx_version` (`version`)

---

### Redis Key Schema

| Key Pattern | Type | TTL | Description |
|-------------|------|-----|-------------|
| `{prefix}:seq:{key}` | String (integer) | None | Current sequence value |
| `{prefix}:cfg:{key}` | String (JSON) | None | Configuration JSON |
| `{prefix}:lock:{key}` | String | TTL-based | Distributed lock (value = owner_id) |
| `{prefix}:schema:version` | String (integer) | None | Schema version marker |

---

### File Storage Schema

| File Path | Format | Description |
|-----------|--------|-------------|
| `{base}/sequences/{key}.json` | JSON | `{"current": 1523, "version": 10, "updated_at": "..."}` |
| `{base}/configs/{key}.json` | JSON | Full configuration object |
| `{base}/locks/{key}.lock` | Lock file | Empty file, uses flock() |
| `{base}/schema_version` | Plain text | Single integer for schema version |

---

## Distributed Deployment

### Architecture

```
                                    ┌─────────────────┐
                                    │  Load Balancer  │
                                    │   (L4/L7)       │
                                    └────────┬────────┘
                                             │
              ┌──────────────────────────────┼──────────────────────────────┐
              │                              │                              │
              ▼                              ▼                              ▼
     ┌─────────────────┐          ┌─────────────────┐          ┌─────────────────┐
     │  Worker Node 1  │          │  Worker Node 2  │          │  Worker Node N  │
     │  worker_id: 1   │          │  worker_id: 2   │          │  worker_id: N   │
     └────────┬────────┘          └────────┬────────┘          └────────┬────────┘
              │                            │                            │
              └──────────────────────────────────────────────────────────┘
                                           │
                                           ▼
                               ┌───────────────────────┐
                               │   Shared Storage      │
                               │  (Redis Cluster /     │
                               │   PostgreSQL / MySQL) │
                               └───────────────────────┘
```

### Worker Registration

1. Worker starts and connects to Controller
2. Controller assigns unique `worker_id`
3. Worker sends periodic heartbeats
4. Controller tracks active workers for load balancing

### ID Range Pre-allocation

For high-throughput scenarios:
1. Worker reserves a batch of IDs from storage (e.g., 1000 IDs)
2. IDs served from local cache until exhausted
3. Prefetch triggered when remaining IDs fall below threshold (e.g., 20%)
4. Reduces storage round-trips significantly

---

## Database Initialization

### Safe Initialization Flow

Multiple worker instances may start simultaneously. Only one should initialize the schema.

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│  Worker 1   │     │  Worker 2   │     │  Worker 3   │
└──────┬──────┘     └──────┬──────┘     └──────┬──────┘
       │                   │                   │
       │ Acquire Lock      │ Acquire Lock      │ Acquire Lock
       ▼                   ▼                   ▼
   ┌───────┐           ┌───────┐           ┌───────┐
   │SUCCESS│           │ FAIL  │           │ FAIL  │
   └───┬───┘           └───┬───┘           └───┬───┘
       │                   │                   │
       ▼                   ▼                   ▼
  Init Schema         Wait & Poll          Wait & Poll
       │                   │                   │
       ▼                   │                   │
  Set Version              │                   │
       │                   │                   │
       ▼                   ▼                   ▼
  Release Lock        Check Version        Check Version
       │                   │                   │
       ▼                   ▼                   ▼
    [READY]             [READY]             [READY]
```

### Lock Mechanisms by Backend

| Backend | Lock Method | TTL Support | Notes |
|---------|-------------|-------------|-------|
| File | flock() | No | Single-node only |
| Redis | SET NX PX | Yes | Redlock for cluster |
| MySQL | GET_LOCK() | Yes (timeout) | Named locks |
| PostgreSQL | pg_try_advisory_lock() | No | Session-based |

### Startup Sequence

1. Load configuration
2. Initialize tracing/logging
3. Create storage backend connection
4. **Acquire distributed lock for schema init**
5. Check if schema already initialized (version check)
6. If not initialized: run migrations, set version
7. Release lock
8. Register with Controller (if configured)
9. Start HTTP server

---

## Configuration

### Configuration File Structure (TOML)

| Section | Key | Type | Default | Description |
|---------|-----|------|---------|-------------|
| `server` | `host` | string | "0.0.0.0" | Bind address |
| `server` | `port` | integer | 8080 | Listen port |
| `server` | `workers` | integer | 4 | Tokio worker threads |
| `storage` | `backend` | string | - | `file`, `redis`, `mysql`, `postgresql` |
| `storage.file` | `path` | string | "./data" | Data directory |
| `storage.redis` | `mode` | string | "standalone" | `standalone` or `cluster` |
| `storage.redis` | `url` | string | - | Redis connection URL |
| `storage.redis` | `pool_size` | integer | 10 | Connection pool size |
| `storage.redis` | `key_prefix` | string | "idbuilder" | Key prefix |
| `storage.mysql` | `url` | string | - | MySQL connection URL |
| `storage.mysql` | `max_connections` | integer | 10 | Max pool connections |
| `storage.postgres` | `url` | string | - | PostgreSQL connection URL |
| `storage.postgres` | `max_connections` | integer | 10 | Max pool connections |
| `controller` | `endpoint` | string | - | Controller URL |
| `controller` | `heartbeat_interval` | integer | 10 | Heartbeat interval (seconds) |
| `sequence` | `default_batch_size` | integer | 1000 | IDs to prefetch |
| `sequence` | `prefetch_threshold` | float | 0.2 | Prefetch when 20% remaining |
| `observability` | `log_level` | string | "info" | Log level |
| `observability` | `log_format` | string | "json" | `json` or `pretty` |
| `observability.metrics` | `enabled` | boolean | true | Enable Prometheus metrics |
| `observability.metrics` | `port` | integer | 9091 | Metrics port |

### Environment Variable Override

Pattern: `IDBUILDER__<SECTION>__<KEY>`

Examples:
- `IDBUILDER__SERVER__PORT=8080`
- `IDBUILDER__STORAGE__BACKEND=postgresql`
- `IDBUILDER__STORAGE__POSTGRES__URL=postgresql://...`

---

## API Design

### Token Types

| Token Type | Header Format | Used For |
|------------|---------------|----------|
| Admin Token | `Authorization: Bearer <admin_token>` | Configuration APIs, Token API |
| Key Token | `Authorization: Bearer <key_token>` | ID Generation APIs |

### Endpoints Overview

| Category | Method | Path | Auth | Description |
|----------|--------|------|------|-------------|
| Health | GET | `/health` | None | Liveness check |
| Health | GET | `/ready` | None | Readiness check |
| Metrics | GET | `/metrics` | None | Prometheus metrics |
| Auth | GET | `/v1/auth/verify` | Admin | Verify admin token validity |
| Auth | GET | `/v1/auth/token` | Admin | Get key token for ID generation |
| Auth | GET | `/v1/auth/tokenreset` | Admin | Reset (regenerate) key token |
| Config | GET | `/v1/config/list` | Admin | List all ID configurations with pagination |
| Config | POST | `/v1/config/increment` | Admin | Create/Update increment config |
| Config | GET | `/v1/config/increment` | Admin | Get increment config |
| Config | POST | `/v1/config/snowflake` | Admin | Create/Update snowflake config |
| Config | GET | `/v1/config/snowflake` | Admin | Get snowflake config |
| Config | POST | `/v1/config/formatted` | Admin | Create/Update formatted config |
| Config | GET | `/v1/config/formatted` | Admin | Get formatted config |
| ID Gen | GET | `/v1/id/increment` | Key | Generate auto-increment IDs |
| ID Gen | GET | `/v1/id/snowflake` | Key | Get snowflake config + worker_id |
| ID Gen | GET | `/v1/id/formatted` | Key | Generate formatted string IDs |

---

### Authentication API

#### GET `/v1/auth/verify`

Verify that the admin token is valid. This endpoint can be used by clients to check if their admin token is correct before making other API calls.

**Request Parameters:** None

**Response:**

| Field | Type | Description |
|-------|------|-------------|
| `code` | integer | 0 for success |
| `message` | string | "success" |

**Example Response (200 OK):**

```json
{
  "code": 0,
  "message": "success"
}
```

**Error Responses:**

| HTTP Status | Code | Description |
|-------------|------|-------------|
| 401 | 2001 | Missing or invalid admin token |

---

#### GET `/v1/auth/token`

Retrieve a key token for accessing ID generation APIs.

**Request Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `key` | string | Yes | Identifier for the ID configuration |

**Response:**

| Field | Type | Description |
|-------|------|-------------|
| `key` | string | The requested key identifier |
| `token` | string | Key token for ID generation API |
| `expires_at` | string | Token expiration time (ISO 8601) |

---

#### GET `/v1/auth/tokenreset`

Reset (regenerate) the key token for a given key. The previous token is invalidated.

**Request Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `key` | string | Yes | Identifier for the ID configuration |

**Response:**

| Field | Type | Description |
|-------|------|-------------|
| `key` | string | The requested key identifier |
| `token` | string | Newly generated key token for ID generation API |
| `expires_at` | string | Token expiration time (ISO 8601) |

---

### Configuration List API

#### GET `/v1/config/list`

List all ID configurations with cursor-based pagination. Returns configurations across all types (increment, snowflake, formatted).

**Request Parameters:**

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `key` | string | No | - | Filter by key prefix (optional) |
| `from` | string | No | - | Cursor for pagination (last key from previous batch) |
| `size` | integer | No | 20 | Number of records to return (1-100) |

**Response:**

| Field | Type | Description |
|-------|------|-------------|
| `items` | array | List of configuration summaries |
| `items[].key` | string | Configuration key/name |
| `items[].id_type` | string | Type: `increment`, `snowflake`, or `formatted` |
| `next_cursor` | string | Cursor for next page (null if no more records) |
| `has_more` | boolean | Whether there are more records |

**Example Response:**

```json
{
  "code": 0,
  "message": "success",
  "data": {
    "items": [
      {"key": "orders", "id_type": "increment"},
      {"key": "invoices", "id_type": "formatted"},
      {"key": "events", "id_type": "snowflake"}
    ],
    "next_cursor": "events",
    "has_more": true
  }
}
```

---

### Configuration APIs

#### Auto-Increment Configuration

**POST `/v1/config/increment`** - Create/Update configuration

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `key` | string | Yes | - | Unique identifier |
| `name` | string | No | - | Human-readable name |
| `base` | integer | Yes | - | Starting number |
| `delta` | integer | No | 1 | Default increment |
| `max_request_delta` | integer | No | 100 | Maximum allowed delta per request |
| `rand_delta` | boolean | No | false | Enable randomized delta |

**GET `/v1/config/increment`** - Get configuration

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `key` | string | Yes | Configuration identifier |

**Response includes:** `key`, `name`, `base`, `current`, `delta`, `max_request_delta`, `rand_delta`, `created_at`, `updated_at`

---

#### Snowflake Configuration

**POST `/v1/config/snowflake`** - Create/Update configuration

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `key` | string | Yes | - | Unique identifier |
| `name` | string | No | - | Human-readable name |
| `skip_size` | integer | No | 1 | Leading bits to skip (sign bit) |
| `base_ts` | integer | Yes | - | Base timestamp (epoch ms) |
| `ts_size` | integer | Yes | - | Bit width for timestamp |
| `worker_id_size` | integer | Yes | - | Bit width for worker ID |
| `seq_size` | integer | Yes | - | Bit width for sequence |

**GET `/v1/config/snowflake`** - Get configuration

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `key` | string | Yes | Configuration identifier |

**Response includes:** `key`, `name`, `skip_size`, `base_ts`, `ts_size`, `worker_id_size`, `seq_size`, `active_workers`, `created_at`, `updated_at`

---

#### Formatted ID Configuration

**POST `/v1/config/formatted`** - Create/Update configuration

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `key` | string | Yes | Unique identifier |
| `name` | string | No | Human-readable name |
| `parts` | array | Yes | Ordered list of part configurations |

**Part Types:**

| Part Type | Parameters | Description |
|-----------|------------|-------------|
| `fixed-chars` | `value` | Static characters (e.g., "INV", "-") |
| `fixed-polling-char` | `chars_scope` | Rotating character from set |
| `fixed-random-chars` | `chars_scope`, `length` | Random characters |
| `date-format` | `format`, `time_zone` | Formatted date/time |
| `timestamp` | `base_ts` | Unix timestamp (ms) |
| `unix-seconds` | `base_unix` | Unix timestamp (seconds) |
| `auto-increment` | `length`, `length_fixed`, `padding_mode`, `padding_char`, `number_base`, `reset_scope` | Sequential number |

**Auto-Increment Part Options:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `length` | integer | - | Fixed length with padding |
| `length_fixed` | boolean | false | Enable fixed-length padding |
| `padding_mode` | string | "prefix" | `prefix` or `suffix` |
| `padding_char` | string | "0" | Padding character |
| `number_base` | integer | 10 | Numeric base (2-36) |
| `reset_scope` | string | "none" | `none`, `year`, `month`, `date` |

**GET `/v1/config/formatted`** - Get configuration

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `key` | string | Yes | Configuration identifier |

**Response includes:** `key`, `name`, `parts`, `sample_id`, `created_at`, `updated_at`

---

### ID Generation APIs

#### GET `/v1/id/increment`

Generate auto-increment IDs.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `key` | string | Yes | - | Configuration identifier |
| `size` | integer | No | 1 | Number of IDs (1-1000) |
| `delta` | integer | No | 1 | Increment between IDs |
| `rand_delta` | boolean | No | false | Randomize delta |

**Response:** `{ "id": [1001, 1002, 1003, ...] }`

---

#### GET `/v1/id/snowflake`

Get snowflake configuration for client-side ID generation.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `key` | string | Yes | Configuration identifier |

**Response:**

| Field | Type | Description |
|-------|------|-------------|
| `skip_size` | integer | Leading bits to skip |
| `base_ts` | integer | Base timestamp (ms) |
| `ts_size` | integer | Timestamp bit width |
| `worker_id` | integer | Assigned worker ID for this client |
| `worker_id_size` | integer | Worker ID bit width |
| `seq_size` | integer | Sequence bit width |

---

#### GET `/v1/id/formatted`

Generate formatted string IDs.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `key` | string | Yes | - | Configuration identifier |
| `size` | integer | No | 1 | Number of IDs (1-1000) |

**Response:** `{ "id": ["INV20230111-0001", "INV20230111-0002", ...] }`

---

### Middleware Stack

1. **Tracing**: Request/response logging with trace IDs
2. **Metrics**: Request count, latency histograms
3. **Auth**: Token validation for `/v1/id/*` routes
4. **Rate Limit**: Per-token rate limiting
5. **Compression**: gzip response compression
6. **Timeout**: 30-second request timeout

---

## Error Handling

### Error Codes

| Code | HTTP Status | Description |
|------|-------------|-------------|
| 0 | 200 | Success |
| 1001 | 400 | Invalid request parameters |
| 1002 | 400 | Invalid key format |
| 1003 | 400 | Requested size exceeds limit |
| 1004 | 400 | Requested delta exceeds limit |
| 2001 | 401 | Authentication failed |
| 2002 | 403 | Authorization failed |
| 3001 | 404 | Key not found |
| 4001 | 500 | Internal server error |
| 4002 | 503 | Service unavailable |
| 4003 | 503 | Sequence exhausted |

---

## Observability

### Prometheus Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `idbuilder_requests_total` | Counter | `key`, `id_type` | Total ID generation requests |
| `idbuilder_request_duration_seconds` | Histogram | `key`, `id_type` | Request latency |
| `idbuilder_sequence_current` | Gauge | `key` | Current sequence value |
| `idbuilder_cache_remaining` | Gauge | `key` | Remaining IDs in local cache |
| `idbuilder_storage_errors_total` | Counter | `backend`, `operation` | Storage operation errors |

### Health Endpoints

| Endpoint | Checks | Use Case |
|----------|--------|----------|
| `/health` | None (always 200) | Kubernetes liveness probe |
| `/ready` | Storage connectivity, Controller connectivity | Kubernetes readiness probe |

---

## Summary

| Aspect | Decision |
|--------|----------|
| Language | Rust |
| Framework | Axum |
| Storage Abstraction | Trait-based polymorphism |
| File Storage | JSON files with flock |
| Redis Storage | Standalone/Cluster with Redlock |
| MySQL Storage | InnoDB with optimistic locking |
| PostgreSQL Storage | MVCC with advisory locks |
| Distribution | Stateless workers + shared storage |
| DB Init Locking | Distributed lock per backend |
| Configuration | TOML + env override |
| Observability | Prometheus + OpenTelemetry |
