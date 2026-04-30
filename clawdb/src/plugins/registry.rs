//! `PluginRegistry`: stores and retrieves loaded plugin instances.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::{
    error::{ClawDBError, ClawDBResult},
    plugins::traits::ClawPlugin,
};

/// Central registry for all loaded ClawDB plugins.
pub struct PluginRegistry {
    plugins: RwLock<HashMap<String, Arc<dyn ClawPlugin>>>,
}

impl PluginRegistry {
    /// Creates an empty `PluginRegistry`.
    pub fn new() -> Self {
        Self {
            plugins: RwLock::new(HashMap::new()),
        }
    }

    /// Registers a plugin, replacing any existing plugin with the same name.
    pub fn register(&self, plugin: Arc<dyn ClawPlugin>) -> ClawDBResult<()> {
        let meta = plugin.meta();
        plugin.on_load()?;
        self.plugins
            .write()
            .expect("plugin registry write lock poisoned")
            .insert(meta.name.clone(), plugin);
        Ok(())
    }

    /// Unloads and removes a plugin by name.
    pub fn unload(&self, name: &str) -> ClawDBResult<()> {
        let plugin = self
            .plugins
            .write()
            .expect("plugin registry write lock poisoned")
            .remove(name)
            .ok_or_else(|| ClawDBError::PluginLoad {
                name: name.to_string(),
                reason: "plugin not found".to_string(),
            })?;
        plugin.on_unload()
    }

    /// Returns a list of loaded plugin names.
    pub fn list(&self) -> Vec<String> {
        self.plugins
            .read()
            .expect("plugin registry read lock poisoned")
            .keys()
            .cloned()
            .collect()
    }

    /// Invokes `f` for every registered plugin.
    pub fn for_each<F>(&self, mut f: F)
    where
        F: FnMut(&dyn ClawPlugin),
    {
        let guard = self
            .plugins
            .read()
            .expect("plugin registry read lock poisoned");
        for plugin in guard.values() {
            f(plugin.as_ref());
        }
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}
