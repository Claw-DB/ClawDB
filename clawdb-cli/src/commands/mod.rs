//! CLI subcommand modules — one module per `clawdb` subcommand.
//! The CLI is a pure HTTP client; it does not link against the clawdb library.

pub mod branch;
pub mod completion;
pub mod config;
pub mod init;
pub mod policy;
pub mod recall;
pub mod reflect;
pub mod remember;
pub mod search;
pub mod session;
pub mod start;
pub mod status;
pub mod sync;
