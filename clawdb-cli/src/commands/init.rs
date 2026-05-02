//! `clawdb init` — create ~/.clawdb directory and write a default config file.

use std::path::PathBuf;

use clap::Args;
use uuid::Uuid;

use crate::config::CliConfig;
use crate::error::CliResult;
use crate::output::{print_success, OutputFormat};

#[derive(Debug, Clone, Args)]
pub struct InitArgs {
    /// Data directory (default: ~/.clawdb).
    #[arg(long)]
    pub data_dir: Option<PathBuf>,

    /// Also write a reflect config stub to .env.reflect.
    #[arg(long)]
    pub with_reflect: bool,

    /// Workspace UUID (auto-generated if omitted).
    #[arg(long)]
    pub workspace_id: Option<Uuid>,
}

pub async fn execute(args: InitArgs, fmt: &OutputFormat, quiet: bool) -> CliResult<()> {
    let dir = args.data_dir.unwrap_or_else(CliConfig::config_dir);
    let cfg_path = dir.join("config.toml");

    if cfg_path.exists() {
        let overwrite = dialoguer::Confirm::with_theme(&dialoguer::theme::ColorfulTheme::default())
            .with_prompt("Already initialised. Overwrite?")
            .default(false)
            .interact()
            .unwrap_or(false);
        if !overwrite {
            return Ok(());
        }
    }

    std::fs::create_dir_all(&dir)?;
    let workspace_id = args.workspace_id.unwrap_or_else(Uuid::new_v4);
    let cfg = CliConfig {
        base_url: Some("http://localhost:8080".to_string()),
        workspace_id: Some(workspace_id),
        data_dir: Some(dir.clone()),
        log_level: Some("info".to_string()),
    };
    let raw = toml::to_string_pretty(&cfg)?;
    std::fs::write(&cfg_path, raw)?;

    if args.with_reflect {
        let env_path = dir.join(".env.reflect");
        std::fs::write(
            &env_path,
            format!(
                "REFLECT_BASE_URL=http://localhost:8002\nWORKSPACE_ID={}\n",
                workspace_id
            ),
        )?;
    }

    print_success(
        &format!("ClawDB initialised at {}", dir.display()),
        fmt,
        quiet,
    );
    Ok(())
}
