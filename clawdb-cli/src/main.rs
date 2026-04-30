//! ClawDB command-line interface.

use clap::{Parser, Subcommand};
use clawdb::{ClawDBConfig, ClawDBEngine};
use clawdb::telemetry::init_tracing;

#[derive(Parser)]
#[command(name = "clawdb-cli", about = "ClawDB CLI", version)]
struct Cli {
    #[arg(long, env = "CLAW_DATA_DIR")]
    data_dir: Option<std::path::PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print health status.
    Health,
    /// Print effective configuration.
    Config,
    /// Print runtime version.
    Version,
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
        Command::Health => {
            let engine = ClawDBEngine::start(config).await?;
            let report = engine.health().await;
            println!("{}", serde_json::to_string_pretty(&report)?);
            engine.shutdown().await?;
        }
        Command::Config => {
            let raw = toml::to_string_pretty(&config).unwrap_or_default();
            println!("{raw}");
        }
        Command::Version => {
            println!("clawdb-cli {}", env!("CARGO_PKG_VERSION"));
        }
    }

    Ok(())
}
