//! Plugin system: loading, sandboxing, and invoking extension plugins.

pub mod registry;
pub mod sandbox;
pub mod traits;

pub use registry::PluginRegistry;
pub use sandbox::PluginSandbox;
pub use traits::ClawPlugin;
