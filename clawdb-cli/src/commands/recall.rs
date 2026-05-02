//! `clawdb recall` — fetch one or more memories by ID.

use clap::Args;
use tabled::Tabled;

use crate::client::ClawDBClient;
use crate::error::CliResult;
use crate::output::{self, OutputFormat};
use crate::types::MemoryRecord;

#[derive(Debug, Clone, Args)]
pub struct RecallArgs {
    /// One or more memory IDs to retrieve.
    #[arg(required = true)]
    pub ids: Vec<String>,
}

#[derive(Tabled, Clone)]
struct MemoryRow {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Type")]
    memory_type: String,
    #[tabled(rename = "Tags")]
    tags: String,
    #[tabled(rename = "Content")]
    content: String,
    #[tabled(rename = "Created At")]
    created_at: String,
}

pub async fn execute(
    args: RecallArgs,
    client: &ClawDBClient,
    fmt: &OutputFormat,
    quiet: bool,
) -> CliResult<()> {
    let mut memories: Vec<MemoryRecord> = Vec::with_capacity(args.ids.len());

    for id in &args.ids {
        let mem: MemoryRecord = client.get(&format!("/v1/memories/{}", id)).await?;
        memories.push(mem);
    }

    match output::effective_format(fmt) {
        OutputFormat::Json => output::print_json(&memories, quiet),
        OutputFormat::Tsv => {
            let rows = to_rows(&memories);
            output::print_tsv(&rows, quiet);
        }
        OutputFormat::Table => {
            let rows = to_rows(&memories);
            output::print_table(&rows, quiet);
        }
    }

    Ok(())
}

fn to_rows(memories: &[MemoryRecord]) -> Vec<MemoryRow> {
    memories
        .iter()
        .map(|m| MemoryRow {
            id: m.id.clone(),
            memory_type: m.memory_type.clone(),
            tags: m.tags.join(", "),
            content: truncate(&m.content, 80),
            created_at: m.created_at.clone().unwrap_or_default(),
        })
        .collect()
}

fn truncate(s: &str, max: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() > max {
        format!("{}…", chars[..max].iter().collect::<String>())
    } else {
        s.to_string()
    }
}
