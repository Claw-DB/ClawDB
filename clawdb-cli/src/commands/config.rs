//! `clawdb config` — read/write ~/.clawdb/config.toml.

use clap::{Args, Subcommand};

use crate::config::CliConfig;
use crate::error::{CliError, CliResult};
use crate::output::{print_success, OutputFormat};

/// Args for `clawdb config` (named ConfigCmdArgs to avoid collision with the config module).
#[derive(Debug, Clone, Args)]
pub struct ConfigCmdArgs {
    #[command(subcommand)]
    pub command: ConfigCmd,
}

#[derive(Debug, Clone, Subcommand)]
pub enum ConfigCmd {
    /// Print a single config value.
    Get {
        /// Key: base_url | workspace_id | data_dir | log_level
        key: Option<String>,
    },
    /// Set a config value and persist it.
    Set { key: String, value: String },
    /// Show entire config (jwt_secret / api_key fields are redacted).
    Show,
}

pub async fn execute(args: ConfigCmdArgs, fmt: &OutputFormat, quiet: bool) -> CliResult<()> {
    match args.command {
        ConfigCmd::Show => {
            let cfg = CliConfig::load()?;
            // Redact nothing in CliConfig (no secrets stored here).
            match fmt {
                OutputFormat::Json => crate::output::print_json(&cfg, quiet),
                _ => {
                    let raw = toml::to_string_pretty(&cfg)?;
                    if !quiet {
                        println!("{}", raw);
                    }
                }
            }
        }

        ConfigCmd::Get { key } => {
            let cfg = CliConfig::load()?;
            if let Some(k) = key {
                let val = get_key(&cfg, &k)?;
                if !quiet {
                    println!("{}", val);
                }
            } else {
                let raw = toml::to_string_pretty(&cfg)?;
                if !quiet {
                    println!("{}", raw);
                }
            }
        }

        ConfigCmd::Set { key, value } => {
            let mut cfg = CliConfig::load()?;
            set_key(&mut cfg, &key, &value)?;
            cfg.save()?;
            print_success(&format!("Set {} = {}", key, value), fmt, quiet);
        }
    }
    Ok(())
}

fn get_key(cfg: &CliConfig, key: &str) -> CliResult<String> {
    Ok(match key {
        "base_url" => cfg.base_url.clone().unwrap_or_default(),
        "workspace_id" => cfg.workspace_id.map(|u| u.to_string()).unwrap_or_default(),
        "data_dir" => cfg
            .data_dir
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_default(),
        "log_level" => cfg.log_level.clone().unwrap_or_default(),
        k => return Err(CliError::Config(format!("unknown config key: {k}"))),
    })
}

fn set_key(cfg: &mut CliConfig, key: &str, value: &str) -> CliResult<()> {
    match key {
        "base_url" => cfg.base_url = Some(value.to_string()),
        "log_level" => cfg.log_level = Some(value.to_string()),
        "workspace_id" => {
            cfg.workspace_id =
                Some(value.parse().map_err(|_| {
                    CliError::Config("workspace_id must be a valid UUID".to_string())
                })?)
        }
        "data_dir" => cfg.data_dir = Some(std::path::PathBuf::from(value)),
        k => return Err(CliError::Config(format!("unknown config key: {k}"))),
    }
    Ok(())
}
