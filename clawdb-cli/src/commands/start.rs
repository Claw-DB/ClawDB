//! `clawdb start` — spawn the clawdb-server binary as a subprocess.

use std::path::PathBuf;

use clap::Args;

use crate::error::{CliError, CliResult};
use crate::output::{print_success, OutputFormat};

#[derive(Debug, Clone, Args)]
pub struct StartArgs {
    /// gRPC port.
    #[arg(long, default_value_t = 50050)]
    pub grpc_port: u16,

    /// HTTP REST port.
    #[arg(long, default_value_t = 8080)]
    pub http_port: u16,

    /// Prometheus metrics port.
    #[arg(long, default_value_t = 9090)]
    pub metrics_port: u16,

    /// Run in the foreground (inherits stdio, blocks until exit).
    #[arg(long)]
    pub foreground: bool,

    /// Path to a server config file.
    #[arg(long)]
    pub config: Option<PathBuf>,
}

pub async fn execute(args: StartArgs, fmt: &OutputFormat, quiet: bool) -> CliResult<()> {
    // Try to find clawdb-server next to the current executable first.
    let server_bin = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("clawdb-server")))
        .unwrap_or_else(|| PathBuf::from("clawdb-server"));

    let mut cmd = std::process::Command::new(&server_bin);
    cmd.arg("--grpc-port")
        .arg(args.grpc_port.to_string())
        .arg("--http-port")
        .arg(args.http_port.to_string())
        .arg("--metrics-port")
        .arg(args.metrics_port.to_string());

    if let Some(cfg_path) = &args.config {
        cmd.arg("--config").arg(cfg_path);
    }

    if args.foreground {
        // Inherit stdio — this call blocks.
        let status = cmd.status()?;
        if !status.success() {
            let code = status.code().unwrap_or(1);
            return Err(CliError::Other(format!(
                "clawdb-server exited with code {code}"
            )));
        }
    } else {
        // Detach from stdio and background the process.
        let child = cmd
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()?;
        let pid = child.id();

        // Write PID file.
        let pid_dir = crate::config::CliConfig::config_dir();
        std::fs::create_dir_all(&pid_dir)?;
        std::fs::write(pid_dir.join("server.pid"), pid.to_string())?;

        print_success(&format!("ClawDB server started (pid: {pid})"), fmt, quiet);
    }

    Ok(())
}
