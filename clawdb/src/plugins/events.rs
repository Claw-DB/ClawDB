//! Plugin event types.

use uuid::Uuid;

/// Events emitted by wrapper operations.
#[derive(Debug, Clone)]
pub enum ClawEvent {
    /// A memory was written.
    MemoryWritten {
        /// Memory identifier.
        memory_id: String,
        /// Workspace identifier.
        workspace_id: Uuid,
    },
    /// A search was executed.
    SearchExecuted {
        /// Search query string.
        query: String,
        /// Number of hits returned.
        hits: usize,
    },
    /// A branch was created.
    BranchCreated {
        /// New branch identifier.
        branch_id: Uuid,
        /// Branch name.
        name: String,
    },
    /// A branch was merged.
    BranchMerged {
        /// Source branch identifier.
        source: Uuid,
        /// Target branch identifier.
        target: Uuid,
        /// Number of merged records.
        merged: u32,
    },
    /// Sync completed.
    SyncCompleted {
        /// Number of pushed updates.
        pushed: u32,
        /// Number of pulled updates.
        pulled: u32,
    },
    /// Reflect completed.
    ReflectCycleRun {
        /// Number of extracted facts.
        facts_extracted: u32,
    },
    /// Policy denied.
    PolicyDenied {
        /// Agent identifier.
        agent_id: Uuid,
        /// Denied resource.
        resource: String,
        /// Denial reason.
        reason: String,
    },
    /// Session created.
    SessionCreated {
        /// Session identifier.
        session_id: Uuid,
        /// Agent identifier.
        agent_id: Uuid,
    },
}
