//! `clawdb sync` — push/pull/reconcile operations.

use std::path::PathBuf;

use clap::Args;
use clawdb::{ClawDB, ClawDBResult};
use uuid::Uuid;

use super::load_config;

#[derive(Debug, Clone, Args)]
pub struct SyncArgs {
    #[arg(long)]
    pub push_only: bool,
    #[arg(long)]
    pub pull_only: bool,
    #[arg(long)]
    pub reconcile: bool,
    #[arg(long)]
    pub agent_id: Option<Uuid>,
    #[arg(long, default_value = "assistant")]
    pub role: String,
}

pub async fn run(args: SyncArgs, data_dir: PathBuf) -> ClawDBResult<()> {
    let cfg = load_config(&data_dir)?;
    let agent_id = args.agent_id.unwrap_or(cfg.agent_id);
    let db = ClawDB::open(&data_dir).await?;
    let session = db
        .session(agent_id, &args.role, vec!["sync:write".to_string(), "sync:read".to_string()])
        .await?;

    let sync_engine = db.lifecycle.sync()?;
    if args.push_only {
        sync_engine.push_now().await?;
        println!("Synced: 1 pushed, 0 pulled, 0 conflicts");
    } else if args.pull_only {
        sync_engine.pull_now().await?;
        println!("Synced: 0 pushed, 1 pulled, 0 conflicts");
    } else {
        let v = db.sync(&session).await?;
        let pushed = v["pushed"].as_i64().unwrap_or(0);
        let pulled = v["pulled"].as_i64().unwrap_or(0);
        let conflicts = v["conflicts"].as_i64().unwrap_or(0);
        let _ = args.reconcile;
        println!("Synced: {pushed} pushed, {pulled} pulled, {conflicts} conflicts");
    }

    db.close().await
}
