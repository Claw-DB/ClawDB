//! `clawdb start` — starts the embedded ClawDB server.

use clap::Args;

/// Arguments for the `start` command.
#[derive(Debug, Args)]
pub struct StartArgs {
    /// gRPC listen port (overrides config).
    #[arg(long)]
    pub grpc_port: Option<u16>,
    /// Run in the foreground (default: true).
    #[arg(long, default_value_t = true)]
    pub foreground: bool,
}

/// Executes the `start` command.
pub async fn run(data_dir: &std::path::Path, args: &StartArgs) -> anyhow::Result<()> {
    let mut cfg = clawdb::ClawDBConfig::load_or_default(data_dir)?;
    if let Some(port) = args.grpc_port {
        cfg.server.grpc_port = port;
    }
    println!("Starting ClawDB on gRPC port {}…", cfg.server.grpc_port);
    let mut engine = clawdb::ClawDBEngine::new(cfg).await?;
    engine.start().await?;
    println!("ClawDB running. Press Ctrl-C to stop.");
    tokio::signal::ctrl_c().await?;
    engine.stop().await?;
    Ok(())
}
