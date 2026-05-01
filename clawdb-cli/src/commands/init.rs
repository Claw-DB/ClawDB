//! `clawdb init` — initialise data directory and bootstrap all components.

use std::path::PathBuf;

use clap::Args;
use clawdb::{ClawDB, ClawDBConfig, ClawDBResult};
use uuid::Uuid;

use super::output_json;

#[derive(Debug, Clone, Args)]
pub struct InitArgs {
    #[arg(long)]
    pub workspace_id: Option<Uuid>,
    #[arg(long)]
    pub agent_id: Option<Uuid>,
    #[arg(long)]
    pub data_dir: Option<PathBuf>,
    #[arg(long)]
    pub embedding_url: Option<String>,
    #[arg(long)]
    pub hub_url: Option<String>,
    #[arg(long)]
    pub with_reflect: bool,
}

pub async fn run(args: InitArgs, data_dir: PathBuf) -> ClawDBResult<()> {
    let data_dir = args.data_dir.unwrap_or(data_dir);
    std::fs::create_dir_all(&data_dir)?;

    let workspace_id = args.workspace_id.unwrap_or_else(Uuid::new_v4);
    let agent_id = args.agent_id.unwrap_or_else(Uuid::new_v4);

    let mut cfg = ClawDBConfig::default_for_dir(&data_dir);
    cfg.data_dir = data_dir.clone();
    cfg.workspace_id = workspace_id;
    cfg.agent_id = agent_id;
    if let Some(url) = args.embedding_url {
        cfg.vector.embedding_service_url = url;
    }
    if let Some(url) = args.hub_url {
        cfg.sync.hub_url = Some(url);
    }
    if args.with_reflect {
        cfg.reflect.service_url = "http://localhost:8002".to_string();
    }

    let cfg_path = data_dir.join("config.toml");
    cfg.save(&cfg_path)?;

    let db = ClawDB::new(cfg).await?;
    db.close().await?;

    if output_json() {
        println!(
            "{}",
            serde_json::json!({
                "ok": true,
                "data_dir": data_dir,
                "workspace_id": workspace_id,
                "agent_id": agent_id,
            })
        );
    } else {
        println!(
            "ClawDB initialised at {}\nWorkspace: {}\nAgent: {}",
            data_dir.display(),
            workspace_id,
            agent_id
        );
    }

    Ok(())
}
