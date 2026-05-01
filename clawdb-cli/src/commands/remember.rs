//! `clawdb remember` — store memory content.

use std::path::PathBuf;

use clap::Args;
use clawdb::{ClawDB, ClawDBResult};
use uuid::Uuid;

use super::{load_config, output_json};

#[derive(Debug, Clone, Args)]
pub struct RememberArgs {
    #[arg(index = 1, required_unless_present = "content")]
    pub positional_content: Option<String>,
    #[arg(long)]
    pub content: Option<String>,
    #[arg(long = "type", default_value = "context")]
    pub memory_type: String,
    #[arg(long)]
    pub tags: Option<String>,
    #[arg(long)]
    pub metadata: Option<String>,
    #[arg(long)]
    pub agent_id: Option<Uuid>,
    #[arg(long, default_value = "assistant")]
    pub role: String,
}

pub async fn run(args: RememberArgs, data_dir: PathBuf) -> ClawDBResult<()> {
    let cfg = load_config(&data_dir)?;
    let agent_id = args.agent_id.unwrap_or(cfg.agent_id);
    let content = args
        .content
        .or(args.positional_content)
        .unwrap_or_default();
    let tags: Vec<String> = args
        .tags
        .as_deref()
        .unwrap_or("")
        .split(',')
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(ToOwned::to_owned)
        .collect();
    let metadata = if let Some(raw) = args.metadata.as_deref() {
        serde_json::from_str(raw)?
    } else {
        serde_json::Value::Null
    };

    let db = ClawDB::open(&data_dir).await?;
    let session = db
        .session(
            agent_id,
            &args.role,
            vec!["memory:write".to_string(), "memory:read".to_string()],
        )
        .await?;
    let res = db
        .remember_typed(&session, &content, &args.memory_type, &tags, metadata)
        .await?;

    if output_json() {
        println!(
            "{}",
            serde_json::json!({
                "memory_id": res.memory_id,
                "memory_type": args.memory_type,
                "importance_score": res.importance_score,
            })
        );
    } else {
        println!("Stored memory {} [{}]", res.memory_id, args.memory_type);
    }

    db.close().await
}
