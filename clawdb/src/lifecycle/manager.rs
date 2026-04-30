//! `ComponentLifecycleManager`: starts, stops, and monitors all six ClawDB subsystems.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::{
    api::reflect_client::ReflectClient,
    config::ClawDBConfig,
    error::{ClawDBError, ClawDBResult},
    events::{bus::EventBus, types::ClawEvent},
    lifecycle::health::{ComponentHealth, HealthReport, HealthStatus},
};

/// Manages the lifecycle of all six ClawDB subsystem engines.
pub struct ComponentLifecycleManager {
    config: Arc<ClawDBConfig>,
    core: Option<Arc<claw_core::ClawEngine>>,
    vector: Option<Arc<claw_vector::VectorEngine>>,
    sync: Option<Arc<claw_sync::SyncEngine>>,
    branch: Option<Arc<claw_branch::BranchEngine>>,
    guard: Option<Arc<claw_guard::GuardEngine>>,
    reflect_client: Option<Arc<ReflectClient>>,
    health: Arc<RwLock<HashMap<String, ComponentHealth>>>,
    event_bus: Arc<EventBus>,
    started_at: std::time::Instant,
}

impl ComponentLifecycleManager {
    /// Creates a new `ComponentLifecycleManager`; does not start any subsystems.
    pub async fn new(
        config: Arc<ClawDBConfig>,
        event_bus: Arc<EventBus>,
    ) -> ClawDBResult<Self> {
        Ok(Self {
            config,
            core: None,
            vector: None,
            sync: None,
            branch: None,
            guard: None,
            reflect_client: None,
            health: Arc::new(RwLock::new(HashMap::new())),
            event_bus,
            started_at: std::time::Instant::now(),
        })
    }

    /// Starts all subsystems in dependency order.
    pub async fn start_all(&mut self) -> ClawDBResult<()> {
        self.start_guard().await?;
        self.start_core().await?;
        self.start_vector().await?;
        self.start_branch().await?;
        self.start_sync().await?;
        self.start_reflect().await;
        Ok(())
    }

    async fn start_guard(&mut self) -> ClawDBResult<()> {
        let cfg = self.config.into_guard_config();
        match claw_guard::GuardEngine::open(cfg).await {
            Ok(engine) => {
                self.guard = Some(Arc::new(engine));
                self.set_health("guard", ComponentHealth::healthy("guard", 0)).await;
                self.event_bus.publish(ClawEvent::ComponentHealthChanged {
                    component: "guard".to_string(),
                    healthy: true,
                });
                Ok(())
            }
            Err(e) => {
                self.set_health("guard", ComponentHealth::unhealthy("guard", e.to_string())).await;
                Err(e.into())
            }
        }
    }

    async fn start_core(&mut self) -> ClawDBResult<()> {
        let cfg = self.config.into_core_config();
        match claw_core::ClawEngine::open(cfg).await {
            Ok(engine) => {
                self.core = Some(Arc::new(engine));
                self.set_health("core", ComponentHealth::healthy("core", 0)).await;
                self.event_bus.publish(ClawEvent::ComponentHealthChanged {
                    component: "core".to_string(),
                    healthy: true,
                });
                Ok(())
            }
            Err(e) => {
                self.set_health("core", ComponentHealth::unhealthy("core", e.to_string())).await;
                Err(e.into())
            }
        }
    }

    async fn start_vector(&mut self) -> ClawDBResult<()> {
        let cfg = self.config.into_vector_config();
        match claw_vector::VectorEngine::open(cfg).await {
            Ok(engine) => {
                self.vector = Some(Arc::new(engine));
                self.set_health("vector", ComponentHealth::healthy("vector", 0)).await;
                self.event_bus.publish(ClawEvent::ComponentHealthChanged {
                    component: "vector".to_string(),
                    healthy: true,
                });
                Ok(())
            }
            Err(e) => {
                self.set_health("vector", ComponentHealth::unhealthy("vector", e.to_string())).await;
                Err(e.into())
            }
        }
    }

    async fn start_branch(&mut self) -> ClawDBResult<()> {
        let cfg = self.config.into_branch_config();
        match claw_branch::BranchEngine::open(cfg).await {
            Ok(engine) => {
                self.branch = Some(Arc::new(engine));
                self.set_health("branch", ComponentHealth::healthy("branch", 0)).await;
                self.event_bus.publish(ClawEvent::ComponentHealthChanged {
                    component: "branch".to_string(),
                    healthy: true,
                });
                Ok(())
            }
            Err(e) => {
                self.set_health("branch", ComponentHealth::unhealthy("branch", e.to_string())).await;
                Err(e.into())
            }
        }
    }

    async fn start_sync(&mut self) -> ClawDBResult<()> {
        let cfg = self.config.into_sync_config();
        match claw_sync::SyncEngine::open(cfg).await {
            Ok(engine) => {
                self.sync = Some(Arc::new(engine));
                self.set_health("sync", ComponentHealth::healthy("sync", 0)).await;
                self.event_bus.publish(ClawEvent::ComponentHealthChanged {
                    component: "sync".to_string(),
                    healthy: true,
                });
                Ok(())
            }
            Err(e) => {
                tracing::warn!("sync engine failed to start (non-fatal): {e}");
                self.set_health("sync", ComponentHealth::unhealthy("sync", e.to_string())).await;
                Ok(())
            }
        }
    }

    async fn start_reflect(&mut self) {
        let client = ReflectClient::new(
            self.config.reflect.service_url.clone(),
            self.config.clone(),
        );
        self.reflect_client = Some(Arc::new(client));
        self.set_health("reflect", ComponentHealth::healthy("reflect", 0)).await;
    }

    /// Stops all subsystems in reverse dependency order.
    pub async fn stop_all(&self) -> ClawDBResult<()> {
        if let Some(sync) = &self.sync {
            if let Err(e) = sync.close().await {
                tracing::warn!("sync close error: {e}");
            }
        }
        if let Some(branch) = &self.branch {
            if let Err(e) = branch.close().await {
                tracing::warn!("branch close error: {e}");
            }
        }
        if let Some(vector) = &self.vector {
            if let Err(e) = vector.close().await {
                tracing::warn!("vector close error: {e}");
            }
        }
        if let Some(core) = &self.core {
            if let Err(e) = core.close().await {
                tracing::warn!("core close error: {e}");
            }
        }
        if let Some(guard) = &self.guard {
            if let Err(e) = guard.flush_audit_log().await {
                tracing::warn!("guard audit flush error: {e}");
            }
            if let Err(e) = guard.close().await {
                tracing::warn!("guard close error: {e}");
            }
        }
        Ok(())
    }

    /// Restarts the named component.
    pub async fn restart_component(&mut self, name: &str) -> ClawDBResult<()> {
        match name {
            "guard" => self.start_guard().await,
            "core" => self.start_core().await,
            "vector" => self.start_vector().await,
            "branch" => self.start_branch().await,
            "sync" => self.start_sync().await,
            "reflect" => {
                self.start_reflect().await;
                Ok(())
            }
            _ => Err(ClawDBError::ComponentNotReady(format!("unknown component: {name}"))),
        }
    }

    /// Returns a reference to the core engine, or an error if not started.
    pub fn core(&self) -> ClawDBResult<Arc<claw_core::ClawEngine>> {
        self.core.clone().ok_or_else(|| ClawDBError::ComponentNotReady("core".to_string()))
    }

    /// Returns a reference to the vector engine, or an error if not started.
    pub fn vector(&self) -> ClawDBResult<Arc<claw_vector::VectorEngine>> {
        self.vector.clone().ok_or_else(|| ClawDBError::ComponentNotReady("vector".to_string()))
    }

    /// Returns a reference to the sync engine, or an error if not started.
    pub fn sync(&self) -> ClawDBResult<Arc<claw_sync::SyncEngine>> {
        self.sync.clone().ok_or_else(|| ClawDBError::ComponentNotReady("sync".to_string()))
    }

    /// Returns a reference to the branch engine, or an error if not started.
    pub fn branch(&self) -> ClawDBResult<Arc<claw_branch::BranchEngine>> {
        self.branch.clone().ok_or_else(|| ClawDBError::ComponentNotReady("branch".to_string()))
    }

    /// Returns a reference to the guard engine, or an error if not started.
    pub fn guard(&self) -> ClawDBResult<Arc<claw_guard::GuardEngine>> {
        self.guard.clone().ok_or_else(|| ClawDBError::ComponentNotReady("guard".to_string()))
    }

    /// Returns a reference to the reflect HTTP client, or an error if not started.
    pub fn reflect(&self) -> ClawDBResult<Arc<ReflectClient>> {
        self.reflect_client
            .clone()
            .ok_or_else(|| ClawDBError::ComponentNotReady("reflect".to_string()))
    }

    /// Builds and returns an aggregate `HealthReport`.
    pub async fn health_report(&self) -> HealthReport {
        let components = self.health.read().await.clone();
        let all_required_healthy = HealthReport::required_components()
            .iter()
            .all(|name| components.get(*name).map(|h| h.healthy).unwrap_or(false));

        let overall = if all_required_healthy {
            HealthStatus::Healthy
        } else {
            let failing: Vec<&str> = HealthReport::required_components()
                .iter()
                .filter(|&&name| !components.get(name).map(|h| h.healthy).unwrap_or(false))
                .copied()
                .collect();
            HealthStatus::Unhealthy {
                reason: format!("components down: {}", failing.join(", ")),
            }
        };

        HealthReport {
            overall,
            components,
            checked_at: chrono::Utc::now(),
            uptime_secs: self.started_at.elapsed().as_secs(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    async fn set_health(&self, name: &str, health: ComponentHealth) {
        self.health.write().await.insert(name.to_string(), health);
    }
}
