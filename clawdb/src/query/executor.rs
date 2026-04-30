//! `QueryExecutor`: executes a `QueryPlan` step by step against the live subsystems.

use std::sync::Arc;
use std::time::Instant;
use uuid::Uuid;

use crate::{
    error::{ClawDBError, ClawDBResult},
    events::{bus::EventBus, types::ClawEvent},
    lifecycle::manager::ComponentLifecycleManager,
    query::types::{Query, QueryPlan, QueryResult, QueryResultData, QueryStep},
    session::context::SessionContext,
};

/// Executes a `QueryPlan` against the live ClawDB subsystem engines.
pub struct QueryExecutor {
    lifecycle: Arc<ComponentLifecycleManager>,
    event_bus: Arc<EventBus>,
}

impl QueryExecutor {
    /// Creates a new `QueryExecutor`.
    pub fn new(lifecycle: Arc<ComponentLifecycleManager>, event_bus: Arc<EventBus>) -> Self {
        Self { lifecycle, event_bus }
    }

    /// Executes `plan` for `session`, returning an aggregated `QueryResult`.
    #[tracing::instrument(skip(self, plan, session), fields(plan_id = %plan.id))]
    pub async fn execute(
        &self,
        plan: QueryPlan,
        session: &SessionContext,
    ) -> ClawDBResult<QueryResult> {
        let started = Instant::now();
        let query_id = plan.id;
        let guard_applied = plan.guard_applied;
        let query = plan.query.clone();

        let mut accumulated: Vec<serde_json::Value> = vec![];
        let mut last_err: Option<ClawDBError> = None;

        for step in &plan.steps {
            let mut attempts = 0u8;
            loop {
                attempts += 1;
                match self.execute_step(step, session, &mut accumulated).await {
                    Ok(()) => break,
                    Err(e) if e.is_transient() && attempts <= 2 => {
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    }
                    Err(e) => {
                        self.event_bus.publish(ClawEvent::ComponentHealthChanged {
                            component: e.component().to_string(),
                            healthy: false,
                        });
                        last_err = Some(e);
                        break;
                    }
                }
            }
            if last_err.is_some() {
                break;
            }
        }

        if let Some(err) = last_err {
            return Err(err);
        }

        let data = if accumulated.is_empty() {
            match &query {
                Query::Sync { .. } | Query::Reflect { .. } | Query::Remember { .. } => {
                    QueryResultData::Unit
                }
                _ => QueryResultData::MemoryEntries(vec![]),
            }
        } else {
            QueryResultData::MemoryEntries(accumulated)
        };

        Ok(QueryResult {
            query_id,
            data,
            latency_ms: started.elapsed().as_millis() as u64,
            guard_applied,
            from_cache: false,
        })
    }

    fn execute_step<'a>(
        &'a self,
        step: &'a QueryStep,
        session: &'a SessionContext,
        accumulated: &'a mut Vec<serde_json::Value>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ClawDBResult<()>> + 'a>> {
        Box::pin(async move { match step {
            QueryStep::GuardCheck { action } => {
                let guard = self.lifecycle.guard()?;
                guard
                    .check_access(&session.token, action, "memory", &[], &session.task_type)
                    .await?;
            }
            QueryStep::CoreRead { sql } => {
                let core = self.lifecycle.core()?;
                let rows = core.execute_raw_read(sql, &[]).await?;
                accumulated.extend(rows);
            }
            QueryStep::CoreWrite { sql } => {
                let core = self.lifecycle.core()?;
                core.execute_raw_write(sql, &[]).await?;
            }
            QueryStep::VectorSearch { collection, top_k } => {
                let vector = self.lifecycle.vector()?;
                let results = vector.search(collection, "", *top_k, None).await?;
                for r in results {
                    accumulated.push(
                        serde_json::to_value(r).unwrap_or(serde_json::Value::Null),
                    );
                }
            }
            QueryStep::VectorUpsert { collection } => {
                let vector = self.lifecycle.vector()?;
                vector
                    .upsert(collection, &Uuid::new_v4().to_string(), "", &serde_json::Value::Null)
                    .await?;
            }
            QueryStep::SyncPush => {
                let sync = self.lifecycle.sync()?;
                sync.push_now().await?;
            }
            QueryStep::SyncPull => {
                let sync = self.lifecycle.sync()?;
                sync.pull_now().await?;
            }
            QueryStep::BranchOp { branch_name } => {
                let _branch = self.lifecycle.branch()?;
                tracing::debug!("BranchOp on {}", branch_name);
            }
            QueryStep::ReflectJob { job_type } => {
                let reflect = self.lifecycle.reflect()?;
                reflect.trigger_reflection(Uuid::nil(), job_type, false).await?;
            }
            QueryStep::Parallel(steps) => {
                for s in steps {
                    self.execute_step(s, session, accumulated).await?;
                }
            }
            QueryStep::Sequential(steps) => {
                for s in steps {
                    self.execute_step(s, session, accumulated).await?;
                }
            }
        }
        Ok(()) })
    }
}
