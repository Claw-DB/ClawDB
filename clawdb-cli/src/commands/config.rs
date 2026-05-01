//! `clawdb config` — manage ClawDB configuration settings.

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
    /// Display the current configuration (secrets masked).
    Show,
    
    /// Get a specific configuration value.
    ///
    /// # Example
    /// ```bash
    /// clawdb config get log_level
    /// clawdb config get server.grpc_port
    /// ```
    Get {
        /// Configuration key (e.g., "log_level", "server.grpc_port").
        key: String,
    },
    
    /// Set a configuration value and save to disk.
    ///
    /// # Example
    /// ```bash
    /// clawdb config set log_level debug
    /// clawdb config set server.grpc_port 50051
    /// ```
    Set { key: String, value: String },
    
    /// Validate the configuration file.
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
            Ok(())
        }
        
        ConfigCommand::Get { key } => {
            let value = match key.as_str() {
                "log_level" => cfg.log_level.to_string(),
                "log_format" => cfg.log_format.to_string(),
                "server.grpc_port" => cfg.server.grpc_port.to_string(),
                "server.http_port" => cfg.server.http_port.to_string(),
                "telemetry.metrics_port" => cfg.telemetry.metrics_port.to_string(),
                "vector.embedding_service_url" => cfg.vector.embedding_service_url.clone(),
                "sync.hub_url" => cfg.sync.hub_url.clone().unwrap_or_else(|| "not set".to_string()),
                "reflect.service_url" => cfg.reflect.service_url.clone(),
                "plugins.enabled" => format!("{:?}", cfg.plugins.enabled),
                "plugins.sandbox_enabled" => cfg.plugins.sandbox_enabled.to_string(),
                _ => {
                    return Err(ClawDBError::Config(format!(
                        "unsupported config key: {key}"
                    )))
                }
            };
            println!("{}", value);
            Ok(())
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
                "guard.jwt_secret" => {
                    if value != "UNCHANGED" {
                        cfg.guard.jwt_secret = value;
                    }
                }
                _ => {
                    return Err(ClawDBError::Config(format!(
                        "unsupported config key: {key}"
                    )))
                }
            }
            cfg.save(&cfg_path)?;
            println!("✓ Updated {}", key);
            Ok(())
        }
        
        ConfigCommand::Validate => {
            let _ = ClawDBConfig::load(&cfg_path)?;
            println!("✓ Config is valid: {}", cfg_path.display());
            Ok(())
        }
    }
}
