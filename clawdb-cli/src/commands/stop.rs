//! `clawdb stop` — stop a detached clawdb-server process using the stored PID.

use clap::Args;

use crate::config::CliConfig;
use crate::error::{CliError, CliResult};
use crate::output::{print_success, OutputFormat};

#[derive(Debug, Clone, Args)]
pub struct StopArgs {}

pub async fn execute(_args: StopArgs, fmt: &OutputFormat, quiet: bool) -> CliResult<()> {
    let pid_path = CliConfig::config_dir().join("server.pid");
    if !pid_path.exists() {
        return Err(CliError::NotFound("no server.pid found".to_string()));
    }

    let pid = std::fs::read_to_string(&pid_path)?
        .trim()
        .parse::<i32>()
        .map_err(|_| CliError::Config("invalid PID in server.pid".to_string()))?;

    let status = std::process::Command::new("kill")
        .arg("-TERM")
        .arg(pid.to_string())
        .status()?;

    if !status.success() {
        return Err(CliError::Other(format!("failed to signal pid {}", pid)));
    }

    let _ = std::fs::remove_file(&pid_path);
    print_success(&format!("Stopped clawdb-server (pid: {pid})"), fmt, quiet);
    Ok(())
}
