//! ClawDB aggregate runtime library.
//!
//! This crate wires together claw-core, claw-vector, claw-sync, claw-branch,
//! and claw-guard into a single high-level API surface.

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

pub use config::ClawDBConfig;
pub use engine::ClawDBEngine;
pub use error::{ClawDBError, ClawDBResult};
