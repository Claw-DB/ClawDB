//! `clawdb remember` — stores a memory entry into ClawDB.

use clap::Args;

/// Arguments for the `remember` command.
#[derive(Debug, Args)]
pub struct RememberArgs {
    /// Content to store.
    pub content: String,
    /// Memory type label.
    #[arg(long, default_value = "general")]
    pub memory_type: String,
    /// Comma-separated tags.
    #[arg(long)]
    pub tags: Option<String>,
    /// Agent ID (defaults to the configured agent).
    #[arg(long)]
    pub agent_id: Option<uuid::Uuid>,
}

/// Executes the `remember` command.
pub async fn run(data_dir: &std::path::Path, args: &RememberArgs) -> anyhow::Result<()> {
    let cfg = clawdb::ClawDBConfig::load_or_default(data_dir)?;
    let agent_id = args.agent_id.unwrap_or(cfg.agent_id);
    let tags: Vec<String> = args
        .tags
        .as_deref()
        .unwrap_or("")
        .split(',')
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect();

    let mut engine = clawdb::ClawDBEngine::new(cfg).await?;
    engine.start().await?;

    let result = engine
        .remember(agent_id, &args.content, &args.memory_type, &tags)
        .await?;

    println!("Stored memory: {}", result.memory_id);
    println!("Importance score: {:.3}", result.importance_score);
    engine.stop().await?;
    Ok(())
}
