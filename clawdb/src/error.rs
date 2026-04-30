//! Unified ClawDB error type that wraps all subsystem errors plus aggregate-layer errors.

use thiserror::Error;
use uuid::Uuid;

/// Unified error type for the ClawDB runtime.
#[derive(Debug, Error)]
pub enum ClawDBError {
    /// Storage engine errors from claw-core.
    #[error("core error: {0}")]
    Core(#[from] claw_core::ClawError),

    /// Semantic memory errors from claw-vector.
    #[error("vector error: {0}")]
    Vector(#[from] claw_vector::VectorError),

    /// Sync engine errors from claw-sync.
    #[error("sync error: {0}")]
    Sync(#[from] claw_sync::SyncError),

    /// Fork/merge errors from claw-branch.
    #[error("branch error: {0}")]
    Branch(#[from] claw_branch::BranchError),

    /// Security/policy errors from claw-guard.
    #[error("guard error: {0}")]
    Guard(#[from] claw_guard::GuardError),

    /// Reflect service errors (HTTP/gRPC).
    #[error("reflect error: {0}")]
    Reflect(String),

    /// Configuration validation errors.
    #[error("config error: {0}")]
    Config(String),

    /// A subsystem is not yet initialised.
    #[error("component not ready: {0}")]
    ComponentNotReady(String),

    /// A subsystem failed during operation.
    #[error("component '{component}' failed: {reason}")]
    ComponentFailed {
        /// The component name.
        component: String,
        /// The failure reason.
        reason: String,
    },

    /// Session not found.
    #[error("session not found: {0}")]
    SessionNotFound(Uuid),

    /// Session has expired.
    #[error("session expired: {0}")]
    SessionExpired(Uuid),

    /// Query planning failed.
    #[error("query plan failed for '{query}': {reason}")]
    QueryPlanFailed {
        /// The original query string.
        query: String,
        /// The planning failure reason.
        reason: String,
    },

    /// Query execution failed.
    #[error("query execution failed at step '{step}': {reason}")]
    QueryExecutionFailed {
        /// The step name.
        step: String,
        /// The execution failure reason.
        reason: String,
    },

    /// Transaction failed.
    #[error("transaction {tx_id} failed: {reason}")]
    TransactionFailed {
        /// The transaction ID.
        tx_id: Uuid,
        /// The failure reason.
        reason: String,
    },

    /// Transaction conflict detected.
    #[error("transaction {tx_id} conflicts with {conflicting_tx}")]
    TransactionConflict {
        /// The transaction ID.
        tx_id: Uuid,
        /// The conflicting transaction ID.
        conflicting_tx: Uuid,
    },

    /// Plugin failed to load.
    #[error("plugin '{name}' failed to load: {reason}")]
    PluginLoad {
        /// The plugin name.
        name: String,
        /// The load failure reason.
        reason: String,
    },

    /// Plugin execution error.
    #[error("plugin '{name}' hook '{hook}' failed: {reason}")]
    PluginExecution {
        /// The plugin name.
        name: String,
        /// The hook name.
        hook: String,
        /// The execution failure reason.
        reason: String,
    },

    /// Plugin capability denied.
    #[error("plugin '{plugin}' denied capability '{capability}'")]
    PluginCapabilityDenied {
        /// The plugin name.
        plugin: String,
        /// The requested capability.
        capability: String,
    },

    /// Event bus error.
    #[error("event bus error: {0}")]
    EventBusError(String),

    /// JSON serialisation error.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// I/O error.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// gRPC transport error.
    #[error("transport error: {0}")]
    Transport(#[from] tonic::Status),

    /// Shutdown error.
    #[error("shutdown error: {0}")]
    Shutdown(String),
}

impl ClawDBError {
    /// Returns the component name that produced this error.
    pub fn component(&self) -> &'static str {
        match self {
            Self::Core(_) => "core",
            Self::Vector(_) => "vector",
            Self::Sync(_) => "sync",
            Self::Branch(_) => "branch",
            Self::Guard(_) => "guard",
            Self::Reflect(_) => "reflect",
            Self::Config(_) | Self::ComponentNotReady(_) | Self::ComponentFailed { .. } => "runtime",
            Self::SessionNotFound(_) | Self::SessionExpired(_) => "session",
            Self::QueryPlanFailed { .. } | Self::QueryExecutionFailed { .. } => "query",
            Self::TransactionFailed { .. } | Self::TransactionConflict { .. } => "transaction",
            Self::PluginLoad { .. } | Self::PluginExecution { .. } | Self::PluginCapabilityDenied { .. } => "plugin",
            Self::EventBusError(_) => "event",
            Self::Serialization(_) => "runtime",
            Self::Io(_) => "runtime",
            Self::Transport(_) => "transport",
            Self::Shutdown(_) => "runtime",
        }
    }

    /// Returns `true` if the error is transient and the operation may be retried.
    pub fn is_transient(&self) -> bool {
        matches!(self, Self::Transport(_) | Self::ComponentNotReady(_))
    }

    /// Returns `true` if the error is an authentication or authorisation failure.
    pub fn is_auth(&self) -> bool {
        if let Self::Guard(e) = self {
            return e.is_auth_error();
        }
        false
    }

    /// Maps this error to an HTTP status code for the REST API.
    pub fn http_status_code(&self) -> u16 {
        match self {
            Self::Guard(_) => 403,
            Self::SessionNotFound(_) | Self::SessionExpired(_) => 401,
            Self::QueryPlanFailed { .. } | Self::QueryExecutionFailed { .. } => 400,
            Self::ComponentNotReady(_) => 503,
            Self::Io(_) | Self::ComponentFailed { .. } => 500,
            Self::Serialization(_) | Self::Config(_) => 400,
            Self::Transport(_) => 502,
            _ => 500,
        }
    }
}

impl From<ClawDBError> for tonic::Status {
    fn from(err: ClawDBError) -> Self {
        match &err {
            ClawDBError::Guard(_) => tonic::Status::permission_denied(err.to_string()),
            ClawDBError::SessionNotFound(_) | ClawDBError::SessionExpired(_) => {
                tonic::Status::unauthenticated(err.to_string())
            }
            ClawDBError::QueryPlanFailed { .. }
            | ClawDBError::QueryExecutionFailed { .. }
            | ClawDBError::Config(_) => tonic::Status::invalid_argument(err.to_string()),
            ClawDBError::ComponentNotReady(_) => tonic::Status::unavailable(err.to_string()),
            ClawDBError::TransactionConflict { .. } => tonic::Status::aborted(err.to_string()),
            ClawDBError::Transport(status) => status.clone(),
            _ => tonic::Status::internal(err.to_string()),
        }
    }
}

/// Convenience alias for a `Result` with `ClawDBError`.
pub type ClawDBResult<T> = Result<T, ClawDBError>;
