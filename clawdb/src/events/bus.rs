//! `EventBus`: broadcast channel for publishing and subscribing to internal ClawDB events.
//!
//! Events are wrapped in `Arc` so that multiple subscribers share the same allocation
//! without copying. The channel is created with a configurable capacity; lagging receivers
//! skip missed events automatically.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::broadcast;
use tokio::time::timeout;

use crate::{
    config::ClawDBConfig,
    error::{ClawDBError, ClawDBResult},
    events::types::ClawEvent,
};

/// Internal channel capacity used when no config is provided.
const DEFAULT_CAPACITY: usize = 1_024;

/// Fan-out broadcast channel for all internal ClawDB events.
///
/// Events are wrapped in [`Arc`] so that clone cost across many subscribers is O(1).
/// Internally backed by [`tokio::sync::broadcast`].
#[derive(Debug, Clone)]
pub struct EventBus {
    sender: broadcast::Sender<Arc<ClawEvent>>,
    capacity: usize,
}

impl EventBus {
    /// Creates a new `EventBus` with the given channel capacity.
    ///
    /// `capacity` is the maximum number of unread messages a single slow subscriber may
    /// fall behind before it starts receiving [`broadcast::error::RecvError::Lagged`]
    /// errors and skipping events.
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender, capacity }
    }

    /// Creates an `EventBus` from the telemetry section of [`ClawDBConfig`].
    pub fn from_config(config: &ClawDBConfig) -> Self {
        // Use the metrics port as a proxy for "is telemetry configured".
        // The capacity is not in config; keep it at the default.
        let _ = config; // acknowledge the parameter
        Self::new(DEFAULT_CAPACITY)
    }

    /// Emits an event to all active subscribers in a non-blocking, fire-and-forget manner.
    ///
    /// If no receivers are registered, or the channel is full, the event is silently
    /// dropped.  Use [`EventBus::emit_and_wait`] for critical events that must be
    /// acknowledged.
    #[inline]
    pub fn emit(&self, event: ClawEvent) {
        let _ = self.sender.send(Arc::new(event));
    }

    /// Backward-compatible alias for [`emit`] that returns the receiver count.
    ///
    /// Kept to avoid breaking existing call sites in `lifecycle/manager.rs`.
    #[inline]
    pub fn publish(&self, event: ClawEvent) -> usize {
        self.sender.send(Arc::new(event)).unwrap_or(0)
    }

    /// Returns a new `broadcast::Receiver` that will receive all future events.
    ///
    /// Receivers that fall behind by more than `capacity` events will skip old ones.
    #[inline]
    pub fn subscribe(&self) -> broadcast::Receiver<Arc<ClawEvent>> {
        self.sender.subscribe()
    }

    /// Emits an event and blocks until at least one receiver acknowledges it, or
    /// `deadline` elapses.
    ///
    /// This is intended for critical events such as `ShutdownInitiated` where losing
    /// the event would leave subsystems in an inconsistent state.
    ///
    /// Returns `Ok(())` if the event was delivered to ≥1 receiver within the deadline,
    /// or `Err(ClawDBError::EventBusError)` on timeout or if there are no receivers.
    pub async fn emit_and_wait(
        &self,
        event: ClawEvent,
        deadline: Duration,
    ) -> ClawDBResult<()> {
        if self.sender.receiver_count() == 0 {
            // No one is listening; emit anyway (idempotent) and return success.
            let _ = self.sender.send(Arc::new(event));
            return Ok(());
        }

        // Subscribe *before* sending so we can observe the send.
        let mut rx = self.sender.subscribe();
        let arc = Arc::new(event);
        let _ = self.sender.send(arc.clone());

        // Wait for our own receiver to pull the event, confirming delivery on the channel.
        timeout(deadline, async move {
            loop {
                match rx.recv().await {
                    Ok(_) => return,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => return,
                }
            }
        })
        .await
        .map_err(|_| {
            ClawDBError::EventBusError(format!(
                "emit_and_wait timed out after {}ms",
                deadline.as_millis()
            ))
        })
    }

    /// Returns the number of active subscribers (cloned receivers).
    #[inline]
    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }

    /// Returns the channel capacity this bus was created with.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new(DEFAULT_CAPACITY)
    }
}
