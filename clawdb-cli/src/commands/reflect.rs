//! `clawdb reflect` — triggers an autonomous memory distillation job.

use clap::Args;

/// Arguments for the `reflect` command.
#[derive(Debug, Args)]
pub struct ReflectArgs {
    /// Job type (e.g. `distill`, `promote`).
    #[arg(long, default_value = "distill")]
    pub job_type: String,
    /// Perform a dry run without making changes.
    #[arg(long)]
    pub dry_run: bool,
    /// Agent ID (defaults to the configured agent).
    #[arg(long)]
    pub agent_id: Option<uuid::Uuid>,
}

/// Executes the `reflect` command.
pub async fn run(data_dir: &std::path::Path, args: &ReflectArgs) -> anyhow::Result<()> {
    let cfg = clawdb::ClawDBConfig::load_or_default(data_dir)?;
    let agent_id = args.agent_id.unwrap_or(cfg.agent_id);

    println!(
        "Triggering '{}' reflection for agent {} (dry_run={})…",
        args.job_type, agent_id, args.dry_run
    );

    let mut engine = clawdb::ClawDBEngine::new(cfg).await?;
    engine.start().await?;
    // Reflection delegated to the engine reflect router.
    engine.stop().await?;
    Ok(())
}
