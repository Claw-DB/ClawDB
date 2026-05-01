//! `ClawDB`: the top-level aggregate runtime that wires all subsystems together.

use std::{path::Path, sync::Arc, time::Instant};

use uuid::Uuid;

use crate::{
    config::ClawDBConfig,
    error::ClawDBResult,
    events::{bus::EventBus, emitter::EventEmitter, subscriber::EventSubscriber},
    lifecycle::{health::HealthReport, manager::ComponentLifecycleManager},
    plugins::{
        interface::{PluginContext, PluginManifest},
        loader::PluginLoader,
        registry::PluginRegistry,
        sandbox::PluginSandbox,
        ClawPlugin,
    },
    query::{
        executor::QueryExecutor,
        optimizer::QueryOptimizer,
        planner::MemoryPlanner,
        router::QueryRouter,
        types::{Query, QueryResult},
    },
    session::{
        context::SessionContext,
        manager::{ClawDBSession, SessionManager},
    },
    telemetry::{Metrics, Telemetry},
    transaction::manager::TransactionManager,
};


// ── ClawDB ────────────────────────────────────────────────────────────────────

/// The ClawDB aggregate runtime.
///
/// Wires every subsystem together and exposes a single coherent API to
/// application code, gRPC handlers, and CLI commands.
pub struct ClawDB {
    /// Parsed, immutable configuration.
    pub config: Arc<ClawDBConfig>,
    /// Lifecycle manager: starts, stops, and health-checks all engine components.
    pub lifecycle: Arc<ComponentLifecycleManager>,
    /// Internal event bus: fan-out broadcast channel.
    pub event_bus: Arc<EventBus>,
    /// Engine-level event emitter (component = "engine").
    pub emitter: EventEmitter,
    /// Query router: guards access and delegates to the correct sub-engine.
    pub router: Arc<QueryRouter>,
    /// Memory planner: decides the optimal storage strategy for new entries.
    pub planner: Arc<MemoryPlanner>,
    /// Query optimiser: rewrites queries for better performance.
    pub optimizer: Arc<QueryOptimizer>,
    /// Query executor: runs the optimised plan across sub-engines.
    pub executor: Arc<QueryExecutor>,
    /// Session manager: issues, validates, and revokes claw-guard sessions.
    pub session_manager: Arc<SessionManager>,
    /// Transaction manager: 2PC across core + vector sub-engines.
    pub tx_manager: Arc<TransactionManager>,
    /// Plugin registry: loaded plugin instances + async hook dispatch.
    pub plugins: Arc<PluginRegistry>,
    /// Plugin sandbox: capability allowlist.
    pub sandbox: Arc<PluginSandbox>,
    /// Prometheus metrics + registry.
    pub telemetry: Arc<Telemetry>,
    /// Engine start timestamp (used for uptime reporting).
    started_at: Instant,
}

impl ClawDB {
    // ── Constructors ─────────────────────────────────────────────────────────

    /// Creates a `ClawDB` instance and starts all subsystems.
    ///
    /// Equivalent to calling [`ClawDB::build`] followed by [`ClawDB::start`].
    pub async fn new(config: ClawDBConfig) -> ClawDBResult<Self> {
        let mut engine = Self::build(config).await?;
        engine.start().await?;
        Ok(engine)
    }

    /// Creates a `ClawDB` from the default data directory (`~/.clawdb`).
    pub async fn open_default() -> ClawDBResult<Self> {
        let config = ClawDBConfig::from_env()?;
        Self::new(config).await
    }

    /// Creates a `ClawDB` rooted at `data_dir`.
    pub async fn open(data_dir: &Path) -> ClawDBResult<Self> {
        let mut config = ClawDBConfig::from_env()?;
        config.data_dir = data_dir.to_path_buf();
        Self::new(config).await
    }

    /// Builds the engine without starting subsystems (useful for testing).
    pub async fn build(config: ClawDBConfig) -> ClawDBResult<Self> {
        let config = Arc::new(config);
        let event_bus = Arc::new(EventBus::from_config(&config));
        let emitter = EventEmitter::new(event_bus.clone(), "engine");
        let telemetry = Telemetry::new();

        let lifecycle =
            ComponentLifecycleManager::new(config.clone(), event_bus.clone()).await?;
        let lifecycle = Arc::new(lifecycle);

        let router = Arc::new(QueryRouter::new(lifecycle.clone(), event_bus.clone()));
        let planner = Arc::new(MemoryPlanner::new(config.clone()));
        let optimizer = Arc::new(QueryOptimizer::new());
        let executor = Arc::new(QueryExecutor::new(lifecycle.clone(), event_bus.clone()));

        let session_manager = Arc::new(SessionManager::new(
            lifecycle.clone(),
            event_bus.clone(),
            config.clone(),
        ));

        let tx_manager = Arc::new(TransactionManager::new(
            lifecycle.clone(),
            event_bus.clone(),
        ));
        let sandbox = Arc::new(PluginSandbox::new(config.plugins.sandbox_enabled));
        let plugins = Arc::new(PluginRegistry::new(sandbox.clone()));

        Ok(Self {
            config,
            lifecycle,
            event_bus,
            emitter,
            router,
            planner,
            optimizer,
            executor,
            session_manager,
            tx_manager,
            plugins,
            sandbox,
            telemetry,
            started_at: Instant::now(),
        })
    }

    /// Starts all subsystems (idempotent — safe to call multiple times).
    pub async fn start(&mut self) -> ClawDBResult<()> {
        if let Some(lc) = Arc::get_mut(&mut self.lifecycle) {
            lc.start_all().await?;
        }
        self.load_plugins().await?;
        self.start_metrics_server();
        tracing::info!("ClawDB engine started");
        Ok(())
    }

    // ── Plugin lifecycle ─────────────────────────────────────────────────────

    async fn load_plugins(&self) -> ClawDBResult<()> {
        let loader = PluginLoader::new(self.sandbox.clone());
        let pairs = loader
            .load_from_dir(&self.config.plugins.plugins_dir, &self.config.plugins.enabled)
            .await?;
        for (manifest, plugin) in pairs {
            let ctx = PluginContext {
                config: serde_json::Value::Null,
                event_emitter: Arc::new(
                    EventEmitter::new(self.event_bus.clone(), "plugin"),
                ),
            };
            self.plugins.register(manifest, plugin, ctx).await?;
        }
        Ok(())
    }

    fn start_metrics_server(&self) {
        let port = self.config.telemetry.metrics_port;
        if port > 0 {
            crate::telemetry::metrics::serve_metrics(
                port,
                self.telemetry.registry.clone(),
            );
        }
    }

    // ── Session API ───────────────────────────────────────────────────────────

    /// Creates a new session for `agent_id` with the given `role`.
    pub async fn session(
        &self,
        agent_id: Uuid,
        role: &str,
        scopes: Vec<String>,
    ) -> ClawDBResult<ClawDBSession> {
        let sess = self.session_manager.create(agent_id, role, scopes, None).await?;
        self.telemetry.metrics.inc_session(role);
        Ok(sess)
    }

    /// Creates a session with an explicit task type annotation.
    pub async fn session_with_task(
        &self,
        agent_id: Uuid,
        role: &str,
        scopes: Vec<String>,
        task_type: &str,
    ) -> ClawDBResult<ClawDBSession> {
        let sess = self
            .session_manager
            .create(agent_id, role, scopes, Some(task_type.to_string()))
            .await?;
        self.telemetry.metrics.inc_session(role);
        Ok(sess)
    }

    /// Validates a guard token and returns the corresponding `SessionContext`.
    pub async fn validate_session(&self, token: &str) -> ClawDBResult<SessionContext> {
        self.session_manager.validate(token).await
    }

    /// Revokes a session by ID.
    pub async fn revoke_session(&self, session_id: Uuid) -> ClawDBResult<()> {
        self.session_manager.revoke(session_id).await
    }

    // ── Memory API ────────────────────────────────────────────────────────────

    /// Stores a memory and returns its ID and importance score.
    pub async fn remember(
        &self,
        session: &ClawDBSession,
        content: &str,
    ) -> ClawDBResult<RememberResult> {
        self.remember_typed(session, content, "general", &[], serde_json::Value::Null)
            .await
    }

    /// Stores a memory with explicit type, tags, and metadata.
    pub async fn remember_typed(
        &self,
        session: &ClawDBSession,
        content: &str,
        memory_type: &str,
        tags: &[String],
        metadata: serde_json::Value,
    ) -> ClawDBResult<RememberResult> {
        let start = Instant::now();
        let core = self.lifecycle.core()?;
        let (memory_id, importance_score) = core
            .insert_memory(
                &session.agent_id.to_string(),
                content,
                memory_type,
                &metadata,
                tags,
            )
            .await?;
        let secs = start.elapsed().as_secs_f64();
        self.telemetry.metrics.inc_query("remember", "core", "ok");
        self.telemetry.metrics.record_query_duration("remember", "core", secs);
        self.emitter.memory_added(session.agent_id, memory_id.clone(), memory_type.to_string());
        Ok(RememberResult { memory_id, importance_score })
    }

    // ── Search API ────────────────────────────────────────────────────────────

    /// Keyword or semantic search over memories.
    pub async fn search(
        &self,
        session: &ClawDBSession,
        query: &str,
    ) -> ClawDBResult<Vec<serde_json::Value>> {
        self.search_with_options(session, query, 10, true, None).await
    }

    /// Search with full control over top-k, semantic flag, and filter.
    pub async fn search_with_options(
        &self,
        session: &ClawDBSession,
        query: &str,
        top_k: usize,
        semantic: bool,
        filter: Option<serde_json::Value>,
    ) -> ClawDBResult<Vec<serde_json::Value>> {
        let start = Instant::now();
        let core = self.lifecycle.core()?;
        let results = if semantic {
            let vector = self.lifecycle.vector()?;
            let hits = vector.search("memories", query, top_k, filter).await?;
            let ids: Vec<String> = hits.iter().map(|r| r.id.clone()).collect();
            core.get_memories(&session.agent_id.to_string(), &ids).await?
        } else {
            core.search_content(&session.agent_id.to_string(), query).await?
        };
        let secs = start.elapsed().as_secs_f64();
        let kind = if semantic { "semantic" } else { "keyword" };
        self.telemetry.metrics.inc_query(kind, "core", "ok");
        self.telemetry.metrics.record_query_duration(kind, "core", secs);
        self.emitter.search_executed(
            session.agent_id,
            query.chars().take(80).collect::<String>(),
            results.len(),
            (secs * 1000.0) as u64,
        );
        Ok(results)
    }

    /// Retrieves specific memories by ID.
    pub async fn recall(
        &self,
        session: &ClawDBSession,
        memory_ids: &[String],
    ) -> ClawDBResult<Vec<serde_json::Value>> {
        let core = self.lifecycle.core()?;
        Ok(core.get_memories(&session.agent_id.to_string(), memory_ids).await?)
    }

    // ── Query router API ─────────────────────────────────────────────────────

    /// Routes a structured query through the guard and subsystem layers.
    pub async fn execute(
        &self,
        query: Query,
        session: &SessionContext,
    ) -> ClawDBResult<QueryResult> {
        self.router.route(query, session).await
    }

    // ── Branch API ────────────────────────────────────────────────────────────

    /// Creates a named branch snapshot and returns its ID.
    pub async fn branch(&self, _session: &ClawDBSession, name: &str) -> ClawDBResult<Uuid> {
        let branch = self.lifecycle.branch()?;
        let id = branch.create_snapshot(name).await?;
        self.emitter.branch_created(
            _session.agent_id,
            id,
            name.to_string(),
        );
        Ok(id)
    }

    /// Merges `source` snapshot into `target`.
    pub async fn merge(
        &self,
        _session: &ClawDBSession,
        source: Uuid,
        target: Uuid,
    ) -> ClawDBResult<serde_json::Value> {
        let branch = self.lifecycle.branch()?;
        branch.merge_snapshot(source, target).await?;
        Ok(serde_json::json!({ "source": source, "target": target, "status": "merged" }))
    }

    /// Diffs two snapshots.
    pub async fn diff(
        &self,
        _session: &ClawDBSession,
        branch_a: Uuid,
        branch_b: Uuid,
    ) -> ClawDBResult<serde_json::Value> {
        let branch = self.lifecycle.branch()?;
        let diff = branch.diff_snapshots(branch_a, branch_b).await?;
        Ok(diff)
    }

    // ── Sync API ──────────────────────────────────────────────────────────────

    /// Triggers a push+pull sync cycle and returns a summary.
    pub async fn sync(&self, session: &ClawDBSession) -> ClawDBResult<serde_json::Value> {
        let sync = self.lifecycle.sync()?;
        sync.push_now().await?;
        sync.pull_now().await?;
        self.emitter.sync_completed(session.agent_id, 1, 1);
        Ok(serde_json::json!({ "status": "ok", "pushed": 1, "pulled": 1 }))
    }

    // ── Reflect API ───────────────────────────────────────────────────────────

    /// Triggers a reflect job and returns the job ID.
    pub async fn reflect(&self, session: &ClawDBSession) -> ClawDBResult<String> {
        use crate::api::reflect_client::ReflectClient;
        let client = ReflectClient::new(&self.config.reflect.service_url);
        let job_id = client.start_reflection(&session.agent_id.to_string()).await?;
        Ok(job_id)
    }

    // ── Transaction API ───────────────────────────────────────────────────────

    /// Executes `f` within a single 2PC transaction.
    ///
    /// Automatically commits on success and rolls back on error.
    pub async fn transaction<F, T, Fut>(
        &self,
        session: &ClawDBSession,
        f: F,
    ) -> ClawDBResult<T>
    where
        F: FnOnce(Uuid) -> Fut,
        Fut: std::future::Future<Output = ClawDBResult<T>>,
    {
        let ctx = session.as_context();
        let tx_id = self.tx_manager.begin(&ctx).await?;
        match f(tx_id).await {
            Ok(result) => {
                self.tx_manager.commit(tx_id).await?;
                Ok(result)
            }
            Err(e) => {
                let _ = self.tx_manager.rollback(tx_id).await;
                Err(e)
            }
        }
    }

    // ── Event subscription API ────────────────────────────────────────────────

    /// Returns a new subscriber that receives all events.
    pub fn subscribe(&self) -> EventSubscriber {
        EventSubscriber::new(self.event_bus.subscribe())
    }

    // ── Health ────────────────────────────────────────────────────────────────

    /// Returns an aggregate health report for all subsystems.
    pub async fn health(&self) -> ClawDBResult<HealthReport> {
        Ok(self.lifecycle.health_report().await)
    }

    /// Engine uptime in seconds.
    pub fn uptime_secs(&self) -> u64 {
        self.started_at.elapsed().as_secs()
    }

    // ── Shutdown ──────────────────────────────────────────────────────────────

    /// Gracefully shuts down all subsystems.
    pub async fn close(&self) -> ClawDBResult<()> {
        tracing::info!("ClawDB shutting down");
        self.lifecycle.stop_all().await
    }

    /// Alias for `close` (used by CLI commands).
    pub async fn shutdown(&self) -> ClawDBResult<()> {
        self.close().await
    }

    /// Alias for `close` (used by CLI commands).
    pub async fn stop(&self) -> ClawDBResult<()> {
        self.close().await
    }
}

// ── Backward-compat alias ─────────────────────────────────────────────────────

/// Backward-compatible alias for [`ClawDB`].
pub type ClawDBEngine = ClawDB;

// ── Helper types ──────────────────────────────────────────────────────────────

/// Result returned by the `remember` convenience method.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RememberResult {
    /// The newly created memory ID.
    pub memory_id: String,
    /// Computed importance score for the memory.
    pub importance_score: f32,
}
