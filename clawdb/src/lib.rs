//! # ClawDB
//!
//! ClawDB is a production-grade AI-native database runtime that wires together
//! five specialised sub-engines:
//!
//! | Sub-engine     | Purpose                                   |
//! |:-------------- |:----------------------------------------- |
//! | `claw-core`    | SQLite-backed persistent memory storage   |
//! | `claw-vector`  | HNSW semantic index + embedding service   |
//! | `claw-branch`  | Copy-on-write snapshot / branch engine    |
//! | `claw-sync`    | Peer-to-peer replication                  |
//! | `claw-guard`   | JWT auth + OPA policy enforcement         |
//!
//! ## Quick start
//!
//! ```rust,no_run
//! use clawdb::prelude::*;
//!
//! #[tokio::main]
//! async fn main() -> ClawDBResult<()> {
//!     let db = ClawDB::open_default().await?;
//!
//!     let session = db
//!         .session(agent_id, "writer", vec!["memory:write".into()])
//!         .await?;
//!
//!     let result = db.remember(&session, "The sky is blue").await?;
//!     println!("stored: {}", result.memory_id);
//!
//!     let hits = db.search(&session, "sky colour").await?;
//!     println!("found {} memories", hits.len());
//!
//!     db.close().await
//! }
//! ```

// ── Module tree ───────────────────────────────────────────────────────────────

pub mod api;
pub mod config;
pub mod engine;
pub mod error;
pub mod events;
pub mod lifecycle;
pub mod plugins;
pub mod query;
pub mod session;
pub mod telemetry;
pub mod transaction;

// ── Primary re-exports ────────────────────────────────────────────────────────

// Engine
pub use engine::{ClawDB, ClawDBEngine, RememberResult};

// Config
pub use config::{ClawDBConfig, ServerSubConfig, TelemetrySubConfig};

// Errors
pub use error::{ClawDBError, ClawDBResult};

// Session
pub use session::{
	context::SessionContext,
	manager::{ClawDBSession, SessionManager},
};

// Events
pub use events::{
	bus::EventBus,
	emitter::EventEmitter,
	subscriber::EventSubscriber,
	types::ClawEvent,
};

// Lifecycle / health
pub use lifecycle::{
	health::{ComponentHealth, HealthReport, HealthStatus},
	manager::ComponentLifecycleManager,
};

// Query
pub use query::types::{Query, QueryResult};

// Plugins
pub use plugins::{
	ClawPlugin, PluginCapability, PluginContext, PluginManifest,
	PluginRegistry, PluginSandbox,
};

// Telemetry
pub use telemetry::{Metrics, Telemetry};

// ── Prelude ───────────────────────────────────────────────────────────────────

/// Convenience re-exports for typical ClawDB usage.
///
/// ```rust
/// use clawdb::prelude::*;
/// ```
pub mod prelude {
	pub use crate::{
		ClawDB,
		ClawDBConfig,
		ClawDBError,
		ClawDBResult,
		ClawDBSession,
		SessionContext,
		ClawEvent,
		HealthReport,
		HealthStatus,
		Query,
		QueryResult,
		RememberResult,
	};
}
