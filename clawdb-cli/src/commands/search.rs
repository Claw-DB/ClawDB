//! `clawdb search` — searches memories using keyword or semantic search.

use clap::Args;

/// Arguments for the `search` command.
#[derive(Debug, Args)]
pub struct SearchArgs {
    /// Query text.
    pub query: String,
    /// Use semantic (vector) search.
    #[arg(long)]
    pub semantic: bool,
    /// Maximum number of results.
    #[arg(long, default_value_t = 10)]
    pub top_k: usize,
    /// Agent ID (defaults to the configured agent).
    #[arg(long)]
    pub agent_id: Option<uuid::Uuid>,
}

/// Executes the `search` command.
pub async fn run(data_dir: &std::path::Path, args: &SearchArgs) -> anyhow::Result<()> {
    let cfg = clawdb::ClawDBConfig::load_or_default(data_dir)?;
    let agent_id = args.agent_id.unwrap_or(cfg.agent_id);

    let mut engine = clawdb::ClawDBEngine::new(cfg).await?;
    engine.start().await?;

    let results = engine
        .search(agent_id, &args.query, args.semantic, args.top_k)
        .await?;

    println!("Found {} result(s):", results.len());
    for (i, r) in results.iter().enumerate() {
        println!("  [{}] {}", i + 1, serde_json::to_string(r)?);
    }

    engine.stop().await?;
    Ok(())
}
