# Architecture

ClawDB should be experienced as one intelligent database, but built internally as a modular ecosystem of focused repositories. That split gives the platform faster development, clearer ownership, easier testing, independent release cadence, and a more contributor-friendly architecture without fragmenting the user experience.

## Recommended Repository Structure

```text
clawdb                ← Aggregate runtime / public product
claw-core             ← Embedded storage engine
claw-vector           ← Semantic memory engine
claw-sync             ← Local ↔ cloud sync engine
claw-branch           ← Fork / simulate / merge engine
claw-reflect          ← Autonomous summarization engine
claw-guard            ← Security / policy engine
claw-cloud            ← Managed cloud platform
claw-console          ← GUI / observability dashboard
claw-sdk              ← Developer SDKs / CLI
claw-bench            ← Benchmarks / test suite
claw-examples         ← Example apps / templates
```

## Design Principle

Externally, users install and operate ClawDB as one product.

Internally, ClawDB is a coordinated family of systems:

1. Runtime and API surface in `clawdb`
2. Specialized execution engines in subsystem repositories
3. Platform, tooling, and adoption layers around the runtime

This separation keeps the runtime coherent while allowing each subsystem to evolve independently.

## Repository Breakdown

### 1. `clawdb`

Aggregate layer and unified runtime.

Purpose:
The main product repository and the public identity of the platform. This is the repo users feel when they install ClawDB.

Core responsibilities:
1. Unified runtime
2. Query routing
3. Memory planner
4. Transaction orchestration
5. Plugin loading
6. Config management
7. Lifecycle management
8. Session management
9. Event bus
10. Component coordination
11. Public APIs
12. CLI entrypoint

What it solves:
Without this layer, users would have to wire storage, retrieval, sync, branching, and policy systems by hand.

Suggested stack:
1. Rust
2. Tokio
3. Serde
4. Tonic / gRPC
5. tracing
6. Plugin system

Why it matters:
This is the operating runtime of the ecosystem and the contract surface for end users.

### 2. `claw-core`

Embedded local database engine.

Purpose:
The local-first, high-speed storage engine for active memory and short-latency execution.

Core responsibilities:
1. SQLite integration
2. Local tables
3. WAL journaling
4. Transactions
5. Migrations
6. Cache layer
7. Snapshots
8. Schema tools
9. Lightweight indexing
10. Local persistence

Primary stores:
1. Active tasks
2. Tool outputs
3. Session state
4. Temporary context
5. Recent messages

Suggested stack:
1. Rust
2. SQLite
3. `sqlx` or `rusqlite`

Why it matters:
Agents need instant local memory without depending on network round-trips.

### 3. `claw-vector`

Semantic memory engine.

Purpose:
Transforms raw memory into meaning-based, associative, searchable knowledge.

Core responsibilities:
1. Embedding generation
2. Vector storage
3. ANN indexing
4. Semantic search
5. Hybrid search
6. Metadata filters
7. Reranking
8. Local model support
9. Cloud embedding adapters

Example queries:
1. What did the user say about pricing?
2. Which bugs have I solved before that resemble this one?
3. Which leads sound urgent?

Suggested stack:
1. Rust indexing engine
2. Python model service
3. ONNX Runtime
4. `sentence-transformers`
5. HNSW index
6. Tantivy for optional keyword search

Why it matters:
Agents need associative recall, not just exact-match lookups.

### 4. `claw-sync`

Local to cloud synchronization engine.

Purpose:
Synchronizes local memory with remote infrastructure securely, efficiently, and with offline tolerance.

Core responsibilities:
1. Encrypted replication
2. Delta sync
3. Offline queueing
4. Resumable sync
5. Conflict resolution
6. Multi-device continuity
7. State reconciliation
8. Sync logs
9. Workspace sharing

Example use cases:
1. Continue an agent session on another device
2. Recover local state after reset
3. Share workspace state across a team

Suggested stack:
1. Rust
2. gRPC
3. Protocol Buffers
4. CRDT strategies
5. `libsodium` or `ring`

Why it matters:
Local-first systems need durable continuity, not just local persistence.

### 5. `claw-branch`

Fork, simulate, and merge engine.

Purpose:
Allows agents to test multiple paths safely before taking irreversible action.

Core responsibilities:
1. Database snapshots
2. Isolated branches
3. Simulation sandboxes
4. Branch lineage graph
5. Branch comparisons
6. Merge engine
7. Selective commit
8. Discard failed branches
9. Metrics tracking

Example use cases:
1. Test prompts
2. Compare strategies
3. Simulate workflows
4. Evaluate plans

Suggested stack:
1. Rust
2. SQLite snapshots
3. Diff engine
4. DAG lineage model

Why it matters:
Planning agents outperform purely reactive agents.

### 6. `claw-reflect`

Autonomous memory distillation engine.

Purpose:
Continuously transforms noisy execution logs into useful long-term intelligence.

Core responsibilities:
1. Summarization
2. Preference extraction
3. Contradiction detection
4. Memory scoring
5. Duplicate collapse
6. Profile updates
7. Stale memory decay
8. Scheduled reflection jobs
9. Confidence scoring
10. Compression pipelines

Example outputs:
1. User prefers Python
2. Meetings before 10am are usually disliked
3. Customer churn risk is increasing

Suggested stack:
1. Python
2. FastAPI
3. Celery or Arq
4. Redis
5. PostgreSQL
6. LLM adapters
7. spaCy for optional linguistic pipelines

Why it matters:
Without reflection, memory becomes clutter instead of intelligence.

### 7. `claw-guard`

Security and policy engine.

Purpose:
Controls what agents can access based on task, role, scope, and risk.

Core responsibilities:
1. Policy rules
2. Row-level restrictions
3. Intent-aware access control
4. Tool permission checks
5. Data masking
6. Audit logs
7. Session scopes
8. Risk scoring
9. Enterprise governance hooks

Example rule:

```text
if task = scheduling:
deny finance_records
```

Suggested stack:
1. Rust
2. OPA / Rego or custom DSL
3. JWT / OAuth support
4. PostgreSQL audit tables

Why it matters:
Autonomy without governance produces unsafe systems.

### 8. `claw-cloud`

Managed hosted platform.

Purpose:
Commercial hosted infrastructure for teams, enterprises, and large-scale workloads.

Core responsibilities:
1. Hosted databases
2. Team workspaces
3. Replication regions
4. Backups
5. Auth
6. Billing
7. Usage metering
8. Hosted vectors
9. Managed sync hubs
10. API gateway
11. Enterprise controls

Suggested stack:
1. TypeScript
2. Node.js
3. NestJS or Fastify
4. PostgreSQL
5. `pgvector`
6. Kubernetes
7. Terraform
8. Stripe
9. Cloudflare

Why it matters:
Many customers prefer managed convenience over self-hosting.

### 9. `claw-console`

GUI and observability dashboard.

Purpose:
Visual interface for understanding, debugging, and managing agent cognition.

Core responsibilities:
1. Inspect memory
2. Semantic search explorer
3. Branch comparisons
4. Sync health
5. Policy audit trails
6. Memory timelines
7. Workspace management
8. Usage dashboards
9. Admin controls

Suggested stack:
1. Next.js
2. React
3. TypeScript
4. Tailwind
5. shadcn/ui
6. TanStack Query
7. Monaco
8. Recharts or D3

Why it matters:
Developers need visibility into autonomous behavior.

### 10. `claw-sdk`

Developer SDKs and CLI.

Purpose:
Makes ClawDB easy to integrate into any stack.

Core responsibilities:
1. TypeScript SDK
2. Python SDK
3. Rust crate
4. Go SDK
5. CLI
6. Auth helpers
7. Config tools
8. Migrations
9. Schema generators
10. Framework adapters

Example integrations:
1. LangChain
2. OpenAI Agents
3. MCP
4. Custom applications

Why it matters:
Adoption depends on low friction.

### 11. `claw-bench`

Benchmark and reliability suite.

Purpose:
Public performance and quality measurement layer.

Core responsibilities:
1. Latency benchmarks
2. Recall quality tests
3. Sync reliability tests
4. Branch performance
5. Memory growth tests
6. Contradiction handling tests
7. Load tests
8. Regression checks

Why it matters:
Benchmarks build trust and enforce engineering discipline.

### 12. `claw-examples`

Templates and starter apps.

Purpose:
Help developers adopt ClawDB quickly.

Example projects:
1. Personal assistant
2. Coding copilot
3. Support bot
4. Swarm agents
5. Offline note-memory app
6. CRM memory assistant

Why it matters:
Examples accelerate ecosystem growth and reduce time-to-value.

## Full System Relationship

```text
                    ┌────────────────────┐
                    │       clawdb       │
                    │ Unified Runtime    │
                    └─────────┬──────────┘
                              │
     ┌────────────┬───────────┼───────────┬─────────────┐
     │            │           │           │             │
┌────▼────┐ ┌─────▼────┐ ┌────▼────┐ ┌────▼────┐ ┌─────▼────┐
│ core    │ │ vector   │ │ sync    │ │ branch  │ │ reflect  │
└─────────┘ └──────────┘ └─────────┘ └─────────┘ └──────────┘
                              │
                        ┌─────▼─────┐
                        │ guard     │
                        └───────────┘

     ┌──────────────┬──────────────┬───────────────┬──────────────┐
     │              │              │               │              │
┌────▼────┐   ┌─────▼─────┐  ┌─────▼─────┐  ┌──────▼─────┐  ┌──────▼─────┐
│ cloud   │   │ console   │  │ sdk       │  │ bench      │  │ examples    │
└─────────┘   └───────────┘  └───────────┘  └────────────┘  └────────────┘
```

## Recommended Build Order

### Phase 1: Core Product

1. `clawdb`
2. `claw-core`
3. `claw-vector`
4. `claw-sdk`

Goal:
First usable product.

### Phase 2: Persistence and Learning

5. `claw-sync`
6. `claw-reflect`

### Phase 3: Differentiation

7. `claw-branch`
8. `claw-guard`

### Phase 4: Scale and Adoption

9. `claw-cloud`
10. `claw-console`
11. `claw-bench`
12. `claw-examples`

## Runtime Dependency Graph

```text
application / cli / grpc / http
            |
          ClawDB
  +---------+---------+---------+---------+
  |         |         |         |         |
core      vector    branch     sync     guard
  \         |         |         |         /
   +--------+---------+---------+--------+
                    events
                     |
                   plugins
                     |
                  telemetry
```

## Query Lifecycle

1. Caller invokes `ClawDB::{remember,search,recall,execute}`.
2. Session context is resolved and validated through guard and session manager.
3. Query planner builds a step graph across core, vector, branch, sync, and guard surfaces.
4. Optimizer rewrites or short-circuits plans where possible.
5. Executor runs steps and emits timing and outcome metrics.
6. Event emitter publishes lifecycle events.
7. Result is transformed and returned to API and client layers.

## Event Flow

Typical `remember` path:

1. `SessionCreated` when the session is established in the same request flow
2. `MemoryAdded`
3. Plugin hook dispatches if plugins are active
4. `ComponentHealthChanged` on any state transition triggered during execution

Typical `sync` path:

1. `SyncCompleted`
2. Optional `GuardDenied` if policy blocks a sync operation

## Transaction Model

ClawDB transaction orchestration uses a two-phase-commit style model across `claw-core` and `claw-vector`, and coordinates with branch state when required.

1. `begin` registers transaction context
2. `prepare` validates and acquires locks
3. `commit` writes in deterministic order
4. On error, `rollback` best-effort compensates every touched subsystem

## Plugin Execution Model

1. Plugins are discovered and validated against the sandbox allowlist.
2. `on_load` executes with `PluginContext`.
3. Hook dispatch order is registration order.
4. Hook failures are isolated and logged; they do not abort unrelated plugins.
5. Capabilities are enforced before registration completes.

## Startup and Shutdown Sequence

Startup target timings in a normal environment:

1. Config load and tracing init: under 200ms
2. Sub-engine open (`core`, `vector`, `sync`, `branch`, `guard`): under 3s
3. API listeners ready (`gRPC`, `HTTP`): under 1s after engine open
4. Metrics endpoint ready: under 100ms

Shutdown target timings:

1. Signal capture and cancellation broadcast: under 100ms
2. Listener drain and task stop: under 3s
3. Component stop and final flush: under 10s

## Strategic Summary

ClawDB is not one repository in the long-term target state. It is an ecosystem centered on one runtime that unifies:

1. Storage
2. Memory
3. Retrieval
4. Learning
5. Simulation
6. Governance
7. Synchronization
8. Developer experience

That split optimizes for both startup speed and long-term architectural quality.
