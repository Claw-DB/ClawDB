//! `SessionManager`: creates and validates session contexts via claw-guard.

use std::sync::Arc;
use uuid::Uuid;

use crate::{
    error::ClawDBResult,
    lifecycle::manager::ComponentLifecycleManager,
    session::{context::SessionContext, store::SessionStore},
};

/// Manages session lifecycles using the guard engine for token validation.
pub struct SessionManager {
    store: SessionStore,
    lifecycle: Arc<ComponentLifecycleManager>,
}

impl SessionManager {
    /// Creates a new `SessionManager`.
    pub fn new(lifecycle: Arc<ComponentLifecycleManager>) -> Self {
        Self {
            store: SessionStore::new(),
            lifecycle,
        }
    }

    /// Creates a new session for `agent_id` with the given role and scopes.
    pub async fn create_session(
        &self,
        agent_id: Uuid,
        role: &str,
        scopes: Vec<String>,
        task_type: &str,
    ) -> ClawDBResult<SessionContext> {
        let guard = self.lifecycle.guard()?;
        let token = format!("token-{}", Uuid::new_v4());
        let _ = guard.validate_token(&token).await?;

        let expires_at = chrono::Utc::now().timestamp() + 3600;
        let ctx = SessionContext {
            session_id: Uuid::new_v4(),
            agent_id,
            token,
            role: role.to_string(),
            scopes,
            task_type: task_type.to_string(),
            expires_at,
        };
        self.store.put(ctx.clone());
        Ok(ctx)
    }

    /// Retrieves and validates an active session by ID.
    pub fn get_session(&self, session_id: Uuid) -> ClawDBResult<SessionContext> {
        self.store.get(session_id)
    }

    /// Invalidates a session.
    pub fn invalidate(&self, session_id: Uuid) {
        self.store.remove(session_id);
    }

    /// Returns the number of active sessions.
    pub fn active_count(&self) -> usize {
        self.store.count()
    }

    /// Prunes expired sessions and returns how many were removed.
    pub fn prune_expired(&self) -> usize {
        self.store.prune_expired()
    }
}
