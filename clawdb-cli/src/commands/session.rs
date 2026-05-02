//! `clawdb session` — create / revoke / whoami for session tokens.

use clap::{Args, Subcommand};
use uuid::Uuid;

use crate::client::ClawDBClient;
use crate::config::save_session_token;
use crate::error::CliResult;
use crate::output::{print_success, OutputFormat};
use crate::types::SessionInfo;

#[derive(Debug, Clone, Args)]
pub struct SessionArgs {
    #[command(subcommand)]
    pub command: SessionCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub enum SessionCommand {
    /// Create a new session and print the token.
    Create(SessionCreateArgs),
    /// Revoke an existing session.
    Revoke(SessionRevokeArgs),
    /// Show info about the current session token.
    Whoami,
}

#[derive(Debug, Clone, Args)]
pub struct SessionCreateArgs {
    /// Agent UUID this session belongs to.
    #[arg(long)]
    pub agent_id: Uuid,

    /// Role for the session.
    #[arg(long, default_value = "user")]
    pub role: String,

    /// Comma-separated permission scopes.
    #[arg(long, default_value = "memory:read,memory:write")]
    pub scopes: String,

    /// Session TTL in seconds.
    #[arg(long, default_value_t = 3600)]
    pub ttl: u64,
}

#[derive(Debug, Clone, Args)]
pub struct SessionRevokeArgs {
    /// Session ID to revoke.
    pub session_id: String,
}

pub async fn execute(
    args: SessionArgs,
    client: &ClawDBClient,
    fmt: &OutputFormat,
    quiet: bool,
) -> CliResult<()> {
    match args.command {
        SessionCommand::Create(a) => {
            let scopes: Vec<String> = a.scopes.split(',').map(|s| s.trim().to_string()).collect();
            let body = serde_json::json!({
                "agent_id": a.agent_id,
                "role": a.role,
                "scopes": scopes,
                "ttl_secs": a.ttl,
            });
            let info: SessionInfo = client.post("/v1/sessions", &body).await?;

            // Offer to persist the token (only when attached to a TTY).
            if !quiet {
                use std::io::IsTerminal;
                if std::io::stdin().is_terminal() {
                    let save =
                        dialoguer::Confirm::with_theme(&dialoguer::theme::ColorfulTheme::default())
                            .with_prompt("Save token to ~/.clawdb/session.token?")
                            .default(true)
                            .interact()
                            .unwrap_or(false);
                    if save {
                        save_session_token(&info.token)?;
                        eprintln!("Token saved.");
                    }
                }
            }

            match fmt {
                OutputFormat::Json => crate::output::print_json(&info, quiet),
                _ => {
                    print_success(&format!("Session created: {}", info.session_id), fmt, quiet);
                    if !quiet {
                        println!("  Token:   {}", info.token);
                        println!("  Expires: {}", info.expires_at.as_deref().unwrap_or("N/A"));
                    }
                }
            }
        }

        SessionCommand::Revoke(a) => {
            client
                .delete(&format!("/v1/sessions/{}", a.session_id))
                .await?;
            print_success(&format!("Session {} revoked", a.session_id), fmt, quiet);
        }

        SessionCommand::Whoami => {
            let info: SessionInfo = client.get("/v1/sessions/me").await?;
            match fmt {
                OutputFormat::Json => crate::output::print_json(&info, quiet),
                _ => {
                    if !quiet {
                        println!("session_id : {}", info.session_id);
                        println!("agent_id   : {}", info.agent_id);
                        println!("role       : {}", info.role);
                        println!("scopes     : {}", info.scopes.join(", "));
                        println!(
                            "expires_at : {}",
                            info.expires_at.as_deref().unwrap_or("N/A")
                        );
                    }
                }
            }
        }
    }
    Ok(())
}
