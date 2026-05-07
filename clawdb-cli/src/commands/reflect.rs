//! `clawdb reflect` — reflect service subcommands.

use std::time::Duration;

use clap::{Args, Subcommand, ValueEnum};
use indicatif::{ProgressBar, ProgressStyle};
use tabled::Tabled;
use uuid::Uuid;

use crate::client::ClawDBClient;
use crate::error::CliResult;
use crate::output::{self, print_success, OutputFormat};
use crate::types::{Contradiction, ExtractedFact, Preference, ReflectJob, ReflectJobDetail};

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
    #[command(subcommand)]
    pub command: ReflectCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub enum ReflectCommand {
    /// Trigger a reflection job.
    Run {
        /// Job kind to run.
        #[arg(long, value_enum, default_value_t = ReflectJobKind::Full)]
        job: ReflectJobKind,

        /// Dry-run (plan only, no changes).
        #[arg(long)]
        dry_run: bool,

        /// Filter to a specific agent.
        #[arg(long)]
        agent_id: Option<Uuid>,
    },
    /// List recent reflection jobs.
    Jobs {
        /// Filter by agent ID.
        #[arg(long)]
        agent_id: Option<String>,
        /// Filter by status.
        #[arg(long)]
        status: Option<String>,
        /// Max number of results.
        #[arg(long, default_value_t = 20)]
        limit: u32,
    },
    /// Get details for a specific reflection job.
    Job { job_id: String },
    /// Get extracted facts for an agent.
    Facts { agent_id: String },
    /// Get preferences for an agent.
    Preferences { agent_id: String },
    /// Get contradictions for an agent.
    Contradictions { agent_id: String },
    /// Resolve a contradiction.
    Resolve {
        agent_id: String,
        contradiction_id: String,
        #[arg(long, default_value = "accept")]
        strategy: String,
        #[arg(long)]
        merged_value: Option<serde_json::Value>,
    },
}

#[derive(Tabled, Clone)]
struct JobRow {
    #[tabled(rename = "Job ID")]
    job_id: String,
    #[tabled(rename = "Status")]
    status: String,
    #[tabled(rename = "Processed")]
    processed: u64,
    #[tabled(rename = "Summaries")]
    summaries: u64,
}

#[derive(Tabled, Clone)]
struct FactRow {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Confidence")]
    confidence: String,
    #[tabled(rename = "Fact")]
    fact: String,
}

pub async fn execute(
    args: ReflectArgs,
    client: &ClawDBClient,
    fmt: &OutputFormat,
    quiet: bool,
) -> CliResult<()> {
    match args.command {
        ReflectCommand::Run { job, dry_run, agent_id } => {
            let pb = spinner(quiet, format!("Running {} reflection…", job));
            let mut body = serde_json::json!({
                "job": job.to_string(),
                "dry_run": dry_run,
            });
            if let Some(id) = agent_id {
                body["agent_id"] = serde_json::Value::String(id.to_string());
            }
            let result: ReflectJob = client.post("/v1/reflect", &body).await?;
            finish_spinner(pb);
            match fmt {
                OutputFormat::Json => output::print_json(&result, quiet),
                _ => print_success(
                    &format!(
                        "Reflect job {} — status: {}, processed: {}, summaries: {}",
                        result.job_id,
                        result.status,
                        result.memories_processed.unwrap_or(0),
                        result.summaries_created.unwrap_or(0)
                    ),
                    fmt,
                    quiet,
                ),
            }
        }

        ReflectCommand::Jobs { agent_id, status, limit } => {
            let mut params: Vec<(&str, String)> = vec![("limit", limit.to_string())];
            if let Some(ref a) = agent_id {
                params.push(("agent_id", a.clone()));
            }
            if let Some(ref s) = status {
                params.push(("status", s.clone()));
            }
            let jobs: Vec<ReflectJobDetail> = client.get_q("/v1/reflect/jobs", &params).await?;
            match output::effective_format(fmt) {
                OutputFormat::Json => output::print_json(&jobs, quiet),
                _ => {
                    let rows: Vec<JobRow> = jobs
                        .iter()
                        .map(|j| JobRow {
                            job_id: j.job_id.clone(),
                            status: j.status.clone(),
                            processed: j.memories_processed.unwrap_or(0),
                            summaries: j.summaries_created.unwrap_or(0),
                        })
                        .collect();
                    output::print_table(&rows, quiet);
                }
            }
        }

        ReflectCommand::Job { job_id } => {
            let job: ReflectJobDetail = client
                .get(&format!("/v1/reflect/jobs/{}", job_id))
                .await?;
            match fmt {
                OutputFormat::Json => output::print_json(&job, quiet),
                _ => {
                    if !quiet {
                        println!("job_id    : {}", job.job_id);
                        println!("status    : {}", job.status);
                        println!("processed : {}", job.memories_processed.unwrap_or(0));
                        println!("summaries : {}", job.summaries_created.unwrap_or(0));
                        println!("started   : {}", job.started_at.as_deref().unwrap_or("N/A"));
                        println!("completed : {}", job.completed_at.as_deref().unwrap_or("N/A"));
                    }
                }
            }
        }

        ReflectCommand::Facts { agent_id } => {
            let facts: Vec<ExtractedFact> = client
                .get(&format!("/v1/reflect/facts/{}", agent_id))
                .await?;
            match output::effective_format(fmt) {
                OutputFormat::Json => output::print_json(&facts, quiet),
                _ => {
                    let rows: Vec<FactRow> = facts
                        .iter()
                        .map(|f| FactRow {
                            id: f.id.clone(),
                            confidence: format!("{:.2}", f.confidence),
                            fact: f.fact.clone(),
                        })
                        .collect();
                    output::print_table(&rows, quiet);
                }
            }
        }

        ReflectCommand::Preferences { agent_id } => {
            let prefs: Vec<Preference> = client
                .get(&format!("/v1/reflect/preferences/{}", agent_id))
                .await?;
            match fmt {
                OutputFormat::Json => output::print_json(&prefs, quiet),
                _ => {
                    if !quiet {
                        for p in &prefs {
                            println!("{} = {}", p.key, p.value);
                        }
                    }
                }
            }
        }

        ReflectCommand::Contradictions { agent_id } => {
            let contradictions: Vec<Contradiction> = client
                .get(&format!("/v1/reflect/contradictions/{}", agent_id))
                .await?;
            match fmt {
                OutputFormat::Json => output::print_json(&contradictions, quiet),
                _ => {
                    if !quiet {
                        for c in &contradictions {
                            println!("[{}] {} ({})", c.id, c.description, c.status);
                        }
                    }
                }
            }
        }

        ReflectCommand::Resolve {
            agent_id,
            contradiction_id,
            strategy,
            merged_value,
        } => {
            let body = serde_json::json!({
                "strategy": strategy,
                "merged_value": merged_value,
            });
            let result: Contradiction = client
                .post(
                    &format!(
                        "/v1/reflect/contradictions/{}/{}/resolve",
                        agent_id, contradiction_id
                    ),
                    &body,
                )
                .await?;
            match fmt {
                OutputFormat::Json => output::print_json(&result, quiet),
                _ => print_success(
                    &format!(
                        "Contradiction {} resolved (status: {})",
                        result.id, result.status
                    ),
                    fmt,
                    quiet,
                ),
            }
        }
    }
    Ok(())
}

fn spinner(quiet: bool, msg: String) -> Option<ProgressBar> {
    if quiet {
        return None;
    }
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template("{spinner:.cyan} {msg}")
            .unwrap_or_else(|_| ProgressStyle::default_spinner()),
    );
    pb.enable_steady_tick(Duration::from_millis(120));
    pb.set_message(msg);
    Some(pb)
}

fn finish_spinner(pb: Option<ProgressBar>) {
    if let Some(pb) = pb {
        pb.finish_and_clear();
    }
}
