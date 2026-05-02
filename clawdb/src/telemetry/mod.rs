//! Telemetry helpers for `clawdb`.

pub mod metrics;
pub mod tracing_setup;

pub use metrics::{Metrics, PrometheusHandle};
pub use tracing_setup::{init_telemetry, init_tracing, init_tracing_simple};
