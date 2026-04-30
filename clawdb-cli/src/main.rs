//! ClawDB command-line interface — entry point and top-level command dispatch.

mod commands;

use clap::{Parser, Subcommand};
use clawdb::ClawDBConfig;
use clawdb::telemetry::init_tracing;
use commands::{
    branch::BranchArgs, config::ConfigArgs, init::InitArgs, policy::PolicyArgs,
    reflect::ReflectArgs, remember::RememberArgs, search::SearchArgs, start::StartArgs,
    status::StatusArgs, sync::SyncArgs,
};

#[derive(Parser)]
#[command(name = "clawdb", about = "ClawDB — the aggregate AI memory runtime", version)]
struct Cli {
    /// Path to the ClawDB data directory.
    #[arg(long, env = "CLAW_DATA_DIR", global = true)]
    data_dir: Option<std::path::PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Initialise a new ClawDB data directory.
    Init(InitArgs),
    /// Start the embedded ClawDB server.
    Start(StartArgs),
    /// Print runtime health and statistics.
    Status(StatusArgs),
    /// Store a memory entry.
    Remember(RememberArgs),
    /// Search for memories.
    Search(SearchArgs),
    /// Manage branches.
    Branch(BranchArgs),
    /// Trigger a sync cycle.
    Sync(SyncArgs),
    /// Trigger a memory reflection job.
    Reflect(ReflectArgs),
    /// Manage security policies.
    Policy(PolicyArgs),
    /// Read or write ClawDB configuration.
    Config(ConfigArgs),
    /// Print the ClawDB version.
    Version,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let data_dir = cli
        .data_dir
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".clawdb"));

    // Initialise tracing before anything else (best-effort; config may not exist yet).
    if let Ok(cfg) = ClawDBConfig::load_or_default(&data_dir) {
        init_tracing(&cfg.log_level, &cfg.log_format);
    }

    match &cli.command {
        Command::Init(args) => commands::init::run(&data_dir, args).await?,
        Command::Start(args) => commands::start::run(&data_dir, args).await?,
        Command::Status(args) => commands::status::run(&data_dir, args).await?,
        Command::Remember(args) => commands::remember::run(&data_dir, args).await?,
        Command::Search(args) => commands::search::run(&data_dir, args).await?,
        Command::Branch(args) => commands::branch::run(&data_dir, args).await?,
        Command::Sync(args) => commands::sync::run(&data_dir, args).await?,
        Command::Reflect(args) => commands::reflect::run(&data_dir, args).await?,
        Command::Policy(args) => commands::policy::run(&data_dir, args).await?,
        Command::Config(args) => commands::config::run(&data_dir, args).await?,
        Command::Version => {
            println!("clawdb {}", env!("CARGO_PKG_VERSION"));
        }
    }

    Ok(())
}
