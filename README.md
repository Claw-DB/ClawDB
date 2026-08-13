# ClawDB
```
   ________                ____  ____
  / ____/ /___ ___      __/ __ \/ __ )
 / /   / / __ `/ / | /| / / / / / __  |
/ /___/ / /_/ / /| |/ |/ / /_/ / /_/ /
\____/_/\__,_/ / |__/|__/_____/_____/
```

[![crates.io](https://img.shields.io/crates/v/clawdb.svg)](https://crates.io/crates/clawdb)
[![docs.rs](https://img.shields.io/docsrs/clawdb)](https://docs.rs/clawdb)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://github.com/Claw-DB/ClawDB/blob/main/LICENSE)
[![CI](https://github.com/Claw-DB/ClawDB/actions/workflows/ci.yml/badge.svg)](https://github.com/Claw-DB/ClawDB/actions/workflows/ci.yml)

The persistent memory layer AI agents can actually own.

ClawDB is a production-grade memory runtime that unifies durable storage, semantic retrieval, branch/merge workflows, synchronization, reflection pipelines, and policy governance in one API and one operational surface — self-hosted or embedded, not rented from someone else's cloud.

## Why ClawDB

Most agent-memory tools on the market today (Mem0, Zep, Letta, and similar) are built as a **rented utility**: memory lives in someone else's cloud, under someone else's pricing, and serves a single agent in a single app. Stop paying, and the context goes with it.

ClawDB is built differently — as infrastructure a developer can run and own:

- **Self-hosted by default.** Embed `clawdb` directly in a Rust app, or run it as a standalone server via Docker/Kubernetes — no mandatory cloud dependency.
- **One API, not five stitched-together vendors.** Storage, semantic retrieval, branching, sync, reflection, and governance live behind one operational surface.
- **Memory that compounds instead of resetting.** Branch/merge and reflection pipelines let memory improve over time rather than starting from zero on every new session.
- **A path to memory that moves.** See [Roadmap](#roadmap) — memory built for one agent shouldn't have to die with that agent.

## Features

| Capability | Status | Description |
| --- | --- | --- |
| Storage (`claw-core`) | ✅ | Durable, queryable memory with SQLite-backed persistence |
| Semantic Memory (`claw-vector`) | ✅ | Embedding-powered retrieval and approximate nearest-neighbor search |
| Sync (`claw-sync`) | ✅ | Hub-based and peer-oriented memory synchronization |
| Branching (`claw-branch`) | ✅ | Snapshot/fork/merge semantics for experimentation and replay |
| Reflection (`claw-reflect`) | ✅ | Automated distillation, summarization, and memory curation jobs |
| Governance (`claw-guard`) | ✅ | Role and policy enforcement, scoped sessions, and access control |
| Multi-language client SDKs & framework adapters | ✅ | TypeScript, Python, Rust, Go clients; LangChain, OpenAI Agents, Vercel AI, Anthropic, Google GenAI, OpenClaw adapters; MCP server — see [Ecosystem](#ecosystem) |
| Memory Marketplace | 🧭 Planned | Agent-to-agent memory licensing — see [Roadmap](#roadmap) |

## Ecosystem

**This repo is the engine** — the `clawdb` runtime crate, the `clawdb-server` network binary, and the `clawdb-cli` HTTP client. It's one piece of a larger project split across a few repos so each part can move at its own pace:

| Repo | What it is | Use it when |
| --- | --- | --- |
| **[Claw-DB/ClawDB](https://github.com/Claw-DB/ClawDB)** *(this repo)* | The Rust engine: embeddable `clawdb` crate, `clawdb-server`, `clawdb-cli` | You want to embed ClawDB in a Rust binary, or run/self-host `clawdb-server` |
| **[Claw-DB/claw-sdk](https://github.com/Claw-DB/claw-sdk)** | Client SDKs (`@clawdb/sdk` TS, `clawdb` Python, `clawdb-client` Rust, Go), the `@clawdb/cli` init/dev-experience CLI, the stdio MCP adapter, and framework adapters (LangChain, OpenAI Agents, Vercel AI, Anthropic, Google GenAI, OpenClaw) | You're building an agent in TypeScript/Python/Go and want to talk to a running `clawdb-server` over the network, or you want your assistant wired into Claude Desktop/Cursor/VS Code/Zed over MCP |
| **[ClawDB Cloud](https://clawdb.dev)** | Managed hosting for teams who'd rather not run `clawdb-server` themselves — adds a hosted MCP gateway, a dashboard, and billing on top of this same engine | You want zero infrastructure to manage |
| `claw-core`, `claw-vector`, `claw-guard`, `claw-branch`, `claw-reflect`, `claw-sync` | The individual engine subsystems this repo's `clawdb` crate wires together (see [Workspace Crates](#workspace-crates) and [Architecture](#architecture)) | You need one subsystem in isolation, or you're contributing to the engine itself |

Full documentation for all of the above lives at **[docs.clawdb.dev](https://docs.clawdb.dev)**.

## Workspace Crates

ClawDB is a multi-crate workspace. Each crate has a focused responsibility and can be consumed independently when needed.

| Crate | Type | Purpose | Main Artifact |
| --- | --- | --- | --- |
| `clawdb` | Library + bin | Unified runtime facade over memory, search, branch, sync, and guard | `libclawdb` + `clawdb` |
| `clawdb-server` | Binary + lib | Network server exposing HTTP + gRPC APIs and metrics | `clawdb-server` |
| `clawdb-cli` | Binary | Pure HTTP client for operational and developer workflows | `clawdb` / `clawdb-cli` |

### `clawdb` (Runtime API)

The `clawdb` crate is the top-level API for embedding ClawDB in Rust applications.

- Exposes session-oriented methods (`session`, `validate_session`, `revoke_session`)
- Exposes memory methods (`remember`, `remember_typed`, `search`, `recall`)
- Exposes branch methods (`branch`, `fork_branch`, `merge`, `diff`, `list_branches`)
- Exposes sync/reflect entrypoints (`sync`, `reflect`)
- Exposes health and telemetry hooks (`health`, `metrics_handle`)

Every call takes an explicit `&ClawDBSession` — this is a lower-level, embed-it-yourself API. If you'd rather connect to a *running* `clawdb-server` over the network (from Rust or another language) without managing sessions by hand, use `claw-sdk`'s client SDKs instead — see [Ecosystem](#ecosystem).

Primary path in this repo: `clawdb/src`.

### `clawdb-server` (HTTP + gRPC Surface)

The `clawdb-server` crate hosts the runtime as network services.

- HTTP API for clients and automation
- gRPC API with reflection support
- Prometheus metrics endpoint
- Config-driven startup with env overrides (see [Configuration Reference](#configuration-reference))

Primary paths in this repo:

- `clawdb-server/src/http`
- `clawdb-server/src/grpc`
- `clawdb-server/src/state.rs`

### `clawdb-cli` (Pure HTTP Client)

The `clawdb-cli` crate is intentionally decoupled from runtime internals.

- Talks to `clawdb-server` over HTTP only
- No direct linkage to component crates at call time
- Supports table/json/tsv output modes
- 17 top-level commands covering the full server API — see [CLI Reference](#cli-reference)

Primary path in this repo: `clawdb-cli/src`.

This design keeps the CLI small and stable while allowing server/runtime internals to evolve independently.

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

Not writing Rust, or want a server you can point multiple clients at? Run `clawdb-server` (see [Installation](#installation) and [Deployment Guide](#deployment-guide)) and connect to it with one of `claw-sdk`'s client libraries instead — see [Ecosystem](#ecosystem).

## Installation

### From crates.io

```bash
cargo add clawdb
```

### From binary releases

Download `clawdb`, `clawdb-cli`, and `clawdb-server` artifacts from [GitHub Releases](https://github.com/Claw-DB/ClawDB/releases) and add them to your `PATH`.

### From Docker

Single container, all three ports:

```bash
docker build -t clawdb:latest .
docker run --rm -p 50050:50050 -p 8080:8080 -p 9090:9090 clawdb:latest
```

Or the full stack — `clawdb-server` plus Prometheus and Grafana, pre-provisioned with a ClawDB dashboard (`deploy/grafana/provisioning/`):

```bash
docker compose up -d --build
```

This starts `clawdb` (ports `50050`/`8080`/`9090`), `prometheus` (`:9091`), and `grafana` (`:3000`).

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

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the full repo-split rationale, and [docs/PERFORMANCE.md](docs/PERFORMANCE.md) for benchmark methodology (results are still being gathered — treat that page as in progress, not a finished number).

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

Additional methods include `diff`, `reflect`, `validate_session`, `revoke_session`, `health`, `close`, `shutdown`, and `transaction`. Full generated reference: [docs.rs/clawdb](https://docs.rs/clawdb).

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

`clawdb-server` additionally reads `CLAW_GRPC_PORT`, `CLAW_HTTP_PORT`, `CLAW_METRICS_PORT`, `CLAW_TLS_CERT_PATH`, and `CLAW_TLS_KEY_PATH` directly — see [docs/configuration.md](docs/configuration.md) for the full server-specific reference, including TOML config file loading and self-signed cert generation for local TLS testing.

## CLI Reference

`clawdb-cli` (installed as the `clawdb` binary) has 17 top-level commands:

| Command | Purpose |
| --- | --- |
| `init` | Initialize `~/.clawdb` and its config file |
| `start` / `stop` | Start/stop a detached `clawdb-server` process |
| `status` | Check server component health |
| `session` | Create / revoke / whoami |
| `remember` | Store a memory |
| `search` | Semantic or full-text search |
| `recall` | Retrieve one or more memories by ID |
| `memory` | Grouped memory operations (remember/search/list/delete) |
| `branch` | create / list / get / trunk / by-name / merge / diff / archive / discard |
| `sync` | Synchronize with the hub |
| `reflect` | Trigger and query reflection jobs |
| `tx` | Transactions: begin / stage / commit / rollback |
| `policy` | Manage access control policies |
| `config` | Read or write local CLI configuration |
| `mcp` | Install/print MCP editor configuration |
| `completion` | Generate shell completion scripts |

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

### Docker Compose

```bash
docker compose up -d --build
```

Brings up `clawdb-server` plus a pre-provisioned Prometheus + Grafana stack (see [Installation](#installation)).

### Kubernetes

Manifests live in `k8s/`. Apply in dependency order — `namespace` first, then the config/secret/storage/identity objects the `Deployment` references, then the workload itself:

```bash
kubectl apply -f k8s/namespace.yaml
kubectl apply -f k8s/configmap.yaml
kubectl apply -f k8s/secret.yaml        # edit placeholder values first
kubectl apply -f k8s/pvc.yaml
kubectl apply -f k8s/serviceaccount.yaml
kubectl apply -f k8s/deployment.yaml
kubectl apply -f k8s/service.yaml
kubectl apply -f k8s/hpa.yaml
```

Or simply `kubectl apply -f k8s/` to apply all eight at once (Kubernetes tolerates the ordering within a single `apply` call across dependent objects).

## Roadmap

| Initiative | Status | Description |
| --- | --- | --- |
| Decentralized Sync (Filecoin) | 🧭 Planned | A `claw-sync` backend that replicates encrypted memory to Filecoin, so durability doesn't depend on ClawDB's own infrastructure — see [docs/DECENTRALIZED_STORAGE.md](docs/DECENTRALIZED_STORAGE.md) |
| Memory Marketplace | 🧭 Planned | Agents license reusable, distilled memory (a solved debugging trace, a research summary) to other agents at the moment they need it, instead of every agent re-deriving the same context from scratch |

Everything else on this page (storage, semantic memory, sync, branching, reflection, governance) is implemented today — see [Features](#features). Multi-language client SDKs, framework adapters (LangChain, OpenAI Agents, Vercel AI, Anthropic, Google GenAI, OpenClaw), and MCP support are also already shipped, in `claw-sdk` rather than this repo — see [Ecosystem](#ecosystem).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for dev setup, the PR checklist, and the crate dependency diagram. For security issues, open a private GitHub security advisory rather than a public issue.

## License

Apache-2.0, as declared in `Cargo.toml`.
