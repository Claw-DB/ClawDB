//! `clawdb reflect` — POST /v1/reflect and poll job status with a spinner.

use std::time::Duration;

use clap::{Args, ValueEnum};
use indicatif::{ProgressBar, ProgressStyle};
use uuid::Uuid;

use crate::client::ClawDBClient;
use crate::error::CliResult;
use crate::output::{print_success, OutputFormat};
use crate::types::ReflectJob;

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ReflectJobKind {
    Full,
    Summarise,
    Extract,
    Decay,
    All,
}

impl std::fmt::Display for ReflectJobKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReflectJobKind::Full => write!(f, "full"),
            ReflectJobKind::Summarise => write!(f, "summarise"),
            ReflectJobKind::Extract => write!(f, "extract"),
            ReflectJobKind::Decay => write!(f, "decay"),
            ReflectJobKind::All => write!(f, "all"),
        }
    }
}

#[derive(Debug, Clone, Args)]
pub struct ReflectArgs {
    /// Job kind to run.
    #[arg(long, value_enum, default_value_t = ReflectJobKind::Full)]
    pub job: ReflectJobKind,

    /// Dry-run (plan only, no changes).
    #[arg(long)]
    pub dry_run: bool,

    /// Filter to a specific agent.
    #[arg(long)]
    pub agent_id: Option<Uuid>,
}

pub async fn execute(
    args: ReflectArgs,
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
        pb.set_message(format!("Running {} reflection…", args.job));
        Some(pb)
    } else {
        None
    };

    let mut body = serde_json::json!({
        "job": args.job.to_string(),
        "dry_run": args.dry_run,
    });
    if let Some(id) = args.agent_id {
        body["agent_id"] = serde_json::Value::String(id.to_string());
    }

    let job: ReflectJob = client.post("/v1/reflect", &body).await?;

    if let Some(pb) = pb {
        pb.finish_and_clear();
    }

    match fmt {
        OutputFormat::Json => crate::output::print_json(&job, quiet),
        _ => {
            print_success(
                &format!(
                    "Reflect job {} completed — processed: {}, summaries: {}",
                    job.job_id,
                    job.memories_processed.unwrap_or(0),
                    job.summaries_created.unwrap_or(0)
                ),
                fmt,
                quiet,
            );
        }
    }

    Ok(())
}
