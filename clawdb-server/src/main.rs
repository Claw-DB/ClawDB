//! ClawDB gRPC server binary.

use clap::Parser;
use clawdb::{ClawDBConfig, ClawDBEngine};
use clawdb::lifecycle::GracefulShutdown;
use clawdb::telemetry::init_tracing;

#[derive(Parser)]
#[command(name = "clawdb-server", about = "ClawDB gRPC server", version)]
struct Args {
    /// Path to the data directory.
    #[arg(long, env = "CLAW_DATA_DIR")]
    data_dir: Option<std::path::PathBuf>,

    /// gRPC listen port.
    #[arg(long, default_value = "50050", env = "CLAW_GRPC_PORT")]
    port: u16,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let data_dir = args
        .data_dir
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".clawdb"));

    let mut config = ClawDBConfig::load_or_default(&data_dir)?;
    config.server.grpc_port = args.port;

    init_tracing(&config.log_level, &config.log_format);

    tracing::info!(port = config.server.grpc_port, "Starting ClawDB server");
    let engine = ClawDBEngine::start(config).await?;

    let shutdown = GracefulShutdown::new(30);
    shutdown.wait_for_signal().await;
    engine.shutdown().await?;

    Ok(())
}
