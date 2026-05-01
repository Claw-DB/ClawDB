//! `SessionStore`: thread-safe in-memory session storage with optional persistence.
//!
//! The store uses a [`DashMap`] for concurrent in-memory access.  A SQLite-backed
//! persistence layer can be added by calling [`SessionStore::persist_to`] and
//! [`SessionStore::restore_from`] during shutdown/startup respectively.

use std::sync::Arc;

use chrono::Utc;
use dashmap::DashMap;
use uuid::Uuid;

use crate::{
    error::{ClawDBError, ClawDBResult},
    session::context::SessionContext,
};

/// Thread-safe store of active [`SessionContext`] values.
///
/// # In-memory layout
/// All sessions are kept in a single `DashMap<Uuid, SessionContext>` keyed by
/// `session_id`.  An additional index `by_agent` maps `agent_id → Vec<session_id>`
/// for efficient per-agent lookups.
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

    // ── CRUD ──────────────────────────────────────────────────────────────────

    /// Inserts or replaces a session.
    pub fn insert(&self, ctx: SessionContext) {
        self.sessions.insert(ctx.session_id, ctx);
    }

    /// Alias for [`insert`] (kept for backward compatibility).
    #[inline]
    pub fn put(&self, ctx: SessionContext) {
        self.insert(ctx);
    }

    /// Retrieves a live session by ID.
    ///
    /// # Errors
    /// - [`ClawDBError::SessionNotFound`] – session does not exist.
    /// - [`ClawDBError::SessionExpired`] – session has passed its `expires_at`.
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

    /// Removes and returns the session for `session_id`, if it exists.
    ///
    /// Does not error if the session was already missing.
    pub fn remove(&self, session_id: Uuid) -> Option<SessionContext> {
        self.sessions.remove(&session_id).map(|(_, v)| v)
    }

    // ── Agent queries ─────────────────────────────────────────────────────────

    /// Returns all live (non-expired) sessions for `agent_id`.
    pub fn list_for_agent(&self, agent_id: Uuid) -> Vec<SessionContext> {
        let now = Utc::now().timestamp();
        self.sessions
            .iter()
            .filter(|e| e.agent_id == agent_id && e.expires_at > now)
            .map(|e| e.clone())
            .collect()
    }

    /// Returns all session IDs that belong to `agent_id` (live or expired).
    pub fn ids_for_agent(&self, agent_id: Uuid) -> Vec<Uuid> {
        self.sessions
            .iter()
            .filter(|e| e.agent_id == agent_id)
            .map(|e| e.session_id)
            .collect()
    }

    // ── Maintenance ───────────────────────────────────────────────────────────

    /// Evicts all sessions whose `expires_at` is in the past.
    ///
    /// Returns the number of sessions removed.
    pub fn purge_expired(&self) -> usize {
        let now = Utc::now().timestamp();
        let before = self.sessions.len();
        self.sessions.retain(|_, ctx| ctx.expires_at > now);
        before - self.sessions.len()
    }

    /// Returns the total number of sessions (including expired ones not yet purged).
    pub fn count(&self) -> usize {
        self.sessions.len()
    }

    /// Returns the number of live (non-expired) sessions.
    pub fn live_count(&self) -> usize {
        let now = Utc::now().timestamp();
        self.sessions.iter().filter(|e| e.expires_at > now).count()
    }

    // ── Optional persistence ──────────────────────────────────────────────────

    /// Serialises all live sessions to a JSON file at `path`.
    ///
    /// Called during graceful shutdown so sessions survive restarts.
    pub fn persist_to(&self, path: &std::path::Path) -> ClawDBResult<()> {
        let now = Utc::now().timestamp();
        let live: Vec<&SessionContext> = self
            .sessions
            .iter()
            .filter(|e| e.expires_at > now)
            .map(|e| {
                // SAFETY: we just need a reference inside the closure scope;
                // we collect immediately so the DashMap ref is not held across await.
                unsafe { &*(e.value() as *const SessionContext) }
            })
            .collect();
        let json = serde_json::to_vec(&live)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Restores sessions from a JSON file previously written by [`persist_to`].
    ///
    /// Expired sessions read from disk are silently discarded.
    pub fn restore_from(&self, path: &std::path::Path) -> ClawDBResult<usize> {
        if !path.exists() {
            return Ok(0);
        }
        let data = std::fs::read(path)?;
        let sessions: Vec<SessionContext> = serde_json::from_slice(&data)?;
        let now = Utc::now().timestamp();
        let mut loaded = 0usize;
        for ctx in sessions {
            if ctx.expires_at > now {
                self.sessions.insert(ctx.session_id, ctx);
                loaded += 1;
            }
        }
        Ok(loaded)
    }
}

impl Default for SessionStore {
    fn default() -> Self {
        Self::new()
    }
}

}

impl Default for SessionStore {
    fn default() -> Self {
        Self::new()
    }
}
