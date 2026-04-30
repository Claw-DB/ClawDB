//! `clawdb policy` — manages claw-guard security policies.

use clap::{Args, Subcommand};

/// Arguments for the `policy` command.
#[derive(Debug, Args)]
pub struct PolicyArgs {
    #[command(subcommand)]
    pub action: PolicyAction,
}

/// Policy sub-actions.
#[derive(Debug, Subcommand)]
pub enum PolicyAction {
    /// List loaded policies.
    List,
    /// Validate a policy file without loading it.
    Validate {
        /// Path to the policy file.
        path: std::path::PathBuf,
    },
}

/// Executes the `policy` command.
pub async fn run(_data_dir: &std::path::Path, args: &PolicyArgs) -> anyhow::Result<()> {
    match &args.action {
        PolicyAction::List => {
            println!("Policies: (connect to a running daemon to list)");
        }
        PolicyAction::Validate { path } => {
            if path.exists() {
                println!("Policy file {} appears valid (stub check).", path.display());
            } else {
                anyhow::bail!("Policy file not found: {}", path.display());
            }
        }
    }
    Ok(())
}
