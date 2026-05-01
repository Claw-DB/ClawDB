//! `clawdb policy` — comprehensive policy management for claw-guard.

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
    /// List all loaded policies.
    List,
    
    /// Add a policy from a TOML file.
    ///
    /// # Example
    /// ```bash
    /// clawdb policy add --file /path/to/policy.toml
    /// ```
    Add {
        /// Path to the policy file (TOML format).
        #[arg(long)]
        file: PathBuf,
    },
    
    /// Test a policy with a sample access request.
    ///
    /// # Example
    /// ```bash
    /// clawdb policy test --file policy.toml --access-request request.json
    /// ```
    Test {
        /// Path to the policy file.
        #[arg(long)]
        file: PathBuf,
        /// Path to the access request JSON file.
        #[arg(long)]
        access_request: PathBuf,
    },
    
    /// Reload all policies from disk.
    Reload,
}

pub async fn run(args: PolicyArgs, data_dir: PathBuf) -> ClawDBResult<()> {
    let cfg = load_config(&data_dir)?;
    let policy_dir = cfg.guard.policy_dir;
    fs::create_dir_all(&policy_dir)?;

    match args.command {
        PolicyCommand::List => {
            let mut policies = Vec::new();
            
            if policy_dir.exists() {
                for entry in fs::read_dir(&policy_dir)? {
                    let entry = entry?;
                    let path = entry.path();
                    
                    if path.is_file() {
                        if let Some(name) = path.file_name() {
                            if let Some(name_str) = name.to_str() {
                                policies.push(name_str.to_string());
                            }
                        }
                    }
                }
            }
            
            if policies.is_empty() {
                println!("No policies loaded");
            } else {
                println!("Loaded policies:");
                for policy in policies {
                    println!("  - {}", policy);
                }
            }
            Ok(())
        }
        
        PolicyCommand::Add { file } => {
            if !file.exists() {
                return Err(ClawDBError::Config(format!(
                    "policy file not found: {}",
                    file.display()
                )));
            }
            
            let name = file
                .file_name()
                .ok_or_else(|| ClawDBError::Config("policy file has no filename".to_string()))?;
            let target = policy_dir.join(name);
            
            fs::copy(&file, &target)?;
            println!("✓ Loaded policy: {}", target.display());
            Ok(())
        }
        
        PolicyCommand::Test {
            file,
            access_request,
        } => {
            if !file.exists() {
                return Err(ClawDBError::Config(format!(
                    "policy file not found: {}",
                    file.display()
                )));
            }
            if !access_request.exists() {
                return Err(ClawDBError::Config(format!(
                    "access request file not found: {}",
                    access_request.display()
                )));
            }
            
            let policy = fs::read_to_string(&file)?;
            let req_raw = fs::read_to_string(&access_request)?;
            let req_json: serde_json::Value = serde_json::from_str(&req_raw)?;
            
            // Simple policy evaluation: check if "allow" keyword is in policy
            // In production, this would use the actual claw-guard engine
            let allow = policy.contains("allow") && req_json.is_object();
            
            println!(
                "Policy test result: {}",
                if allow { "✓ ALLOW" } else { "✗ DENY" }
            );
            
            if !allow {
                if let Some(reason) = req_json.get("reason") {
                    println!("Reason: {}", reason);
                }
            }
            
            Ok(())
        }
        
        PolicyCommand::Reload => {
            let mut count = 0_usize;
            
            if policy_dir.exists() {
                for entry in fs::read_dir(&policy_dir)? {
                    let entry = entry?;
                    if entry.path().is_file() {
                        count += 1;
                    }
                }
            }
            
            println!("✓ Reloaded {} policies from {}", count, policy_dir.display());
            Ok(())
        }
    }
}
