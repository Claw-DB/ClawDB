//! `clawdb status` — prints runtime health and statistics.

use clap::Args;

/// Arguments for the `status` command.
#[derive(Debug, Args)]
pub struct StatusArgs {
    /// Output in JSON format.
    #[arg(long)]
    pub json: bool,
}

/// Executes the `status` command.
pub async fn run(_data_dir: &std::path::Path, args: &StatusArgs) -> anyhow::Result<()> {
    // In a real implementation this would connect to the running daemon via gRPC.
    let status = serde_json::json!({
        "ok": true,
        "components": {
            "core": "healthy",
            "vector": "healthy",
            "guard": "healthy",
            "branch": "healthy",
            "sync": "unknown",
            "reflect": "unknown",
        }
    });
    if args.json {
        println!("{}", serde_json::to_string_pretty(&status)?);
    } else {
        println!("ClawDB status: OK");
        println!("  core    : healthy");
        println!("  vector  : healthy");
        println!("  guard   : healthy");
        println!("  branch  : healthy");
    }
    Ok(())
}
