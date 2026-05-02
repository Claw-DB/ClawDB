//! `clawdb sync` — POST /v1/sync with a spinner while waiting.

use std::time::Duration;

use clap::Args;
use indicatif::{ProgressBar, ProgressStyle};

use crate::client::ClawDBClient;
use crate::error::CliResult;
use crate::output::OutputFormat;
use crate::types::SyncResult;

#[derive(Debug, Clone, Args)]
pub struct SyncArgs {
    /// Perform a dry-run without committing changes.
    #[arg(long)]
    pub dry_run: bool,
}

pub async fn execute(
    args: SyncArgs,
    client: &ClawDBClient,
    fmt: &OutputFormat,
    quiet: bool,
) -> CliResult<()> {
    let pb = if !quiet {
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::with_template("{spinner:.cyan} {msg}")
                .unwrap_or_else(|_| ProgressStyle::default_spinner()),
        );
        pb.enable_steady_tick(Duration::from_millis(120));
        pb.set_message("Syncing…");
        Some(pb)
    } else {
        None
    };

    let body = serde_json::json!({ "dry_run": args.dry_run });
    let result: SyncResult = client.post("/v1/sync", &body).await?;

    if let Some(pb) = pb {
        pb.finish_and_clear();
    }

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

    Ok(())
}
