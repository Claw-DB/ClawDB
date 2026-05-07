//! `clawdb tx` — transaction commands (begin / stage / commit / rollback).

use clap::{Args, Subcommand};

use crate::client::ClawDBClient;
use crate::error::CliResult;
use crate::output::{print_success, OutputFormat};
use crate::types::{TxBeginResponse, TxCommitResponse, TxRollbackResponse, TxStagedResponse};

#[derive(Debug, Clone, Args)]
pub struct TxArgs {
    #[command(subcommand)]
    pub command: TxCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub enum TxCommand {
    /// Begin a new transaction and print the transaction ID.
    Begin,
    /// Stage a semantic memory inside a transaction.
    Remember {
        /// Transaction ID returned by `tx begin`.
        tx_id: String,
        /// Memory content to stage.
        content: String,
    },
    /// Stage a typed memory inside a transaction.
    RememberTyped {
        /// Transaction ID.
        tx_id: String,
        /// Memory content.
        content: String,
        /// Memory type (e.g. semantic, episodic, procedural).
        #[arg(long, default_value = "semantic")]
        r#type: String,
        /// Comma-separated tags.
        #[arg(long, default_value = "")]
        tags: String,
        /// Metadata as a JSON string.
        #[arg(long, default_value = "{}")]
        metadata: String,
    },
    /// Commit a transaction (writes all staged memories atomically).
    Commit {
        /// Transaction ID to commit.
        tx_id: String,
    },
    /// Rollback a transaction (discards all staged memories).
    Rollback {
        /// Transaction ID to roll back.
        tx_id: String,
    },
}

pub async fn execute(
    args: TxArgs,
    client: &ClawDBClient,
    fmt: &OutputFormat,
    quiet: bool,
) -> CliResult<()> {
    match args.command {
        TxCommand::Begin => {
            let resp: TxBeginResponse = client.post("/v1/tx", &serde_json::json!({})).await?;
            match fmt {
                OutputFormat::Json => crate::output::print_json(&resp, quiet),
                _ => {
                    if !quiet {
                        println!("tx_id: {}", resp.tx_id);
                    }
                }
            }
        }

        TxCommand::Remember { tx_id, content } => {
            let body = serde_json::json!({"content": content});
            let resp: TxStagedResponse = client
                .post(&format!("/v1/tx/{}/memories", tx_id), &body)
                .await?;
            match fmt {
                OutputFormat::Json => crate::output::print_json(&resp, quiet),
                _ => print_success("Memory staged in transaction", fmt, quiet),
            }
        }

        TxCommand::RememberTyped {
            tx_id,
            content,
            r#type,
            tags,
            metadata,
        } => {
            let tags_vec: Vec<String> = tags
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            let metadata_val: serde_json::Value =
                serde_json::from_str(&metadata).unwrap_or(serde_json::json!({}));
            let body = serde_json::json!({
                "content": content,
                "type": r#type,
                "tags": tags_vec,
                "metadata": metadata_val,
            });
            let resp: TxStagedResponse = client
                .post(&format!("/v1/tx/{}/memories/typed", tx_id), &body)
                .await?;
            match fmt {
                OutputFormat::Json => crate::output::print_json(&resp, quiet),
                _ => print_success("Typed memory staged in transaction", fmt, quiet),
            }
        }

        TxCommand::Commit { tx_id } => {
            let resp: TxCommitResponse = client
                .post(&format!("/v1/tx/{}/commit", tx_id), &serde_json::json!({}))
                .await?;
            match fmt {
                OutputFormat::Json => crate::output::print_json(&resp, quiet),
                _ => {
                    if resp.committed {
                        print_success("Transaction committed", fmt, quiet);
                    } else if !quiet {
                        eprintln!("Transaction was not committed.");
                    }
                }
            }
        }

        TxCommand::Rollback { tx_id } => {
            let resp: TxRollbackResponse = client
                .post(
                    &format!("/v1/tx/{}/rollback", tx_id),
                    &serde_json::json!({}),
                )
                .await?;
            match fmt {
                OutputFormat::Json => crate::output::print_json(&resp, quiet),
                _ => print_success("Transaction rolled back", fmt, quiet),
            }
        }
    }
    Ok(())
}
