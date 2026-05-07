//! `clawdb sync` — synchronisation subcommands.

use std::time::Duration;

use clap::{Args, Subcommand};
use indicatif::{ProgressBar, ProgressStyle};

use crate::client::ClawDBClient;
use crate::error::CliResult;
use crate::output::{print_success, OutputFormat};
use crate::types::{SyncActionResult, SyncResult, SyncStatusResponse};

#[derive(Debug, Clone, Args)]
pub struct SyncArgs {
    #[command(subcommand)]
    pub command: SyncCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub enum SyncCommand {
    /// Run a full bi-directional sync round.
    Run {
        /// Perform a dry-run without committing changes.
        #[arg(long)]
        dry_run: bool,
    },
    /// Push local changes to the hub.
    Push,
    /// Pull remote changes from the hub.
    Pull,
    /// Reconcile divergent history with the hub.
    Reconcile,
    /// Show current sync engine status.
    Status,
}

pub async fn execute(
    args: SyncArgs,
    client: &ClawDBClient,
    fmt: &OutputFormat,
    quiet: bool,
) -> CliResult<()> {
    match args.command {
        SyncCommand::Run { dry_run } => {
            let pb = spinner(quiet, "Syncing…");
            let body = serde_json::json!({ "dry_run": dry_run });
            let result: SyncResult = client.post("/v1/sync", &body).await?;
            finish_spinner(pb);
            match fmt {
                OutputFormat::Json => crate::output::print_json(&result, quiet),
                _ => {
                    if !quiet {
                        println!(
                            "↑ {} pushed  ↓ {} pulled  △ {} conflicts",
                            result.pushed, result.pulled, result.conflicts
                        );
                    }
                }
            }
        }

        SyncCommand::Push => {
            let pb = spinner(quiet, "Pushing…");
            let result: SyncActionResult = client
                .post("/v1/sync/push", &serde_json::json!({}))
                .await?;
            finish_spinner(pb);
            match fmt {
                OutputFormat::Json => crate::output::print_json(&result, quiet),
                _ => print_success(
                    &format!(
                        "Push complete — sent: {}",
                        result.deltas_sent
                    ),
                    fmt,
                    quiet,
                ),
            }
        }

        SyncCommand::Pull => {
            let pb = spinner(quiet, "Pulling…");
            let result: SyncActionResult = client
                .post("/v1/sync/pull", &serde_json::json!({}))
                .await?;
            finish_spinner(pb);
            match fmt {
                OutputFormat::Json => crate::output::print_json(&result, quiet),
                _ => print_success(
                    &format!(
                        "Pull complete — received: {}, applied: {}, skipped: {}",
                        result.deltas_received, result.ops_applied, result.ops_skipped
                    ),
                    fmt,
                    quiet,
                ),
            }
        }

        SyncCommand::Reconcile => {
            let pb = spinner(quiet, "Reconciling…");
            let result: SyncActionResult = client
                .post("/v1/sync/reconcile", &serde_json::json!({}))
                .await?;
            finish_spinner(pb);
            match fmt {
                OutputFormat::Json => crate::output::print_json(&result, quiet),
                _ => print_success("Reconcile complete", fmt, quiet),
            }
        }

        SyncCommand::Status => {
            let status: SyncStatusResponse = client.get("/v1/sync/status").await?;
            match fmt {
                OutputFormat::Json => crate::output::print_json(&status, quiet),
                _ => {
                    if !quiet {
                        println!("connected    : {}", status.connected);
                        println!(
                            "last_sync_at : {}",
                            status.last_sync_at.as_deref().unwrap_or("never")
                        );
                        if let Some(err) = &status.last_error {
                            println!("last_error   : {}", err);
                        }
                        println!("pending_push : {}", status.pending_push);
                        println!("pending_pull : {}", status.pending_pull);
                    }
                }
            }
        }
    }
    Ok(())
}

fn spinner(quiet: bool, msg: &str) -> Option<ProgressBar> {
    if quiet {
        return None;
    }
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template("{spinner:.cyan} {msg}")
            .unwrap_or_else(|_| ProgressStyle::default_spinner()),
    );
    pb.enable_steady_tick(Duration::from_millis(120));
    pb.set_message(msg.to_string());
    Some(pb)
}

fn finish_spinner(pb: Option<ProgressBar>) {
    if let Some(pb) = pb {
        pb.finish_and_clear();
    }
}
