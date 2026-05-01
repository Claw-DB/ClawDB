//! `EventEmitter`: a convenience wrapper for components that publish events to the bus.
//!
//! Structs that need to emit events should hold an `Arc<EventEmitter>` rather than
//! an `Arc<EventBus>` directly; the emitter adds tracing context and typed helper
//! methods for every domain event.

use std::sync::Arc;

use uuid::Uuid;

use crate::events::{bus::EventBus, types::ClawEvent};

/// Convenience wrapper for publishing events from a named component.
///
/// Each call to an emitter method wraps the event in a tracing span so that
/// every published event is automatically correlated with the component that
/// produced it.
#[derive(Debug, Clone)]
pub struct EventEmitter {
    bus: Arc<EventBus>,
    /// The logical component name that will appear in tracing spans.
    component: &'static str,
}

impl EventEmitter {
    /// Creates a new `EventEmitter` backed by `bus`, tagged with `component`.
    pub fn new(bus: Arc<EventBus>, component: &'static str) -> Self {
        Self { bus, component }
    }

    /// Emits an arbitrary event, adding a tracing span with the component name.
    #[inline]
    pub fn emit(&self, event: ClawEvent) {
        let _span = tracing::debug_span!("event.emit",
            component = self.component,
            event_type = event.event_type()
        )
        .entered();
        self.bus.emit(event);
    }

    // ── Typed helper methods ─────────────────────────────────────────────────

    /// Emits a [`ClawEvent::MemoryAdded`] event.
    pub fn memory_added(&self, agent_id: Uuid, memory_id: Uuid, memory_type: &str) {
        self.emit(ClawEvent::MemoryAdded {
            agent_id,
            memory_id,
            memory_type: memory_type.to_string(),
        });
    }

    /// Emits a [`ClawEvent::SearchExecuted`] event.
    pub fn search_executed(
        &self,
        agent_id: Uuid,
        query_preview: &str,
        count: usize,
        latency_ms: u64,
    ) {
        self.emit(ClawEvent::SearchExecuted {
            agent_id,
            query_preview: query_preview
                .chars()
                .take(120)
                .collect::<String>(),
            result_count: count,
            latency_ms,
        });
    }

    /// Emits a [`ClawEvent::BranchCreated`] event.
    pub fn branch_created(&self, agent_id: Uuid, branch_id: Uuid, name: &str) {
        self.emit(ClawEvent::BranchCreated {
            agent_id,
            branch_id,
            name: name.to_string(),
        });
    }

    /// Emits a [`ClawEvent::SyncCompleted`] event.
    pub fn sync_completed(&self, agent_id: Uuid, pushed: u32, pulled: u32) {
        self.emit(ClawEvent::SyncCompleted {
            agent_id,
            pushed,
            pulled,
        });
    }

    /// Emits a [`ClawEvent::GuardDenied`] event.
    pub fn guard_denied(
        &self,
        agent_id: Uuid,
        action: &str,
        resource: &str,
        reason: &str,
    ) {
        self.emit(ClawEvent::GuardDenied {
            agent_id,
            action: action.to_string(),
            resource: resource.to_string(),
            reason: reason.to_string(),
        });
    }

    /// Emits a [`ClawEvent::ComponentHealthChanged`] event.
    pub fn component_health_changed(&self, component: &str, healthy: bool) {
        self.emit(ClawEvent::ComponentHealthChanged {
            component: component.to_string(),
            healthy,
        });
    }

    /// Returns a reference to the underlying [`EventBus`].
    pub fn bus(&self) -> &Arc<EventBus> {
        &self.bus
    }
}
