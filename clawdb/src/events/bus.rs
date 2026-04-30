//! `EventBus`: broadcast channel for publishing and subscribing to internal ClawDB events.

use tokio::sync::broadcast;
use crate::events::types::ClawEvent;

const BUS_CAPACITY: usize = 1024;

/// A broadcast channel hub for all internal ClawDB events.
#[derive(Debug, Clone)]
pub struct EventBus {
    sender: broadcast::Sender<ClawEvent>,
}

impl EventBus {
    /// Creates a new `EventBus` with the default channel capacity.
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(BUS_CAPACITY);
        Self { sender }
    }

    /// Publishes an event to all active subscribers; returns the number of receivers that got it.
    pub fn publish(&self, event: ClawEvent) -> usize {
        self.sender.send(event).unwrap_or(0)
    }

    /// Returns a new `broadcast::Receiver` that will receive future events.
    pub fn subscribe(&self) -> broadcast::Receiver<ClawEvent> {
        self.sender.subscribe()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}
