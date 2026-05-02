//! Prometheus metrics for the wrapper.

use std::sync::{Arc, Mutex};

use prometheus_client::{
    encoding::text::encode,
    metrics::{
        counter::Counter,
        family::Family,
        histogram::{exponential_buckets, Histogram},
    },
    registry::Registry,
};

fn make_search_hits_histogram() -> Histogram {
    Histogram::new(exponential_buckets(1.0, 2.0, 10))
}

/// Label set for workspace-scoped counters.
#[derive(Clone, Debug, Hash, PartialEq, Eq, prometheus_client::encoding::EncodeLabelSet)]
pub struct WorkspaceLabels {
    /// Workspace identifier.
    pub workspace_id: String,
}

/// Label set for remember calls.
#[derive(Clone, Debug, Hash, PartialEq, Eq, prometheus_client::encoding::EncodeLabelSet)]
pub struct RememberLabels {
    /// Workspace identifier.
    pub workspace_id: String,
    /// Result label.
    pub result: String,
}

/// Label set for search counters.
#[derive(Clone, Debug, Hash, PartialEq, Eq, prometheus_client::encoding::EncodeLabelSet)]
pub struct SearchLabels {
    /// Workspace identifier.
    pub workspace_id: String,
    /// Search mode.
    pub mode: String,
}

/// Label set for branch operations.
#[derive(Clone, Debug, Hash, PartialEq, Eq, prometheus_client::encoding::EncodeLabelSet)]
pub struct BranchLabels {
    /// Workspace identifier.
    pub workspace_id: String,
    /// Branch operation.
    pub op: String,
}

/// Prometheus registry handle.
#[derive(Clone)]
pub struct PrometheusHandle {
    registry: Arc<Mutex<Registry>>,
}

impl PrometheusHandle {
    /// Renders all current metrics in Prometheus text format.
    pub fn render(&self) -> String {
        let mut out = String::new();
        if let Ok(registry) = self.registry.lock() {
            let _ = encode(&mut out, &registry);
        }
        out
    }
}

/// Wrapper metrics registry and instruments.
pub struct Metrics {
    registry: Arc<Mutex<Registry>>,
    pub(crate) remember_total_family: Family<RememberLabels, Counter>,
    pub(crate) search_total_family: Family<SearchLabels, Counter>,
    pub(crate) search_hits_family: Family<WorkspaceLabels, Histogram, fn() -> Histogram>,
    pub(crate) branch_ops_family: Family<BranchLabels, Counter>,
    pub(crate) sync_pushed_family: Family<WorkspaceLabels, Counter>,
    pub(crate) sync_pulled_family: Family<WorkspaceLabels, Counter>,
    pub(crate) session_created: Counter,
    pub(crate) session_denied: Counter,
}

impl Metrics {
    /// Creates and registers all wrapper metrics.
    pub fn new() -> Arc<Self> {
        let remember_total_family = Family::default();
        let search_total_family = Family::default();
        let search_hits_family =
            Family::new_with_constructor(make_search_hits_histogram as fn() -> Histogram);
        let branch_ops_family = Family::default();
        let sync_pushed_family = Family::default();
        let sync_pulled_family = Family::default();
        let session_created = Counter::default();
        let session_denied = Counter::default();

        let mut registry = Registry::default();
        registry.register(
            "clawdb_remember_total",
            "Remember calls",
            remember_total_family.clone(),
        );
        registry.register(
            "clawdb_search_total",
            "Search calls",
            search_total_family.clone(),
        );
        registry.register(
            "clawdb_search_hits",
            "Search hit histogram",
            search_hits_family.clone(),
        );
        registry.register(
            "clawdb_branch_ops",
            "Branch operations",
            branch_ops_family.clone(),
        );
        registry.register(
            "clawdb_sync_pushed",
            "Pushed sync delta sets",
            sync_pushed_family.clone(),
        );
        registry.register(
            "clawdb_sync_pulled",
            "Pulled sync delta sets",
            sync_pulled_family.clone(),
        );
        registry.register(
            "clawdb_session_created",
            "Created sessions",
            session_created.clone(),
        );
        registry.register(
            "clawdb_session_denied",
            "Denied session operations",
            session_denied.clone(),
        );

        Arc::new(Self {
            registry: Arc::new(Mutex::new(registry)),
            remember_total_family,
            search_total_family,
            search_hits_family,
            branch_ops_family,
            sync_pushed_family,
            sync_pulled_family,
            session_created,
            session_denied,
        })
    }

    /// Returns a Prometheus handle.
    pub fn handle(&self) -> PrometheusHandle {
        PrometheusHandle {
            registry: self.registry.clone(),
        }
    }

    /// Increments the remember counter.
    pub fn remember_total(&self, workspace_id: &str, result: &str) {
        self.remember_total_family
            .get_or_create(&RememberLabels {
                workspace_id: workspace_id.to_string(),
                result: result.to_string(),
            })
            .inc();
    }

    /// Increments the search counter.
    pub fn search_total(&self, workspace_id: &str, mode: &str) {
        self.search_total_family
            .get_or_create(&SearchLabels {
                workspace_id: workspace_id.to_string(),
                mode: mode.to_string(),
            })
            .inc();
    }

    /// Records the search-hit histogram.
    pub fn search_hits(&self, workspace_id: &str, hits: f64) {
        self.search_hits_family
            .get_or_create(&WorkspaceLabels {
                workspace_id: workspace_id.to_string(),
            })
            .observe(hits);
    }

    /// Increments branch operations.
    pub fn branch_ops(&self, workspace_id: &str, op: &str) {
        self.branch_ops_family
            .get_or_create(&BranchLabels {
                workspace_id: workspace_id.to_string(),
                op: op.to_string(),
            })
            .inc();
    }

    /// Adds to the pushed sync counter.
    pub fn sync_pushed(&self, workspace_id: &str, value: u64) {
        self.sync_pushed_family
            .get_or_create(&WorkspaceLabels {
                workspace_id: workspace_id.to_string(),
            })
            .inc_by(value);
    }

    /// Adds to the pulled sync counter.
    pub fn sync_pulled(&self, workspace_id: &str, value: u64) {
        self.sync_pulled_family
            .get_or_create(&WorkspaceLabels {
                workspace_id: workspace_id.to_string(),
            })
            .inc_by(value);
    }
}
