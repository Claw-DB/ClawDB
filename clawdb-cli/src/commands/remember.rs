//! `clawdb remember` — POST /v1/memories to store a memory.

use std::path::PathBuf;

use clap::Args;

use crate::client::ClawDBClient;
use crate::error::CliResult;
use crate::output::{print_success, OutputFormat};
use crate::types::CreateMemoryResponse;

#[derive(Debug, Clone, Args)]
pub struct RememberArgs {
    /// Memory content (positional; use --file to read from a file instead).
    #[arg(index = 1, required_unless_present = "file")]
    pub content: Option<String>,

    /// Memory type (default: message).
    #[arg(long = "type", default_value = "message")]
    pub memory_type: String,

    /// Comma-separated tags.
    #[arg(long)]
    pub tags: Option<String>,

    /// Metadata as a JSON string.
    #[arg(long)]
    pub metadata: Option<String>,

    /// Read content from a file instead of the positional argument.
    #[arg(long)]
    pub file: Option<PathBuf>,
}

pub async fn execute(
    args: RememberArgs,
    client: &ClawDBClient,
    fmt: &OutputFormat,
    quiet: bool,
) -> CliResult<()> {
    let content = if let Some(f) = args.file {
        std::fs::read_to_string(f)?
    } else {
        args.content.unwrap_or_default()
    };

    let tags: Vec<String> = args
        .tags
        .as_deref()
        .unwrap_or("")
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
        .collect();

    let metadata: serde_json::Value = if let Some(raw) = &args.metadata {
        serde_json::from_str(raw)?
    } else {
        serde_json::Value::Null
    };

    let body = serde_json::json!({
        "content": content,
        "memory_type": args.memory_type,
        "tags": tags,
        "metadata": metadata,
    });

    let resp: CreateMemoryResponse = client.post("/v1/memories", &body).await?;
    print_success(&format!("Stored (id: {})", resp.id), fmt, quiet);
    Ok(())
}
