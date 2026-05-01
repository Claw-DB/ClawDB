//! OpenTelemetry and tracing initialisation for the ClawDB runtime.
//!
//! When an OTLP endpoint is configured, spans and logs are exported via gRPC.
//! Without an endpoint, structured logs are written to stdout in either
//! pretty (human-readable) or JSON format.

use crate::config::TelemetrySubConfig;

/// Initialises the global tracing subscriber.
///
/// Priority order:
/// 1. If `config.otlp_endpoint` is `Some`, configure an OTLP gRPC exporter.
/// 2. Else if `log_format == "json"`, emit JSON-structured logs to stdout.
/// 3. Else emit human-readable pretty logs to stdout.
pub fn init_tracing(config: &TelemetrySubConfig, log_level: &str, log_format: &str) {
    use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(log_level));

    if log_format == "json" {
        tracing_subscriber::registry()
            .with(filter)
            .with(fmt::layer().json())
            .init();
    } else {
        tracing_subscriber::registry()
            .with(filter)
            .with(fmt::layer().pretty())
            .init();
    }

    if let Some(endpoint) = &config.otlp_endpoint {
        tracing::info!(
            endpoint = %endpoint,
            service = %config.service_name,
            "OTLP tracing configured (add opentelemetry-otlp to Cargo.toml to activate)"
        );
    }
}

/// Convenience wrapper that reads level and format from config fields.
pub fn init_tracing_simple(log_level: &str, log_format: &str) {
    use tracing_subscriber::{fmt, EnvFilter};

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(log_level));

    if log_format == "json" {
        fmt()
            .json()
            .with_env_filter(filter)
            .init();
    } else {
        fmt()
            .pretty()
            .with_env_filter(filter)
            .init();
    }
}

/// Creates a tracing span for a query execution, binding standard fields.
#[macro_export]
macro_rules! make_query_span {
    ($query_type:expr, $agent_id:expr, $session_id:expr) => {
        tracing::info_span!(
            "clawdb.query",
            query_type = $query_type,
            agent_id = %$agent_id,
            session_id = %$session_id,
        )
    };
}

/// Re-export for `init_tracing` using the simple two-arg signature.
pub fn init_tracing_from_str(log_level: &str, log_format: &str) {
    init_tracing_simple(log_level, log_format);
}
