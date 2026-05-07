# clawdb

`clawdb` is the unified runtime crate for ClawDB.

It composes all published component crates into a single, coherent API surface:

| Component crate | Capability |
|---|---|
| `claw-core` | Durable memory storage (SQLite-backed) |
| `claw-vector` | Semantic indexing & search (optional) |
| `claw-branch` | Branch / fork / merge / diff / archive workflows |
| `claw-sync` | Hub synchronisation (push / pull / reconcile) |
| `claw-guard` | JWT sessions, auth, RBAC scopes |
| `claw-reflect-client` | Reflection jobs, facts, preferences, contradictions |

## Install

```bash
cargo add clawdb
```

## Quick Start

```rust
use clawdb::prelude::*;

#[tokio::main]
async fn main() -> ClawDBResult<()> {
    let db = ClawDB::open_default().await?;

    let session = db
        .session(uuid::Uuid::new_v4(), "agent", vec!["*".to_string()])
        .await?;

    // Store memories
    let mem = db.remember(&session, "ClawDB unified runtime works").await?;
    db.remember_typed(&session, "typed note", "episodic", vec!["tag1".into()], None).await?;
    db.update_memory(&session, mem.id, "ClawDB unified runtime — updated").await?;

    // Search & recall
    let hits = db.search(&session, "unified runtime").await?;
    let record = db.recall_one(&session, mem.id).await?;
    let records = db.recall(&session, &[mem.id]).await?;
    println!("hits={}, record={}", hits.len(), record.content);

    // Branches
    let branch = db.branch(&session, "feature-x", None).await?;
    let same = db.get_branch_by_name(&session, "feature-x").await?;
    let trunk = db.trunk_branch(&session).await?;
    db.archive_branch(&session, branch.id).await?;

    // Transactions
    let tx = db.transaction(&session).await?;
    // ... stage memories on the tx then commit/rollback

    // Sync (if hub is configured)
    let status = db.sync_status(&session).await?;
    println!("pending_push={}", status.pending_push.unwrap_or(0));

    // Sessions
    let count = db.active_session_count().await?;
    println!("active sessions: {count}");

    db.close().await?;
    Ok(())
}
```

## Full API Surface

### Memory
| Method | Description |
|---|---|
| `remember(session, content)` | Store a memory |
| `remember_typed(session, content, type, tags, metadata)` | Store a typed memory |
| `update_memory(session, id, content)` | Update memory content |
| `search(session, query)` | Full-text / semantic search |
| `search_with_options(session, query, top_k, semantic, filter)` | Search with options |
| `recall(session, ids)` | Retrieve multiple memories by ID |
| `recall_one(session, id)` | Retrieve a single memory by ID |
| `list_memories(session, type, limit)` | List memories with filters |
| `delete_memory(session, id)` | Delete a memory |

### Branches
| Method | Description |
|---|---|
| `branch(session, name, from)` | Create / fork a branch |
| `fork_branch(session, parent_id, name)` | Fork from a specific branch |
| `get_branch(session, id)` | Get branch by ID |
| `get_branch_by_name(session, name)` | Get branch by name |
| `trunk_branch(session)` | Get the trunk branch |
| `list_branches(session)` | List all branches |
| `merge(session, branch_id)` | Merge branch into trunk |
| `merge_with_strategy(session, branch_id, strategy)` | Merge with conflict strategy |
| `diff(session, branch_id)` | Show diff between branch and trunk |
| `discard_branch(session, id)` | Delete a branch |
| `archive_branch(session, id)` | Archive a branch |

### Transactions
| Method | Description |
|---|---|
| `transaction(session)` | Begin a new transaction |
| (tx methods on the returned tx handle) | Stage, commit, rollback |

### Sync
| Method | Description |
|---|---|
| `sync(session)` | Bidirectional sync |
| `push_sync(session)` | Push local changes to hub |
| `pull_sync(session)` | Pull remote changes from hub |
| `reconcile_sync(session)` | Reconcile conflicts |
| `sync_status(session)` | Get sync status |

### Reflect
| Method | Description |
|---|---|
| `reflect(session)` | Trigger a reflection job |
| `reflect_get_facts(session, agent_id)` | Get extracted facts |
| `reflect_list_jobs(session, agent_id, status, limit)` | List reflection jobs |
| `reflect_get_job(session, job_id)` | Get job details |
| `reflect_get_preferences(session, agent_id)` | Get agent preferences |
| `reflect_get_contradictions(session, agent_id)` | Get contradictions |
| `reflect_resolve_contradiction(session, agent_id, contradiction_id, strategy, merged)` | Resolve contradiction |

### Sessions & Auth
| Method | Description |
|---|---|
| `session(agent_id, role, scopes)` | Create a session (1 hr TTL) |
| `session_with_ttl(agent_id, role, scopes, ttl_secs)` | Create a session with custom TTL |
| `validate_session(token)` | Validate a session token |
| `revoke_session(session, token)` | Revoke a session |
| `active_session_count()` | Count of active (non-expired) sessions |

### Lifecycle
| Method | Description |
|---|---|
| `health()` | Component health booleans |
| `close()` / `shutdown()` | Graceful shutdown |

## Configuration

Use `ClawDBConfig::from_env()` or provide a config file.

Required environment variable:
- `CLAW_GUARD_JWT_SECRET`

See the workspace root `README.md` for deployment and server/CLI usage.
