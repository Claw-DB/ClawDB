//! `ClawEvent` enum: all system events published on the internal event bus.

use uuid::Uuid;

/// All events that can be published on the ClawDB event bus.
#[derive(Debug, Clone)]
pub enum ClawEvent {
    /// A new memory entry was stored.
    MemoryAdded {
        agent_id: Uuid,
        memory_id: Uuid,
        memory_type: String,
    },
    /// A memory entry was archived.
    MemoryArchived {
        agent_id: Uuid,
        memory_id: Uuid,
        reason: String,
    },
    /// A search query was executed.
    SearchExecuted {
        agent_id: Uuid,
        query_preview: String,
        result_count: usize,
        latency_ms: u64,
    },
    /// A new branch was created.
    BranchCreated {
        agent_id: Uuid,
        branch_id: Uuid,
        name: String,
    },
    /// Two branches were merged.
    BranchMerged {
        agent_id: Uuid,
        source: String,
        target: String,
        applied: u32,
    },
    /// A sync cycle completed.
    SyncCompleted {
        agent_id: Uuid,
        pushed: u32,
        pulled: u32,
    },
    /// A reflect job finished.
    ReflectionCompleted {
        agent_id: Uuid,
        job_id: String,
        archived: u32,
        promoted: u32,
    },
    /// A new session was created.
    SessionCreated {
        agent_id: Uuid,
        session_id: Uuid,
    },
    /// A session expired.
    SessionExpired {
        agent_id: Uuid,
        session_id: Uuid,
    },
    /// An access attempt was denied by claw-guard.
    GuardDenied {
        agent_id: Uuid,
        action: String,
        resource: String,
        reason: String,
    },
    /// A component's health status changed.
    ComponentHealthChanged {
        component: String,
        healthy: bool,
    },
    /// A plugin was successfully loaded.
    PluginLoaded {
        name: String,
        version: String,
    },
    /// A plugin was unloaded.
    PluginUnloaded {
        name: String,
    },
    /// Graceful shutdown was initiated.
    ShutdownInitiated {
        reason: String,
    },
}

impl ClawEvent {
    /// Returns a stable string tag for this event variant.
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::MemoryAdded { .. } => "memory.added",
            Self::MemoryArchived { .. } => "memory.archived",
            Self::SearchExecuted { .. } => "search.executed",
            Self::BranchCreated { .. } => "branch.created",
            Self::BranchMerged { .. } => "branch.merged",
            Self::SyncCompleted { .. } => "sync.completed",
            Self::ReflectionCompleted { .. } => "reflect.completed",
            Self::SessionCreated { .. } => "session.created",
            Self::SessionExpired { .. } => "session.expired",
            Self::GuardDenied { .. } => "guard.denied",
            Self::ComponentHealthChanged { .. } => "component.health_changed",
            Self::PluginLoaded { .. } => "plugin.loaded",
            Self::PluginUnloaded { .. } => "plugin.unloaded",
            Self::ShutdownInitiated { .. } => "shutdown.initiated",
        }
    }

    /// Returns the agent ID associated with this event, if any.
    pub fn agent_id(&self) -> Option<Uuid> {
        match self {
            Self::MemoryAdded { agent_id, .. }
            | Self::MemoryArchived { agent_id, .. }
            | Self::SearchExecuted { agent_id, .. }
            | Self::BranchCreated { agent_id, .. }
            | Self::BranchMerged { agent_id, .. }
            | Self::SyncCompleted { agent_id, .. }
            | Self::ReflectionCompleted { agent_id, .. }
            | Self::SessionCreated { agent_id, .. }
            | Self::SessionExpired { agent_id, .. }
            | Self::GuardDenied { agent_id, .. } => Some(*agent_id),
            _ => None,
        }
    }

    /// Returns `true` for events that should be forwarded to the security audit log.
    pub fn is_security_event(&self) -> bool {
        matches!(self, Self::GuardDenied { .. } | Self::SessionExpired { .. })
    }
}
