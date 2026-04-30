//! `ClawDBEngine`: the top-level aggregate runtime that wires all subsystems together.

use std::sync::Arc;

use crate::{
    config::ClawDBConfig,
    error::ClawDBResult,
    events::bus::EventBus,
    lifecycle::manager::ComponentLifecycleManager,
    plugins::{registry::PluginRegistry, sandbox::PluginSandbox},
    query::{
        executor::QueryExecutor,
        optimizer::QueryOptimizer,
        planner::MemoryPlanner,
        router::QueryRouter,
        types::{Query, QueryResult},
    },
    session::{context::SessionContext, manager::SessionManager},
    telemetry::metrics::Metrics,
    transaction::manager::TransactionManager,
};

/// The ClawDB aggregate runtime engine.
pub struct ClawDBEngine {
    pub config: Arc<ClawDBConfig>,
    pub lifecycle: Arc<ComponentLifecycleManager>,
    pub event_bus: Arc<EventBus>,
    pub router: Arc<QueryRouter>,
    pub planner: Arc<MemoryPlanner>,
    pub optimizer: Arc<QueryOptimizer>,
    pub executor: Arc<QueryExecutor>,
    pub sessions: Arc<SessionManager>,
    pub transactions: Arc<TransactionManager>,
    pub plugins: Arc<PluginRegistry>,
    pub sandbox: Arc<PluginSandbox>,
    pub metrics: Arc<Metrics>,
}

impl ClawDBEngine {
    /// Initialises all subsystems and returns a ready `ClawDBEngine`.
    pub async fn new(config: ClawDBConfig) -> ClawDBResult<Self> {
        let config = Arc::new(config);
        let event_bus = Arc::new(EventBus::new());
        let metrics = Metrics::new();

        let lifecycle =
            ComponentLifecycleManager::new(config.clone(), event_bus.clone()).await?;
        let lifecycle = Arc::new(lifecycle);

        let router = Arc::new(QueryRouter::new(lifecycle.clone(), event_bus.clone()));
        let planner = Arc::new(MemoryPlanner::new(config.clone()));
        let optimizer = Arc::new(QueryOptimizer::new());
        let executor = Arc::new(QueryExecutor::new(lifecycle.clone(), event_bus.clone()));
        let sessions = Arc::new(SessionManager::new(lifecycle.clone()));
        let transactions = Arc::new(TransactionManager::new());
        let plugins = Arc::new(PluginRegistry::new());
        let sandbox = Arc::new(PluginSandbox::new(config.plugins.sandbox_enabled));

        Ok(Self {
            config,
            lifecycle,
            event_bus,
            router,
            planner,
            optimizer,
            executor,
            sessions,
            transactions,
            plugins,
            sandbox,
            metrics,
        })
    }

    /// Starts all subsystems (call after `new`).
    pub async fn start(&mut self) -> ClawDBResult<()> {
        // SAFETY: we need mutable access to lifecycle to call start_all.
        // We use Arc::get_mut, which succeeds because we hold the only reference at this point.
        if let Some(lc) = Arc::get_mut(&mut self.lifecycle) {
            lc.start_all().await?;
        }
        Ok(())
    }

    /// Initialises and starts all subsystems in one call.
    pub async fn start_with(config: ClawDBConfig) -> ClawDBResult<Self> {
        let config = Arc::new(config);
        let event_bus = Arc::new(EventBus::new());
        let metrics = Metrics::new();

        let mut lifecycle =
            ComponentLifecycleManager::new(config.clone(), event_bus.clone()).await?;
        lifecycle.start_all().await?;
        let lifecycle = Arc::new(lifecycle);

        let router = Arc::new(QueryRouter::new(lifecycle.clone(), event_bus.clone()));
        let planner = Arc::new(MemoryPlanner::new(config.clone()));
        let optimizer = Arc::new(QueryOptimizer::new());
        let executor = Arc::new(QueryExecutor::new(lifecycle.clone(), event_bus.clone()));
        let sessions = Arc::new(SessionManager::new(lifecycle.clone()));
        let transactions = Arc::new(TransactionManager::new());
        let plugins = Arc::new(PluginRegistry::new());
        let sandbox = Arc::new(PluginSandbox::new(config.plugins.sandbox_enabled));

        Ok(Self {
            config,
            lifecycle,
            event_bus,
            router,
            planner,
            optimizer,
            executor,
            sessions,
            transactions,
            plugins,
            sandbox,
            metrics,
        })
    }

    /// Routes a query through the guard and subsystem layers.
    pub async fn execute(&self, query: Query, session: &SessionContext) -> ClawDBResult<QueryResult> {
        self.router.route(query, session).await
    }

    /// Returns an aggregate health report for all subsystems.
    pub async fn health(&self) -> crate::lifecycle::health::HealthReport {
        self.lifecycle.health_report().await
    }

    /// Performs a graceful shutdown of all subsystems.
    pub async fn shutdown(&self) -> ClawDBResult<()> {
        tracing::info!("ClawDB engine shutting down");
        self.lifecycle.stop_all().await
    }

    /// Alias for `shutdown` — used by CLI commands.
    pub async fn stop(&self) -> ClawDBResult<()> {
        self.shutdown().await
    }

    /// Convenience: stores a memory entry and returns basic result info.
    pub async fn remember(
        &self,
        agent_id: uuid::Uuid,
        content: &str,
        memory_type: &str,
        tags: &[String],
    ) -> ClawDBResult<RememberResult> {
        let core = self.lifecycle.core()?;
        let (memory_id, importance_score) = core
            .insert_memory(
                &agent_id.to_string(),
                content,
                memory_type,
                &serde_json::Value::Null,
                tags,
            )
            .await?;
        Ok(RememberResult { memory_id, importance_score })
    }

    /// Convenience: searches memories and returns raw JSON values.
    pub async fn search(
        &self,
        agent_id: uuid::Uuid,
        query: &str,
        semantic: bool,
        top_k: usize,
    ) -> ClawDBResult<Vec<serde_json::Value>> {
        let core = self.lifecycle.core()?;
        if semantic {
            let vector = self.lifecycle.vector()?;
            let results = vector.search("memories", query, top_k, None).await?;
            let ids: Vec<String> = results.iter().map(|r| r.id.clone()).collect();
            core.get_memories(&agent_id.to_string(), &ids).await.map_err(Into::into)
        } else {
            core.search_content(&agent_id.to_string(), query).await.map_err(Into::into)
        }
    }
}

/// Result returned by the `remember` convenience method.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RememberResult {
    /// The newly created memory ID.
    pub memory_id: String,
    /// Computed importance score for the memory.
    pub importance_score: f32,
}
