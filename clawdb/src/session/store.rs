//! `SessionStore`: in-memory concurrent map of active sessions.

use std::sync::Arc;
use dashmap::DashMap;
use uuid::Uuid;

use crate::{
    error::{ClawDBError, ClawDBResult},
    session::context::SessionContext,
};

/// Thread-safe store of active `SessionContext` values.
#[derive(Debug, Clone)]
pub struct SessionStore {
    sessions: Arc<DashMap<Uuid, SessionContext>>,
}

impl SessionStore {
    /// Creates an empty `SessionStore`.
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(DashMap::new()),
        }
    }

    /// Inserts or replaces a session.
    pub fn put(&self, ctx: SessionContext) {
        self.sessions.insert(ctx.session_id, ctx);
    }

    /// Retrieves a session by ID, returning an error if not found or expired.
    pub fn get(&self, session_id: Uuid) -> ClawDBResult<SessionContext> {
        let ctx = self
            .sessions
            .get(&session_id)
            .ok_or(ClawDBError::SessionNotFound(session_id))?
            .clone();
        if !ctx.is_valid() {
            return Err(ClawDBError::SessionExpired(session_id));
        }
        Ok(ctx)
    }

    /// Removes a session from the store.
    pub fn remove(&self, session_id: Uuid) {
        self.sessions.remove(&session_id);
    }

    /// Returns the number of active sessions.
    pub fn count(&self) -> usize {
        self.sessions.len()
    }

    /// Removes all expired sessions and returns how many were pruned.
    pub fn prune_expired(&self) -> usize {
        let now = chrono::Utc::now().timestamp();
        let before = self.sessions.len();
        self.sessions.retain(|_, ctx| ctx.expires_at > now);
        before - self.sessions.len()
    }
}

impl Default for SessionStore {
    fn default() -> Self {
        Self::new()
    }
}
