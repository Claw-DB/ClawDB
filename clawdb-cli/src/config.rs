use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::CliResult;

/// Persistent CLI configuration stored at ~/.clawdb/config.toml.
#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct CliConfig {
    pub base_url: Option<String>,
    pub workspace_id: Option<Uuid>,
    pub data_dir: Option<PathBuf>,
    pub log_level: Option<String>,
}

impl CliConfig {
    /// Returns the config directory: $CLAW_DATA_DIR or ~/.clawdb.
    pub fn config_dir() -> PathBuf {
        if let Ok(d) = std::env::var("CLAW_DATA_DIR") {
            PathBuf::from(d)
        } else {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".clawdb")
        }
    }

    /// Load config from ~/.clawdb/config.toml (returns Default if the file is missing).
    pub fn load() -> CliResult<Self> {
        let path = Self::config_dir().join("config.toml");
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&path)?;
        let cfg: Self = toml::from_str(&content)?;
        Ok(cfg)
    }

    /// Persist config to ~/.clawdb/config.toml, creating the directory if needed.
    pub fn save(&self) -> CliResult<()> {
        let dir = Self::config_dir();
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("config.toml");
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}

/// Load session token from ~/.clawdb/session.token, checking 0600 permissions.
pub fn load_session_token() -> Option<String> {
    let path = CliConfig::config_dir().join("session.token");
    if !path.exists() {
        return None;
    }
    check_token_permissions(&path);
    std::fs::read_to_string(&path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Write session token to ~/.clawdb/session.token with 0600 permissions.
pub fn save_session_token(token: &str) -> CliResult<()> {
    let dir = CliConfig::config_dir();
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("session.token");
    std::fs::write(&path, token)?;
    set_token_permissions(&path)?;
    Ok(())
}

#[cfg(unix)]
fn check_token_permissions(path: &std::path::Path) {
    use std::os::unix::fs::MetadataExt;
    if let Ok(meta) = std::fs::metadata(path) {
        if meta.mode() & 0o777 != 0o600 {
            eprintln!(
                "⚠ Warning: {} has insecure permissions — run: chmod 600 {}",
                path.display(),
                path.display()
            );
        }
    }
}

#[cfg(not(unix))]
fn check_token_permissions(_path: &std::path::Path) {}

#[cfg(unix)]
fn set_token_permissions(path: &std::path::Path) -> CliResult<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_token_permissions(_path: &std::path::Path) -> CliResult<()> {
    Ok(())
}

/// Resolve the effective base_url using priority:
/// CLI flag/env (already parsed by clap) > config.toml > built-in default.
pub fn resolve_base_url(from_cli: &str, cfg: &CliConfig) -> String {
    let default = "http://localhost:8080";
    if from_cli != default {
        // User explicitly set it via flag or env
        return from_cli.to_string();
    }
    cfg.base_url.clone().unwrap_or_else(|| default.to_string())
}
