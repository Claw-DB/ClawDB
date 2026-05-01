//! Prometheus metrics for the ClawDB runtime.
//!
//! All instruments are registered in a shared [`prometheus_client::registry::Registry`]
//! via [`init_metrics`].  The rendered text format is returned by [`metrics_handler`].
//! An optional HTTP server exposing `/metrics` can be started with [`serve_metrics`].
//!
//! # Instruments
//!
//! | Kind        | Name                                | Labels                              |
//! |:----------- |:----------------------------------- |:----------------------------------- |
//! | Counter     | `queries_total`                     | `query_type`, `component`, `status` |
//! | Counter     | `events_emitted_total`              | `event_type`                        |
//! | Counter     | `plugin_hooks_total`                | `plugin_name`, `hook_name`, `status`|
//! | Counter     | `transactions_total`                | `status`                            |
//! | Counter     | `sessions_created_total`            | `role`                              |
//! | Counter     | `cache_hits_total`                  | `cache_name`                        |
//! | Counter     | `cache_misses_total`                | `cache_name`                        |
//! | Histogram   | `query_duration_seconds`            | `query_type`, `component`           |
//! | Histogram   | `transaction_duration_seconds`      | (none)                              |
//! | Histogram   | `plugin_hook_duration_seconds`      | `plugin_name`, `hook_name`          |
//! | Gauge       | `active_sessions_total`             | (none)                              |
//! | Gauge       | `active_transactions_total`         | (none)                              |
//! | Gauge       | `plugin_count`                      | (none)                              |
//! | Gauge       | `component_healthy`                 | `component`                         |

use std::sync::Arc;

use prometheus_client::{
    encoding::text::encode,
    metrics::{
        counter::Counter,
        family::Family,
        gauge::Gauge,
        histogram::{exponential_buckets, Histogram},
    },
    registry::Registry,
};

// ── Label types ─────────────────────────────────────────────────────────────

/// Labels for `queries_total` and `query_duration_seconds`.
#[derive(Clone, Debug, Hash, PartialEq, Eq, prometheus_client::encoding::EncodeLabelSet)]
pub struct QueryLabels {
    pub query_type: String,
    pub component: String,
    pub status: String,
}

/// Labels for `events_emitted_total`.
#[derive(Clone, Debug, Hash, PartialEq, Eq, prometheus_client::encoding::EncodeLabelSet)]
pub struct EventLabels {
    pub event_type: String,
}

/// Labels for `plugin_hooks_total` and `plugin_hook_duration_seconds`.
#[derive(Clone, Debug, Hash, PartialEq, Eq, prometheus_client::encoding::EncodeLabelSet)]
pub struct PluginHookLabels {
    pub plugin_name: String,
    pub hook_name: String,
    pub status: String,
}

/// Labels for `plugin_hook_duration_seconds` (no status dimension).
#[derive(Clone, Debug, Hash, PartialEq, Eq, prometheus_client::encoding::EncodeLabelSet)]
pub struct PluginDurationLabels {
    pub plugin_name: String,
    pub hook_name: String,
}

/// Labels for `transactions_total`.
#[derive(Clone, Debug, Hash, PartialEq, Eq, prometheus_client::encoding::EncodeLabelSet)]
pub struct TransactionLabels {
    pub status: String,
}

/// Labels for `sessions_created_total`.
#[derive(Clone, Debug, Hash, PartialEq, Eq, prometheus_client::encoding::EncodeLabelSet)]
pub struct SessionLabels {
    pub role: String,
}

/// Labels for `cache_hits_total` / `cache_misses_total`.
#[derive(Clone, Debug, Hash, PartialEq, Eq, prometheus_client::encoding::EncodeLabelSet)]
pub struct CacheLabels {
    pub cache_name: String,
}

/// Labels for `component_healthy`.
#[derive(Clone, Debug, Hash, PartialEq, Eq, prometheus_client::encoding::EncodeLabelSet)]
pub struct ComponentLabels {
    pub component: String,
}

/// Labels for `query_duration_seconds` (no status dimension).
#[derive(Clone, Debug, Hash, PartialEq, Eq, prometheus_client::encoding::EncodeLabelSet)]
pub struct QueryDurationLabels {
    pub query_type: String,
    pub component: String,
}

// ── Metrics ──────────────────────────────────────────────────────────────────

/// All Prometheus instruments for the ClawDB runtime, held in a single struct
/// and registered together via [`init_metrics`].
#[derive(Clone)]
pub struct Metrics {
    // ── Counters ─────────────────────────────────────────────────────────────
    pub queries_total: Family<QueryLabels, Counter>,
    pub events_emitted_total: Family<EventLabels, Counter>,
    pub plugin_hooks_total: Family<PluginHookLabels, Counter>,
    pub transactions_total: Family<TransactionLabels, Counter>,
    pub sessions_created_total: Family<SessionLabels, Counter>,
    pub cache_hits_total: Family<CacheLabels, Counter>,
    pub cache_misses_total: Family<CacheLabels, Counter>,

    // ── Histograms ────────────────────────────────────────────────────────────
    pub query_duration_seconds: Family<QueryDurationLabels, Histogram>,
    pub transaction_duration_seconds: Family<TransactionLabels, Histogram>,
    pub plugin_hook_duration_seconds: Family<PluginDurationLabels, Histogram>,

    // ── Gauges ────────────────────────────────────────────────────────────────
    pub active_sessions_total: Gauge,
    pub active_transactions_total: Gauge,
    pub plugin_count: Gauge,
    pub component_healthy: Family<ComponentLabels, Gauge>,
}

impl Metrics {
    /// Creates a new zeroed `Metrics` instance (instruments are not yet registered).
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            queries_total: Family::default(),
            events_emitted_total: Family::default(),
            plugin_hooks_total: Family::default(),
            transactions_total: Family::default(),
            sessions_created_total: Family::default(),
            cache_hits_total: Family::default(),
            cache_misses_total: Family::default(),

            query_duration_seconds: Family::new_with_constructor(|| {
                Histogram::new(
                    [0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0]
                        .iter()
                        .cloned(),
                )
            }),
            transaction_duration_seconds: Family::new_with_constructor(|| {
                Histogram::new(
                    [0.01, 0.05, 0.1, 0.5, 1.0, 5.0, 30.0]
                        .iter()
                        .cloned(),
                )
            }),
            plugin_hook_duration_seconds: Family::new_with_constructor(|| {
                Histogram::new(
                    [0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0]
                        .iter()
                        .cloned(),
                )
            }),

            active_sessions_total: Gauge::default(),
            active_transactions_total: Gauge::default(),
            plugin_count: Gauge::default(),
            component_healthy: Family::default(),
        })
    }

    // ── Convenience increment helpers ─────────────────────────────────────────

    pub fn inc_query(&self, query_type: &str, component: &str, status: &str) {
        self.queries_total
            .get_or_create(&QueryLabels {
                query_type: query_type.to_string(),
                component: component.to_string(),
                status: status.to_string(),
            })
            .inc();
    }

    pub fn record_query_duration(&self, query_type: &str, component: &str, secs: f64) {
        self.query_duration_seconds
            .get_or_create(&QueryDurationLabels {
                query_type: query_type.to_string(),
                component: component.to_string(),
            })
            .observe(secs);
    }

    pub fn inc_event(&self, event_type: &str) {
        self.events_emitted_total
            .get_or_create(&EventLabels { event_type: event_type.to_string() })
            .inc();
    }

    pub fn inc_transaction(&self, status: &str) {
        self.transactions_total
            .get_or_create(&TransactionLabels { status: status.to_string() })
            .inc();
    }

    pub fn record_transaction_duration(&self, status: &str, secs: f64) {
        self.transaction_duration_seconds
            .get_or_create(&TransactionLabels { status: status.to_string() })
            .observe(secs);
    }

    pub fn inc_session(&self, role: &str) {
        self.sessions_created_total
            .get_or_create(&SessionLabels { role: role.to_string() })
            .inc();
    }

    pub fn set_component_health(&self, component: &str, healthy: bool) {
        self.component_healthy
            .get_or_create(&ComponentLabels { component: component.to_string() })
            .set(i64::from(healthy));
    }

    pub fn inc_plugin_hook(&self, plugin: &str, hook: &str, status: &str) {
        self.plugin_hooks_total
            .get_or_create(&PluginHookLabels {
                plugin_name: plugin.to_string(),
                hook_name: hook.to_string(),
                status: status.to_string(),
            })
            .inc();
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Arc::try_unwrap(Self::new()).expect("only one reference")
    }
}

// ── Registry helpers ─────────────────────────────────────────────────────────

/// Registers all `Metrics` instruments into `registry`.
pub fn init_metrics(metrics: &Arc<Metrics>, registry: &mut Registry) {
    registry.register(
        "queries_total",
        "Total number of queries executed",
        metrics.queries_total.clone(),
    );
    registry.register(
        "events_emitted_total",
        "Total events emitted on the internal bus",
        metrics.events_emitted_total.clone(),
    );
    registry.register(
        "plugin_hooks_total",
        "Total plugin hook invocations",
        metrics.plugin_hooks_total.clone(),
    );
    registry.register(
        "transactions_total",
        "Total transaction lifecycle transitions",
        metrics.transactions_total.clone(),
    );
    registry.register(
        "sessions_created_total",
        "Total sessions created",
        metrics.sessions_created_total.clone(),
    );
    registry.register(
        "cache_hits_total",
        "Total cache hits",
        metrics.cache_hits_total.clone(),
    );
    registry.register(
        "cache_misses_total",
        "Total cache misses",
        metrics.cache_misses_total.clone(),
    );
    registry.register(
        "query_duration_seconds",
        "Query execution latency",
        metrics.query_duration_seconds.clone(),
    );
    registry.register(
        "transaction_duration_seconds",
        "Transaction total duration",
        metrics.transaction_duration_seconds.clone(),
    );
    registry.register(
        "plugin_hook_duration_seconds",
        "Plugin hook execution latency",
        metrics.plugin_hook_duration_seconds.clone(),
    );
    registry.register(
        "active_sessions_total",
        "Number of currently active sessions",
        metrics.active_sessions_total.clone(),
    );
    registry.register(
        "active_transactions_total",
        "Number of currently active transactions",
        metrics.active_transactions_total.clone(),
    );
    registry.register(
        "plugin_count",
        "Number of loaded plugins",
        metrics.plugin_count.clone(),
    );
    registry.register(
        "component_healthy",
        "Component health (1=healthy, 0=unhealthy)",
        metrics.component_healthy.clone(),
    );
}

/// Renders the Prometheus text format for all registered metrics.
pub fn metrics_handler(registry: &Registry) -> String {
    let mut buf = String::new();
    encode(&mut buf, registry).expect("metrics encode failed");
    buf
}

/// Starts a minimal axum HTTP server on `port` serving `/metrics`.
///
/// Returns immediately; the server runs in the background.
pub fn serve_metrics(port: u16, registry: std::sync::Arc<std::sync::Mutex<Registry>>) {
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    tokio::spawn(async move {
        use axum::{routing::get, Router};

        let app = Router::new().route(
            "/metrics",
            get(move || {
                let reg = registry.clone();
                async move {
                    let r = reg.lock().expect("metrics registry lock");
                    metrics_handler(&r)
                }
            }),
        );

        let listener = tokio::net::TcpListener::bind(addr).await.expect("metrics bind");
        tracing::info!(port, "metrics server listening");
        axum::serve(listener, app)
            .await
            .expect("metrics server failed");
    });
}
