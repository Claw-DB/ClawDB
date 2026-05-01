//! ClawDB CLI entrypoint with subcommands and interactive REPL.

mod commands;

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use commands::{
    branch::{BranchArgs, BranchCommand},
    config::ConfigArgs,
    init::InitArgs,
    policy::PolicyArgs,
    reflect::ReflectArgs,
    remember::RememberArgs,
    search::SearchArgs,
    start::StartArgs,
    status::StatusArgs,
    sync::SyncArgs,
};
use rustyline::{
    completion::{Completer, Pair},
    error::ReadlineError,
    highlight::Highlighter,
    hint::Hinter,
    validate::Validator,
    Helper,
};

#[derive(Parser)]
#[command(name = "clawdb", version, about = "The cognitive database for AI agents")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
    #[arg(global = true, long, env = "CLAW_DATA_DIR")]
    data_dir: Option<PathBuf>,
    #[arg(global = true, long, env = "CLAW_LOG_LEVEL", default_value = "warn")]
    log_level: String,
    #[arg(global = true, long)]
    json: bool,
}

#[derive(Subcommand)]
enum Commands {
    Init(InitArgs),
    Start(StartArgs),
    Status(StatusArgs),
    Remember(RememberArgs),
    Search(SearchArgs),
    Branch(BranchArgs),
    Sync(SyncArgs),
    Reflect(ReflectArgs),
    Policy(PolicyArgs),
    Config(ConfigArgs),
}

#[derive(Clone)]
struct ReplHelper {
    keywords: Vec<String>,
}

impl Helper for ReplHelper {}
impl Validator for ReplHelper {}
impl Highlighter for ReplHelper {}
impl Hinter for ReplHelper {
    type Hint = String;
}

impl Completer for ReplHelper {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &rustyline::Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        let safe_pos = pos.min(line.len());
        let head = &line[..safe_pos];
        let token = head.split_whitespace().next_back().unwrap_or_default();
        let start = safe_pos.saturating_sub(token.len());
        let out = self
            .keywords
            .iter()
            .filter(|k| k.starts_with(token))
            .map(|k| Pair {
                display: k.clone(),
                replacement: k.clone(),
            })
            .collect();
        Ok((start, out))
    }
}

async fn run_repl(data_dir: PathBuf) -> anyhow::Result<()> {
    let mut rl = rustyline::DefaultEditor::new()?;
    rl.set_helper(Some(ReplHelper {
        keywords: vec![
            "remember".to_string(),
            "search".to_string(),
            "branch".to_string(),
            "merge".to_string(),
            "sync".to_string(),
            "reflect".to_string(),
            "status".to_string(),
            "exit".to_string(),
            "quit".to_string(),
        ],
    }));

    let history_path = data_dir.join(".clawdb-cli-history");
    if history_path.exists() {
        let _ = rl.load_history(&history_path);
    }

    loop {
        match rl.readline("> ") {
            Ok(line) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let _ = rl.add_history_entry(trimmed);
                let mut parts = trimmed.split_whitespace();
                let cmd = parts.next().unwrap_or_default();
                let rest = parts.collect::<Vec<_>>();

                let res = match cmd {
                    "remember" => {
                        let text = rest.join(" ");
                        commands::remember::run(
                            RememberArgs {
                                positional_content: Some(text),
                                content: None,
                                memory_type: "context".to_string(),
                                tags: None,
                                metadata: None,
                                agent_id: None,
                                role: "assistant".to_string(),
                            },
                            data_dir.clone(),
                        )
                        .await
                        .map_err(anyhow::Error::from)
                    }
                    "search" => {
                        let query = rest.join(" ");
                        commands::search::run(
                            SearchArgs {
                                query,
                                top_k: 5,
                                semantic: true,
                                agent_id: None,
                                role: "assistant".to_string(),
                                filter: None,
                                show_scores: false,
                            },
                            data_dir.clone(),
                        )
                        .await
                        .map_err(anyhow::Error::from)
                    }
                    "branch" => {
                        let name = rest.first().map(|s| (*s).to_string()).unwrap_or_default();
                        commands::branch::run(
                            BranchArgs {
                                command: BranchCommand::Create {
                                    name,
                                    description: None,
                                    from_parent: "trunk".to_string(),
                                },
                                agent_id: None,
                                role: "assistant".to_string(),
                            },
                            data_dir.clone(),
                        )
                        .await
                        .map_err(anyhow::Error::from)
                    }
                    "merge" => {
                        if rest.len() < 2 {
                            Err(anyhow::anyhow!("usage: merge <source> <target>"))
                        } else {
                            commands::branch::run(
                                BranchArgs {
                                    command: BranchCommand::Merge {
                                        source: rest[0].to_string(),
                                        target: rest[1].to_string(),
                                        strategy: commands::branch::MergeStrategy::Union,
                                    },
                                    agent_id: None,
                                    role: "assistant".to_string(),
                                },
                                data_dir.clone(),
                            )
                            .await
                            .map_err(anyhow::Error::from)
                        }
                    }
                    "sync" => {
                        commands::sync::run(
                            SyncArgs {
                                push_only: false,
                                pull_only: false,
                                reconcile: false,
                                agent_id: None,
                                role: "assistant".to_string(),
                            },
                            data_dir.clone(),
                        )
                        .await
                        .map_err(anyhow::Error::from)
                    }
                    "reflect" => {
                        commands::reflect::run(
                            ReflectArgs {
                                job_type: commands::reflect::ReflectJobType::Full,
                                dry_run: false,
                                watch: false,
                                agent_id: None,
                                role: "assistant".to_string(),
                            },
                            data_dir.clone(),
                        )
                        .await
                        .map_err(anyhow::Error::from)
                    }
                    "status" => {
                        commands::status::run(StatusArgs {}, data_dir.clone())
                            .await
                            .map_err(anyhow::Error::from)
                    }
                    "exit" | "quit" => break,
                    other => Err(anyhow::anyhow!("unknown command: {other}")),
                };

                if let Err(err) = res {
                    eprintln!("error: {err}");
                }
            }
            Err(ReadlineError::Interrupted | ReadlineError::Eof) => break,
            Err(err) => return Err(anyhow::anyhow!(err.to_string())),
        }
    }

    if let Some(path) = history_path.to_str() {
        let _ = rl.save_history(path);
    }
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let data_dir = cli
        .data_dir
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".clawdb"));

    std::env::set_var("CLAW_OUTPUT_JSON", if cli.json { "1" } else { "0" });
    clawdb::telemetry::tracing_setup::init_tracing_simple(&cli.log_level, if cli.json { "json" } else { "pretty" });

    match cli.command {
        Some(Commands::Init(args)) => commands::init::run(args, data_dir).await?,
        Some(Commands::Start(args)) => commands::start::run(args, data_dir).await?,
        Some(Commands::Status(args)) => commands::status::run(args, data_dir).await?,
        Some(Commands::Remember(args)) => commands::remember::run(args, data_dir).await?,
        Some(Commands::Search(args)) => commands::search::run(args, data_dir).await?,
        Some(Commands::Branch(args)) => commands::branch::run(args, data_dir).await?,
        Some(Commands::Sync(args)) => commands::sync::run(args, data_dir).await?,
        Some(Commands::Reflect(args)) => commands::reflect::run(args, data_dir).await?,
        Some(Commands::Policy(args)) => commands::policy::run(args, data_dir).await?,
        Some(Commands::Config(args)) => commands::config::run(args, data_dir).await?,
        None => run_repl(data_dir).await?,
    }

    Ok(())
}
