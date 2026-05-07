# clawdb-cli

`clawdb-cli` is the full-featured HTTP command-line client for `clawdb-server`.
It covers every REST endpoint exposed by the server — memories, branches, sync,
reflect, transactions, and session management.

## Install

```bash
cargo install clawdb-cli
```

## Usage

```bash
clawdb --help
clawdb status
```

## Command Catalog

### Sessions

```bash
clawdb session create --agent-id <uuid> --role agent --scopes "memory:*,branch:*"
clawdb session revoke --token <token>
clawdb session whoami
clawdb session active-count          # GET /v1/sessions/active/count
```

### Memories

```bash
clawdb remember "Important fact"
clawdb remember --type episodic --tags tag1,tag2 "Episodic memory"

clawdb memory remember "Another fact"
clawdb memory search "important"
clawdb memory list --type semantic --limit 20
clawdb memory get <memory-id>
clawdb memory update <memory-id> "Updated content"    # PATCH /v1/memories/:id
clawdb memory delete <memory-id>
```

### Search & Recall

```bash
clawdb search "query"
clawdb recall <memory-id> [<memory-id> ...]
```

### Branches

```bash
clawdb branch create --name feature-x
clawdb branch list
clawdb branch get <branch-id>                        # GET /v1/branches/:id
clawdb branch by-name feature-x                      # GET /v1/branches/by-name/:name
clawdb branch trunk                                  # GET /v1/branches/trunk
clawdb branch merge <branch-id>
clawdb branch diff <branch-id>
clawdb branch archive <branch-id>                    # POST /v1/branches/:id/archive
clawdb branch discard <branch-id>
```

### Sync

```bash
clawdb sync run                    # POST /v1/sync  (bidirectional)
clawdb sync run --dry-run
clawdb sync push                   # POST /v1/sync/push
clawdb sync pull                   # POST /v1/sync/pull
clawdb sync reconcile              # POST /v1/sync/reconcile
clawdb sync status                 # GET  /v1/sync/status
```

### Reflect

```bash
clawdb reflect run                           # POST /v1/reflect
clawdb reflect run --job summarise --dry-run
clawdb reflect jobs                          # GET /v1/reflect/jobs
clawdb reflect jobs --agent-id <uuid> --status running --limit 10
clawdb reflect job <job-id>                  # GET /v1/reflect/jobs/:id
clawdb reflect facts <agent-id>             # GET /v1/reflect/facts/:agent_id
clawdb reflect preferences <agent-id>       # GET /v1/reflect/preferences/:agent_id
clawdb reflect contradictions <agent-id>    # GET /v1/reflect/contradictions/:agent_id
clawdb reflect resolve <agent-id> <contradiction-id> --strategy accept
```

### Transactions

```bash
clawdb tx begin                                           # POST /v1/tx  → prints tx_id
clawdb tx remember <tx-id> "Staged memory"               # POST /v1/tx/:id/memories
clawdb tx remember-typed <tx-id> "Typed fact" \
    --type episodic --tags tag1,tag2                      # POST /v1/tx/:id/memories/typed
clawdb tx commit <tx-id>                                  # POST /v1/tx/:id/commit
clawdb tx rollback <tx-id>                                # POST /v1/tx/:id/rollback
```

### MCP / Editor Integration

```bash
clawdb mcp install vscode
clawdb mcp install cursor
clawdb mcp print
```

### Other

```bash
clawdb start [--background]          # start clawdb-server
clawdb stop                          # stop server
clawdb status                        # health + server info
clawdb init [--dir <path>]           # initialise workspace
clawdb config get <key>
clawdb config set <key> <value>
clawdb policy list
clawdb completion bash | zsh | fish | powershell
```

## Output Modes

| Flag | Description |
|---|---|
| `--output table` | Human-readable table (default on TTY) |
| `--output json` | JSON (default on non-TTY / CI) |
| `--output tsv` | Tab-separated values |
| `--quiet` | Suppress informational output |

## Auth

The CLI reads the session token from `~/.clawdb/session.token` or the
`CLAWDB_SESSION_TOKEN` environment variable.

Run `clawdb session create` to obtain and save a token after starting the server.

## Local Files

| Path | Purpose |
|---|---|
| `~/.clawdb/config.toml` | Base URL, workspace settings |
| `~/.clawdb/session.token` | Persisted session token |

## Shell Completion

```bash
clawdb completion bash   >> ~/.bashrc
clawdb completion zsh    >> ~/.zshrc
clawdb completion fish   > ~/.config/fish/completions/clawdb.fish
```

