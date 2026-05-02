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
    pub id: String,
}
