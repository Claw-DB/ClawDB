//! Best-fit `clawdb` wrapper over the currently published component crates.

use std::{path::Path, sync::Arc, time::Instant};

use anyhow::Context;
use claw_guard::error::GuardError;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::Executor;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::{
    error::{ClawDBError, ClawDBResult},
    plugins::{events::ClawEvent, manager::PluginManager},
    telemetry::{Metrics, PrometheusHandle},
    types::{
        BranchDiff, ClawTransaction, HealthStatus, MemoryRecord, MergeResult, ReflectSummary,
        RememberResult, SearchHit, SyncSummary,
    },
};

pub use crate::config::ClawDBConfig;

/// Public session type returned by the wrapper.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClawDBSession {
    /// Session identifier.
    pub id: Uuid,
    /// Agent identifier.
    pub agent_id: Uuid,
    /// Workspace identifier.
    pub workspace_id: Uuid,
    /// Session role.
    pub role: String,
    /// Granted scopes.
    pub scopes: Vec<String>,
    /// Guard token.
    pub token: String,
    /// Expiry timestamp.
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

/// Unified database wrapper built from the current component crates.
pub struct ClawDB {
    /// User configuration.
    pub config: ClawDBConfig,
    core: Arc<claw_core::ClawEngine>,
    vector: Option<Arc<claw_vector::VectorEngine>>,
    branch: Arc<claw_branch::BranchEngine>,
    sync: Arc<claw_sync::SyncEngine>,
    guard: Arc<claw_guard::Guard>,
    reflect: Option<Arc<claw_reflect_client::ReflectClient>>,
    shutdown: CancellationToken,
    metrics: Arc<Metrics>,
    plugins: Arc<Mutex<PluginManager>>,
    started_at: Instant,
    sync_local_only: bool,
}

impl ClawDB {
    /// Creates a new wrapper and initializes all enabled components.
    pub async fn new(config: ClawDBConfig) -> ClawDBResult<Self> {
        crate::telemetry::init_telemetry(&config.telemetry)?;

        let core_config = claw_core::ClawConfig::builder()
            .db_path(config.core.db_path.clone())
            .max_connections(config.core.max_connections)
            .wal_enabled(config.core.wal_enabled)
            .cache_size_mb(config.core.cache_size_mb)
            .build()
            .map_err(ClawDBError::Core)?;
        let core = Arc::new(claw_core::ClawEngine::open(core_config).await?);
        core.migrate().await?;
        core.pool()
            .execute(
                "CREATE TABLE IF NOT EXISTS memory_records (
                    id TEXT PRIMARY KEY
                )",
            )
            .await
            .map_err(|error| ClawDBError::ComponentInit("core", error.to_string()))?;
        core.pool()
            .execute(
                "CREATE TABLE IF NOT EXISTS tool_outputs (
                    id TEXT PRIMARY KEY,
                    session_id TEXT
                )",
            )
            .await
            .map_err(|error| ClawDBError::ComponentInit("core", error.to_string()))?;

        let vector = if config.vector.enabled {
            let vector_config = claw_vector::VectorConfig::builder()
                .db_path(config.vector.db_path.clone())
                .index_dir(config.vector.index_dir.clone())
                .embedding_service_url(config.vector.embedding_service_url.clone())
                .default_workspace_id(config.workspace_id.to_string())
                .default_dimensions(config.vector.default_dimensions)
                .build()
                .map_err(ClawDBError::Vector)?;
            let engine = Arc::new(
                claw_vector::VectorEngine::new(vector_config)
                    .await
                    .map_err(|error| ClawDBError::ComponentInit("vector", error.to_string()))?,
            );
            ensure_vector_collection(&engine, &config.workspace_id.to_string()).await?;
            Some(engine)
        } else {
            None
        };

        let branch_config = claw_branch::BranchConfig::builder()
            .workspace_id(config.workspace_id)
            .branches_dir(config.branch.branches_dir.clone())
            .registry_db_path(config.branch.registry_db_path.clone())
            .max_branches_per_workspace(config.branch.max_branches_per_workspace)
            .gc_interval_secs(config.branch.gc_interval_secs)
            .trunk_branch_name(config.branch.trunk_branch_name.clone())
            .build()
            .map_err(ClawDBError::Branch)?;
        let branch = Arc::new(
            claw_branch::BranchEngine::new(branch_config, &config.core.db_path)
                .await
                .map_err(|error| ClawDBError::ComponentInit("branch", error.to_string()))?,
        );
        branch.start_gc_scheduler().await?;

        let sync_local_only = config.sync.hub_url.is_none();
        let sync_config = claw_sync::SyncConfig {
            workspace_id: config.workspace_id,
            device_id: config.agent_id,
            hub_endpoint: config
                .sync
                .hub_url
                .clone()
                .unwrap_or_else(|| "http://127.0.0.1:50051".to_string()),
            data_dir: config.sync.data_dir.clone(),
            db_path: config.sync.db_path.clone(),
            tls_enabled: config.sync.tls_enabled,
            connect_timeout_secs: config.sync.connect_timeout_secs,
            request_timeout_secs: config.sync.request_timeout_secs,
            sync_interval_secs: config.sync.sync_interval_secs,
            heartbeat_interval_secs: config.sync.sync_interval_secs.max(1),
            max_retries: 5,
            retry_base_ms: 500,
            max_delta_rows: config.sync.max_delta_rows,
            max_chunk_bytes: config.sync.max_chunk_bytes,
            max_pull_chunks: config.sync.max_pull_chunks,
            max_push_inflight: config.sync.max_push_inflight,
        };
        let sync = Arc::new(
            claw_sync::SyncEngine::new(sync_config, core.pool().clone())
                .await
                .map_err(|error| ClawDBError::ComponentInit("sync", error.to_string()))?,
        );

        let guard_config = claw_guard::GuardConfig {
            db_path: config.guard.db_path.clone(),
            jwt_secret: claw_guard::ZeroizeString::new(config.guard.jwt_secret.clone()),
            policy_dir: config.guard.policy_dir.clone(),
            tls_cert_path: config.guard.tls_cert_path.clone(),
            tls_key_path: config.guard.tls_key_path.clone(),
            risk_thresholds: claw_guard::RiskThresholds::default(),
            sensitive_resources: config.guard.sensitive_resources.clone(),
            audit_flush_interval_ms: config.guard.audit_flush_interval_ms,
            audit_batch_size: config.guard.audit_batch_size,
        };
        let guard = Arc::new(
            claw_guard::Guard::new(guard_config)
                .await
                .map_err(|error| ClawDBError::ComponentInit("guard", error.to_string()))?,
        );

        let reflect = match (&config.reflect.base_url, &config.reflect.api_key) {
            (Some(base_url), Some(api_key)) => Some(Arc::new(
                claw_reflect_client::ReflectClient::new(base_url.clone(), api_key.clone())
                    .map_err(|error| ClawDBError::ComponentInit("reflect", error.to_string()))?,
            )),
            _ => {
                tracing::warn!("reflect client disabled because base URL or API key is missing");
                None
            }
        };

        let metrics = Metrics::new();
        let (mut plugin_manager, mut plugin_rx) = PluginManager::new();
        let _ = plugin_manager.load_from_dir(&config.plugins.plugins_dir);
        let plugins = Arc::new(Mutex::new(plugin_manager));
        let plugins_task = plugins.clone();
        tokio::spawn(async move {
            while let Ok(event) = plugin_rx.recv().await {
                let mut manager = plugins_task.lock().await;
                manager.dispatch(&event).await;
            }
        });

        tracing::info!(
            core = true,
            vector = vector.is_some(),
            branch = true,
            sync = true,
            reflect = reflect.is_some(),
            "ClawDB components initialized"
        );

        Ok(Self {
            config,
            core,
            vector,
            branch,
            sync,
            guard,
            reflect,
            shutdown: CancellationToken::new(),
            metrics,
            plugins,
            started_at: Instant::now(),
            sync_local_only,
        })
    }

    /// Compatibility constructor used by binaries.
    pub async fn start_with(config: ClawDBConfig) -> ClawDBResult<Self> {
        Self::new(config).await
    }

    /// Opens the default data directory using environment-backed configuration.
    pub async fn open_default() -> ClawDBResult<Self> {
        Self::new(ClawDBConfig::from_env()?).await
    }

    /// Opens a specific data directory.
    pub async fn open(data_dir: &Path) -> ClawDBResult<Self> {
        let mut config = ClawDBConfig::load_or_default(data_dir)?;
        config.data_dir = data_dir.to_path_buf();
        Self::new(config).await
    }

    /// Returns the current uptime in seconds.
    pub fn uptime_secs(&self) -> u64 {
        self.started_at.elapsed().as_secs()
    }

    /// Returns the underlying core engine.
    pub fn core_engine(&self) -> &Arc<claw_core::ClawEngine> {
        &self.core
    }

    /// Returns the underlying branch engine.
    pub fn branch_engine(&self) -> &Arc<claw_branch::BranchEngine> {
        &self.branch
    }

    /// Returns the underlying sync engine.
    pub fn sync_engine(&self) -> &Arc<claw_sync::SyncEngine> {
        &self.sync
    }

    /// Returns the underlying guard engine.
    pub fn guard_engine(&self) -> &Arc<claw_guard::Guard> {
        &self.guard
    }

    /// Returns the optional vector engine.
    pub fn vector_engine(&self) -> Option<&Arc<claw_vector::VectorEngine>> {
        self.vector.as_ref()
    }

    /// Returns the optional reflect client.
    pub fn reflect_client(&self) -> Option<&Arc<claw_reflect_client::ReflectClient>> {
        self.reflect.as_ref()
    }

    /// Returns a Prometheus handle for scraping metrics.
    pub fn metrics_handle(&self) -> PrometheusHandle {
        self.metrics.handle()
    }

    /// Creates a session with the default one-hour TTL.
    #[tracing::instrument(skip(self, scopes), fields(workspace_id = %self.config.workspace_id, agent_id = %agent_id))]
    pub async fn session(
        &self,
        agent_id: Uuid,
        role: &str,
        scopes: Vec<String>,
    ) -> ClawDBResult<ClawDBSession> {
        self.session_with_ttl(agent_id, role, scopes, 3600).await
    }

    /// Creates a session with a custom TTL.
    #[tracing::instrument(skip(self, scopes), fields(workspace_id = %self.config.workspace_id, agent_id = %agent_id))]
    pub async fn session_with_ttl(
        &self,
        agent_id: Uuid,
        role: &str,
        scopes: Vec<String>,
        ttl_secs: i64,
    ) -> ClawDBResult<ClawDBSession> {
        let session = self
            .guard
            .session_manager
            .create_session(agent_id, role, scopes.clone(), ttl_secs)
            .await?;
        self.metrics.session_created.inc();
        self.emit(ClawEvent::SessionCreated {
            session_id: session.session_id,
            agent_id,
        })
        .await;
        Ok(ClawDBSession {
            id: session.session_id,
            agent_id: session.agent_id,
            workspace_id: self.config.workspace_id,
            role: session.role,
            scopes,
            token: session.token,
            expires_at: session.expires_at,
        })
    }

    /// Stores a semantic memory with default tags and type mapping.
    #[tracing::instrument(skip(self, session, content), fields(workspace_id = %session.workspace_id, agent_id = %session.agent_id))]
    pub async fn remember(
        &self,
        session: &ClawDBSession,
        content: &str,
    ) -> ClawDBResult<RememberResult> {
        self.remember_typed(session, content, "semantic", &[], serde_json::Value::Null)
            .await
    }

    /// Stores a memory using the current component-crate capabilities.
    #[tracing::instrument(skip(self, session, content, tags, metadata), fields(workspace_id = %session.workspace_id, agent_id = %session.agent_id))]
    pub async fn remember_typed(
        &self,
        session: &ClawDBSession,
        content: &str,
        memory_type: &str,
        tags: &[String],
        metadata: serde_json::Value,
    ) -> ClawDBResult<RememberResult> {
        self.authorize(session, &["memory:write", "memory:*", "*"])
            .await?;

        let record = claw_core::MemoryRecord::new(
            content,
            parse_memory_type(memory_type),
            tags.to_vec(),
            None,
        );
        let memory_id = self.core.insert_memory(&record).await?;

        let mut indexed = false;
        if let Some(vector) = &self.vector {
            let vector_metadata = json!({
                "memory_id": memory_id,
                "memory_type": record.memory_type.as_str(),
                "tags": record.tags,
                "metadata": metadata,
            });
            match vector
                .upsert_in_workspace(
                    &session.workspace_id.to_string(),
                    "memories",
                    content,
                    vector_metadata,
                )
                .await
            {
                Ok(_) => indexed = true,
                Err(error) => {
                    tracing::warn!(error = %error, "vector upsert failed after core write")
                }
            }
        }

        self.metrics
            .remember_total(&session.workspace_id.to_string(), "ok");
        self.emit(ClawEvent::MemoryWritten {
            memory_id: memory_id.to_string(),
            workspace_id: session.workspace_id,
        })
        .await;

        Ok(RememberResult { memory_id, indexed })
    }

    /// Searches memory using semantic search when available, else SQLite FTS.
    #[tracing::instrument(skip(self, session, query), fields(workspace_id = %session.workspace_id, agent_id = %session.agent_id))]
    pub async fn search(
        &self,
        session: &ClawDBSession,
        query: &str,
    ) -> ClawDBResult<Vec<SearchHit>> {
        self.search_with_options(session, query, 10, self.vector.is_some(), None)
            .await
    }

    /// Searches memory with current component-crate semantics.
    #[tracing::instrument(skip(self, session, query, filter), fields(workspace_id = %session.workspace_id, agent_id = %session.agent_id))]
    pub async fn search_with_options(
        &self,
        session: &ClawDBSession,
        query: &str,
        top_k: usize,
        semantic: bool,
        filter: Option<serde_json::Value>,
    ) -> ClawDBResult<Vec<SearchHit>> {
        self.authorize(session, &["memory:read", "memory:search", "memory:*", "*"])
            .await?;

        let workspace_id = session.workspace_id.to_string();
        let use_semantic = semantic && self.vector.is_some();
        let hits = if use_semantic {
            let vector = self
                .vector
                .as_ref()
                .ok_or(ClawDBError::ComponentDisabled("vector"))?;
            let mut response = vector
                .search_text_in_workspace(
                    &workspace_id,
                    "memories",
                    query,
                    top_k.saturating_mul(3).max(top_k),
                )
                .await?;
            if let Some(filter_value) = filter {
                response
                    .results
                    .retain(|result| metadata_matches(&result.metadata, &filter_value));
            }
            response
                .results
                .into_iter()
                .take(top_k)
                .map(search_result_to_hit)
                .collect::<ClawDBResult<Vec<_>>>()?
        } else {
            self.core
                .fts_search(query)
                .await?
                .into_iter()
                .filter(|record| {
                    filter
                        .as_ref()
                        .map_or(true, |value| memory_record_matches(record, value))
                })
                .take(top_k)
                .map(|record| SearchHit {
                    id: record.id,
                    score: 1.0,
                    content: record.content,
                    memory_type: record.memory_type.as_str().to_string(),
                    tags: record.tags,
                    metadata: serde_json::Value::Null,
                })
                .collect()
        };

        let mode = if use_semantic { "semantic" } else { "fts" };
        self.metrics.search_total(&workspace_id, mode);
        self.metrics.search_hits(&workspace_id, hits.len() as f64);
        self.emit(ClawEvent::SearchExecuted {
            query: query.to_string(),
            hits: hits.len(),
        })
        .await;
        Ok(hits)
    }

    /// Recalls specific memories from the core engine.
    #[tracing::instrument(skip(self, session, memory_ids), fields(workspace_id = %session.workspace_id, agent_id = %session.agent_id))]
    pub async fn recall(
        &self,
        session: &ClawDBSession,
        memory_ids: &[Uuid],
    ) -> ClawDBResult<Vec<MemoryRecord>> {
        self.authorize(session, &["memory:read", "memory:*", "*"])
            .await?;
        let mut records = Vec::with_capacity(memory_ids.len());
        for id in memory_ids {
            records.push(self.core.get_memory(*id).await?);
        }
        Ok(records)
    }

    /// Forks a new branch from trunk.
    #[tracing::instrument(skip(self, session, name), fields(workspace_id = %session.workspace_id, agent_id = %session.agent_id))]
    pub async fn branch(&self, session: &ClawDBSession, name: &str) -> ClawDBResult<Uuid> {
        self.authorize(session, &["branch:write", "branch:*", "*"])
            .await?;
        let branch = self.branch.fork_trunk(name).await?;
        self.metrics
            .branch_ops(&session.workspace_id.to_string(), "fork");
        self.emit(ClawEvent::BranchCreated {
            branch_id: branch.id,
            name: branch.name,
        })
        .await;
        Ok(branch.id)
    }

    /// Forks a new branch from an explicit parent branch.
    #[tracing::instrument(skip(self, session, name), fields(workspace_id = %session.workspace_id, agent_id = %session.agent_id, parent = %parent))]
    pub async fn fork_branch(
        &self,
        session: &ClawDBSession,
        parent: Uuid,
        name: &str,
    ) -> ClawDBResult<Uuid> {
        self.authorize(session, &["branch:write", "branch:*", "*"])
            .await?;
        let branch = self.branch.fork(parent, name, None).await?;
        self.metrics
            .branch_ops(&session.workspace_id.to_string(), "fork");
        self.emit(ClawEvent::BranchCreated {
            branch_id: branch.id,
            name: branch.name,
        })
        .await;
        Ok(branch.id)
    }

    /// Returns a branch by identifier.
    #[tracing::instrument(skip(self, session), fields(workspace_id = %session.workspace_id, agent_id = %session.agent_id, branch_id = %branch_id))]
    pub async fn get_branch(
        &self,
        session: &ClawDBSession,
        branch_id: Uuid,
    ) -> ClawDBResult<claw_branch::Branch> {
        self.authorize(session, &["branch:read", "branch:*", "*"])
            .await?;
        Ok(self.branch.get(branch_id).await?)
    }

    /// Lists all branches in the current workspace.
    #[tracing::instrument(skip(self, session), fields(workspace_id = %session.workspace_id, agent_id = %session.agent_id))]
    pub async fn list_branches(
        &self,
        session: &ClawDBSession,
    ) -> ClawDBResult<Vec<claw_branch::Branch>> {
        self.authorize(session, &["branch:read", "branch:*", "*"])
            .await?;
        Ok(self.branch.list(None).await?)
    }

    /// Merges a source branch into a target branch.
    #[tracing::instrument(skip(self, session), fields(workspace_id = %session.workspace_id, agent_id = %session.agent_id))]
    pub async fn merge(
        &self,
        session: &ClawDBSession,
        source: Uuid,
        target: Uuid,
    ) -> ClawDBResult<MergeResult> {
        self.merge_with_strategy(session, source, target, claw_branch::MergeStrategy::Theirs)
            .await
    }

    /// Merges a source branch into a target branch using an explicit strategy.
    #[tracing::instrument(skip(self, session), fields(workspace_id = %session.workspace_id, agent_id = %session.agent_id, source = %source, target = %target))]
    pub async fn merge_with_strategy(
        &self,
        session: &ClawDBSession,
        source: Uuid,
        target: Uuid,
        strategy: claw_branch::MergeStrategy,
    ) -> ClawDBResult<MergeResult> {
        self.authorize(session, &["branch:write", "branch:*", "*"])
            .await?;
        let result = self.branch.merge(source, target, strategy).await?;
        self.metrics
            .branch_ops(&session.workspace_id.to_string(), "merge");
        self.emit(ClawEvent::BranchMerged {
            source,
            target,
            merged: result.applied,
        })
        .await;
        Ok(result)
    }

    /// Diffs two branches.
    #[tracing::instrument(skip(self, session), fields(workspace_id = %session.workspace_id, agent_id = %session.agent_id))]
    pub async fn diff(
        &self,
        session: &ClawDBSession,
        source: Uuid,
        target: Uuid,
    ) -> ClawDBResult<BranchDiff> {
        self.authorize(session, &["branch:read", "branch:*", "*"])
            .await?;
        Ok(self.branch.diff(source, target).await?)
    }

    /// Runs a sync round or returns a no-op summary in local-only mode.
    #[tracing::instrument(skip(self, session), fields(workspace_id = %session.workspace_id, agent_id = %session.agent_id))]
    pub async fn sync(&self, session: &ClawDBSession) -> ClawDBResult<SyncSummary> {
        self.authorize(session, &["sync:write", "sync:*", "*"])
            .await?;
        if self.sync_local_only {
            return Ok(SyncSummary {
                pushed: 0,
                pulled: 0,
                conflicts: 0,
                duration_ms: 0,
            });
        }
        let round = self.sync.sync_now().await?;
        self.metrics.sync_pushed(
            &session.workspace_id.to_string(),
            round.push.deltas_sent as u64,
        );
        self.metrics.sync_pulled(
            &session.workspace_id.to_string(),
            round.pull.deltas_received as u64,
        );
        self.emit(ClawEvent::SyncCompleted {
            pushed: round.push.deltas_sent,
            pulled: round.pull.deltas_received,
        })
        .await;
        Ok(SyncSummary {
            pushed: round.push.deltas_sent,
            pulled: round.pull.deltas_received,
            conflicts: round.pull.ops_skipped,
            duration_ms: round.duration_ms,
        })
    }

    /// Triggers a reflect job when the reflect client is configured.
    #[tracing::instrument(skip(self, session), fields(workspace_id = %session.workspace_id, agent_id = %session.agent_id))]
    pub async fn reflect(&self, session: &ClawDBSession) -> ClawDBResult<ReflectSummary> {
        self.authorize(session, &["reflect:run", "reflect:write", "reflect:*", "*"])
            .await?;
        let Some(reflect) = &self.reflect else {
            return Ok(ReflectSummary::skipped());
        };
        let job = reflect
            .trigger_job("full", &session.workspace_id.to_string(), false)
            .await?;
        self.emit(ClawEvent::ReflectCycleRun { facts_extracted: 0 })
            .await;
        Ok(ReflectSummary {
            job_id: Some(job.job_id),
            status: job.status,
            message: job.message,
            skipped: false,
        })
    }

    /// Starts a transaction over the core engine and stages vector work for commit.
    #[tracing::instrument(skip(self, session), fields(workspace_id = %session.workspace_id, agent_id = %session.agent_id))]
    pub async fn transaction<'a>(
        &'a self,
        session: &ClawDBSession,
    ) -> ClawDBResult<ClawTransaction<'a>> {
        self.authorize(session, &["memory:write", "memory:*", "*"])
            .await?;
        Ok(ClawTransaction {
            inner: self.core.begin_transaction().await?,
            vector: self.vector.clone(),
            workspace_id: session.workspace_id.to_string(),
            pending_vector_upserts: Vec::new(),
        })
    }

    /// Validates a session token.
    #[tracing::instrument(skip(self, token))]
    pub async fn validate_session(&self, token: &str) -> ClawDBResult<ClawDBSession> {
        let session = self.guard.session_manager.validate_session(token).await?;
        Ok(ClawDBSession {
            id: session.session_id,
            agent_id: session.agent_id,
            workspace_id: self.config.workspace_id,
            role: session.role,
            scopes: session.scopes,
            token: session.token,
            expires_at: session.expires_at,
        })
    }

    /// Revokes a session by identifier.
    #[tracing::instrument(skip(self))]
    pub async fn revoke_session(&self, session_id: Uuid) -> ClawDBResult<()> {
        self.guard
            .session_manager
            .revoke_session(session_id)
            .await?;
        Ok(())
    }

    /// Returns the number of active, non-revoked sessions recorded by guard.
    #[tracing::instrument(skip(self))]
    pub async fn active_session_count(&self) -> ClawDBResult<u64> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sessions WHERE revoked = 0 AND expires_at > CURRENT_TIMESTAMP",
        )
        .fetch_one(self.guard.pool())
        .await
        .map_err(|error| ClawDBError::ComponentInit("guard", error.to_string()))?;
        Ok(count.max(0) as u64)
    }

    /// Returns aggregate component health booleans.
    #[tracing::instrument(skip(self))]
    pub async fn health(&self) -> ClawDBResult<HealthStatus> {
        let mut components = std::collections::HashMap::new();

        components.insert("core".to_string(), self.core.stats().await.is_ok());
        components.insert(
            "vector".to_string(),
            if let Some(vector) = &self.vector {
                let _ = vector.stats().await;
                true
            } else {
                true
            },
        );
        components.insert("branch".to_string(), true);
        components.insert(
            "sync".to_string(),
            if self.sync_local_only {
                true
            } else {
                let status = self.sync.status();
                status.connected || status.last_error.is_none()
            },
        );
        components.insert("guard".to_string(), true);
        components.insert(
            "reflect".to_string(),
            if let Some(reflect) = &self.reflect {
                // The reflect client currently has no cheap health probe API.
                // Treat "configured" as healthy and surface runtime failures from reflect calls.
                let _ = reflect;
                true
            } else {
                true
            },
        );

        let ok = components.values().all(|healthy| *healthy);
        Ok(HealthStatus { ok, components })
    }

    /// Closes background tasks owned by the wrapper.
    #[tracing::instrument(skip(self))]
    pub async fn close(&self) -> ClawDBResult<()> {
        self.shutdown.cancel();
        self.branch.shutdown().await?;
        self.sync.close().await?;
        Ok(())
    }

    /// Compatibility alias for `close`.
    pub async fn shutdown(&self) -> ClawDBResult<()> {
        self.close().await
    }

    async fn authorize(
        &self,
        session: &ClawDBSession,
        accepted_scopes: &[&str],
    ) -> ClawDBResult<()> {
        self.guard
            .session_manager
            .validate_session(&session.token)
            .await
            .map_err(map_guard_session_error)?;
        if accepted_scopes.iter().any(|required| {
            session
                .scopes
                .iter()
                .any(|granted| scope_matches(granted, required))
        }) {
            return Ok(());
        }
        self.metrics.session_denied.inc();
        self.emit(ClawEvent::PolicyDenied {
            agent_id: session.agent_id,
            resource: accepted_scopes
                .first()
                .copied()
                .unwrap_or("unknown")
                .to_string(),
            reason: "required scope missing".to_string(),
        })
        .await;
        Err(ClawDBError::PermissionDenied(
            "required scope missing".to_string(),
        ))
    }

    async fn emit(&self, event: ClawEvent) {
        let manager = self.plugins.clone();
        let manager = manager.lock().await;
        manager.emit(event);
    }
}

impl<'a> ClawTransaction<'a> {
    /// Stages a default semantic memory inside the transaction.
    pub async fn remember(&mut self, content: &str) -> ClawDBResult<Uuid> {
        self.remember_typed(content, "semantic", &[], serde_json::Value::Null)
            .await
    }

    /// Stages a typed memory inside the transaction.
    pub async fn remember_typed(
        &mut self,
        content: &str,
        memory_type: &str,
        tags: &[String],
        metadata: serde_json::Value,
    ) -> ClawDBResult<Uuid> {
        let record = claw_core::MemoryRecord::new(
            content,
            parse_memory_type(memory_type),
            tags.to_vec(),
            None,
        );
        let id = self.inner.insert_memory(&record).await?;
        self.pending_vector_upserts.push((
            content.to_string(),
            json!({
                "memory_id": id,
                "memory_type": record.memory_type.as_str(),
                "tags": record.tags,
                "metadata": metadata,
            }),
        ));
        Ok(id)
    }

    /// Commits the transaction and flushes staged vector writes best-effort.
    pub async fn commit(mut self) -> ClawDBResult<()> {
        self.inner.commit().await?;
        if let Some(vector) = &self.vector {
            for (content, metadata) in std::mem::take(&mut self.pending_vector_upserts) {
                if let Err(error) = vector
                    .upsert_in_workspace(&self.workspace_id, "memories", &content, metadata)
                    .await
                {
                    tracing::warn!(error = %error, "vector upsert failed after transaction commit");
                }
            }
        }
        Ok(())
    }

    /// Rolls the transaction back.
    pub async fn rollback(self) -> ClawDBResult<()> {
        self.inner.rollback().await?;
        Ok(())
    }
}

/// Compatibility alias.
pub type ClawDBEngine = ClawDB;

async fn ensure_vector_collection(
    vector: &claw_vector::VectorEngine,
    workspace_id: &str,
) -> ClawDBResult<()> {
    let existing = vector.list_collections_in_workspace(workspace_id).await?;
    if existing
        .iter()
        .any(|collection| collection.name == "memories")
    {
        return Ok(());
    }
    vector
        .create_collection_in_workspace(
            workspace_id,
            "memories",
            vector.config.default_dimensions,
            claw_vector::DistanceMetric::Cosine,
        )
        .await
        .context("failed to create default memories collection")
        .map_err(|error| ClawDBError::ComponentInit("vector", error.to_string()))?;
    Ok(())
}

fn parse_memory_type(value: &str) -> claw_core::MemoryType {
    match value.trim().to_ascii_lowercase().as_str() {
        "semantic" | "context" | "message" => claw_core::MemoryType::Semantic,
        "episodic" => claw_core::MemoryType::Episodic,
        "working" => claw_core::MemoryType::Working,
        "procedural" => claw_core::MemoryType::Procedural,
        _ => claw_core::MemoryType::Semantic,
    }
}

fn metadata_matches(metadata: &serde_json::Value, filter: &serde_json::Value) -> bool {
    match filter {
        serde_json::Value::Object(expected) => expected
            .iter()
            .all(|(key, value)| metadata.get(key) == Some(value)),
        _ => true,
    }
}

fn memory_record_matches(record: &MemoryRecord, filter: &serde_json::Value) -> bool {
    let tags = serde_json::Value::Array(
        record
            .tags
            .iter()
            .cloned()
            .map(serde_json::Value::String)
            .collect(),
    );
    let view = json!({
        "id": record.id.to_string(),
        "content": record.content.clone(),
        "memory_type": record.memory_type.as_str(),
        "tags": tags,
    });
    metadata_matches(&view, filter)
}

fn search_result_to_hit(result: claw_vector::SearchResult) -> ClawDBResult<SearchHit> {
    let memory_type = result
        .metadata
        .get("memory_type")
        .and_then(|value| value.as_str())
        .unwrap_or("semantic")
        .to_string();
    let tags = result
        .metadata
        .get("tags")
        .and_then(|value| value.as_array())
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_str().map(ToOwned::to_owned))
                .collect()
        })
        .unwrap_or_default();
    Ok(SearchHit {
        id: result.id,
        score: result.score,
        content: result.text.unwrap_or_default(),
        memory_type,
        tags,
        metadata: result.metadata,
    })
}

fn scope_matches(granted: &str, required: &str) -> bool {
    granted == "*"
        || granted == required
        || granted
            .strip_suffix(":*")
            .is_some_and(|prefix| required.starts_with(&format!("{prefix}:")))
}

fn map_guard_session_error(error: GuardError) -> ClawDBError {
    match error {
        GuardError::SessionExpired { .. }
        | GuardError::SessionRevoked(_)
        | GuardError::SessionNotFound(_)
        | GuardError::TokenInvalid(_)
        | GuardError::TokenExpired { .. } => ClawDBError::SessionInvalid,
        other => ClawDBError::Guard(other),
    }
}
