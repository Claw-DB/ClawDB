//! Plugin interface.

use async_trait::async_trait;

use crate::{error::ClawDBResult, plugins::events::ClawEvent};

/// Trait implemented by dynamic ClawDB plugins.
#[async_trait]
pub trait ClawPlugin: Send + Sync {
    /// Returns the plugin name.
    fn name(&self) -> &str;

    /// Handles an emitted wrapper event.
    async fn on_event(&mut self, _event: &ClawEvent) -> ClawDBResult<()> {
        Ok(())
    }
}
