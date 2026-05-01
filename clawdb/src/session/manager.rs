//! `SessionManager`: creates, validates, refreshes, and revokes ClawDB sessions.
//!
//! Every session is backed by a scoped token issued by `claw-guard`.  The manager
//! acts as a bridge between ClawDB's internal session model ([`SessionContext`]) and
//! the guard engine's authentication and authorisation primitives.
//!
//! # Session lifecycle
//! ```text
//! create() → guard.issue_session_token() → SessionContext → store.insert() → event
//! validate() → guard.validate_token() → store.get() → expiry check → SessionContext
//! refresh() → guard.issue_session_token() (new token) → store.insert() → SessionContext
//! revoke() → guard.revoke_session() → store.remove() → event
//! revoke_all_for_agent() → iterate + revoke each
//! ```

use std::sync::Arc;

use chrono::Utc;
use uuid::Uuid;

use crate::{
    config::ClawDBConfig,
    error::{ClawDBError, ClawDBResult},
    events::{bus::EventBus, types::ClawEvent},
    lifecycle::manager::ComponentLifecycleManager,
    session::{context::SessionContext, store::SessionStore},
};

/// Session duration granted when no explicit `expires_in` is requested (1 hour).
const DEFAULT_SESSION_SECS: i64 = 3_600;

// ── ClawDBSession ─────────────────────────────────────────────────────────────

/// A fully-materialised ClawDB session returned to callers.
///
/// Distinct from [`SessionContext`] (the internal security context) in that it
/// carries the guard token and full metadata needed by external callers.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ClawDBSession {
    /// Unique session identifier.
    pub id: Uuid,
    /// Agent this session belongs to.
    pub agent_id: Uuid,
    /// Workspace this session operates in.
    pub workspace_id: Uuid,
    /// Role granted by the guard engine.
    pub role: String,
    /// Permission scopes granted for this session.
    pub scopes: Vec<String>,
    /// Optional task type that narrows policy evaluation.
    pub task_type: Option<String>,
    /// Scoped bearer token issued by `claw-guard`.
    pub guard_token: String,
    /// Session creation timestamp.
    pub created_at: chrono::DateTime<Utc>,
    /// Session expiry timestamp.
    pub expires_at: chrono::DateTime<Utc>,
    /// Arbitrary metadata attached by the caller.
    pub metadata: serde_json::Value,
}

impl ClawDBSession {
    /// Converts this session into the internal [`SessionContext`] used for guard checks.
    pub fn as_context(&self) -> SessionContext {
        SessionContext {
            session_id: self.id,
            agent_id: self.agent_id,
            token: self.guard_token.clone(),
            role: self.role.clone(),
            scopes: self.scopes.clone(),
            task_type: self.task_type.clone().unwrap_or_default(),
            expires_at: self.expires_at.timestamp(),
        }
    }

    /// Returns `true` if the session has not yet expired.
    pub fn is_valid(&self) -> bool {
        self.expires_at > Utc::now()
    }
}

impl ClawDBSession {
    /// Reconstructs a `ClawDBSession` from an internal [`SessionContext`].
    ///
    /// Some fields (e.g. `workspace_id`, `metadata`) cannot be recovered from a
    /// `SessionContext` alone and will be set to sensible defaults.
    pub fn from_context(ctx: SessionContext) -> Self {
        let expires_at = chrono::DateTime::from_timestamp(ctx.expires_at, 0)
            .unwrap_or_else(|| Utc::now() + chrono::Duration::hours(1));
        Self {
            id: ctx.session_id,
            agent_id: ctx.agent_id,
            workspace_id: Uuid::nil(),
            role: ctx.role.clone(),
            scopes: ctx.scopes.clone(),
            task_type: if ctx.task_type.is_empty() {
                None
            } else {
                Some(ctx.task_type.clone())
            },
            guard_token: ctx.token.clone(),
            created_at: Utc::now(),
            expires_at,
            metadata: serde_json::Value::Null,
        }
    }
}

// ── SessionManager ────────────────────────────────────────────────────────────

/// Manages ClawDB session lifecycles, bridging with `claw-guard` for token issuance
/// and validation.
pub struct SessionManager {
    lifecycle: Arc<ComponentLifecycleManager>,
    store: Arc<SessionStore>,
    event_bus: Arc<EventBus>,
    config: Arc<ClawDBConfig>,
}

impl SessionManager {
    /// Creates a new `SessionManager`.
    pub fn new(
        lifecycle: Arc<ComponentLifecycleManager>,
        event_bus: Arc<EventBus>,
        config: Arc<ClawDBConfig>,
    ) -> Self {
        Self {
            lifecycle,
            store: Arc::new(SessionStore::new()),
            event_bus,
            config,
        }
    }

    /// Returns a reference to the underlying [`SessionStore`].
    pub fn store(&self) -> &Arc<SessionStore> {
        &self.store
    }

    // ── create ────────────────────────────────────────────────────────────────

    /// Creates a new session for `agent_id`, issuing a scoped token from `claw-guard`.
    ///
    /// # Steps
    /// 1. Calls `guard.issue_session_token(principal, task_type)`.
    /// 2. Constructs a [`ClawDBSession`] with the returned token.
    /// 3. Stores the session in the in-memory [`SessionStore`].
    /// 4. Emits [`ClawEvent::SessionCreated`].
    /// 5. Returns the session.
    #[tracing::instrument(skip(self), fields(agent_id = %agent_id, role = role))]
    pub async fn create(
        &self,
        agent_id: Uuid,
        role: &str,
        scopes: Vec<String>,
        task_type: Option<String>,
    ) -> ClawDBResult<ClawDBSession> {
        let guard = self.lifecycle.guard()?;
        let workspace_id = self.config.workspace_id;

        let principal = format!("agent:{agent_id}");
        let task = task_type.as_deref().unwrap_or("default");

        // Issue a scoped bearer token from claw-guard.
        let guard_token = guard
            .issue_session_token(&principal, task)
            .await
            .map_err(ClawDBError::Guard)?;

        let now = Utc::now();
        let expires_at = now + chrono::Duration::seconds(DEFAULT_SESSION_SECS);

        let session = ClawDBSession {
            id: Uuid::new_v4(),
            agent_id,
            workspace_id,
            role: role.to_string(),
            scopes: scopes.clone(),
            task_type: task_type.clone(),
            guard_token,
            created_at: now,
            expires_at,
            metadata: serde_json::Value::Null,
        };

        // Persist to in-memory store.
        let ctx = session.as_context();
        self.store.insert(ctx);

        // Emit event.
        self.event_bus.emit(ClawEvent::SessionCreated {
            agent_id,
            session_id: session.id,
        });

        tracing::info!(session_id = %session.id, agent_id = %agent_id, role, "session created");
        Ok(session)
    }

    // ── validate ──────────────────────────────────────────────────────────────

    /// Validates a bearer token and returns the associated [`ClawDBSession`].
    ///
    /// # Steps
    /// 1. Calls `guard.validate_token(token)` to obtain [`GuardClaims`].
    /// 2. Looks up the session in the store by `session_id` from the claims.
    /// 3. Checks that the session has not expired.
    #[tracing::instrument(skip(self, guard_token))]
    pub async fn validate(&self, guard_token: &str) -> ClawDBResult<SessionContext> {
        let guard = self.lifecycle.guard()?;

        // Validate with claw-guard; this checks signature and expiry.
        let claims = guard
            .validate_token(guard_token)
            .await
            .map_err(ClawDBError::Guard)?;

        // Resolve session from local store.
        let session_id = claims.session_id();
        let ctx = self.store.get(session_id)?;

        Ok(ctx)
    }

    // ── refresh ───────────────────────────────────────────────────────────────

    /// Issues a new guard token for `session_id` and extends its expiry.
    #[tracing::instrument(skip(self), fields(session_id = %session_id))]
    pub async fn refresh(&self, session_id: Uuid) -> ClawDBResult<SessionContext> {
        let existing = self.store.get(session_id)?;
        let guard = self.lifecycle.guard()?;

        let principal = format!("agent:{}", existing.agent_id);
        let new_token = guard
            .issue_session_token(&principal, &existing.task_type)
            .await
            .map_err(ClawDBError::Guard)?;

        let new_expiry =
            Utc::now().timestamp() + DEFAULT_SESSION_SECS;

        let refreshed = SessionContext {
            token: new_token,
            expires_at: new_expiry,
            ..existing
        };

        self.store.insert(refreshed.clone());
        tracing::debug!(session_id = %session_id, "session refreshed");
        Ok(refreshed)
    }

    // ── revoke ────────────────────────────────────────────────────────────────

    /// Revokes a single session, invalidating its guard token.
    #[tracing::instrument(skip(self), fields(session_id = %session_id))]
    pub async fn revoke(&self, session_id: Uuid) -> ClawDBResult<()> {
        let ctx = self
            .store
            .remove(session_id)
            .ok_or(ClawDBError::SessionNotFound(session_id))?;

        if let Ok(guard) = self.lifecycle.guard() {
            if let Err(e) = guard.revoke_session(session_id).await {
                tracing::warn!(session_id = %session_id, "guard revoke_session failed: {e}");
            }
        }

        self.event_bus.emit(ClawEvent::SessionExpired {
            agent_id: ctx.agent_id,
            session_id,
        });

        tracing::info!(session_id = %session_id, "session revoked");
        Ok(())
    }

    // ── revoke_all_for_agent ─────────────────────────────────────────────────

    /// Revokes all sessions belonging to `agent_id`.
    ///
    /// Returns the number of sessions revoked.
    pub async fn revoke_all_for_agent(&self, agent_id: Uuid) -> ClawDBResult<u32> {
        let ids = self.store.ids_for_agent(agent_id);
        let mut count = 0u32;
        for session_id in ids {
            if let Err(e) = self.revoke(session_id).await {
                tracing::warn!(session_id = %session_id, "revoke failed: {e}");
            } else {
                count += 1;
            }
        }
        Ok(count)
    }

    // ── legacy API ────────────────────────────────────────────────────────────

    /// Legacy: creates a session using the old call signature.
    ///
    /// Prefer [`create`] for new code.
    pub async fn create_session(
        &self,
        agent_id: Uuid,
        role: &str,
        scopes: Vec<String>,
        task_type: &str,
    ) -> ClawDBResult<SessionContext> {
        let session = self
            .create(agent_id, role, scopes, Some(task_type.to_string()))
            .await?;
        Ok(session.as_context())
    }

    /// Legacy: retrieves a session by ID.
    pub fn get_session(&self, session_id: Uuid) -> ClawDBResult<SessionContext> {
        self.store.get(session_id)
    }

    /// Legacy: invalidates a session without guard revocation.
    pub fn invalidate(&self, session_id: Uuid) {
        self.store.remove(session_id);
    }
}

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
