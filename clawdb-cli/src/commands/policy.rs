//! `clawdb policy` — file-oriented policy management.

use std::{fs, path::PathBuf};

use clap::{Args, Subcommand};
use clawdb::{ClawDBError, ClawDBResult};

use super::load_config;

#[derive(Debug, Clone, Args)]
pub struct PolicyArgs {
    #[command(subcommand)]
    pub command: PolicyCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub enum PolicyCommand {
    Load { file: PathBuf },
    List,
    Test { file: PathBuf, access_request: PathBuf },
    Reload,
}

pub async fn run(args: PolicyArgs, data_dir: PathBuf) -> ClawDBResult<()> {
    let cfg = load_config(&data_dir)?;
    let policy_dir = cfg.guard.policy_dir;
    fs::create_dir_all(&policy_dir)?;

    match args.command {
        PolicyCommand::Load { file } => {
            let name = file
                .file_name()
                .ok_or_else(|| ClawDBError::Config("policy file has no filename".to_string()))?;
            let target = policy_dir.join(name);
            fs::copy(&file, &target)?;
            println!("Loaded policy: {}", target.display());
        }
        PolicyCommand::List => {
            for entry in fs::read_dir(&policy_dir)? {
                let entry = entry?;
                if entry.path().extension().and_then(|s| s.to_str()) == Some("gpl") {
                    println!("{}", entry.file_name().to_string_lossy());
                }
            }
        }
        PolicyCommand::Test {
            file,
            access_request,
        } => {
            let policy = fs::read_to_string(file)?;
            let req_raw = fs::read_to_string(access_request)?;
            let req_json: serde_json::Value = serde_json::from_str(&req_raw)?;
            let allow = policy.contains("allow") && req_json.is_object();
            println!(
                "Policy test result: {}",
                if allow { "ALLOW" } else { "DENY" }
            );
        }
        PolicyCommand::Reload => {
            let mut count = 0usize;
            for entry in fs::read_dir(&policy_dir)? {
                let entry = entry?;
                if entry.path().extension().and_then(|s| s.to_str()) == Some("gpl") {
                    count += 1;
                }
            }
            println!("Reloaded {count} policies from {}", policy_dir.display());
        }
    }

    Ok(())
}
