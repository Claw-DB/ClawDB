# Changelog

## v0.1.0

### Added

1. Unified ClawDB runtime over core/vector/sync/branch/guard subsystems.
2. New `clawdb` CLI with init, start, memory, search, branch, sync, reflect, policy, and config workflows.
3. gRPC and HTTP APIs with session validation and health/metrics endpoints.
4. Docker, Compose, Kubernetes, and CI/CD pipeline templates.

### Changed

1. Engine API consolidated under `ClawDB` with stable aliases and prelude exports.
2. Telemetry migrated to `prometheus-client` and structured tracing setup.

### Fixed

1. HTTP error mapping normalized to unified `ClawDBError` status handling.
2. Server component error wrapping aligned with `ComponentFailed` semantics.

### Security

1. Plugin sandbox capability validation hardened.
2. Session and guard integration propagated through API and CLI flows.
