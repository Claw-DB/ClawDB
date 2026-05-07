use serde::{Deserialize, Serialize};

/// Generic API error body returned by the server.
#[derive(Debug, Deserialize, Serialize, Default, Clone)]
pub struct ApiErrorBody {
    #[serde(default)]
    pub error: String,
    #[serde(default)]
    pub detail: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
}

/// A stored memory record.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MemoryRecord {
    pub id: String,
    pub content: String,
    #[serde(default)]
    pub memory_type: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub score: Option<f64>,
    pub created_at: Option<String>,
}

/// A single search result hit.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SearchHit {
    pub id: String,
    pub content: String,
    #[serde(default)]
    pub score: f64,
    #[serde(default)]
    pub tags: Vec<String>,
    pub memory_type: Option<String>,
}

/// Server health response.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct HealthResponse {
    pub ok: bool,
    #[serde(default)]
    pub components: std::collections::HashMap<String, serde_json::Value>,
    pub uptime_secs: Option<f64>,
}

/// Session info returned by the server.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SessionInfo {
    #[serde(default, alias = "id")]
    pub session_id: String,
    pub token: String,
    #[serde(default)]
    pub agent_id: String,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub scopes: Vec<String>,
    pub expires_at: Option<String>,
}

/// A branch record.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BranchRecord {
    #[serde(alias = "branch_id")]
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub status: String,
    pub created_at: Option<String>,
    pub parent_id: Option<String>,
}

/// Merge result.
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct MergeResult {
    #[serde(default)]
    pub merged: u64,
    #[serde(default)]
    pub conflicts: u64,
}

/// Branch diff result.
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct DiffResult {
    #[serde(default)]
    pub added: u64,
    #[serde(default)]
    pub modified: u64,
    #[serde(default)]
    pub removed: u64,
}

/// Sync result.
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct SyncResult {
    #[serde(default)]
    pub pushed: u64,
    #[serde(default)]
    pub pulled: u64,
    #[serde(default)]
    pub conflicts: u64,
}

/// Reflect job status.
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct ReflectJob {
    #[serde(default)]
    pub job_id: String,
    #[serde(default)]
    pub status: String,
    pub memories_processed: Option<u64>,
    pub summaries_created: Option<u64>,
}

/// A policy record.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PolicyRecord {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub effect: String,
    pub created_at: Option<String>,
}

/// Result of a policy test.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PolicyTestResult {
    pub allowed: bool,
    pub reason: Option<String>,
}

/// Response body from POST /v1/memories.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CreateMemoryResponse {
    #[serde(alias = "memory_id")]
    pub id: String,
}

/// Response from GET /v1/sessions/active/count.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ActiveSessionCountResponse {
    pub count: u64,
}

/// Response from POST /v1/tx (begin transaction).
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TxBeginResponse {
    pub tx_id: String,
}

/// Response from POST /v1/tx/:id/memories or /typed.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TxStagedResponse {
    #[serde(default)]
    pub staged: bool,
    /// Present when the memory is committed inline (tx_remember_typed returns a memory_id).
    pub memory_id: Option<String>,
}

/// Response from POST /v1/tx/:id/commit.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TxCommitResponse {
    pub committed: bool,
    #[serde(default)]
    pub count: u64,
}

/// Response from POST /v1/tx/:id/rollback.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TxRollbackResponse {
    #[serde(default)]
    pub rolled_back: bool,
}

/// Status returned by GET /v1/sync/status.
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct SyncStatusResponse {
    #[serde(default)]
    pub connected: bool,
    pub last_sync_at: Option<String>,
    pub last_error: Option<String>,
    #[serde(default)]
    pub pending_push: u64,
    #[serde(default)]
    pub pending_pull: u64,
}

/// Summarised push or pull result returned by /v1/sync/push and /v1/sync/pull.
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct SyncActionResult {
    #[serde(default)]
    pub deltas_sent: u64,
    #[serde(default)]
    pub deltas_received: u64,
    #[serde(default)]
    pub ops_applied: u64,
    #[serde(default)]
    pub ops_skipped: u64,
    #[serde(default)]
    pub duration_ms: u64,
    /// Raw JSON summary returned by the server (schema may vary).
    pub summary_json: Option<String>,
}

/// A single extracted fact from the reflect service.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ExtractedFact {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub agent_id: String,
    #[serde(default)]
    pub fact: String,
    #[serde(default)]
    pub confidence: f64,
    pub created_at: Option<String>,
}

/// Detailed reflect job record.
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct ReflectJobDetail {
    #[serde(default)]
    pub job_id: String,
    #[serde(default)]
    pub status: String,
    pub memories_processed: Option<u64>,
    pub summaries_created: Option<u64>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

/// A user preference extracted by the reflect service.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Preference {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub agent_id: String,
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub value: serde_json::Value,
    pub updated_at: Option<String>,
}

/// A contradiction detected by the reflect service.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Contradiction {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub agent_id: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub status: String,
    pub created_at: Option<String>,
}
