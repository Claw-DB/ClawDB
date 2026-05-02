//! `clawdb branch` — manage memory branches via the HTTP API.

use clap::{Args, Subcommand, ValueEnum};
use tabled::Tabled;

use crate::client::ClawDBClient;
use crate::error::CliResult;
use crate::output::{self, print_success, OutputFormat};
use crate::types::{BranchRecord, DiffResult, MergeResult};

#[derive(Debug, Clone, Args)]
pub struct BranchArgs {
    #[command(subcommand)]
    pub command: BranchCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub enum BranchCommand {
    /// Create a new branch.
    Create {
        name: String,
        #[arg(long)]
        from: Option<String>,
    },
    /// List branches.
    List {
        #[arg(long, value_enum)]
        status: Option<BranchStatus>,
    },
    /// Merge one branch into another.
    Merge {
        source_id: String,
        target_id: String,
        #[arg(long, value_enum, default_value_t = MergeStrategy::LastWrite)]
        strategy: MergeStrategy,
    },
    /// Show diff between two branches.
    Diff {
        source_id: String,
        target_id: String,
    },
    /// Discard a branch.
    Discard { branch_id: String },
}

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq)]
pub enum BranchStatus {
    Active,
    Merged,
    Discarded,
}

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq)]
pub enum MergeStrategy {
    #[value(name = "last-write")]
    LastWrite,
    #[value(name = "source-wins")]
    SourceWins,
    Theirs,
}

impl std::fmt::Display for MergeStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MergeStrategy::LastWrite => write!(f, "last_write_wins"),
            MergeStrategy::SourceWins => write!(f, "source_wins"),
            MergeStrategy::Theirs => write!(f, "theirs"),
        }
    }
}

#[derive(Tabled, Clone)]
struct BranchRow {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Status")]
    status: String,
    #[tabled(rename = "Created At")]
    created_at: String,
}

pub async fn execute(
    args: BranchArgs,
    client: &ClawDBClient,
    fmt: &OutputFormat,
    quiet: bool,
) -> CliResult<()> {
    match args.command {
        BranchCommand::Create { name, from } => {
            let mut body = serde_json::json!({ "name": name });
            if let Some(parent) = from {
                body["from"] = serde_json::Value::String(parent);
            }
            let branch: BranchRecord = client.post("/v1/branches", &body).await?;
            print_success(&format!("Branch created (id: {})", branch.id), fmt, quiet);
        }

        BranchCommand::List { status } => {
            let path = match status {
                Some(s) => format!("/v1/branches?status={}", status_str(s)),
                None => "/v1/branches".to_string(),
            };
            let branches: Vec<BranchRecord> = client.get(&path).await?;
            match output::effective_format(fmt) {
                OutputFormat::Json => output::print_json(&branches, quiet),
                OutputFormat::Tsv => {
                    let rows = to_rows(&branches);
                    output::print_tsv(&rows, quiet);
                }
                OutputFormat::Table => {
                    let rows = to_rows(&branches);
                    output::print_table(&rows, quiet);
                }
            }
        }

        BranchCommand::Merge {
            source_id,
            target_id,
            strategy,
        } => {
            let body = serde_json::json!({
                "target_id": target_id,
                "strategy": strategy.to_string(),
            });
            let result: MergeResult = client
                .post(&format!("/v1/branches/{}/merge", source_id), &body)
                .await?;
            if !quiet {
                println!("merged: {}  conflicts: {}", result.merged, result.conflicts);
            }
        }

        BranchCommand::Diff {
            source_id,
            target_id,
        } => {
            let result: DiffResult = client
                .get(&format!(
                    "/v1/branches/{}/diff?target={}",
                    source_id, target_id
                ))
                .await?;
            if !quiet {
                println!(
                    "+{} modified:{} -{}",
                    result.added, result.modified, result.removed
                );
            }
        }

        BranchCommand::Discard { branch_id } => {
            client
                .delete(&format!("/v1/branches/{}", branch_id))
                .await?;
            print_success(&format!("Branch {} discarded", branch_id), fmt, quiet);
        }
    }
    Ok(())
}

fn to_rows(branches: &[BranchRecord]) -> Vec<BranchRow> {
    branches
        .iter()
        .map(|b| BranchRow {
            id: b.id.clone(),
            name: b.name.clone(),
            status: b.status.clone(),
            created_at: b.created_at.clone().unwrap_or_default(),
        })
        .collect()
}

fn status_str(s: BranchStatus) -> &'static str {
    match s {
        BranchStatus::Active => "active",
        BranchStatus::Merged => "merged",
        BranchStatus::Discarded => "discarded",
    }
}
