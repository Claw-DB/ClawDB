//! Structured health reporting for all ClawDB components.

use std::collections::HashMap;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// The overall health status of a component.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HealthStatus {
    /// Component is operating normally.
    Healthy,
    /// Component is operating with degraded performance.
    Degraded { reason: String },
    /// Component has failed and is not serving requests.
    Unhealthy { reason: String },
    /// Component health has not been checked yet.
    Unknown,
}

/// Health state snapshot for a single ClawDB component.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    pub name: String,
    pub healthy: bool,
    pub last_check: DateTime<Utc>,
    pub error: Option<String>,
    pub latency_ms: Option<u64>,
    pub metadata: serde_json::Value,
}

impl ComponentHealth {
    /// Creates a healthy `ComponentHealth` record.
    pub fn healthy(name: impl Into<String>, latency_ms: u64) -> Self {
        Self {
            name: name.into(),
            healthy: true,
            last_check: Utc::now(),
            error: None,
            latency_ms: Some(latency_ms),
            metadata: serde_json::Value::Null,
        }
    }

    /// Creates an unhealthy `ComponentHealth` record.
    pub fn unhealthy(name: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            healthy: false,
            last_check: Utc::now(),
            error: Some(error.into()),
            latency_ms: None,
            metadata: serde_json::Value::Null,
        }
    }
}

/// Aggregate health report for the entire ClawDB runtime.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthReport {
    pub overall: HealthStatus,
    pub components: HashMap<String, ComponentHealth>,
    pub checked_at: DateTime<Utc>,
    pub uptime_secs: u64,
    pub version: String,
}

impl HealthReport {
    /// Returns `true` if all required components are healthy.
    pub fn is_ready(&self) -> bool {
        Self::required_components().iter().all(|name| {
            self.components
                .get(*name)
                .map(|h| h.healthy)
                .unwrap_or(false)
        })
    }

    /// Components that must be healthy for the runtime to be ready.
    pub fn required_components() -> &'static [&'static str] {
        &["core", "vector", "guard", "branch"]
    }

    /// Optional components (degraded/absent does not block readiness).
    pub fn optional_components() -> &'static [&'static str] {
        &["sync", "reflect"]
    }
}
