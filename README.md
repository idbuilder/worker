# IDBuilder Worker

The Worker is the core component of IDBuilder that handles ID generation requests and serves the admin console. It processes requests for auto-increment, snowflake, and formatted string IDs while ensuring uniqueness across distributed deployments.

## Features

- **ID Generation**: Auto-increment, Snowflake, and Formatted string IDs
- **Admin Console**: Built-in web UI for configuration management
- **Multiple Storage Backends**: File, Redis, MySQL, PostgreSQL
- **Authentication**: Admin tokens and per-key tokens

## Quick Start

```bash
# Build and run
make build
./target/release/idbuilder-worker

# Server starts at http://localhost:8080
# Admin console available at http://localhost:8080/admin/
```

## Admin Console

The worker includes a built-in admin web UI accessible at `/admin/`.

**Login**: Use your admin token (default: `admin_change_me_in_production`)

**Features**:
- View and manage ID configurations
- Create increment, snowflake, and formatted configs
- Manage key tokens for applications

### Disable Admin Console

```bash
# Via environment variable
IDBUILDER_WORKER__ADMIN__ENABLED=false ./target/release/idbuilder-worker
```

Or in `config/default.toml`:
```toml
[admin]
enabled = false
```

## Supported ID Types

| ID Type | Description | Example | Generation |
|---------|-------------|---------|------------|
| Auto-increment | Sequential numeric IDs | `1001, 1002, 1003` | Server-side |
| Snowflake | Time-based distributed IDs | `6982386234567892992` | Client-side |
| Formatted String | Custom format IDs | `INV20250126-0001` | Server-side |

## API Endpoints

### Health & Metrics (No Auth)

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/health` | GET | Health check |
| `/ready` | GET | Readiness check |
| `/metrics` | GET | Prometheus metrics |

### Configuration (Admin Auth)

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/v1/config/list` | GET | List all configurations |
| `/v1/config/increment` | GET/POST | Get/Create increment config |
| `/v1/config/snowflake` | GET/POST | Get/Create snowflake config |
| `/v1/config/formatted` | GET/POST | Get/Create formatted config |

### ID Generation (Key Auth)

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/v1/id/increment` | GET | Generate auto-increment IDs |
| `/v1/id/snowflake` | GET | Get snowflake config for client |
| `/v1/id/formatted` | GET | Generate formatted string IDs |

### Token Management (Admin Auth)

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/v1/auth/verify` | GET | Verify admin token |
| `/v1/auth/token` | GET | Get or create key token |
| `/v1/auth/tokenreset` | GET | Reset key token |

## Usage Examples

### Create a Configuration

```bash
# Create an increment config
curl -X POST "http://localhost:8080/v1/config/increment" \
  -H "Authorization: Bearer <admin_token>" \
  -H "Content-Type: application/json" \
  -d '{"key": "order-id", "start": 1000, "step": 1}'
```

### Get a Key Token

```bash
curl -X GET "http://localhost:8080/v1/auth/token?key=order-id" \
  -H "Authorization: Bearer <admin_token>"
```

### Generate IDs

```bash
# Generate 5 auto-increment IDs
curl -X GET "http://localhost:8080/v1/id/increment?key=order-id&size=5" \
  -H "Authorization: Bearer <key_token>"

# Response: {"code":0,"message":"success","data":{"ids":[1000,1001,1002,1003,1004]}}
```

## Configuration

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `IDBUILDER_WORKER__SERVER__HOST` | Bind address | `0.0.0.0` |
| `IDBUILDER_WORKER__SERVER__PORT` | Listen port | `8080` |
| `IDBUILDER_WORKER__STORAGE__BACKEND` | Storage backend | `file` |
| `IDBUILDER_WORKER__AUTH__ADMIN_TOKEN` | Admin token | `admin_change_me_in_production` |
| `IDBUILDER_WORKER__ADMIN__ENABLED` | Enable admin UI | `true` |
| `IDBUILDER_WORKER__ADMIN__PATH` | Static files path | `./static` |

### Configuration File

See `config/default.toml` for all configuration options.

## Development

```bash
make build          # Build release binary
make test           # Run all tests
make lint           # Run clippy + format check
make fmt            # Format code
make check          # Quick cargo check
make build-docker   # Build Docker image
```

## Related Components

- [IDBuilder CLI](../controller/README.md) - Command-line management tool
- [Design Document](../proposal/design/001-basic-design.md) - Full specification

## License

Apache License 2.0 - see [LICENSE](LICENSE) for details.
