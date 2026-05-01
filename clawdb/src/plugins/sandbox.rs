//! `PluginSandbox`: validates capability declarations and enforces allowlists.
//!
//! The sandbox sits between the manifest parser and the plugin loader.  Before
//! a plugin's shared library is even loaded, the sandbox checks that:
//!
//! 1. Every capability in the manifest is on the allowlist.
//! 2. No dangerous *combination* of capabilities is present (e.g. WriteMemory +
//!    AccessGuard would let a plugin bypass guard policies).

use crate::{
    error::{ClawDBError, ClawDBResult},
    plugins::interface::{PluginCapability, PluginManifest},
};

/// Enforces capability-based access control for plugins.
pub struct PluginSandbox {
    enabled: bool,
    /// Capabilities that are permitted when the sandbox is active.
    /// An empty allowlist permits nothing.
    allowlist: Vec<PluginCapability>,
}

impl PluginSandbox {
    /// Creates a sandbox with the default allowlist (all safe capabilities).
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            allowlist: vec![
                PluginCapability::ReadMemory,
                PluginCapability::ModifySearchResults,
                PluginCapability::HookTransactions,
                PluginCapability::EmitEvents,
            ],
        }
    }

    /// Creates a sandbox with an explicit custom allowlist.
    pub fn with_allowlist(enabled: bool, allowlist: Vec<PluginCapability>) -> Self {
        Self { enabled, allowlist }
    }

    // ── Validation ────────────────────────────────────────────────────────────

    /// Validates a plugin manifest's capabilities against the sandbox allowlist.
    ///
    /// Returns `Ok(())` if all capabilities are permitted, or an error naming
    /// the first denied capability.
    pub fn validate_capabilities(&self, manifest: &PluginManifest) -> ClawDBResult<()> {
        if !self.enabled {
            return Ok(());
        }

        for cap in &manifest.capabilities {
            if !self.allowlist.contains(cap) {
                return Err(ClawDBError::PluginCapabilityDenied {
                    plugin: manifest.name.clone(),
                    capability: format!("{cap:?}"),
                });
            }
        }

        // Reject dangerous combinations.
        self.reject_dangerous_combinations(&manifest.name, &manifest.capabilities)?;

        Ok(())
    }

    fn reject_dangerous_combinations(
        &self,
        plugin_name: &str,
        capabilities: &[PluginCapability],
    ) -> ClawDBResult<()> {
        let has_write = capabilities.contains(&PluginCapability::WriteMemory);
        let has_guard = capabilities.contains(&PluginCapability::AccessGuard);
        let has_sync = capabilities.contains(&PluginCapability::AccessSync);

        // WriteMemory + AccessGuard could be used to exfiltrate data while
        // circumventing policy checks.
        if has_write && has_guard {
            return Err(ClawDBError::PluginCapabilityDenied {
                plugin: plugin_name.to_string(),
                capability: "WriteMemory+AccessGuard (dangerous combination)".to_string(),
            });
        }

        // WriteMemory + AccessSync could be used to push malicious data to peers.
        if has_write && has_sync {
            return Err(ClawDBError::PluginCapabilityDenied {
                plugin: plugin_name.to_string(),
                capability: "WriteMemory+AccessSync (dangerous combination)".to_string(),
            });
        }

        Ok(())
    }

    // ── Legacy compat (traits.rs API) ─────────────────────────────────────────

    /// Legacy: checks whether `plugin_name` is allowed to use `capability` given
    /// its `granted` capability list.
    pub fn assert_capability(
        &self,
        plugin_name: &str,
        capability: &crate::plugins::traits::PluginCapability,
        granted: &[crate::plugins::traits::PluginCapability],
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
