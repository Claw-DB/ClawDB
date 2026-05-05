//! `clawdb mcp` — install editor MCP config blocks and print config JSON.

use std::path::PathBuf;

use clap::{Args, Subcommand, ValueEnum};

use crate::error::{CliError, CliResult};
use crate::output::{print_success, OutputFormat};

#[derive(Debug, Clone, Args)]
pub struct McpArgs {
    #[command(subcommand)]
    pub command: McpCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub enum McpCommand {
    InstallClaude,
    InstallCursor,
    InstallVscode,
    InstallContinue,
    InstallZed,
    PrintConfig {
        #[arg(long, value_enum, default_value_t = McpHost::Vscode)]
        host: McpHost,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum McpHost {
    Claude,
    Cursor,
    Vscode,
    Continue,
    Zed,
}

pub async fn execute(args: McpArgs, fmt: &OutputFormat, quiet: bool) -> CliResult<()> {
    match args.command {
        McpCommand::InstallClaude => install("claude", claude_path()?, fmt, quiet),
        McpCommand::InstallCursor => install("cursor", cursor_path()?, fmt, quiet),
        McpCommand::InstallVscode => install("vscode", vscode_path()?, fmt, quiet),
        McpCommand::InstallContinue => install("continue", continue_path()?, fmt, quiet),
        McpCommand::InstallZed => install("zed", zed_path()?, fmt, quiet),
        McpCommand::PrintConfig { host } => {
            let value = config_json(host);
            println!("{}", serde_json::to_string_pretty(&value)?);
            Ok(())
        }
    }
}

fn install(editor: &str, path: PathBuf, fmt: &OutputFormat, quiet: bool) -> CliResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let existing = if path.exists() {
        std::fs::read_to_string(&path)?
    } else {
        "{}".to_string()
    };

    let mut json: serde_json::Value =
        serde_json::from_str(&existing).map_err(|e| CliError::Config(e.to_string()))?;
    if !json.is_object() {
        json = serde_json::json!({});
    }

    if let Some(obj) = json.as_object_mut() {
        obj.insert("clawdb".to_string(), config_json(McpHost::Vscode));
    }

    std::fs::write(&path, serde_json::to_string_pretty(&json)?)?;
    print_success(
        &format!("ClawDB installed for {editor}. Restart to activate."),
        fmt,
        quiet,
    );
    Ok(())
}

fn config_json(_host: McpHost) -> serde_json::Value {
    serde_json::json!({
        "command": "clawdb",
        "args": ["mcp", "serve"],
        "env": {
            "CLAWDB_BASE_URL": std::env::var("CLAWDB_BASE_URL").unwrap_or_else(|_| "http://localhost:8080".to_string())
        }
    })
}

fn home() -> CliResult<PathBuf> {
    dirs::home_dir().ok_or_else(|| CliError::Config("home directory not found".to_string()))
}

fn claude_path() -> CliResult<PathBuf> {
    Ok(home()?.join(".claude").join("mcp.json"))
}

fn cursor_path() -> CliResult<PathBuf> {
    Ok(home()?.join(".cursor").join("mcp.json"))
}

fn vscode_path() -> CliResult<PathBuf> {
    Ok(home()?.join("Library/Application Support/Code/User/mcp.json"))
}

fn continue_path() -> CliResult<PathBuf> {
    Ok(home()?.join(".continue").join("mcp.json"))
}

fn zed_path() -> CliResult<PathBuf> {
    Ok(home()?.join(".config/zed/mcp.json"))
}
