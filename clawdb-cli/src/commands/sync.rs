//! `clawdb sync` — triggers a sync cycle with the configured hub.

use clap::Args;

/// Arguments for the `sync` command.
#[derive(Debug, Args)]
pub struct SyncArgs {
    /// Agent ID (defaults to the configured agent).
    #[arg(long)]
    pub agent_id: Option<uuid::Uuid>,
}

/// Executes the `sync` command.
pub async fn run(data_dir: &std::path::Path, args: &SyncArgs) -> anyhow::Result<()> {
    let cfg = clawdb::ClawDBConfig::load_or_default(data_dir)?;
    let agent_id = args.agent_id.unwrap_or(cfg.agent_id);

    let mut engine = clawdb::ClawDBEngine::new(cfg).await?;
    engine.start().await?;
    println!("Syncing agent {}…", agent_id);
    // Sync logic delegated to the engine.
    engine.stop().await?;
    Ok(())
}
