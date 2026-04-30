//! Application metrics counters and gauges.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Shared, atomically-updated runtime metrics.
#[derive(Debug, Default)]
pub struct Metrics {
    pub memories_stored: AtomicU64,
    pub searches_executed: AtomicU64,
    pub sessions_created: AtomicU64,
    pub syncs_completed: AtomicU64,
    pub reflections_completed: AtomicU64,
    pub guard_denials: AtomicU64,
    pub errors_total: AtomicU64,
}

impl Metrics {
    /// Creates a new zeroed `Metrics` instance.
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn inc_memories_stored(&self) {
        self.memories_stored.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_searches(&self) {
        self.searches_executed.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_sessions(&self) {
        self.sessions_created.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_syncs(&self) {
        self.syncs_completed.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_reflections(&self) {
        self.reflections_completed.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_guard_denials(&self) {
        self.guard_denials.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_errors(&self) {
        self.errors_total.fetch_add(1, Ordering::Relaxed);
    }

    /// Serialises the current counter values to JSON.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "memories_stored":       self.memories_stored.load(Ordering::Relaxed),
            "searches_executed":     self.searches_executed.load(Ordering::Relaxed),
            "sessions_created":      self.sessions_created.load(Ordering::Relaxed),
            "syncs_completed":       self.syncs_completed.load(Ordering::Relaxed),
            "reflections_completed": self.reflections_completed.load(Ordering::Relaxed),
            "guard_denials":         self.guard_denials.load(Ordering::Relaxed),
            "errors_total":          self.errors_total.load(Ordering::Relaxed),
        })
    }
}
