# clawdb-cli

`clawdb-cli` provides a pure HTTP command-line client for `clawdb-server`.

## Install

```bash
cargo install clawdb-cli
```

## Usage

```bash
clawdb --help
clawdb status
clawdb session create --agent-id <uuid> --role agent --scopes memory:read,memory:write
clawdb remember "Important memory"
clawdb search "important"
```

## Output Modes

- `--output table` (default on TTY)
- `--output json` (default on non-TTY)
- `--output tsv`

## Local Files

- Config: `~/.clawdb/config.toml`
- Session token: `~/.clawdb/session.token`

The `CLAWDB_SESSION_TOKEN` environment variable overrides the token file.

## Shell Completion

```bash
clawdb completion bash
clawdb completion zsh
clawdb completion fish
clawdb completion powershell
```
