//! Tracing initialisation for the ClawDB runtime.

/// Initialises the global tracing subscriber.
///
/// Respects the `log_level` and `log_format` fields from config.
pub fn init_tracing(log_level: &str, log_format: &str) {
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
