//! `clawdb config` — reads and writes the ClawDB configuration.

use clap::{Args, Subcommand};

/// Arguments for the `config` command.
#[derive(Debug, Args)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub action: ConfigAction,
}

/// Config sub-actions.
#[derive(Debug, Subcommand)]
pub enum ConfigAction {
    /// Print the effective configuration.
    Show,
    /// Write the default configuration to the data directory.
    Init,
}

/// Executes the `config` command.
pub async fn run(data_dir: &std::path::Path, args: &ConfigArgs) -> anyhow::Result<()> {
    match &args.action {
        ConfigAction::Show => {
            let cfg = clawdb::ClawDBConfig::load_or_default(data_dir)?;
            let toml_str = toml::to_string_pretty(&cfg)
                .map_err(|e| anyhow::anyhow!("serialisation error: {e}"))?;
            println!("{toml_str}");
        }
        ConfigAction::Init => {
            let cfg = clawdb::ClawDBConfig::default_for_dir(data_dir);
            let path = data_dir.join("config.toml");
            cfg.save(&path)?;
            println!("Wrote default config to {}", path.display());
        }
    }
    Ok(())
}
