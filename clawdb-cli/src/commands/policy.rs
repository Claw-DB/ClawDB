//! `clawdb policy` — CRUD for /v1/policies/* endpoints.

use std::path::PathBuf;

use clap::{Args, Subcommand};
use tabled::Tabled;
use uuid::Uuid;

use crate::client::ClawDBClient;
use crate::error::CliResult;
use crate::output::{self, print_success, OutputFormat};
use crate::types::{PolicyRecord, PolicyTestResult};

#[derive(Debug, Clone, Args)]
pub struct PolicyArgs {
    #[command(subcommand)]
    pub command: PolicyCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub enum PolicyCommand {
    /// List all policies.
    List,
    /// Upload a policy from a TOML file.
    Add {
        #[arg(long)]
        file: PathBuf,
    },
    /// Delete a policy by ID.
    Remove { policy_id: String },
    /// Test whether an access request is allowed.
    Test {
        #[arg(long)]
        agent_id: Uuid,
        #[arg(long)]
        role: String,
        #[arg(long)]
        task: String,
        #[arg(long)]
        resource: String,
    },
}

#[derive(Tabled, Clone)]
struct PolicyRow {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Effect")]
    effect: String,
    #[tabled(rename = "Created At")]
    created_at: String,
}

pub async fn execute(
    args: PolicyArgs,
    client: &ClawDBClient,
    fmt: &OutputFormat,
    quiet: bool,
) -> CliResult<()> {
    match args.command {
        PolicyCommand::List => {
            let policies: Vec<PolicyRecord> = client.get("/v1/policies").await?;
            match output::effective_format(fmt) {
                OutputFormat::Json => output::print_json(&policies, quiet),
                OutputFormat::Tsv => {
                    let rows = to_rows(&policies);
                    output::print_tsv(&rows, quiet);
                }
                OutputFormat::Table => {
                    let rows = to_rows(&policies);
                    output::print_table(&rows, quiet);
                }
            }
        }

        PolicyCommand::Add { file } => {
            let content = std::fs::read_to_string(&file)?;
            // Parse TOML and re-encode as JSON for the API.
            let parsed: toml::Value = toml::from_str(&content)?;
            let as_json = serde_json::to_value(parsed)?;
            let policy: PolicyRecord = client.post("/v1/policies", &as_json).await?;
            print_success(&format!("Policy added (id: {})", policy.id), fmt, quiet);
        }

        PolicyCommand::Remove { policy_id } => {
            client
                .delete(&format!("/v1/policies/{}", policy_id))
                .await?;
            print_success(&format!("Policy {} removed", policy_id), fmt, quiet);
        }

        PolicyCommand::Test {
            agent_id,
            role,
            task,
            resource,
        } => {
            let body = serde_json::json!({
                "agent_id": agent_id,
                "role": role,
                "task": task,
                "resource": resource,
            });
            let result: PolicyTestResult = client.post("/v1/policies/test", &body).await?;
            match fmt {
                OutputFormat::Json => output::print_json(&result, quiet),
                _ => {
                    if !quiet {
                        let verdict = if result.allowed { "ALLOW" } else { "DENY" };
                        println!("{}", verdict);
                        if let Some(reason) = &result.reason {
                            println!("Reason: {}", reason);
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn to_rows(policies: &[PolicyRecord]) -> Vec<PolicyRow> {
    policies
        .iter()
        .map(|p| PolicyRow {
            id: p.id.clone(),
            name: p.name.clone(),
            effect: p.effect.clone(),
            created_at: p.created_at.clone().unwrap_or_default(),
        })
        .collect()
}
