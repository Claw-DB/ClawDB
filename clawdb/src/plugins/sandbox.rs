//! `PluginSandbox`: enforces capability restrictions on plugin code paths.

use crate::{
    error::{ClawDBError, ClawDBResult},
    plugins::traits::PluginCapability,
};

/// Enforces capability-based access control for plugins.
pub struct PluginSandbox {
    enabled: bool,
}

impl PluginSandbox {
    /// Creates a new `PluginSandbox`.
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }

    /// Asserts that a plugin is allowed to exercise `capability`.
    ///
    /// When the sandbox is disabled, all capabilities are permitted.
    pub fn assert_capability(
        &self,
        plugin_name: &str,
        capability: &PluginCapability,
        granted: &[PluginCapability],
    ) -> ClawDBResult<()> {
        if !self.enabled {
            return Ok(());
        }
        if !granted.contains(capability) {
            return Err(ClawDBError::PluginCapabilityDenied {
                plugin: plugin_name.to_string(),
                capability: format!("{capability:?}"),
            });
        }
        Ok(())
    }

    /// Returns `true` if the sandbox is active.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}
