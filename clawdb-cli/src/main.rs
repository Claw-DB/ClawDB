//! ClawDB CLI binary — pure HTTP client for clawdb-server.

mod cli;
mod client;
mod commands;
mod config;
mod error;
mod output;
mod types;

use clap::Parser;
use tracing_subscriber::{fmt, EnvFilter};

use cli::{Cli, Commands};
use client::ClawDBClient;
use config::{load_session_token, resolve_base_url, CliConfig};
use error::CliError;
use output::{print_error, OutputFormat};

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Initialise tracing at the log level from config/env.
    let cfg = CliConfig::load().unwrap_or_default();
    let log_level = cfg.log_level.as_deref().unwrap_or("warn");
    fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(log_level)),
        )
        .with_target(false)
        .compact()
        .init();

    let fmt = cli.output.clone();
    let quiet = cli.quiet;

    if let Err(e) = run(cli, cfg, &fmt, quiet).await {
        print_error(&e.to_string(), &fmt);
        let code = match &e {
            CliError::Unauthorized(_) => 77,
            CliError::PermissionDenied(_) => 77,
            CliError::NotFound(_) => 1,
            CliError::RateLimited { .. } => 75,
            CliError::ServiceUnavailable => 69,
            CliError::Connection(_) => 68,
            _ => 1,
        };
        std::process::exit(code);
    }
}

async fn run(cli: Cli, cfg: CliConfig, fmt: &OutputFormat, quiet: bool) -> Result<(), CliError> {
    // Resolve effective base URL (CLI flag > env > config.toml > default).
    let base_url = resolve_base_url(&cli.base_url, &cfg);

    // Resolve token (CLI flag/env > ~/.clawdb/session.token).
    let token = cli.token.or_else(load_session_token);

    // Build the HTTP client — cheap, no network calls yet.
    let client = ClawDBClient::new(base_url, token)?;

    match cli.command {
        Commands::Init(args) => commands::init::execute(args, fmt, quiet).await,
        Commands::Start(args) => commands::start::execute(args, fmt, quiet).await,
        Commands::Status(args) => commands::status::execute(args, &client, fmt, quiet).await,
        Commands::Session(args) => commands::session::execute(args, &client, fmt, quiet).await,
        Commands::Remember(args) => commands::remember::execute(args, &client, fmt, quiet).await,
        Commands::Search(args) => commands::search::execute(args, &client, fmt, quiet).await,
        Commands::Recall(args) => commands::recall::execute(args, &client, fmt, quiet).await,
        Commands::Branch(args) => commands::branch::execute(args, &client, fmt, quiet).await,
        Commands::Sync(args) => commands::sync::execute(args, &client, fmt, quiet).await,
        Commands::Reflect(args) => commands::reflect::execute(args, &client, fmt, quiet).await,
        Commands::Policy(args) => commands::policy::execute(args, &client, fmt, quiet).await,
        Commands::Config(args) => commands::config::execute(args, fmt, quiet).await,
        Commands::Completion(args) => {
            let mut app = <Cli as clap::CommandFactory>::command();
            commands::completion::execute(args, &mut app);
            Ok(())
        }
    }
}
