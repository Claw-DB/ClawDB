# Contributing

## Development Setup

1. Install Rust stable toolchain.
2. Clone ClawDB and sibling subsystem repositories (`claw-core`, `claw-vector`, `claw-sync`, `claw-branch`, `claw-guard`) in expected relative paths.
3. Build workspace:

```bash
cargo check --workspace
```

## Running Tests

```bash
cargo test --workspace --all-features -- --nocapture
```

For integration with services:

1. Start PostgreSQL and Redis.
2. Configure `TEST_DATABASE_URL` and `TEST_REDIS_URL`.

## PR Checklist

1. Code compiles (`cargo check --workspace --all-features`).
2. Formatting and lints pass (`cargo fmt`, `cargo clippy`).
3. New features include tests and docs.
4. Security-sensitive changes include threat and policy impact notes.
5. Changelog updated when user-visible behavior changes.

## Crate Dependency Diagram

```
clawdb
  -> claw-core
  -> claw-vector
  -> claw-sync
  -> claw-branch
  -> claw-guard

clawdb-cli
  -> clawdb

clawdb-server
  -> clawdb
```
