//! ClawDB command-line entry point.

use clap::{Parser, Subcommand};
use clawdb::lifecycle::GracefulShutdown;
use clawdb::telemetry::init_tracing;
use clawdb::{ClawDBConfig, ClawDBEngine};

#[derive(Parser)]
#[command(name = "clawdb", about = "ClawDB – the agent memory database", version)]
struct Cli {
    /// Path to the data directory.
    #[arg(long, env = "CLAW_DATA_DIR")]
    data_dir: Option<std::path::PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Start the ClawDB gRPC server.
    Serve {
        /// gRPC listen port.
        #[arg(long, default_value = "50050")]
        port: u16,
    },
    /// Print the current health status and exit.
    Health,
    /// Print the default configuration and exit.
    Config,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let data_dir = cli
        .data_dir
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".clawdb"));

    let config = ClawDBConfig::load_or_default(&data_dir)?;
    init_tracing(&config.log_level, &config.log_format);

    match cli.command {
        Command::Serve { port: _ } => {
            tracing::info!("Starting ClawDB engine…");
            let engine = ClawDBEngine::start_with(config).await?;
            let shutdown = GracefulShutdown::new(30);
            shutdown.wait_for_signal().await;
            engine.shutdown().await?;
            tracing::info!("ClawDB stopped cleanly");
        }
        Command::Health => {
            let engine = ClawDBEngine::start_with(config).await?;
            let report = engine.health().await?;
            println!("{}", serde_json::to_string_pretty(&report)?);
            engine.shutdown().await?;
        }
        Command::Config => {
            let raw = toml::to_string_pretty(&config).unwrap_or_default();
            println!("{raw}");
        }
    }

    Ok(())
}
