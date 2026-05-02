//! Tracing initialization.

use crate::{config::TelemetryConfig, error::ClawDBResult};

/// Initializes tracing from the wrapper telemetry config.
pub fn init_telemetry(config: &TelemetryConfig) -> ClawDBResult<()> {
    init_tracing("info", "json");
    if let Some(endpoint) = &config.otel_endpoint {
        tracing::info!(endpoint = %endpoint, service_name = %config.service_name, "OTLP endpoint configured");
    }
    Ok(())
}

/// Initializes tracing with a level and format.
pub fn init_tracing(log_level: &str, log_format: &str) {
    use tracing_subscriber::{fmt, EnvFilter};

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(log_level));
    if log_format == "json" {
        let _ = fmt().json().with_env_filter(filter).try_init();
    } else {
        let _ = fmt().pretty().with_env_filter(filter).try_init();
    }
}

/// Compatibility helper used by CLI entrypoints.
pub fn init_tracing_simple(log_level: &str, log_format: &str) {
    init_tracing(log_level, log_format);
}
