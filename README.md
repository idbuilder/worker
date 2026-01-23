# IDBuilder Worker

The Worker is the core component of IDBuilder that handles ID generation requests from clients. It processes requests for auto-increment, snowflake, and formatted string IDs while ensuring uniqueness across distributed deployments.

## Overview

```
┌─────────────┐                          ┌─────────────┐                    ┌─────────────┐
│             │  ──── ID Request ────▶   │             │                    │             │
│ Client/SDK  │                          │   Worker    │ ◀──── Config ────  │ Controller  │
│             │  ◀─── ID Response ────   │             │                    │             │
└─────────────┘                          └─────────────┘                    └─────────────┘
```

The Worker:
- Receives ID generation configuration from the Controller
- Processes ID generation requests from clients
- Ensures ID uniqueness within its assigned range
- Supports horizontal scaling with multiple worker instances

## Supported ID Types

| ID Type | Description | Generation |
|---------|-------------|------------|
| Auto-increment | Sequential numeric IDs (`1001, 1002, ...`) | Server-side |
| Snowflake | Time-based distributed IDs (`6982386234567892992`) | Client-side (config from worker) |
| Formatted String | Custom format IDs (`INV20230101-0001`) | Server-side |

## API Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/v1/id/increment` | GET | Generate auto-increment IDs |
| `/v1/id/snowflake` | GET | Get snowflake config for client generation |
| `/v1/id/formatted` | GET | Generate formatted string IDs |
| `/v1/auth/token` | GET | Get key token for ID generation |

## Quick Start

```bash
# Get a key token (requires admin token)
curl -X GET "http://localhost:8080/v1/auth/token?key=order-id" \
  -H "Authorization: Bearer <admin_token>"

# Generate auto-increment IDs
curl -X GET "http://localhost:8080/v1/id/increment?key=order-id&size=5" \
  -H "Authorization: Bearer <key_token>"
```

## Configuration

The Worker receives its configuration from the Controller, including:
- ID generation rules for each key
- Authentication settings
- Worker coordination parameters

## Related Components

- [Controller](../controller/README.md) — Manages configuration and worker coordination
- [Design Document](../proposal/design/001-basic-design.md) — Full API specification
