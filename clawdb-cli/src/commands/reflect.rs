//! `clawdb reflect` — run and optionally watch reflection jobs.

use std::{path::PathBuf, time::Duration};

use clap::{Args, ValueEnum};
use clawdb::{ClawDB, ClawDBResult};
use uuid::Uuid;

use super::load_config;

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ReflectJobType {
    Full,
    Summarise,
    Extract,
    Deduplicate,
}

#[derive(Debug, Clone, Args)]
pub struct ReflectArgs {
    #[arg(long, value_enum, default_value_t = ReflectJobType::Full)]
    pub job_type: ReflectJobType,
    #[arg(long)]
    pub dry_run: bool,
    #[arg(long)]
    pub watch: bool,
    #[arg(long)]
    pub agent_id: Option<Uuid>,
    #[arg(long, default_value = "assistant")]
    pub role: String,
}

pub async fn run(args: ReflectArgs, data_dir: PathBuf) -> ClawDBResult<()> {
    let cfg = load_config(&data_dir)?;
    let agent_id = args.agent_id.unwrap_or(cfg.agent_id);
    let db = ClawDB::open(&data_dir).await?;
    let _session = db
        .session(agent_id, &args.role, vec!["reflect:run".to_string(), "memory:read".to_string()])
        .await?;

    if args.dry_run {
        println!(
            "Dry run: would run {:?} reflection against {}",
            args.job_type,
            cfg.reflect.service_url
        );
        return db.close().await;
    }

    let session = db
        .session(agent_id, &args.role, vec!["reflect:run".to_string(), "memory:read".to_string()])
        .await?;
    let job_id = db.reflect(&session).await?;
    println!("Reflect job started: {}", job_id);

    if args.watch {
        let base = cfg.reflect.service_url.trim_end_matches('/');
        let url = format!("{}/jobs/{}", base, job_id);
        let client = reqwest::Client::new();
        let mut ticks: u64 = 0;
        loop {
            tokio::time::sleep(Duration::from_secs(2)).await;
            ticks += 1;
            let resp = client.get(&url).send().await;
            match resp {
                Ok(r) => {
                    let value: serde_json::Value = r.json().await.unwrap_or_else(|_| serde_json::json!({}));
                    let status = value
                        .get("status")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    let progress = value
                        .get("progress")
                        .and_then(|v| v.as_f64())
                        .unwrap_or((ticks as f64 % 100.0) / 100.0);
                    let width = 30usize;
                    let filled = ((progress.clamp(0.0, 1.0)) * width as f64).round() as usize;
                    let bar = format!("{}{}", "#".repeat(filled), "-".repeat(width.saturating_sub(filled)));
                    println!("[{}] {:>3}% status={}", bar, (progress * 100.0) as u64, status);
                    if status.eq_ignore_ascii_case("completed")
                        || status.eq_ignore_ascii_case("failed")
                        || status.eq_ignore_ascii_case("cancelled")
                    {
                        break;
                    }
                }
                Err(err) => {
                    println!("watch poll error: {}", err);
                }
            }
        }
    }

    db.close().await
}
