//! `ClawPlugin` trait: the interface every ClawDB plugin must implement.

use crate::error::ClawDBResult;

/// The set of capabilities a plugin may request.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PluginCapability {
    /// Read access to memory records.
    MemoryRead,
    /// Write access to memory records.
    MemoryWrite,
    /// Access to the event bus.
    EventBus,
    /// Outbound network access.
    Network,
}

/// Metadata returned by a plugin at load time.
#[derive(Debug, Clone)]
pub struct PluginMeta {
    pub name: String,
    pub version: String,
    pub description: String,
    pub capabilities: Vec<PluginCapability>,
}

/// The interface every ClawDB plugin must implement.
pub trait ClawPlugin: Send + Sync + 'static {
    /// Returns static metadata describing this plugin.
    fn meta(&self) -> PluginMeta;

    /// Called once after the plugin is loaded; perform any initialisation here.
    fn on_load(&self) -> ClawDBResult<()> {
        Ok(())
    }

    /// Called once before the plugin is unloaded; perform cleanup here.
    fn on_unload(&self) -> ClawDBResult<()> {
        Ok(())
    }

    /// Called when a memory entry is stored.
    fn on_memory_added(&self, _memory_id: &str, _content: &str) -> ClawDBResult<()> {
        Ok(())
    }

    /// Called when a search is executed; the plugin may augment `results`.
    fn on_search_complete(
        &self,
        _query: &str,
        _results: &mut Vec<serde_json::Value>,
    ) -> ClawDBResult<()> {
        Ok(())
    }
}
