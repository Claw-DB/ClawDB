//! `clawdb init` — initialises a new ClawDB data directory.

use clap::Args;

/// Arguments for the `init` command.
#[derive(Debug, Args)]
pub struct InitArgs {
    /// Force re-initialisation of an existing data directory.
    #[arg(long, short)]
    pub force: bool,
}

/// Executes the `init` command.
pub async fn run(data_dir: &std::path::Path, args: &InitArgs) -> anyhow::Result<()> {
    if data_dir.exists() && !args.force {
        anyhow::bail!(
            "data directory {} already exists; use --force to reinitialise",
            data_dir.display()
        );
    }
    std::fs::create_dir_all(data_dir)?;
    let cfg = clawdb::ClawDBConfig::default_for_dir(data_dir);
    cfg.save(&data_dir.join("config.toml"))?;
    println!("Initialised ClawDB at {}", data_dir.display());
    Ok(())
}
