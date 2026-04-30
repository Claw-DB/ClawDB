//! `clawdb branch` — manages branches (create, list, merge, diff).

use clap::{Args, Subcommand};

/// Arguments for the `branch` command.
#[derive(Debug, Args)]
pub struct BranchArgs {
    #[command(subcommand)]
    pub action: BranchAction,
}

/// Branch sub-actions.
#[derive(Debug, Subcommand)]
pub enum BranchAction {
    /// List existing branches.
    List,
    /// Create a new branch.
    Create {
        /// New branch name.
        name: String,
        /// Parent branch (default: trunk).
        #[arg(long, default_value = "trunk")]
        parent: String,
    },
    /// Merge one branch into another.
    Merge {
        /// Source branch.
        source: String,
        /// Target branch.
        target: String,
        /// Merge strategy.
        #[arg(long, default_value = "crdt")]
        strategy: String,
    },
    /// Show the diff between two branches.
    Diff {
        /// First branch.
        branch_a: String,
        /// Second branch.
        branch_b: String,
    },
}

/// Executes the `branch` command.
pub async fn run(data_dir: &std::path::Path, args: &BranchArgs) -> anyhow::Result<()> {
    let cfg = clawdb::ClawDBConfig::load_or_default(data_dir)?;
    let mut engine = clawdb::ClawDBEngine::new(cfg).await?;
    engine.start().await?;

    match &args.action {
        BranchAction::List => {
            println!("Branches: (not implemented in stub)");
        }
        BranchAction::Create { name, parent } => {
            println!("Creating branch '{name}' from '{parent}'…");
        }
        BranchAction::Merge { source, target, strategy } => {
            println!("Merging '{source}' → '{target}' using strategy '{strategy}'…");
        }
        BranchAction::Diff { branch_a, branch_b } => {
            println!("Diffing '{branch_a}' vs '{branch_b}'…");
        }
    }

    engine.stop().await?;
    Ok(())
}
