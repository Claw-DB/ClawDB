//! `clawdb branch` — create/list/diff/merge/discard branch snapshots.

use std::{collections::BTreeMap, fs, path::PathBuf};

use chrono::Utc;
use clap::{Args, Subcommand, ValueEnum};
use clawdb::{ClawDB, ClawDBError, ClawDBResult};
use uuid::Uuid;

use super::{load_config, output_json};

#[derive(Debug, Clone, Args)]
pub struct BranchArgs {
    #[command(subcommand)]
    pub command: BranchCommand,
    #[arg(long)]
    pub agent_id: Option<Uuid>,
    #[arg(long, default_value = "assistant")]
    pub role: String,
}

#[derive(Debug, Clone, Subcommand)]
pub enum BranchCommand {
    Create {
        name: String,
        #[arg(long)]
        description: Option<String>,
        #[arg(long = "from", default_value = "trunk")]
        from_parent: String,
    },
    List {
        #[arg(long)]
        all: bool,
    },
    Diff {
        branch_a: String,
        branch_b: String,
    },
    Merge {
        source: String,
        #[arg(long = "into", default_value = "trunk")]
        target: String,
        #[arg(long, value_enum, default_value_t = MergeStrategy::Union)]
        strategy: MergeStrategy,
    },
    Discard {
        name: String,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum MergeStrategy {
    Ours,
    Theirs,
    Union,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct BranchRecord {
    id: Uuid,
    parent: String,
    description: Option<String>,
    created_at: String,
    status: String,
}

fn index_path(data_dir: &std::path::Path) -> PathBuf {
    data_dir.join("branches").join("cli_index.json")
}

fn load_index(data_dir: &std::path::Path) -> ClawDBResult<BTreeMap<String, BranchRecord>> {
    let path = index_path(data_dir);
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    let raw = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&raw).map_err(|e| ClawDBError::Config(e.to_string()))?)
}

fn save_index(data_dir: &std::path::Path, index: &BTreeMap<String, BranchRecord>) -> ClawDBResult<()> {
    let path = index_path(data_dir);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let raw = serde_json::to_string_pretty(index)?;
    fs::write(path, raw)?;
    Ok(())
}

fn resolve_branch(input: &str, index: &BTreeMap<String, BranchRecord>) -> Option<Uuid> {
    if let Ok(id) = Uuid::parse_str(input) {
        return Some(id);
    }
    index.get(input).map(|r| r.id)
}

pub async fn run(args: BranchArgs, data_dir: PathBuf) -> ClawDBResult<()> {
    let cfg = load_config(&data_dir)?;
    let agent_id = args.agent_id.unwrap_or(cfg.agent_id);
    let db = ClawDB::open(&data_dir).await?;
    let session = db
        .session(agent_id, &args.role, vec!["branch:write".to_string(), "branch:read".to_string()])
        .await?;

    let mut index = load_index(&data_dir)?;

    match args.command {
        BranchCommand::Create {
            name,
            description,
            from_parent,
        } => {
            let id = db.branch(&session, &name).await?;
            index.insert(
                name.clone(),
                BranchRecord {
                    id,
                    parent: from_parent.clone(),
                    description,
                    created_at: Utc::now().to_rfc3339(),
                    status: "active".to_string(),
                },
            );
            save_index(&data_dir, &index)?;
            println!("Created branch '{}' from '{}' (id: {})", name, from_parent, id);
        }
        BranchCommand::List { all } => {
            if output_json() {
                let rows: Vec<_> = index
                    .iter()
                    .filter(|(_, v)| all || v.status == "active")
                    .map(|(name, v)| {
                        serde_json::json!({
                            "name": name,
                            "status": v.status,
                            "parent": v.parent,
                            "created_at": v.created_at,
                            "id": v.id,
                        })
                    })
                    .collect();
                println!("{}", serde_json::to_string_pretty(&rows)?);
            } else {
                println!("{:<24} {:<10} {:<24} {}", "NAME", "STATUS", "PARENT", "CREATED_AT");
                for (name, rec) in index.iter().filter(|(_, v)| all || v.status == "active") {
                    println!("{:<24} {:<10} {:<24} {}", name, rec.status, rec.parent, rec.created_at);
                }
            }
        }
        BranchCommand::Diff { branch_a, branch_b } => {
            let a = resolve_branch(&branch_a, &index)
                .ok_or_else(|| ClawDBError::Config(format!("unknown branch: {branch_a}")))?;
            let b = resolve_branch(&branch_b, &index)
                .ok_or_else(|| ClawDBError::Config(format!("unknown branch: {branch_b}")))?;
            let diff = db.diff(&session, a, b).await?;
            let added = diff["added"].as_i64().unwrap_or(0);
            let removed = diff["removed"].as_i64().unwrap_or(0);
            let modified = diff["modified"].as_i64().unwrap_or(0);
            let divergence = diff["divergence_score"].as_f64().unwrap_or(0.0);
            println!(
                "+{} -{} ~{} (divergence: {:.2})",
                added, removed, modified, divergence
            );
        }
        BranchCommand::Merge {
            source,
            target,
            strategy,
        } => {
            let src = resolve_branch(&source, &index)
                .ok_or_else(|| ClawDBError::Config(format!("unknown branch: {source}")))?;
            let dst = resolve_branch(&target, &index)
                .ok_or_else(|| ClawDBError::Config(format!("unknown branch: {target}")))?;
            db.merge(&session, src, dst).await?;
            println!(
                "Merged '{}' into '{}' using {:?}",
                source,
                target,
                strategy
            );
        }
        BranchCommand::Discard { name } => {
            let rec = index
                .get_mut(&name)
                .ok_or_else(|| ClawDBError::Config(format!("unknown branch: {name}")))?;
            rec.status = "discarded".to_string();
            save_index(&data_dir, &index)?;
            println!("Discarded branch '{}'", name);
        }
    }

    db.close().await
}
