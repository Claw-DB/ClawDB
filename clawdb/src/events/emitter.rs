//! `EventEmitter`: thin helper for publishing events to the bus.

use std::sync::Arc;
use crate::events::{bus::EventBus, types::ClawEvent};

/// Helper for publishing events to the shared `EventBus`.
#[derive(Debug, Clone)]
pub struct EventEmitter {
    bus: Arc<EventBus>,
}

impl EventEmitter {
    /// Creates a new `EventEmitter` backed by the given `EventBus`.
    pub fn new(bus: Arc<EventBus>) -> Self {
        Self { bus }
    }

    /// Emits an event; silently drops it if there are no subscribers.
    pub fn emit(&self, event: ClawEvent) {
        let _ = self.bus.publish(event);
    }
}
