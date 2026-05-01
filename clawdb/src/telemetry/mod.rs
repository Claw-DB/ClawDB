//! Observability: structured logging, metrics, and distributed tracing.

pub mod metrics;
pub mod tracing_setup;

pub use metrics::Metrics;
pub use tracing_setup::init_tracing;

use std::sync::{Arc, Mutex};

use prometheus_client::registry::Registry;

/// Top-level telemetry handle: owns the Prometheus registry and the `Metrics`
/// instruments. Shared across the entire engine via `Arc`.
pub struct Telemetry {
	/// Prometheus instruments.
	pub metrics: Arc<Metrics>,
	/// Prometheus metric registry (used to render the `/metrics` endpoint).
	pub registry: Arc<Mutex<Registry>>,
}

impl Telemetry {
	/// Creates a new `Telemetry` instance and registers all instruments.
	pub fn new() -> Arc<Self> {
		let metrics = Metrics::new();
		let mut registry = Registry::default();
		metrics::init_metrics(&metrics, &mut registry);
		Arc::new(Self {
			metrics,
			registry: Arc::new(Mutex::new(registry)),
		})
	}

	/// Renders the Prometheus text format for all registered metrics.
	pub fn render(&self) -> String {
		let reg = self.registry.lock().expect("metrics registry lock poisoned");
		metrics::metrics_handler(&reg)
	}
}
