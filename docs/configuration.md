# Configuration

ClawDB server can load configuration from a TOML file with `--config` and then apply environment overrides, or it can start directly from environment variables.

## Startup

Run the server directly:

```bash
cargo run -p clawdb-server -- --config /path/to/config.toml
```

Override ports without editing the config file:

```bash
cargo run -p clawdb-server -- --grpc-port 50050 --http-port 8080 --metrics-port 9090
```

Generate a development TLS certificate pair:

```bash
cargo run -p clawdb-server -- --config /path/to/config.toml --generate-self-signed-cert
```

## Environment Variables

The server process reads these variables directly:

- `CLAW_DATA_DIR`: root state directory.
- `CLAW_GRPC_PORT`: gRPC listen port.
- `CLAW_HTTP_PORT`: HTTP REST listen port.
- `CLAW_METRICS_PORT`: Prometheus scrape listen port.
- `CLAW_TLS_CERT_PATH`: optional PEM certificate path for gRPC TLS.
- `CLAW_TLS_KEY_PATH`: optional PEM private key path for gRPC TLS.

The shared `clawdb` runtime also honors these overrides:

- `CLAW_WORKSPACE_ID`
- `CLAW_AGENT_ID`
- `CLAW_LOG_LEVEL`
- `CLAW_LOG_FORMAT`
- `CLAW_VECTOR_BASE_URL`
- `CLAW_VECTOR_ENABLED`
- `CLAW_SYNC_HUB_URL`
- `CLAW_GUARD_JWT_SECRET`
- `CLAW_GUARD_POLICY_DIR`
- `CLAW_REFLECT_BASE_URL`
- `CLAW_REFLECT_API_KEY`

`CLAW_GUARD_JWT_SECRET` is required unless the loaded config file already provides a non-default value.

## Minimal `config.toml`

```toml
data_dir = "/var/lib/clawdb"
workspace_id = "11111111-1111-1111-1111-111111111111"
agent_id = "22222222-2222-2222-2222-222222222222"
log_level = "info"
log_format = "json"

[core]
db_path = "/var/lib/clawdb/claw.db"
max_connections = 10
wal_enabled = true
cache_size_mb = 64

[vector]
enabled = false
db_path = "/var/lib/clawdb/claw_vector.db"
index_dir = "/var/lib/clawdb/claw_vector_indices"
embedding_service_url = "http://localhost:50051"
default_dimensions = 384

[branch]
branches_dir = "/var/lib/clawdb/branches"
registry_db_path = "/var/lib/clawdb/branches/branch_registry.db"
max_branches_per_workspace = 50
gc_interval_secs = 3600
trunk_branch_name = "trunk"

[sync]
hub_url = ""
data_dir = "/var/lib/clawdb/sync"
db_path = "/var/lib/clawdb/claw.db"
sync_interval_secs = 30
tls_enabled = false
connect_timeout_secs = 10
request_timeout_secs = 30
max_delta_rows = 1000
max_chunk_bytes = 65536
max_pull_chunks = 128
max_push_inflight = 4

[guard]
db_path = "/var/lib/clawdb/claw_guard.db"
jwt_secret = "replace-me"
policy_dir = "/var/lib/clawdb/policies"
sensitive_resources = ["memory", "branch"]
audit_flush_interval_ms = 100
audit_batch_size = 500
tls_cert_path = "/var/lib/clawdb/certs/server.crt"
tls_key_path = "/var/lib/clawdb/certs/server.key"

[reflect]
base_url = ""
api_key = ""

[server]
grpc_port = 50050
http_port = 8080

[plugins]
plugins_dir = "/var/lib/clawdb/plugins"
enabled = []
sandbox_enabled = true

[telemetry]
metrics_port = 9090
otel_endpoint = ""
service_name = "clawdb"
```

Use empty strings only when your deployment template requires them. In committed config files, prefer omitting optional values entirely.

## Interfaces

- gRPC: `0.0.0.0:50050`
- HTTP REST: `0.0.0.0:8080`
- Metrics: `0.0.0.0:9090`

The REST surface exposes `/v1/health`, `/v1/metrics`, session management, memory APIs, branch APIs, sync, and reflect. The metrics listener exposes `/`, `/metrics`, and `/v1/metrics` for scrape compatibility.
