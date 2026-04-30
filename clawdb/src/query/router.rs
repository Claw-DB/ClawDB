//! `QueryRouter`: dispatches a `Query` to the correct subsystem(s) after guard checks.

use std::sync::Arc;
use std::time::Instant;
use uuid::Uuid;

use crate::{
    error::{ClawDBError, ClawDBResult},
    events::{bus::EventBus, types::ClawEvent},
    lifecycle::manager::ComponentLifecycleManager,
    query::types::{Query, QueryResult, QueryResultData},
    session::context::SessionContext,
};

/// Routes high-level queries to the appropriate ClawDB subsystem.
pub struct QueryRouter {
    lifecycle: Arc<ComponentLifecycleManager>,
    event_bus: Arc<EventBus>,
}

impl QueryRouter {
    /// Creates a new `QueryRouter`.
    pub fn new(lifecycle: Arc<ComponentLifecycleManager>, event_bus: Arc<EventBus>) -> Self {
        Self { lifecycle, event_bus }
    }

    /// Routes `query` for the given `session`, returning a populated `QueryResult`.
    #[tracing::instrument(skip(self, query, session), fields(component = query.target_component()))]
    pub async fn route(
        &self,
        query: Query,
        session: &SessionContext,
    ) -> ClawDBResult<QueryResult> {
        let started = Instant::now();
        let guard = self.lifecycle.guard()?;

        let decision = guard
            .check_access(
                &session.token,
                query.target_component(),
                "memory",
                &[],
                &session.task_type,
            )
            .await?;

        if let claw_guard::AccessDecision::Deny { reason } = &decision {
            let agent_id = query.agent_id();
            self.event_bus.publish(ClawEvent::GuardDenied {
                agent_id,
                action: query.target_component().to_string(),
                resource: "memory".to_string(),
                reason: reason.clone(),
            });
            return Err(ClawDBError::Guard(claw_guard::GuardError::Denied(reason.clone())));
        }

        let guard_applied = !matches!(decision, claw_guard::AccessDecision::Allow);

        let result_data = match &query {
            Query::Remember { .. } => self.route_remember(&query).await?,
            Query::Search { semantic: true, .. } => self.route_semantic_search(&query).await?,
            Query::Search { semantic: false, .. } => self.route_keyword_search(&query).await?,
            Query::Recall { .. } => self.route_recall(&query).await?,
            Query::Branch { .. } => self.route_branch_op(&query).await?,
            Query::Merge { .. } => self.route_merge(&query).await?,
            Query::Diff { .. } => self.route_diff(&query).await?,
            Query::Sync { .. } => self.route_sync(&query).await?,
            Query::Reflect { .. } => self.route_reflect(&query).await?,
            Query::Raw { sql, params, .. } => {
                let core = self.lifecycle.core()?;
                let rows = core.execute_raw_read(sql, params).await?;
                QueryResultData::MemoryEntries(rows)
            }
        };

        Ok(QueryResult {
            query_id: Uuid::new_v4(),
            data: result_data,
            latency_ms: started.elapsed().as_millis() as u64,
            guard_applied,
            from_cache: false,
        })
    }

    async fn route_remember(&self, q: &Query) -> ClawDBResult<QueryResultData> {
        let Query::Remember { agent_id, content, memory_type, metadata, tags } = q else {
            return Err(ClawDBError::QueryExecutionFailed {
                step: "remember".to_string(),
                reason: "unexpected query variant".to_string(),
            });
        };
        let core = self.lifecycle.core()?;
        let (memory_id, importance) = core
            .insert_memory(&agent_id.to_string(), content, memory_type, metadata, tags)
            .await?;

        let vector = self.lifecycle.vector()?;
        let _ = vector.upsert("memories", &memory_id, content, metadata).await;

        self.event_bus.publish(ClawEvent::MemoryAdded {
            agent_id: *agent_id,
            memory_id: memory_id.parse().unwrap_or(Uuid::nil()),
            memory_type: memory_type.clone(),
        });

        Ok(QueryResultData::MemoryEntries(vec![serde_json::json!({
            "memory_id": memory_id,
            "importance_score": importance,
        })]))
    }

    async fn route_semantic_search(&self, q: &Query) -> ClawDBResult<QueryResultData> {
        let Query::Search { agent_id, text, top_k, filter, alpha, .. } = q else {
            return Err(ClawDBError::QueryExecutionFailed {
                step: "semantic_search".to_string(),
                reason: "unexpected query variant".to_string(),
            });
        };
        let vector = self.lifecycle.vector()?;
        let results = vector
            .hybrid_search("memories", text, *top_k, filter.as_ref(), *alpha)
            .await?;

        let ids: Vec<String> = results.iter().map(|r| r.id.clone()).collect();
        let core = self.lifecycle.core()?;
        let memories = core.get_memories(&agent_id.to_string(), &ids).await?;

        self.event_bus.publish(ClawEvent::SearchExecuted {
            agent_id: *agent_id,
            query_preview: text.chars().take(80).collect(),
            result_count: memories.len(),
            latency_ms: 0,
        });

        Ok(QueryResultData::SearchResults(memories))
    }

    async fn route_keyword_search(&self, q: &Query) -> ClawDBResult<QueryResultData> {
        let Query::Search { agent_id, text, .. } = q else {
            return Err(ClawDBError::QueryExecutionFailed {
                step: "keyword_search".to_string(),
                reason: "unexpected query variant".to_string(),
            });
        };
        let core = self.lifecycle.core()?;
        let mut results = core.search_content(&agent_id.to_string(), text).await?;
        let tag_results = core
            .search_memories_by_tag(&agent_id.to_string(), &[text.clone()])
            .await?;
        results.extend(tag_results);
        Ok(QueryResultData::SearchResults(results))
    }

    async fn route_recall(&self, q: &Query) -> ClawDBResult<QueryResultData> {
        let Query::Recall { agent_id, memory_ids } = q else {
            return Err(ClawDBError::QueryExecutionFailed {
                step: "recall".to_string(),
                reason: "unexpected query variant".to_string(),
            });
        };
        let core = self.lifecycle.core()?;
        let ids: Vec<String> = memory_ids.iter().map(|id| id.to_string()).collect();
        let memories = core.get_memories(&agent_id.to_string(), &ids).await?;
        Ok(QueryResultData::MemoryEntries(memories))
    }

    async fn route_branch_op(&self, q: &Query) -> ClawDBResult<QueryResultData> {
        let Query::Branch { parent, name, description, .. } = q else {
            return Err(ClawDBError::QueryExecutionFailed {
                step: "branch_op".to_string(),
                reason: "unexpected query variant".to_string(),
            });
        };
        let branch = self.lifecycle.branch()?;
        let info = branch.create_branch(parent, name, description.as_deref()).await?;
        Ok(QueryResultData::BranchInfo(serde_json::to_value(info)?))
    }

    async fn route_merge(&self, q: &Query) -> ClawDBResult<QueryResultData> {
        let Query::Merge { source, target, strategy, .. } = q else {
            return Err(ClawDBError::QueryExecutionFailed {
                step: "merge".to_string(),
                reason: "unexpected query variant".to_string(),
            });
        };
        let branch = self.lifecycle.branch()?;
        let stats = branch.merge(source, target, strategy).await?;
        Ok(QueryResultData::BranchInfo(serde_json::json!({
            "applied": stats.applied,
            "conflicts": stats.conflicts,
        })))
    }

    async fn route_diff(&self, q: &Query) -> ClawDBResult<QueryResultData> {
        let Query::Diff { branch_a, branch_b, .. } = q else {
            return Err(ClawDBError::QueryExecutionFailed {
                step: "diff".to_string(),
                reason: "unexpected query variant".to_string(),
            });
        };
        let branch = self.lifecycle.branch()?;
        let stats = branch.diff(branch_a, branch_b).await?;
        Ok(QueryResultData::BranchInfo(serde_json::json!({
            "added": stats.added,
            "removed": stats.removed,
            "modified": stats.modified,
            "divergence_score": stats.divergence_score,
        })))
    }

    async fn route_sync(&self, q: &Query) -> ClawDBResult<QueryResultData> {
        let Query::Sync { agent_id } = q else {
            return Err(ClawDBError::QueryExecutionFailed {
                step: "sync".to_string(),
                reason: "unexpected query variant".to_string(),
            });
        };
        let sync = self.lifecycle.sync()?;
        let push = sync.push_now().await?;
        let pull = sync.pull_now().await?;
        self.event_bus.publish(ClawEvent::SyncCompleted {
            agent_id: *agent_id,
            pushed: push.pushed,
            pulled: pull.pulled,
        });
        Ok(QueryResultData::SyncResult(serde_json::json!({
            "pushed": push.pushed,
            "pulled": pull.pulled,
        })))
    }

    async fn route_reflect(&self, q: &Query) -> ClawDBResult<QueryResultData> {
        let Query::Reflect { agent_id, job_type, dry_run } = q else {
            return Err(ClawDBError::QueryExecutionFailed {
                step: "reflect".to_string(),
                reason: "unexpected query variant".to_string(),
            });
        };
        let reflect = self.lifecycle.reflect()?;
        let result = reflect.trigger_reflection(*agent_id, job_type, *dry_run).await?;
        Ok(QueryResultData::ReflectResult(serde_json::to_value(result)?))
    }
}
