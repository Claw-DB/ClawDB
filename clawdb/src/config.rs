//! ClawDBConfig: unified configuration builder for all ClawDB subsystems.

use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{ClawDBError, ClawDBResult};

/// Unified top-level ClawDB configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClawDBConfig {
    /// Root data directory (env: `CLAW_DATA_DIR`, default: `~/.clawdb`).
    pub data_dir: PathBuf,
    /// Workspace identifier (env: `CLAW_WORKSPACE_ID`).
    pub workspace_id: Uuid,
    /// Agent identifier; auto-generated on first run (env: `CLAW_AGENT_ID`).
    pub agent_id: Uuid,
    /// Log level (env: `CLAW_LOG_LEVEL`, default: `INFO`).
    #[serde(default = "default_log_level")]
    pub log_level: String,
    /// Log format: `pretty` or `json` (env: `CLAW_LOG_FORMAT`, default: `pretty`).
    #[serde(default = "default_log_format")]
    pub log_format: String,

    /// Configuration for the claw-core storage engine.
    pub core: CoreSubConfig,
    /// Configuration for the claw-vector semantic index.
    pub vector: VectorSubConfig,
    /// Configuration for the claw-sync engine.
    pub sync: SyncSubConfig,
    /// Configuration for the claw-branch engine.
    pub branch: BranchSubConfig,
    /// Configuration for the claw-guard security engine.
    pub guard: GuardSubConfig,
    /// Configuration for the claw-reflect microservice.
    pub reflect: ReflectSubConfig,
    /// gRPC / HTTP server configuration.
    pub server: ServerSubConfig,
    /// Plugin system configuration.
    pub plugins: PluginsSubConfig,
    /// Telemetry configuration.
    pub telemetry: TelemetrySubConfig,
}

fn default_log_level() -> String { "INFO".to_string() }
fn default_log_format() -> String { "pretty".to_string() }

/// Configuration for the claw-core SQLite storage engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreSubConfig {
    /// Path to the SQLite database file.
    pub database_path: PathBuf,
    /// Whether WAL mode is enabled.
    #[serde(default = "default_true")]
    pub wal_enabled: bool,
    /// In-memory page cache size in megabytes.
    #[serde(default = "default_cache_mb")]
    pub cache_size_mb: u32,
}

/// Configuration for the claw-vector HNSW index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorSubConfig {
    /// Path to the vector index file.
    pub index_path: PathBuf,
    /// URL of the embedding microservice.
    #[serde(default = "default_embedding_url")]
    pub embedding_service_url: String,
    /// Vector dimensionality.
    #[serde(default = "default_dimensions")]
    pub dimensions: usize,
}

/// Configuration for the claw-sync engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncSubConfig {
    /// Optional hub URL; if absent, sync is disabled.
    pub hub_url: Option<String>,
    /// Sync interval in seconds.
    #[serde(default = "default_sync_interval")]
    pub sync_interval_secs: u64,
    /// Path to the device identity key file.
    pub device_identity_path: PathBuf,
}

/// Configuration for the claw-branch fork/merge engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchSubConfig {
    /// Directory that holds branch snapshots.
    pub branches_dir: PathBuf,
    /// Name of the trunk/main branch.
    #[serde(default = "default_trunk")]
    pub trunk_name: String,
}

/// Configuration for the claw-guard security engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardSubConfig {
    /// Optional guard database URL.
    pub database_url: Option<String>,
    /// JWT signing secret.
    #[serde(default = "default_jwt_secret")]
    pub jwt_secret: String,
    /// Directory containing Rego / policy files.
    pub policy_dir: PathBuf,
}

/// Configuration for the claw-reflect Python microservice.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReflectSubConfig {
    /// Base URL of the reflect HTTP service.
    #[serde(default = "default_reflect_url")]
    pub service_url: String,
    /// How often (in seconds) to poll for job status.
    #[serde(default = "default_reflect_poll")]
    pub poll_interval_secs: u64,
}

/// gRPC and HTTP server configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerSubConfig {
    /// gRPC listen port.
    #[serde(default = "default_grpc_port")]
    pub grpc_port: u16,
    /// HTTP listen port.
    #[serde(default = "default_http_port")]
    pub http_port: u16,
    /// Path to the TLS certificate file.
    pub tls_cert_path: Option<PathBuf>,
    /// Path to the TLS private key file.
    pub tls_key_path: Option<PathBuf>,
    /// Maximum number of simultaneous connections.
    #[serde(default = "default_max_connections")]
    pub max_connections: usize,
}

/// Plugin system configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginsSubConfig {
    /// Directory that contains plugin `.so`/`.dylib` files.
    pub plugins_dir: PathBuf,
    /// List of plugin names to load at startup.
    #[serde(default)]
    pub enabled: Vec<String>,
    /// Whether to enforce the plugin sandbox.
    #[serde(default = "default_true")]
    pub sandbox_enabled: bool,
}

/// Observability and telemetry configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetrySubConfig {
    /// Prometheus metrics scrape port.
    #[serde(default = "default_metrics_port")]
    pub metrics_port: u16,
    /// Optional OpenTelemetry Collector endpoint.
    pub otlp_endpoint: Option<String>,
    /// Service name emitted in traces.
    #[serde(default = "default_service_name")]
    pub service_name: String,
}

// ── Default helpers ──────────────────────────────────────────────────────────

fn default_true() -> bool { true }
fn default_cache_mb() -> u32 { 64 }
fn default_embedding_url() -> String { "http://localhost:8001".to_string() }
fn default_dimensions() -> usize { 1536 }
fn default_sync_interval() -> u64 { 300 }
fn default_trunk() -> String { "trunk".to_string() }
fn default_jwt_secret() -> String { "change-me-in-production".to_string() }
fn default_reflect_url() -> String { "http://localhost:8002".to_string() }
fn default_reflect_poll() -> u64 { 60 }
fn default_grpc_port() -> u16 { 50050 }
fn default_http_port() -> u16 { 8080 }
fn default_max_connections() -> usize { 1000 }
fn default_metrics_port() -> u16 { 9090 }
fn default_service_name() -> String { "clawdb".to_string() }

// ── Implementation ───────────────────────────────────────────────────────────

impl ClawDBConfig {
    /// Deserialises a `ClawDBConfig` from a TOML file at `path`.
    pub fn load(path: &Path) -> ClawDBResult<Self> {
        let raw = std::fs::read_to_string(path)?;
        toml::from_str(&raw).map_err(|e| ClawDBError::Config(e.to_string()))
    }

    /// Serialises this config to a TOML file at `path`.
    pub fn save(&self, path: &Path) -> ClawDBResult<()> {
        let raw = toml::to_string_pretty(self).map_err(|e| ClawDBError::Config(e.to_string()))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, raw)?;
        Ok(())
    }

    /// Builds a config from `CLAW_*` environment variables, using defaults where absent.
    pub fn from_env() -> ClawDBResult<Self> {
        let data_dir = std::env::var("CLAW_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".clawdb")
            });

        let workspace_id = std::env::var("CLAW_WORKSPACE_ID")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(Uuid::new_v4);

        let agent_id = std::env::var("CLAW_AGENT_ID")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(Uuid::new_v4);

        let log_level = std::env::var("CLAW_LOG_LEVEL").unwrap_or_else(|_| "INFO".to_string());
        let log_format = std::env::var("CLAW_LOG_FORMAT").unwrap_or_else(|_| "pretty".to_string());

        let mut cfg = Self::default_for_dir(&data_dir);
        cfg.workspace_id = workspace_id;
        cfg.agent_id = agent_id;
        cfg.log_level = log_level;
        cfg.log_format = log_format;
        Ok(cfg)
    }

    /// Loads from `data_dir/config.toml` if it exists; otherwise returns the default config.
    pub fn load_or_default(data_dir: &Path) -> ClawDBResult<Self> {
        let cfg_path = data_dir.join("config.toml");
        if cfg_path.exists() {
            Self::load(&cfg_path)
        } else {
            Ok(Self::default_for_dir(data_dir))
        }
    }

    /// Creates a default config with all sub-config paths rooted at `data_dir`.
    pub fn default_for_dir(data_dir: &Path) -> Self {
        Self {
            data_dir: data_dir.to_path_buf(),
            workspace_id: Uuid::new_v4(),
            agent_id: Uuid::new_v4(),
            log_level: default_log_level(),
            log_format: default_log_format(),
            core: CoreSubConfig {
                database_path: data_dir.join("core.db"),
                wal_enabled: true,
                cache_size_mb: 64,
            },
            vector: VectorSubConfig {
                index_path: data_dir.join("vector.idx"),
                embedding_service_url: default_embedding_url(),
                dimensions: 1536,
            },
            sync: SyncSubConfig {
                hub_url: None,
                sync_interval_secs: 300,
                device_identity_path: data_dir.join("device.key"),
            },
            branch: BranchSubConfig {
                branches_dir: data_dir.join("branches"),
                trunk_name: "trunk".to_string(),
            },
            guard: GuardSubConfig {
                database_url: None,
                jwt_secret: default_jwt_secret(),
                policy_dir: data_dir.join("policies"),
            },
            reflect: ReflectSubConfig {
                service_url: default_reflect_url(),
                poll_interval_secs: 60,
            },
            server: ServerSubConfig {
                grpc_port: 50050,
                http_port: 8080,
                tls_cert_path: None,
                tls_key_path: None,
                max_connections: 1000,
            },
            plugins: PluginsSubConfig {
                plugins_dir: data_dir.join("plugins"),
                enabled: vec![],
                sandbox_enabled: true,
            },
            telemetry: TelemetrySubConfig {
                metrics_port: 9090,
                otlp_endpoint: None,
                service_name: "clawdb".to_string(),
            },
        }
    }

    /// Converts this config into a `claw_core::ClawConfig`.
    pub fn into_core_config(&self) -> claw_core::ClawConfig {
        claw_core::ClawConfig {
            database_path: self.core.database_path.clone(),
            wal_enabled: self.core.wal_enabled,
            cache_size_mb: self.core.cache_size_mb,
        }
    }

    /// Converts this config into a `claw_vector::VectorConfig`.
    pub fn into_vector_config(&self) -> claw_vector::VectorConfig {
        claw_vector::VectorConfig {
            index_path: self.vector.index_path.clone(),
            embedding_service_url: self.vector.embedding_service_url.clone(),
            dimensions: self.vector.dimensions,
        }
    }

    /// Converts this config into a `claw_sync::SyncConfig`.
    pub fn into_sync_config(&self) -> claw_sync::SyncConfig {
        claw_sync::SyncConfig {
            hub_url: self.sync.hub_url.clone(),
            sync_interval_secs: self.sync.sync_interval_secs,
            device_identity_path: self.sync.device_identity_path.clone(),
        }
    }

    /// Converts this config into a `claw_branch::BranchConfig`.
    pub fn into_branch_config(&self) -> claw_branch::BranchConfig {
        claw_branch::BranchConfig {
            branches_dir: self.branch.branches_dir.clone(),
            trunk_name: self.branch.trunk_name.clone(),
        }
    }

    /// Converts this config into a `claw_guard::GuardConfig`.
    pub fn into_guard_config(&self) -> claw_guard::GuardConfig {
        claw_guard::GuardConfig {
            database_url: self.guard.database_url.clone(),
            jwt_secret: self.guard.jwt_secret.clone(),
            policy_dir: self.guard.policy_dir.clone(),
        }
    }
}
