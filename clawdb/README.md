# clawdb

`clawdb` is the unified runtime crate for ClawDB.

It composes the published component crates into one API:
- `claw-core` for durable memory storage
- `claw-vector` for semantic indexing/search
- `claw-branch` for branch/fork/merge workflows
- `claw-sync` for synchronization
- `claw-guard` for authz/session controls
- `claw-reflect-client` for reflection jobs

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

    db.remember(&session, "ClawDB unified runtime works").await?;

    let hits = db.search(&session, "unified runtime").await?;
    println!("hits={}", hits.len());

    db.close().await?;
    Ok(())
}
```

## Configuration

Use `ClawDBConfig::from_env()` or provide a config file.

Required environment variable:
- `CLAW_GUARD_JWT_SECRET`

See the workspace root `README.md` for deployment and server/CLI usage.
