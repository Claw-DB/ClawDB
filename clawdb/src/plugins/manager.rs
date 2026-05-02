//! Plugin manager for dynamic libraries.

use std::path::Path;

use tokio::sync::broadcast;

use crate::{
    error::ClawDBResult,
    plugins::{events::ClawEvent, interface::ClawPlugin},
};

/// Dynamic plugin manager.
pub struct PluginManager {
    plugins: Vec<Box<dyn ClawPlugin>>,
    event_tx: broadcast::Sender<ClawEvent>,
}

impl PluginManager {
    /// Creates a new plugin manager and an event receiver.
    pub fn new() -> (Self, broadcast::Receiver<ClawEvent>) {
        let (event_tx, event_rx) = broadcast::channel(256);
        (
            Self {
                plugins: Vec::new(),
                event_tx,
            },
            event_rx,
        )
    }

    /// Loads all supported shared libraries from `dir`.
    ///
    /// Safety invariant: the loaded library must export a `clawdb_create_plugin`
    /// symbol with signature `fn() -> Box<dyn ClawPlugin>`. Libraries are leaked
    /// after loading so plugin vtables remain valid for the lifetime of the process.
    pub fn load_from_dir(&mut self, dir: &Path) -> ClawDBResult<usize> {
        if !dir.exists() {
            return Ok(0);
        }

        let mut loaded = 0usize;
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            if !is_dynamic_library(&path) {
                continue;
            }
            // SAFETY: The caller controls the plugin directory and we validate the symbol name.
            unsafe {
                let library = libloading::Library::new(&path).map_err(|error| {
                    crate::error::ClawDBError::ComponentInit("plugin", error.to_string())
                })?;
                let constructor: libloading::Symbol<unsafe fn() -> Box<dyn ClawPlugin>> =
                    library.get(b"clawdb_create_plugin").map_err(|error| {
                        crate::error::ClawDBError::ComponentInit("plugin", error.to_string())
                    })?;
                let plugin = constructor();
                self.plugins.push(plugin);
                std::mem::forget(library);
                loaded += 1;
            }
        }

        Ok(loaded)
    }

    /// Emits an event to subscribed listeners.
    pub fn emit(&self, event: ClawEvent) {
        let _ = self.event_tx.send(event);
    }

    /// Dispatches an event to all loaded plugins.
    pub async fn dispatch(&mut self, event: &ClawEvent) {
        for plugin in &mut self.plugins {
            let _ = plugin.on_event(event).await;
        }
    }
}

fn is_dynamic_library(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|value| value.to_str()),
        Some("so") | Some("dylib") | Some("dll")
    )
}
