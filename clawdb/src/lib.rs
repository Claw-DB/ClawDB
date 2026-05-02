#![deny(missing_docs)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
//! The cognitive database for AI agents.
//!
//! # Quick Start
//! ```rust,no_run
//! use clawdb::prelude::*;
//! # tokio_test::block_on(async {
//! let dir = tempfile::tempdir().unwrap();
//! let db = ClawDB::open(dir.path()).await.unwrap();
//! let session = db.session(uuid::Uuid::new_v4(), "assistant",
//!     vec!["memory:write".into()]).await.unwrap();
//! db.remember(&session, "Hello from ClawDB").await.unwrap();
//! # })
//! ```

pub mod api;
pub mod config;
pub mod engine;
pub mod error;
pub mod lifecycle;
pub mod plugins;
pub mod prelude;
pub mod telemetry;
pub mod types;

pub use config::ClawDBConfig;
pub use engine::{ClawDB, ClawDBEngine, ClawDBSession};
pub use error::{ClawDBError, ClawDBResult};
pub use types::{
    BranchDiff, ClawTransaction, HealthStatus, MergeResult, ReflectSummary, RememberResult,
    SearchHit, SyncSummary,
};
