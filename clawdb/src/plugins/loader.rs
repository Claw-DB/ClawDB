//! `PluginLoader`: discovers and loads plugin shared libraries from a directory.
//!
//! # Directory layout
//! ```text
//! plugins/
//!   my-plugin/
//!     manifest.toml   ← required
//!     my-plugin.so    ← (or .dylib on macOS, .dll on Windows)
//! ```
//!
//! The loader:
//! 1. Reads each subdirectory looking for `manifest.toml`.
//! 2. Filters to the `enabled` list from config.
//! 3. Validates the manifest against the [`PluginSandbox`] allowlist.
//! 4. Opens the shared library with [`libloading`] and calls the `entry_symbol`
//!    to obtain a `Box<dyn ClawPlugin>`.
//! 5. Returns `(PluginManifest, Box<dyn ClawPlugin>)` for each successfully loaded plugin.
//!
//! # Safety
//! Dynamic library loading is inherently unsafe.  The plugin's ABI must match the
//! host exactly (same `clawdb` crate version, same compiler).

use std::path::Path;
use std::sync::Arc;

use crate::{
    error::{ClawDBError, ClawDBResult},
    plugins::{
        interface::{ClawPlugin, PluginManifest},
        sandbox::PluginSandbox,
    },
};

/// Discovers and loads plugin shared libraries.
pub struct PluginLoader {
    sandbox: Arc<PluginSandbox>,
}

impl PluginLoader {
    /// Creates a new `PluginLoader` that enforces the given sandbox rules.
    pub fn new(sandbox: Arc<PluginSandbox>) -> Self {
        Self { sandbox }
    }

    /// Scans `plugins_dir` and loads every subdirectory whose name appears in
    /// `enabled`.
    ///
    /// Returns a list of `(manifest, plugin)` pairs that are ready to be
    /// registered with [`PluginRegistry`](super::registry::PluginRegistry).
    pub async fn load_from_dir(
        &self,
        plugins_dir: &Path,
        enabled: &[String],
    ) -> ClawDBResult<Vec<(PluginManifest, Box<dyn ClawPlugin>)>> {
        if !plugins_dir.exists() {
            tracing::debug!(
                path = %plugins_dir.display(),
                "plugins directory does not exist; skipping plugin loading"
            );
            return Ok(vec![]);
        }

        let mut loaded = Vec::new();

        let mut read_dir = tokio::fs::read_dir(plugins_dir)
            .await
            .map_err(|e| ClawDBError::Io(e))?;

        while let Some(entry) = read_dir
            .next_entry()
            .await
            .map_err(|e| ClawDBError::Io(e))?
        {
            let dir_path = entry.path();
            if !dir_path.is_dir() {
                continue;
            }

            let dir_name = dir_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();

            if !enabled.is_empty() && !enabled.contains(&dir_name) {
                tracing::debug!(plugin = %dir_name, "skipping disabled plugin");
                continue;
            }

            match self.load_one(&dir_path).await {
                Ok((manifest, plugin)) => {
                    tracing::info!(plugin = %manifest.name, version = %manifest.version, "plugin loaded");
                    loaded.push((manifest, plugin));
                }
                Err(e) => {
                    tracing::error!(plugin = %dir_name, err = %e, "failed to load plugin");
                    // Non-fatal: log and continue.
                }
            }
        }

        Ok(loaded)
    }

    async fn load_one(
        &self,
        plugin_dir: &Path,
    ) -> ClawDBResult<(PluginManifest, Box<dyn ClawPlugin>)> {
        // Read manifest.
        let manifest_path = plugin_dir.join("manifest.toml");
        let manifest_str = tokio::fs::read_to_string(&manifest_path)
            .await
            .map_err(|e| ClawDBError::PluginLoad {
                name: plugin_dir.display().to_string(),
                reason: format!("cannot read manifest.toml: {e}"),
            })?;

        let manifest: PluginManifest =
            toml::from_str(&manifest_str).map_err(|e| ClawDBError::PluginLoad {
                name: plugin_dir.display().to_string(),
                reason: format!("manifest parse error: {e}"),
            })?;

        // Validate capabilities before loading the library.
        self.sandbox.validate_capabilities(&manifest)?;

        // Find the shared library.
        let lib_path = self.find_library(plugin_dir, &manifest.name)?;

        // Load the library and call the entry symbol.
        // SAFETY: We verify the manifest first.  The caller is responsible for
        //         ensuring the library ABI matches the host.
        let plugin = unsafe { self.load_library(&lib_path, &manifest)? };

        Ok((manifest, plugin))
    }

    fn find_library(&self, plugin_dir: &Path, name: &str) -> ClawDBResult<std::path::PathBuf> {
        // Platform-specific extension.
        let extensions: &[&str] = if cfg!(target_os = "macos") {
            &["dylib", "so"]
        } else if cfg!(windows) {
            &["dll"]
        } else {
            &["so"]
        };

        for ext in extensions {
            let candidate = plugin_dir.join(format!("{name}.{ext}"));
            if candidate.exists() {
                return Ok(candidate);
            }
            // Also try lib prefix on Unix.
            let candidate = plugin_dir.join(format!("lib{name}.{ext}"));
            if candidate.exists() {
                return Ok(candidate);
            }
        }

        Err(ClawDBError::PluginLoad {
            name: name.to_string(),
            reason: format!(
                "no shared library found in {}",
                plugin_dir.display()
            ),
        })
    }

    /// # Safety
    /// The caller must ensure the library implements the correct `ClawPlugin` ABI.
    unsafe fn load_library(
        &self,
        lib_path: &Path,
        manifest: &PluginManifest,
    ) -> ClawDBResult<Box<dyn ClawPlugin>> {
        // Load the shared object.
        let lib = libloading::Library::new(lib_path).map_err(|e| {
            ClawDBError::PluginLoad {
                name: manifest.name.clone(),
                reason: format!("dlopen failed: {e}"),
            }
        })?;

        // Resolve the entry symbol.
        type CreatePlugin = unsafe extern "C" fn() -> *mut dyn ClawPlugin;
        let create_fn: libloading::Symbol<CreatePlugin> = lib
            .get(manifest.entry_symbol.as_bytes())
            .map_err(|e| ClawDBError::PluginLoad {
                name: manifest.name.clone(),
                reason: format!("entry symbol '{}' not found: {e}", manifest.entry_symbol),
            })?;

        let raw = create_fn();
        if raw.is_null() {
            return Err(ClawDBError::PluginLoad {
                name: manifest.name.clone(),
                reason: "entry symbol returned null".to_string(),
            });
        }

        // Convert raw pointer to Box.  The library is intentionally leaked so it
        // remains loaded for the lifetime of the plugin.
        std::mem::forget(lib);
        let plugin = Box::from_raw(raw);

        Ok(plugin)
    }
}
