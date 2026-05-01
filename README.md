# ClawDB
```
   ________                ____  ____
  / ____/ /___ ___      __/ __ \/ __ )
 / /   / / __ `/ / | /| / / / / / __  |
/ /___/ / /_/ / /| |/ |/ / /_/ / /_/ /
\____/_/\__,_/ / |__/|__/_____/_____/
```

The cognitive database for AI agents.

ClawDB is a production-grade memory runtime that unifies durable storage, semantic retrieval, branch/merge workflows, synchronization, reflection pipelines, and policy governance in one API and one operational surface.

## Features

| Capability | Status | Description |
| --- | --- | --- |
| Storage (`claw-core`) | ✅ | Durable, queryable memory with SQLite-backed persistence |
| Semantic Memory (`claw-vector`) | ✅ | Embedding-powered retrieval and approximate nearest-neighbor search |
| Sync (`claw-sync`) | ✅ | Hub-based and peer-oriented memory synchronization |
| Branching (`claw-branch`) | ✅ | Snapshot/fork/merge semantics for experimentation and replay |
| Reflection (`claw-reflect`) | ✅ | Automated distillation, summarization, and memory curation jobs |
| Governance (`claw-guard`) | ✅ | Role and policy enforcement, scoped sessions, and access control |

## Quick Start

1. Add the crate:

```bash
cargo add clawdb
```

2. Use ClawDB in your app:

```rust
use clawdb::prelude::*;

#[tokio::main]
async fn main() -> ClawDBResult<()> {
	let db = ClawDB::open_default().await?;
	let session = db.session(uuid::Uuid::new_v4(), "assistant", vec!["memory:write".into()]).await?;
	let _ = db.remember(&session, "Hello ClawDB").await?;
	let hits = db.search(&session, "hello").await?;
	println!("It works: {} result(s)", hits.len());
	db.close().await
}
```

3. Run:

```bash
cargo run
```

Expected output includes: `It works: <n> result(s)`.

## Installation

### From crates.io

```bash
cargo add clawdb
```

### From binary releases

Download artifacts from GitHub Releases and add `clawdb`, `clawdb-cli`, and `clawdb-server` to your `PATH`.

### From Docker

```bash
docker build -t clawdb:latest .
docker run --rm -p 50050:50050 -p 8080:8080 -p 9090:9090 clawdb:latest
```

## Architecture

```
				 ┌─────────────────────┐
				 │       ClawDB         │
				 │  Unified Runtime     │
				 └──────────┬──────────┘
	  ┌─────────────────────┼─────────────────────┐
	  │           │         │         │            │
 ┌────▼───┐ ┌─────▼───┐ ┌──▼─────┐ ┌─▼──────┐ ┌──▼──────┐ ┌──▼────┐
 │  core  │ │ vector  │ │  sync  │ │ branch │ │ reflect │ │ guard │
 └────────┘ └─────────┘ └────────┘ └────────┘ └─────────┘ └───────┘
```

## API Reference

Primary engine methods:

1. `async fn new(config: ClawDBConfig) -> ClawDBResult<ClawDB>`
2. `async fn open_default() -> ClawDBResult<ClawDB>`
3. `async fn open(data_dir: &Path) -> ClawDBResult<ClawDB>`
4. `async fn session(agent_id: Uuid, role: &str, scopes: Vec<String>) -> ClawDBResult<ClawDBSession>`
5. `async fn remember(session: &ClawDBSession, content: &str) -> ClawDBResult<RememberResult>`
6. `async fn remember_typed(session: &ClawDBSession, content: &str, memory_type: &str, tags: &[String], metadata: serde_json::Value) -> ClawDBResult<RememberResult>`
7. `async fn search(session: &ClawDBSession, query: &str) -> ClawDBResult<Vec<serde_json::Value>>`
8. `async fn search_with_options(session: &ClawDBSession, query: &str, top_k: usize, semantic: bool, filter: Option<serde_json::Value>) -> ClawDBResult<Vec<serde_json::Value>>`
9. `async fn recall(session: &ClawDBSession, memory_ids: &[String]) -> ClawDBResult<Vec<serde_json::Value>>`
10. `async fn branch(session: &ClawDBSession, name: &str) -> ClawDBResult<Uuid>`
11. `async fn merge(session: &ClawDBSession, source: Uuid, target: Uuid) -> ClawDBResult<serde_json::Value>`
12. `async fn sync(session: &ClawDBSession) -> ClawDBResult<serde_json::Value>`

Additional methods include `diff`, `reflect`, `validate_session`, `revoke_session`, `health`, `close`, `shutdown`, and `transaction`.

## Configuration Reference

Top-level `ClawDBConfig` fields:

1. `data_dir` (`CLAW_DATA_DIR`)
2. `workspace_id` (`CLAW_WORKSPACE_ID`)
3. `agent_id` (`CLAW_AGENT_ID`)
4. `log_level` (`CLAW_LOG_LEVEL`)
5. `log_format` (`CLAW_LOG_FORMAT`)
6. `core`
7. `vector`
8. `sync`
9. `branch`
10. `guard`
11. `reflect`
12. `server`
13. `plugins`
14. `telemetry`

## CLI Reference

Commands:

1. `clawdb init`
2. `clawdb start`
3. `clawdb status`
4. `clawdb remember`
5. `clawdb search`
6. `clawdb branch`
7. `clawdb sync`
8. `clawdb reflect`
9. `clawdb policy`
10. `clawdb config`

Examples:

```bash
clawdb init --with-reflect
clawdb start --grpc-port 50050 --http-port 8080
clawdb remember "deploy started" --type event --tags deploy,prod
clawdb search "deploy" --top-k 5 --semantic
clawdb branch create hotfix-42 --from trunk
```

## Plugin Development Guide

Implement `ClawPlugin` from `clawdb::plugins::interface` and provide a plugin manifest:

```toml
name = "my_plugin"
version = "0.1.0"
description = "Example ClawDB plugin"
capabilities = ["ReadMemory", "EmitEvents"]
entry_symbol = "create_plugin"
```

Build as dynamic library and place under `plugins_dir`.

## Deployment Guide

### Docker Compose Quick Start

```bash
docker compose up -d --build
```

### Kubernetes

```bash
kubectl apply -f k8s/deployment.yaml
kubectl apply -f k8s/service.yaml
kubectl apply -f k8s/hpa.yaml
```

## Ecosystem

| Crate | Role | Version |
| --- | --- | --- |
| `claw-core` | Durable storage | ![v](https://img.shields.io/badge/version-0.1.0-blue) |
| `claw-vector` | Semantic retrieval | ![v](https://img.shields.io/badge/version-0.1.0-blue) |
| `claw-sync` | Synchronization | ![v](https://img.shields.io/badge/version-0.1.0-blue) |
| `claw-branch` | Branch/merge runtime | ![v](https://img.shields.io/badge/version-0.1.0-blue) |
| `claw-guard` | Governance and access control | ![v](https://img.shields.io/badge/version-0.1.0-blue) |
| `clawdb` | Unified runtime API | ![v](https://img.shields.io/badge/version-0.1.0-blue) |
