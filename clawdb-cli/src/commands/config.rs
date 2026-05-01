//! `clawdb config` — show/set/validate config.

use std::path::PathBuf;

use clap::{Args, Subcommand};
use clawdb::{ClawDBConfig, ClawDBError, ClawDBResult};

use super::load_config;

#[derive(Debug, Clone, Args)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub command: ConfigCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub enum ConfigCommand {
    Show,
    Set { key: String, value: String },
    Validate,
}

fn mask_secrets(mut cfg: ClawDBConfig) -> ClawDBConfig {
    if !cfg.guard.jwt_secret.is_empty() {
        cfg.guard.jwt_secret = "********".to_string();
    }
    cfg
}

pub async fn run(args: ConfigArgs, data_dir: PathBuf) -> ClawDBResult<()> {
    let mut cfg = load_config(&data_dir)?;
    let cfg_path = data_dir.join("config.toml");

    match args.command {
        ConfigCommand::Show => {
            let masked = mask_secrets(cfg);
            let raw = toml::to_string_pretty(&masked)
                .map_err(|e| ClawDBError::Config(e.to_string()))?;
            println!("{raw}");
        }
        ConfigCommand::Set { key, value } => {
            match key.as_str() {
                "log_level" => cfg.log_level = value,
                "log_format" => cfg.log_format = value,
                "server.grpc_port" => {
                    cfg.server.grpc_port = value
                        .parse()
                        .map_err(|_| ClawDBError::Config("invalid grpc port".to_string()))?
                }
                "server.http_port" => {
                    cfg.server.http_port = value
                        .parse()
                        .map_err(|_| ClawDBError::Config("invalid http port".to_string()))?
                }
                "telemetry.metrics_port" => {
                    cfg.telemetry.metrics_port = value
                        .parse()
                        .map_err(|_| ClawDBError::Config("invalid metrics port".to_string()))?
                }
                "vector.embedding_service_url" => cfg.vector.embedding_service_url = value,
                "sync.hub_url" => cfg.sync.hub_url = Some(value),
                "reflect.service_url" => cfg.reflect.service_url = value,
                "guard.jwt_secret" => cfg.guard.jwt_secret = value,
                _ => {
                    return Err(ClawDBError::Config(format!(
                        "unsupported config key: {key}"
                    )))
                }
            }
            cfg.save(&cfg_path)?;
            println!("Updated {}", key);
        }
        ConfigCommand::Validate => {
            let _ = ClawDBConfig::load(&cfg_path)?;
            println!("Config valid: {}", cfg_path.display());
        }
    }

    Ok(())
}
