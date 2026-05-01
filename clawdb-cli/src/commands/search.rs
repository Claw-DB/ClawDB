//! `clawdb search` — keyword/semantic search over memory.

use std::path::PathBuf;

use clap::Args;
use clawdb::{ClawDB, ClawDBResult};
use uuid::Uuid;

use super::{load_config, output_json};

#[derive(Debug, Clone, Args)]
pub struct SearchArgs {
    pub query: String,
    #[arg(long, default_value_t = 5)]
    pub top_k: u32,
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    pub semantic: bool,
    #[arg(long)]
    pub agent_id: Option<Uuid>,
    #[arg(long, default_value = "assistant")]
    pub role: String,
    #[arg(long)]
    pub filter: Option<String>,
    #[arg(long)]
    pub show_scores: bool,
}

pub async fn run(args: SearchArgs, data_dir: PathBuf) -> ClawDBResult<()> {
    let cfg = load_config(&data_dir)?;
    let agent_id = args.agent_id.unwrap_or(cfg.agent_id);
    let filter = if let Some(raw) = args.filter.as_deref() {
        Some(serde_json::from_str(raw)?)
    } else {
        None
    };

    let db = ClawDB::open(&data_dir).await?;
    let session = db
        .session(
            agent_id,
            &args.role,
            vec!["memory:read".to_string(), "memory:search".to_string()],
        )
        .await?;
    let results = db
        .search_with_options(&session, &args.query, args.top_k as usize, args.semantic, filter)
        .await?;

    if output_json() {
        println!("{}", serde_json::to_string_pretty(&results)?);
    } else {
        for (idx, item) in results.iter().enumerate() {
            let content = item
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let truncated = if content.chars().count() > 80 {
                format!("{}...", content.chars().take(80).collect::<String>())
            } else {
                content.to_string()
            };
            let memory_type = item
                .get("memory_type")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let tags = item
                .get("tags")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str())
                        .collect::<Vec<_>>()
                        .join(",")
                })
                .unwrap_or_default();
            if args.show_scores {
                let score = item.get("score").and_then(|v| v.as_f64()).unwrap_or_default();
                println!(
                    "{}. {} [{}] tags=[{}] score={:.4}",
                    idx + 1,
                    truncated,
                    memory_type,
                    tags,
                    score
                );
            } else {
                println!("{}. {} [{}] tags=[{}]", idx + 1, truncated, memory_type, tags);
            }
        }
    }

    db.close().await
}
