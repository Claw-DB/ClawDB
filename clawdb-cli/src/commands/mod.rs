//! CLI subcommand modules — one module per `clawdb` command.

pub mod branch;
pub mod config;
pub mod init;
pub mod policy;
pub mod reflect;
pub mod remember;
pub mod search;
pub mod start;
pub mod status;
pub mod sync;

use std::path::Path;

pub(crate) fn output_json() -> bool {
	std::env::var("CLAW_OUTPUT_JSON")
		.ok()
		.map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
		.unwrap_or(false)
}

pub(crate) fn load_config(data_dir: &Path) -> clawdb::ClawDBResult<clawdb::ClawDBConfig> {
	clawdb::ClawDBConfig::load_or_default(data_dir)
}
