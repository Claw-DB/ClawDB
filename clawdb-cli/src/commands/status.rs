//! `clawdb status` — GET /v1/health and display component status.

use clap::Args;
use tabled::Tabled;

use crate::client::ClawDBClient;
use crate::error::CliResult;
use crate::output::{self, print_warning, OutputFormat};
use crate::types::HealthResponse;

#[derive(Debug, Clone, Args)]
pub struct StatusArgs {}

#[derive(Tabled, Clone)]
struct ComponentRow {
    #[tabled(rename = "Component")]
    component: String,
    #[tabled(rename = "Status")]
    status: String,
}

pub async fn execute(
    _args: StatusArgs,
    client: &ClawDBClient,
    fmt: &OutputFormat,
    quiet: bool,
) -> CliResult<()> {
    let health: HealthResponse = client.get("/v1/health").await?;

    match output::effective_format(fmt) {
        OutputFormat::Json => output::print_json(&health, quiet),
        OutputFormat::Tsv => {
            let rows = build_rows(&health, fmt, quiet);
            output::print_tsv(&rows, quiet);
        }
        OutputFormat::Table => {
            let rows = build_rows(&health, fmt, quiet);
            output::print_table(&rows, quiet);
        }
    }

    Ok(())
}

fn build_rows(health: &HealthResponse, fmt: &OutputFormat, quiet: bool) -> Vec<ComponentRow> {
    health
        .components
        .iter()
        .map(|(name, val)| {
            let healthy = val.as_bool().unwrap_or(false);
            if !healthy {
                print_warning(&format!("component '{}' is degraded", name), fmt, quiet);
            }
            ComponentRow {
                component: name.clone(),
                status: if healthy {
                    "✓ ok".to_string()
                } else {
                    "✗ degraded".to_string()
                },
            }
        })
        .collect()
}
