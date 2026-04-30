//! Component lifecycle management: startup, shutdown, health monitoring.

pub mod health;
pub mod manager;
pub mod shutdown;

pub use health::{ComponentHealth, HealthReport, HealthStatus};
pub use manager::ComponentLifecycleManager;
pub use shutdown::GracefulShutdown;
