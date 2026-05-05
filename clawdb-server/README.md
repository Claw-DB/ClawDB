# clawdb-server

`clawdb-server` exposes ClawDB over:
- gRPC on `50050`
- HTTP REST on `8080`
- Prometheus metrics on `9090`

## Features

- Typed gRPC API with tonic reflection enabled
- HTTP API with auth middleware and structured JSON errors
- Per-token rate limiting for gRPC and HTTP
- Optional gRPC TLS via `CLAW_TLS_CERT_PATH` and `CLAW_TLS_KEY_PATH`
- Graceful shutdown for SIGINT/SIGTERM

## Run

```bash
cargo run -p clawdb-server
```

## Key Endpoints

- `GET /v1/health`
- `GET /v1/ready`
- `GET /v1/metrics`

## Configuration

Set `CLAW_GUARD_JWT_SECRET` and optional `CLAW_*` environment variables.

See the workspace root `README.md` for Docker/Kubernetes deployment examples.
