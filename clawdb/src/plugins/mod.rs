//! Plugin system: loading, sandboxing, and invoking extension plugins.

pub mod interface;
pub mod loader;
pub mod registry;
pub mod sandbox;
pub mod traits;

pub use interface::{ClawPlugin, PluginCapability, PluginContext, PluginManifest};
pub use loader::PluginLoader;
pub use registry::PluginRegistry;
pub use sandbox::PluginSandbox;
pub use traits::ClawPlugin as LegacyClawPlugin;
