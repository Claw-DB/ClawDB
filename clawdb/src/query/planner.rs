//! `MemoryPlanner`: decomposes `Query` values into ordered `QueryPlan` step sequences.

use std::sync::Arc;
use uuid::Uuid;

use crate::{
    config::ClawDBConfig,
    error::ClawDBResult,
    query::types::{Query, QueryPlan, QueryStep},
    session::context::SessionContext,
};

/// Transforms a `Query` into an optimisable `QueryPlan`.
pub struct MemoryPlanner {
    #[allow(dead_code)]
    config: Arc<ClawDBConfig>,
}

impl MemoryPlanner {
    /// Creates a new `MemoryPlanner` with the given config.
    pub fn new(config: Arc<ClawDBConfig>) -> Self {
        Self { config }
    }

    /// Plans the execution of `query` for the given `session`.
    pub fn plan(&self, query: &Query, _session: &SessionContext) -> ClawDBResult<QueryPlan> {
        let (steps, estimated_ms) = match query {
            Query::Remember { .. } => (self.plan_remember(), Some(10)),
            Query::Search { semantic: true, top_k, .. } => {
                (self.plan_semantic_search(*top_k), Some(30))
            }
            Query::Search { semantic: false, .. } => (self.plan_keyword_search(), Some(15)),
            Query::Recall { .. } => (self.plan_recall(), Some(5)),
            Query::Branch { .. } => (self.plan_branch_fork(), Some(100)),
            Query::Merge { target, .. } => (self.plan_merge(target), Some(100)),
            Query::Diff { .. } => (self.plan_diff(), Some(20)),
            Query::Sync { .. } => (self.plan_sync(), None),
            Query::Reflect { job_type, .. } => (self.plan_reflect(job_type), None),
            Query::Raw { sql, .. } => (
                vec![
                    QueryStep::GuardCheck { action: "raw_sql".to_string() },
                    QueryStep::CoreRead { sql: sql.clone() },
                ],
                Some(10),
            ),
        };

        Ok(QueryPlan {
            id: Uuid::new_v4(),
            query: query.clone(),
            steps,
            estimated_ms,
            guard_applied: true,
        })
    }

    fn plan_remember(&self) -> Vec<QueryStep> {
        vec![
            QueryStep::GuardCheck { action: "memory.write".to_string() },
            QueryStep::Parallel(vec![
                QueryStep::CoreWrite { sql: "INSERT INTO memory_records ...".to_string() },
                QueryStep::VectorUpsert { collection: "memories".to_string() },
            ]),
        ]
    }

    fn plan_semantic_search(&self, top_k: usize) -> Vec<QueryStep> {
        vec![
            QueryStep::GuardCheck { action: "memory.read".to_string() },
            QueryStep::VectorSearch { collection: "memories".to_string(), top_k: top_k * 2 },
            QueryStep::CoreRead { sql: "SELECT * FROM memory_records WHERE id IN (...)".to_string() },
            QueryStep::GuardCheck { action: "memory.read.row_level".to_string() },
        ]
    }

    fn plan_keyword_search(&self) -> Vec<QueryStep> {
        vec![
            QueryStep::GuardCheck { action: "memory.read".to_string() },
            QueryStep::CoreRead { sql: "SELECT * FROM memory_records WHERE content LIKE ?".to_string() },
        ]
    }

    fn plan_recall(&self) -> Vec<QueryStep> {
        vec![
            QueryStep::GuardCheck { action: "memory.read".to_string() },
            QueryStep::CoreRead { sql: "SELECT * FROM memory_records WHERE id IN (...)".to_string() },
        ]
    }

    fn plan_branch_fork(&self) -> Vec<QueryStep> {
        vec![
            QueryStep::GuardCheck { action: "branch.create".to_string() },
            QueryStep::CoreRead { sql: "PRAGMA wal_checkpoint(TRUNCATE)".to_string() },
            QueryStep::BranchOp { branch_name: String::new() },
        ]
    }

    fn plan_merge(&self, target: &str) -> Vec<QueryStep> {
        vec![
            QueryStep::GuardCheck { action: "branch.merge".to_string() },
            QueryStep::BranchOp { branch_name: format!("{target}.preview") },
            QueryStep::GuardCheck { action: "branch.merge.conflict_resources".to_string() },
            QueryStep::BranchOp { branch_name: target.to_string() },
        ]
    }

    fn plan_diff(&self) -> Vec<QueryStep> {
        vec![
            QueryStep::GuardCheck { action: "branch.diff".to_string() },
            QueryStep::BranchOp { branch_name: "diff".to_string() },
        ]
    }

    fn plan_sync(&self) -> Vec<QueryStep> {
        vec![
            QueryStep::GuardCheck { action: "sync".to_string() },
            QueryStep::SyncPush,
            QueryStep::SyncPull,
        ]
    }

    fn plan_reflect(&self, job_type: &str) -> Vec<QueryStep> {
        vec![
            QueryStep::GuardCheck { action: "reflect".to_string() },
            QueryStep::ReflectJob { job_type: job_type.to_string() },
        ]
    }
}
