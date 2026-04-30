//! `SessionContext`: the per-request security context threaded through query execution.

use uuid::Uuid;

/// Immutable security context attached to every in-flight request.
#[derive(Debug, Clone)]
pub struct SessionContext {
    /// Session identifier.
    pub session_id: Uuid,
    /// Agent identifier.
    pub agent_id: Uuid,
    /// Opaque bearer token used for guard checks.
    pub token: String,
    /// Role granted for this session.
    pub role: String,
    /// Granted permission scopes.
    pub scopes: Vec<String>,
    /// The task type driving this session (used by guard policy).
    pub task_type: String,
    /// Unix timestamp at which this session expires.
    pub expires_at: i64,
}

impl SessionContext {
    /// Creates an anonymous/system-level session context with no expiry.
    pub fn system() -> Self {
        Self {
            session_id: Uuid::nil(),
            agent_id: Uuid::nil(),
            token: "system".to_string(),
            role: "system".to_string(),
            scopes: vec!["*".to_string()],
            task_type: "system".to_string(),
            expires_at: i64::MAX,
        }
    }

    /// Returns `true` if the session has not yet expired.
    pub fn is_valid(&self) -> bool {
        let now = chrono::Utc::now().timestamp();
        self.expires_at > now
    }

    /// Returns `true` if this session holds the given scope.
    pub fn has_scope(&self, scope: &str) -> bool {
        self.scopes.iter().any(|s| s == "*" || s == scope)
    }
}
