//! Minimal plugin system.

pub mod events;
pub mod interface;
pub mod manager;

pub use events::ClawEvent;
pub use interface::ClawPlugin;
pub use manager::PluginManager;
