use std::{
    collections::HashMap,
    num::NonZeroU32,
    sync::{Arc, Mutex},
    time::Duration,
};

use clawdb::{ClawDB, ClawDBSession};
use governor::{
    clock::{Clock, DefaultClock},
    DefaultKeyedRateLimiter, Quota,
};
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
use tokio::sync::Mutex as AsyncMutex;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct RequestId(pub String);

#[derive(Clone, Debug, Hash, PartialEq, Eq, prometheus_client::encoding::EncodeLabelSet)]
pub struct HttpLabels {
    pub method: String,
    pub path: String,
    pub status: String,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, prometheus_client::encoding::EncodeLabelSet)]
pub struct HttpPathLabel {
    pub path: String,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, prometheus_client::encoding::EncodeLabelSet)]
pub struct GrpcLabels {
    pub method: String,
    pub status: String,
}

fn duration_histogram() -> Histogram {
    Histogram::new(exponential_buckets(0.001, 2.0, 16))
}

#[derive(Clone)]
pub struct ServerMetrics {
    registry: Arc<Mutex<Registry>>,
    http_requests_total: Family<HttpLabels, Counter>,
    http_request_duration_seconds: Family<HttpPathLabel, Histogram, fn() -> Histogram>,
    grpc_requests_total: Family<GrpcLabels, Counter>,
    active_sessions: Gauge,
}

impl ServerMetrics {
    pub fn new() -> Self {
        let http_requests_total = Family::default();
        let http_request_duration_seconds =
            Family::new_with_constructor(duration_histogram as fn() -> Histogram);
        let grpc_requests_total = Family::default();
        let active_sessions = Gauge::default();

        let mut registry = Registry::default();
        registry.register(
            "clawdb_http_requests_total",
            "HTTP requests",
            http_requests_total.clone(),
        );
        registry.register(
            "clawdb_http_request_duration_seconds",
            "HTTP request duration",
            http_request_duration_seconds.clone(),
        );
        registry.register(
            "clawdb_grpc_requests_total",
            "gRPC requests",
            grpc_requests_total.clone(),
        );
        registry.register(
            "clawdb_active_sessions",
            "Active sessions",
            active_sessions.clone(),
        );

        Self {
            registry: Arc::new(Mutex::new(registry)),
            http_requests_total,
            http_request_duration_seconds,
            grpc_requests_total,
            active_sessions,
        }
    }

    pub fn observe_http(&self, method: &str, path: &str, status: u16, duration: Duration) {
        self.http_requests_total
            .get_or_create(&HttpLabels {
                method: method.to_string(),
                path: path.to_string(),
                status: status.to_string(),
            })
            .inc();
        self.http_request_duration_seconds
            .get_or_create(&HttpPathLabel {
                path: path.to_string(),
            })
            .observe(duration.as_secs_f64());
    }

    pub fn observe_grpc(&self, method: &str, status: &str) {
        self.grpc_requests_total
            .get_or_create(&GrpcLabels {
                method: method.to_string(),
                status: status.to_string(),
            })
            .inc();
    }

    pub fn set_active_sessions(&self, count: u64) {
        self.active_sessions
            .set(i64::try_from(count).unwrap_or(i64::MAX));
    }

    pub fn render(&self, claw_metrics: String) -> String {
        let mut out = claw_metrics;
        let mut buffer = String::new();
        if let Ok(registry) = self.registry.lock() {
            let _ = encode(&mut buffer, &registry);
        }
        out.push_str(&buffer);
        out
    }
}

impl Default for ServerMetrics {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
pub struct PendingTransaction {
    pub id: Uuid,
    pub session: ClawDBSession,
}

pub struct AppState {
    pub db: Arc<ClawDB>,
    pub metrics: ServerMetrics,
    pub transactions: AsyncMutex<HashMap<Uuid, PendingTransaction>>,
    pub grpc_limiter: DefaultKeyedRateLimiter<String>,
    pub http_read_limiter: DefaultKeyedRateLimiter<String>,
    pub http_write_limiter: DefaultKeyedRateLimiter<String>,
}

impl AppState {
    pub fn new(db: Arc<ClawDB>) -> Self {
        let non_zero = |value| NonZeroU32::new(value).unwrap_or(NonZeroU32::MIN);

        Self {
            db,
            metrics: ServerMetrics::new(),
            transactions: AsyncMutex::new(HashMap::new()),
            grpc_limiter: DefaultKeyedRateLimiter::keyed(Quota::per_minute(non_zero(1000))),
            http_read_limiter: DefaultKeyedRateLimiter::keyed(Quota::per_minute(non_zero(2000))),
            http_write_limiter: DefaultKeyedRateLimiter::keyed(Quota::per_minute(non_zero(500))),
        }
    }

    pub fn retry_after_seconds(
        not_until: &governor::NotUntil<<DefaultClock as Clock>::Instant>,
    ) -> u64 {
        not_until
            .wait_time_from(DefaultClock::default().now())
            .as_secs()
            .max(1)
    }
}
