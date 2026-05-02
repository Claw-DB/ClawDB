use clap::{Parser, Subcommand};

use crate::commands::{
    branch::BranchArgs, completion::CompletionArgs, config::ConfigCmdArgs, init::InitArgs,
    policy::PolicyArgs, recall::RecallArgs, reflect::ReflectArgs, remember::RememberArgs,
    search::SearchArgs, session::SessionArgs, start::StartArgs, status::StatusArgs, sync::SyncArgs,
};
use crate::output::OutputFormat;

#[derive(Parser)]
#[command(
    name = "clawdb",
    about = "The cognitive database for AI agents.",
    version,
    author
)]
pub struct Cli {
    /// ClawDB server base URL.
    #[arg(long, env = "CLAWDB_BASE_URL", default_value = "http://localhost:8080")]
    pub base_url: String,

    /// Session token for authenticated requests.
    #[arg(long, env = "CLAWDB_SESSION_TOKEN", global = true)]
    pub token: Option<String>,

    /// Output format: table (default), json, tsv.
    #[arg(long, global = true, default_value = "table")]
    pub output: OutputFormat,

    /// Suppress all output except errors.
    #[arg(long, short = 'q', global = true)]
    pub quiet: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialise the ~/.clawdb directory and config file.
    Init(InitArgs),
    /// Start the ClawDB server process.
    Start(StartArgs),
    /// Check server component health.
    Status(StatusArgs),
    /// Manage sessions (create / revoke / whoami).
    Session(SessionArgs),
    /// Store a memory in ClawDB.
    Remember(RememberArgs),
    /// Search memories semantically or with full-text search.
    Search(SearchArgs),
    /// Retrieve one or more memories by ID.
    Recall(RecallArgs),
    /// Manage memory branches (create / list / merge / diff / discard).
    Branch(BranchArgs),
    /// Synchronise memories with the hub.
    Sync(SyncArgs),
    /// Trigger reflection jobs.
    Reflect(ReflectArgs),
    /// Manage access control policies.
    Policy(PolicyArgs),
    /// Read or write local CLI configuration.
    Config(ConfigCmdArgs),
    /// Generate shell completion scripts.
    Completion(CompletionArgs),
}
