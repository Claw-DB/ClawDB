//! `EventSubscriber`: typed subscription API for receiving `ClawEvent` messages.
//!
//! Wraps a [`tokio::sync::broadcast::Receiver`] and provides:
//! - Async `recv` / `recv_matching` helpers.
//! - Conversion to a [`futures::Stream`] via
//!   [`tokio_stream::wrappers::BroadcastStream`].

use std::sync::Arc;
use std::time::Duration;

use futures::Stream;
use tokio::sync::broadcast;
use tokio::time::timeout;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt as _;

use crate::{
    error::{ClawDBError, ClawDBResult},
    events::types::ClawEvent,
};

/// A typed receiver of [`Arc<ClawEvent>`] messages from the [`super::EventBus`].
///
/// Create one by calling [`super::EventBus::subscribe`] and wrapping the result with
/// [`EventSubscriber::new`].
pub struct EventSubscriber {
    receiver: broadcast::Receiver<Arc<ClawEvent>>,
}

impl EventSubscriber {
    /// Wraps a raw broadcast receiver as a typed `EventSubscriber`.
    pub fn new(receiver: broadcast::Receiver<Arc<ClawEvent>>) -> Self {
        Self { receiver }
    }

    /// Receives the next event from the bus, waiting asynchronously until one arrives.
    ///
    /// Lagged events (channel overflows) are skipped transparently.
    ///
    /// # Errors
    /// Returns [`ClawDBError::EventBusError`] if the sending side of the channel is
    /// closed (i.e., the `EventBus` was dropped).
    pub async fn recv(&mut self) -> ClawDBResult<Arc<ClawEvent>> {
        loop {
            match self.receiver.recv().await {
                Ok(event) => return Ok(event),
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    tracing::debug!(skipped, "EventSubscriber lagged, skipping events");
                    continue;
                }
                Err(broadcast::error::RecvError::Closed) => {
                    return Err(ClawDBError::EventBusError(
                        "event bus channel closed".to_string(),
                    ));
                }
            }
        }
    }

    /// Waits up to `deadline` for the next event that satisfies `predicate`.
    ///
    /// Returns `Ok(Some(event))` if a matching event arrives within the deadline,
    /// `Ok(None)` on timeout, or an error if the channel is closed.
    ///
    /// # Type parameters
    /// * `F` — a synchronous predicate `fn(&ClawEvent) -> bool`.
    pub async fn recv_matching<F>(
        &mut self,
        predicate: F,
        deadline: Duration,
    ) -> ClawDBResult<Option<Arc<ClawEvent>>>
    where
        F: Fn(&ClawEvent) -> bool,
    {
        let result = timeout(deadline, async {
            loop {
                let event = self.recv().await?;
                if predicate(&event) {
                    return Ok::<_, ClawDBError>(event);
                }
            }
        })
        .await;

        match result {
            Ok(Ok(event)) => Ok(Some(event)),
            Ok(Err(e)) => Err(e),
            Err(_elapsed) => Ok(None),
        }
    }

    /// Converts this subscriber into an infinite [`Stream`] of `Arc<ClawEvent>`.
    ///
    /// Lagged events are filtered out silently (they arrive as `Err(Lagged)` from
    /// `BroadcastStream` and are mapped to `None`).
    pub fn into_stream(self) -> impl Stream<Item = Arc<ClawEvent>> {
        BroadcastStream::new(self.receiver).filter_map(|res| res.ok())
    }
}
