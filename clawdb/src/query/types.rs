//! Core query domain types: `Query`, `QueryPlan`, `QueryResult`, and related enums.

use uuid::Uuid;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A high-level query directed at one or more ClawDB subsystems.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Query {
    /// Store a new memory entry.
    Remember {
        agent_id: Uuid,
        content: String,
        memory_type: String,
        metadata: Value,
        tags: Vec<String>,
    },
    /// Search for memories.
    Search {
        agent_id: Uuid,
        text: String,
        semantic: bool,
        top_k: usize,
        filter: Option<Value>,
        alpha: f32,
    },
    /// Retrieve specific memories by ID.
    Recall {
        agent_id: Uuid,
        memory_ids: Vec<Uuid>,
    },
    /// Create a new branch.
    Branch {
        agent_id: Uuid,
        parent: String,
        name: String,
        description: Option<String>,
    },
    /// Merge two branches.
    Merge {
        agent_id: Uuid,
        source: String,
        target: String,
        strategy: String,
    },
    /// Compute the diff between two branches.
    Diff {
        agent_id: Uuid,
        branch_a: String,
        branch_b: String,
    },
    /// Trigger a sync cycle.
    Sync {
        agent_id: Uuid,
    },
    /// Trigger a reflection job.
    Reflect {
        agent_id: Uuid,
        job_type: String,
        dry_run: bool,
    },
    /// Execute a raw SQL query.
    Raw {
        sql: String,
        params: Vec<Value>,
        entity_type: String,
    },
}

impl Query {
    /// Returns the primary subsystem component this query targets.
    pub fn target_component(&self) -> &'static str {
        match self {
            Self::Remember { .. } | Self::Recall { .. } => "core",
            Self::Search { semantic: true, .. } => "vector",
            Self::Search { semantic: false, .. } => "core",
            Self::Branch { .. } | Self::Merge { .. } | Self::Diff { .. } => "branch",
            Self::Sync { .. } => "sync",
            Self::Reflect { .. } => "reflect",
            Self::Raw { .. } => "core",
        }
    }

    /// Returns `true` if this query mutates state.
    pub fn is_write(&self) -> bool {
        matches!(
            self,
            Self::Remember { .. }
                | Self::Branch { .. }
                | Self::Merge { .. }
                | Self::Sync { .. }
                | Self::Reflect { .. }
                | Self::Raw { .. }
        )
    }

    /// Returns the agent ID associated with this query, or a nil UUID for raw queries.
    pub fn agent_id(&self) -> Uuid {
        match self {
            Self::Remember { agent_id, .. }
            | Self::Search { agent_id, .. }
            | Self::Recall { agent_id, .. }
            | Self::Branch { agent_id, .. }
            | Self::Merge { agent_id, .. }
            | Self::Diff { agent_id, .. }
            | Self::Sync { agent_id, .. }
            | Self::Reflect { agent_id, .. } => *agent_id,
            Self::Raw { .. } => Uuid::nil(),
        }
    }
}

/// An ordered, annotated execution plan produced by the `MemoryPlanner`.
#[derive(Debug, Clone)]
pub struct QueryPlan {
    pub id: Uuid,
    pub query: Query,
    pub steps: Vec<QueryStep>,
    pub estimated_ms: Option<u64>,
    pub guard_applied: bool,
}

/// A single step within a `QueryPlan`.
#[derive(Debug, Clone)]
pub enum QueryStep {
    /// Read from the core SQLite engine.
    CoreRead { sql: String },
    /// Write to the core SQLite engine.
    CoreWrite { sql: String },
    /// Search the vector index.
    VectorSearch { collection: String, top_k: usize },
    /// Upsert into the vector index.
    VectorUpsert { collection: String },
    /// Push local changes to the sync hub.
    SyncPush,
    /// Pull remote changes from the sync hub.
    SyncPull,
    /// Execute a branch operation.
    BranchOp { branch_name: String },
    /// Submit a reflect job.
    ReflectJob { job_type: String },
    /// Evaluate an access policy.
    GuardCheck { action: String },
    /// Execute child steps in parallel.
    Parallel(Vec<QueryStep>),
    /// Execute child steps in sequence.
    Sequential(Vec<QueryStep>),
}

/// The result of executing a `QueryPlan`.
#[derive(Debug, Clone)]
pub struct QueryResult {
    pub query_id: Uuid,
    pub data: QueryResultData,
    pub latency_ms: u64,
    pub guard_applied: bool,
    pub from_cache: bool,
}

/// Payload variants for a `QueryResult`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QueryResultData {
    /// A list of raw memory entry JSON objects.
    MemoryEntries(Vec<Value>),
    /// A ranked list of search result JSON objects.
    SearchResults(Vec<Value>),
    /// Branch metadata.
    BranchInfo(Value),
    /// Sync statistics.
    SyncResult(Value),
    /// Reflect job outcome.
    ReflectResult(Value),
    /// No meaningful payload (e.g. write-only operations).
    Unit,
    /// An error occurred during execution.
    Error(String),
}
