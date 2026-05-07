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

## Health and Metrics

- `GET /v1/health`
- `GET /v1/ready`
- `GET /v1/metrics`

## Request Authentication and Metadata

Protected endpoints require either:
- HTTP bearer auth (`Authorization: Bearer <token>`) or session passthrough header (`x-claw-session`)
- gRPC metadata (`authorization: Bearer <token>` or `x-claw-session`)

Optional correlation metadata:
- `x-request-id` on both HTTP and gRPC

## gRPC API Surface (`clawdb.v1.ClawDBService`)

### Health and Sessions
- `Health`
- `CreateSession`
- `ValidateSession`
- `RevokeSession`
- `ActiveSessionCount`

### Memory
- `Remember`
- `RememberTyped`
- `UpdateMemory`
- `Search`
- `Recall`
- `ListMemories`
- `DeleteMemory`

### Branching
- `Branch`
- `GetBranch`
- `GetBranchByName`
- `GetTrunkBranch`
- `ListBranches`
- `DiscardBranch`
- `ArchiveBranch`
- `Merge`
- `Diff`

### Sync
- `Sync`
- `PushSync`
- `PullSync`
- `ReconcileSync`
- `SyncStatus`

### Reflect
- `Reflect`
- `ReflectGetFacts`
- `ReflectListJobs`
- `ReflectGetJob`
- `ReflectGetPreferences`
- `ReflectGetContradictions`
- `ReflectResolveContradiction`

### Transactions
- `BeginTx`
- `TxRemember`
- `TxRememberTyped`
- `CommitTx`
- `RollbackTx`

## HTTP REST API Surface

### Public
- `GET /v1/health`
- `GET /v1/ready`
- `GET /v1/metrics`

### Sessions
- `GET /v1/sessions/me`
- `DELETE /v1/sessions/:id`
- `GET /v1/sessions/active/count`

### Memory
- `POST /v1/memories`
- `GET /v1/memories`
- `GET /v1/memories/search`
- `GET /v1/memories/:id`
- `PATCH /v1/memories/:id`
- `DELETE /v1/memories/:id`

### Branching
- `POST /v1/branches`
- `GET /v1/branches`
- `GET /v1/branches/trunk`
- `GET /v1/branches/by-name/:name`
- `GET /v1/branches/:id`
- `DELETE /v1/branches/:id`
- `POST /v1/branches/:id/archive`
- `POST /v1/branches/:id/merge`
- `GET /v1/branches/:id/diff`

### Sync
- `POST /v1/sync`
- `POST /v1/sync/push`
- `POST /v1/sync/pull`
- `POST /v1/sync/reconcile`
- `GET /v1/sync/status`

### Reflect
- `POST /v1/reflect`
- `GET /v1/reflect/jobs`
- `GET /v1/reflect/jobs/:job_id`
- `GET /v1/reflect/facts/:agent_id`
- `GET /v1/reflect/preferences/:agent_id`
- `GET /v1/reflect/contradictions/:agent_id`
- `POST /v1/reflect/contradictions/:agent_id/:contradiction_id/resolve`

### Transactions
- `POST /v1/tx`
- `POST /v1/tx/:id/memories`
- `POST /v1/tx/:id/memories/typed`
- `POST /v1/tx/:id/commit`
- `POST /v1/tx/:id/rollback`

## Configuration

Set `CLAW_GUARD_JWT_SECRET` and optional `CLAW_*` environment variables.

See the workspace root `README.md` for Docker/Kubernetes deployment examples.
