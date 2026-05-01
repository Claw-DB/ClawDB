//! Stable plugin ABI: the traits and types that every ClawDB plugin must implement.
//!
//! Plugin authors depend on this module.  Changes here must maintain backward
//! compatibility; any breaking change requires a major version bump of the
//! `clawdb` crate.

use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    error::ClawDBResult,
    events::{emitter::EventEmitter, types::ClawEvent},
};

// ── PluginCapability ──────────────────────────────────────────────────────────

/// Capabilities a plugin may declare in its manifest.
///
/// The [`PluginSandbox`](super::sandbox::PluginSandbox) validates these against
/// an allowlist before the plugin is loaded.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum PluginCapability {
    /// Read stored memory entries.
    ReadMemory,
    /// Write or modify memory entries.
    WriteMemory,
    /// Intercept and augment search results.
    ModifySearchResults,
    /// Hook into the transaction lifecycle.
    HookTransactions,
    /// Emit events onto the internal event bus.
    EmitEvents,
    /// Access the guard engine for policy decisions.
    AccessGuard,
    /// Access the sync engine.
    AccessSync,
}

// ── PluginManifest ────────────────────────────────────────────────────────────

/// Static metadata declared by a plugin.
///
/// Serialised from `manifest.toml` in the plugin directory and validated
/// at load time by [`super::loader::PluginLoader`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    /// Human-readable plugin name (e.g. `"sentiment-tagger"`).
    pub name: String,
    /// Semantic version string (e.g. `"1.0.0"`).
    pub version: String,
    /// One-line description.
    pub description: String,
    /// Capabilities this plugin requires.
    pub capabilities: Vec<PluginCapability>,
    /// Minimum ClawDB version required (semver, inclusive).
    pub min_clawdb_version: String,
    /// Symbol name to call in the shared library to obtain a `Box<dyn ClawPlugin>`.
    ///
    /// Convention: `create_plugin`.
    pub entry_symbol: String,
}

// ── PluginContext ─────────────────────────────────────────────────────────────

/// Runtime context injected into a plugin when it is loaded.
///
/// Gives the plugin controlled access to ClawDB's subsystems.
pub struct PluginContext {
    /// Config blob specific to this plugin, sourced from the ClawDB config file.
    pub config: serde_json::Value,
    /// An event emitter the plugin can use to publish events.
    pub event_emitter: Arc<EventEmitter>,
}

// ── ClawPlugin ────────────────────────────────────────────────────────────────

/// The trait every ClawDB plugin must implement.
///
/// All hook methods have default no-op implementations so that plugins only
/// need to override the hooks they care about.
///
/// # Safety
/// Plugins loaded via the dynamic library path must be compiled against the same
/// version of the `clawdb` crate as the host to ensure ABI compatibility.
#[async_trait]
pub trait ClawPlugin: Send + Sync {
    /// Returns the plugin's human-readable name.  Must match `manifest.name`.
    fn name(&self) -> &str;

    /// Returns the plugin's version string.  Must match `manifest.version`.
    fn version(&self) -> &str;

    /// Returns the capabilities this plugin actually uses at runtime.
    fn capabilities(&self) -> Vec<PluginCapability>;

    /// Called exactly once after the plugin is registered.  Perform one-time
    /// initialisation (open files, set up state, etc.) here.
    async fn on_load(&mut self, ctx: PluginContext) -> ClawDBResult<()>;

    /// Called exactly once before the plugin is unregistered.  Release all
    /// resources here.
    async fn on_unload(&mut self) -> ClawDBResult<()>;

    // ── Optional hooks ────────────────────────────────────────────────────────

    /// Called after a memory entry is persisted.
    ///
    /// `memory` is the raw JSON representation of the entry.
    async fn on_memory_added(
        &self,
        memory: &serde_json::Value,
    ) -> ClawDBResult<()> {
        let _ = memory;
        Ok(())
    }

    /// Called after a search completes, before results are returned to the caller.
    ///
    /// The plugin may inspect, reorder, or augment the `results` slice.
    async fn on_search_result(
        &self,
        results: &mut Vec<serde_json::Value>,
    ) -> ClawDBResult<()> {
        let _ = results;
        Ok(())
    }

    /// Called during phase-1 (prepare) of a transaction commit.
    ///
    /// Return an error to veto the commit.
    async fn on_before_commit(&self, tx_id: Uuid) -> ClawDBResult<()> {
        let _ = tx_id;
        Ok(())
    }

    /// Called for every event emitted on the internal event bus.
    async fn on_event(&self, event: &ClawEvent) -> ClawDBResult<()> {
        let _ = event;
        Ok(())
    }
}
