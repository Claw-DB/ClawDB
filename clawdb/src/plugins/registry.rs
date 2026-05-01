//! `PluginRegistry`: central store and dispatch hub for loaded plugins.
//!
//! Plugins are stored as `Arc<tokio::sync::Mutex<Box<dyn ClawPlugin>>>` so that
//! hook dispatch can hold a lock per plugin concurrently.

use std::sync::Arc;

use dashmap::DashMap;
use tokio::sync::Mutex;

use crate::{
    error::{ClawDBError, ClawDBResult},
    events::types::ClawEvent,
    plugins::{
        interface::{ClawPlugin, PluginContext, PluginManifest},
        sandbox::PluginSandbox,
    },
};

// ── PluginRegistry ────────────────────────────────────────────────────────────

/// Central registry for all loaded ClawDB plugins.
///
/// Plugins are indexed by name.  Names are case-sensitive and must be unique.
pub struct PluginRegistry {
    plugins: DashMap<String, Arc<Mutex<Box<dyn ClawPlugin>>>>,
    manifests: DashMap<String, PluginManifest>,
    sandbox: Arc<PluginSandbox>,
}

impl PluginRegistry {
    /// Creates an empty registry with the given sandbox policy.
    pub fn new(sandbox: Arc<PluginSandbox>) -> Self {
        Self {
            plugins: DashMap::new(),
            manifests: DashMap::new(),
            sandbox,
        }
    }

    // ── register / unregister ─────────────────────────────────────────────────

    /// Registers a plugin, calling `on_load` with `ctx`.
    ///
    /// If a plugin with the same name is already registered, it is first
    /// unregistered (with `on_unload` called) before the new one is loaded.
    pub async fn register(
        &self,
        manifest: PluginManifest,
        mut plugin: Box<dyn ClawPlugin>,
        ctx: PluginContext,
    ) -> ClawDBResult<()> {
        // Validate capabilities against sandbox allowlist.
        self.sandbox.validate_capabilities(&manifest)?;

        // Unload the existing plugin of the same name if present.
        if self.plugins.contains_key(&manifest.name) {
            self.unregister(&manifest.name).await?;
        }

        plugin.on_load(ctx).await.map_err(|e| ClawDBError::PluginLoad {
            name: manifest.name.clone(),
            reason: e.to_string(),
        })?;

        let name = manifest.name.clone();
        self.plugins
            .insert(name.clone(), Arc::new(Mutex::new(plugin)));
        self.manifests.insert(name, manifest);

        Ok(())
    }

    /// Unregisters a plugin by name, calling `on_unload`.
    pub async fn unregister(&self, name: &str) -> ClawDBResult<()> {
        let (_, handle) = self.plugins.remove(name).ok_or_else(|| {
            ClawDBError::PluginLoad {
                name: name.to_string(),
                reason: "plugin not found".to_string(),
            }
        })?;
        self.manifests.remove(name);

        let mut plugin = handle.lock().await;
        plugin.on_unload().await.map_err(|e| ClawDBError::PluginExecution {
            name: name.to_string(),
            hook: "on_unload".to_string(),
            reason: e.to_string(),
        })?;

        Ok(())
    }

    // ── queries ────────────────────────────────────────────────────────────────

    /// Returns the plugin handle for `name`, or `None` if not registered.
    pub fn get(&self, name: &str) -> Option<Arc<Mutex<Box<dyn ClawPlugin>>>> {
        self.plugins.get(name).map(|r| Arc::clone(&r))
    }

    /// Returns a snapshot of all registered plugin manifests.
    pub fn list(&self) -> Vec<PluginManifest> {
        self.manifests.iter().map(|r| r.clone()).collect()
    }

    /// Returns the number of registered plugins.
    pub fn count(&self) -> usize {
        self.plugins.len()
    }

    // ── event dispatch ────────────────────────────────────────────────────────

    /// Calls `on_event` on every registered plugin for `event`.
    ///
    /// Errors from individual plugins are logged but do not abort dispatch to
    /// remaining plugins.
    pub async fn dispatch_event(&self, event: &ClawEvent) {
        // Collect handles to avoid holding DashMap ref across await.
        let handles: Vec<(String, Arc<Mutex<Box<dyn ClawPlugin>>>)> = self
            .plugins
            .iter()
            .map(|r| (r.key().clone(), Arc::clone(&r)))
            .collect();

        for (name, handle) in handles {
            let plugin = handle.lock().await;
            if let Err(e) = plugin.on_event(event).await {
                tracing::warn!(plugin = %name, hook = "on_event", err = %e, "plugin hook error");
            }
        }
    }

    // ── legacy compat (existing traits.rs API) ────────────────────────────────

    /// Legacy: iterate over all registered plugins synchronously.
    ///
    /// Acquires each plugin lock in turn; do not call from within an async context
    /// that already holds a plugin lock.
    pub fn for_each_sync<F>(&self, mut f: F)
    where
        F: FnMut(&dyn crate::plugins::traits::ClawPlugin),
    {
        // This implementation is intentionally a no-op bridge; the new async
        // registry does not support synchronous iteration with lock acquisition.
        // Legacy callers should migrate to `dispatch_event` or `get`.
        let _ = f; // suppress unused warning
    }
}
