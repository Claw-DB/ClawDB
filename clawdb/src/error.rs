//! Error types for the `clawdb` wrapper.

use claw_guard::error::GuardError;

/// Result alias for the wrapper crate.
pub type ClawDBResult<T> = Result<T, ClawDBError>;

/// Unified error type for the wrapper.
#[derive(Debug, thiserror::Error)]
pub enum ClawDBError {
    /// Storage error from claw-core.
    #[error("storage error: {0}")]
    Core(#[from] claw_core::ClawError),
    /// Vector error from claw-vector.
    #[error("vector error: {0}")]
    Vector(#[from] claw_vector::VectorError),
    /// Branch error from claw-branch.
    #[error("branch error: {0}")]
    Branch(#[from] claw_branch::BranchError),
    /// Sync error from claw-sync.
    #[error("sync error: {0}")]
    Sync(#[from] claw_sync::SyncError),
    /// Guard error from claw-guard.
    #[error("guard error: {0}")]
    Guard(#[from] GuardError),
    /// Reflect error from claw-reflect-client.
    #[error("reflect error: {0}")]
    Reflect(#[from] claw_reflect_client::ReflectError),
    /// Permission denied.
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    /// Component disabled.
    #[error("component disabled: {0}")]
    ComponentDisabled(&'static str),
    /// Component initialization error.
    #[error("component init failed ({0}): {1}")]
    ComponentInit(&'static str, String),
    /// Configuration error.
    #[error("configuration error: {0}")]
    Config(String),
    /// Session invalid or expired.
    #[error("session expired or invalid")]
    SessionInvalid,
    /// Transaction error.
    #[error("transaction error: {0}")]
    Transaction(String),
    /// Serialization error.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    /// I/O error.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
