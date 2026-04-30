//! `EventSubscriber`: typed subscription API for receiving ClawDB events.

use tokio::sync::broadcast;
use crate::events::types::ClawEvent;

/// A typed receiver of `ClawEvent` messages from the `EventBus`.
pub struct EventSubscriber {
    receiver: broadcast::Receiver<ClawEvent>,
}

impl EventSubscriber {
    /// Wraps an existing broadcast receiver as a typed subscriber.
    pub fn new(receiver: broadcast::Receiver<ClawEvent>) -> Self {
        Self { receiver }
    }

    /// Receives the next event, waiting asynchronously until one is available.
    pub async fn recv(&mut self) -> Option<ClawEvent> {
        loop {
            match self.receiver.recv().await {
                Ok(event) => return Some(event),
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => return None,
            }
        }
    }
}
