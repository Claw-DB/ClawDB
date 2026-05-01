//! `clawdb status` — component health/status view.

use std::path::PathBuf;

use clap::Args;
use clawdb::{ClawDB, ClawDBResult};

use super::output_json;

#[derive(Debug, Clone, Args)]
pub struct StatusArgs {}

pub async fn run(_args: StatusArgs, data_dir: PathBuf) -> ClawDBResult<()> {
    let db = ClawDB::open(&data_dir).await?;
    let report = db.health().await?;
    let ok = matches!(report.overall, clawdb::HealthStatus::Healthy);

    if output_json() {
        let components = report
            .components
            .iter()
            .map(|(name, c)| {
                (
                    name.clone(),
                    serde_json::json!({
                        "status": format!("{:?}", c.status),
                        "latency_ms": c.latency_ms,
                        "error": c.last_error,
                    }),
                )
            })
            .collect::<serde_json::Map<String, serde_json::Value>>();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "ok": ok,
                "uptime_secs": db.uptime_secs(),
                "components": components,
            }))?
        );
    } else {
        println!("ClawDB status: {}", if ok { "OK" } else { "DEGRADED" });
        println!("Uptime: {}s", db.uptime_secs());
        for (name, c) in report.components {
            println!("  {:<10}: {:?}", name, c.status);
        }
    }

    db.close().await
}
