//! Public wrapper types.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Re-exported memory record type from claw-core.
pub type MemoryRecord = claw_core::MemoryRecord;
/// Re-exported merge result type from claw-branch.
pub type MergeResult = claw_branch::MergeResult;
/// Re-exported branch diff type from claw-branch.
pub type BranchDiff = claw_branch::DiffResult;

/// Result returned after storing a memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RememberResult {
    /// Newly created memory identifier.
    pub memory_id: Uuid,
    /// Whether semantic indexing succeeded.
    pub indexed: bool,
}

/// A normalized search hit returned by the wrapper.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    /// Memory identifier.
    pub id: Uuid,
    /// Search score if available.
    pub score: f32,
    /// Memory content.
    pub content: String,
    /// Memory type.
    pub memory_type: String,
    /// Memory tags.
    pub tags: Vec<String>,
    /// Search metadata.
    pub metadata: serde_json::Value,
}

/// Summary of a sync round.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncSummary {
    /// Number of pushed delta sets.
    pub pushed: u32,
    /// Number of pulled delta sets.
    pub pulled: u32,
    /// Number of conflicts.
    pub conflicts: u32,
    /// Total duration in milliseconds.
    pub duration_ms: u64,
}

/// Summary of a reflect run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReflectSummary {
    /// Reflect job identifier when one was created.
    pub job_id: Option<String>,
    /// Returned job status.
    pub status: String,
    /// Human-readable message.
    pub message: String,
    /// Whether reflection was skipped.
    pub skipped: bool,
}

impl ReflectSummary {
    /// Returns a skipped reflect summary.
    pub fn skipped() -> Self {
        Self {
            job_id: None,
            status: "skipped".to_string(),
            message: "reflect client not configured".to_string(),
            skipped: true,
        }
    }
}

/// Aggregate wrapper health status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    /// True when all components report healthy.
    pub ok: bool,
    /// Per-component health booleans.
    pub components: HashMap<String, bool>,
}

/// Wrapper around a core transaction plus deferred vector work.
pub struct ClawTransaction<'a> {
    pub(crate) inner: claw_core::ClawTransaction<'a>,
    pub(crate) vector: Option<std::sync::Arc<claw_vector::VectorEngine>>,
    pub(crate) workspace_id: String,
    pub(crate) pending_vector_upserts: Vec<(String, serde_json::Value)>,
}
