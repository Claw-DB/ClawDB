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
    pub async fn start(config: ClawDBConfig) -> ClawDBResult<Self> {
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
}
