//! `clawdb memory` — grouped memory commands.

use clap::{Args, Subcommand};
use tabled::Tabled;

use crate::client::ClawDBClient;
use crate::error::CliResult;
use crate::output::{self, OutputFormat};
use crate::types::MemoryRecord;

#[derive(Debug, Clone, Args)]
pub struct MemoryArgs {
    #[command(subcommand)]
    pub command: MemoryCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub enum MemoryCommand {
    Remember(super::remember::RememberArgs),
    Search(super::search::SearchArgs),
    List {
        #[arg(long)]
        r#type: Option<String>,
        #[arg(long, default_value_t = 50)]
        limit: usize,
    },
    Delete { id: String },
}

#[derive(Tabled, Clone)]
struct MemoryRow {
    id: String,
    memory_type: String,
    content: String,
}

pub async fn execute(
    args: MemoryArgs,
    client: &ClawDBClient,
    fmt: &OutputFormat,
    quiet: bool,
) -> CliResult<()> {
    match args.command {
        MemoryCommand::Remember(args) => super::remember::execute(args, client, fmt, quiet).await,
        MemoryCommand::Search(args) => super::search::execute(args, client, fmt, quiet).await,
        MemoryCommand::List { r#type, limit } => {
            let mut params = vec![("limit", limit.to_string())];
            if let Some(value) = r#type {
                params.push(("type", value));
            }
            let memories: Vec<MemoryRecord> = client.get_q("/v1/memories", &params).await?;
            match output::effective_format(fmt) {
                OutputFormat::Json => output::print_json(&memories, quiet),
                OutputFormat::Tsv => output::print_tsv(&rows(&memories), quiet),
                OutputFormat::Table => output::print_table(&rows(&memories), quiet),
            }
            Ok(())
        }
        MemoryCommand::Delete { id } => {
            client.delete(&format!("/v1/memories/{id}")).await?;
            output::print_success(&format!("Deleted memory {id}"), fmt, quiet);
            Ok(())
        }
    }
}

fn rows(memories: &[MemoryRecord]) -> Vec<MemoryRow> {
    memories
        .iter()
        .map(|m| MemoryRow {
            id: m.id.clone(),
            memory_type: m.memory_type.clone(),
            content: m.content.clone(),
        })
        .collect()
}
