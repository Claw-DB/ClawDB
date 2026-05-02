//! Configuration for the `clawdb` wrapper crate.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{ClawDBError, ClawDBResult};

/// Top-level ClawDB configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ClawDBConfig {
    /// Root data directory.
    pub data_dir: PathBuf,
    /// Stable workspace identifier.
    pub workspace_id: Uuid,
    /// Default agent identifier used by local tools.
    pub agent_id: Uuid,
    /// Log level for tracing.
    pub log_level: String,
    /// Log format, usually `json` or `console`.
    pub log_format: String,
    /// Embedded storage configuration.
    pub core: CoreConfig,
    /// Semantic search configuration.
    pub vector: VectorConfig,
    /// Branching engine configuration.
    pub branch: BranchConfig,
    /// Synchronisation configuration.
    pub sync: SyncConfig,
    /// Guard engine configuration.
    pub guard: GuardConfig,
    /// Reflection service configuration.
    pub reflect: ReflectConfig,
    /// gRPC and HTTP server configuration.
    pub server: ServerConfig,
    /// Plugin configuration.
    pub plugins: PluginsConfig,
    /// Telemetry configuration.
    pub telemetry: TelemetryConfig,
}

/// Storage configuration used to build `claw_core::ClawConfig`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CoreConfig {
    /// SQLite database path.
    pub db_path: PathBuf,
    /// Connection pool size.
    pub max_connections: u32,
    /// Whether WAL mode is enabled.
    pub wal_enabled: bool,
    /// In-memory cache size in MiB.
    pub cache_size_mb: usize,
}

/// Vector engine configuration used to build `claw_vector::VectorConfig`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct VectorConfig {
    /// Whether semantic indexing is enabled.
    pub enabled: bool,
    /// SQLite metadata path for vectors.
    pub db_path: PathBuf,
    /// Index directory for vector files.
    pub index_dir: PathBuf,
    /// Embedding service URL.
    pub embedding_service_url: String,
    /// Default embedding dimensions.
    pub default_dimensions: usize,
}

/// Branch engine configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BranchConfig {
    /// Branch snapshot directory.
    pub branches_dir: PathBuf,
    /// Branch registry database path.
    pub registry_db_path: PathBuf,
    /// Maximum branches per workspace.
    pub max_branches_per_workspace: usize,
    /// Background garbage-collection interval.
    pub gc_interval_secs: u64,
    /// Canonical trunk branch name.
    pub trunk_branch_name: String,
}

/// Sync engine configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SyncConfig {
    /// Optional sync hub URL. When absent, wrapper sync runs in local-only mode.
    pub hub_url: Option<String>,
    /// Data directory for sync artefacts.
    pub data_dir: PathBuf,
    /// Local SQLite path exposed to claw-sync.
    pub db_path: PathBuf,
    /// Background sync interval.
    pub sync_interval_secs: u64,
    /// Whether TLS is enabled.
    pub tls_enabled: bool,
    /// Connection timeout in seconds.
    pub connect_timeout_secs: u64,
    /// Request timeout in seconds.
    pub request_timeout_secs: u64,
    /// Maximum delta rows extracted in a sync pass.
    pub max_delta_rows: usize,
    /// Maximum chunk size for outbound payloads.
    pub max_chunk_bytes: usize,
    /// Maximum pull chunks requested per round.
    pub max_pull_chunks: u32,
    /// Maximum in-flight push requests.
    pub max_push_inflight: usize,
}

/// Guard engine configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GuardConfig {
    /// SQLite path for the guard database.
    pub db_path: String,
    /// JWT signing secret.
    pub jwt_secret: String,
    /// Policy directory.
    pub policy_dir: PathBuf,
    /// Sensitive resources that increase risk scoring.
    pub sensitive_resources: Vec<String>,
    /// Audit flush interval in milliseconds.
    pub audit_flush_interval_ms: u64,
    /// Audit batch size.
    pub audit_batch_size: usize,
    /// TLS certificate path for guard server mode.
    pub tls_cert_path: PathBuf,
    /// TLS key path for guard server mode.
    pub tls_key_path: PathBuf,
}

/// Reflection service configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ReflectConfig {
    /// Optional service URL.
    #[serde(alias = "service_url")]
    pub base_url: Option<String>,
    /// Optional API key.
    pub api_key: Option<String>,
}

/// Server configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    /// gRPC port.
    pub grpc_port: u16,
    /// HTTP port.
    pub http_port: u16,
}

/// Plugin configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PluginsConfig {
    /// Plugin directory.
    pub plugins_dir: PathBuf,
    /// Enabled plugin names.
    pub enabled: Vec<String>,
    /// Whether sandboxing is enabled.
    pub sandbox_enabled: bool,
}

/// Telemetry configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TelemetryConfig {
    /// Prometheus scrape port.
    pub metrics_port: u16,
    /// Optional OTLP endpoint.
    pub otel_endpoint: Option<String>,
    /// Service name to emit in telemetry.
    pub service_name: String,
}

impl ClawDBConfig {
    /// Creates a default config rooted at `data_dir`.
    pub fn default_for_dir(data_dir: &Path) -> Self {
        let host = host_identity();
        let workspace_id = Uuid::new_v5(&Uuid::NAMESPACE_DNS, host.as_bytes());
        let agent_seed = format!("{host}:agent");
        let agent_id = Uuid::new_v5(&Uuid::NAMESPACE_DNS, agent_seed.as_bytes());
        Self {
            data_dir: data_dir.to_path_buf(),
            workspace_id,
            agent_id,
            log_level: "info".to_string(),
            log_format: "json".to_string(),
            core: CoreConfig {
                db_path: data_dir.join("claw.db"),
                max_connections: 10,
                wal_enabled: true,
                cache_size_mb: 64,
            },
            vector: VectorConfig {
                enabled: true,
                db_path: data_dir.join("claw_vector.db"),
                index_dir: data_dir.join("claw_vector_indices"),
                embedding_service_url: "http://localhost:50051".to_string(),
                default_dimensions: 384,
            },
            branch: BranchConfig {
                branches_dir: data_dir.join("branches"),
                registry_db_path: data_dir.join("branches").join("branch_registry.db"),
                max_branches_per_workspace: 50,
                gc_interval_secs: 3600,
                trunk_branch_name: "trunk".to_string(),
            },
            sync: SyncConfig {
                hub_url: None,
                data_dir: data_dir.join("sync"),
                db_path: data_dir.join("claw.db"),
                sync_interval_secs: 30,
                tls_enabled: false,
                connect_timeout_secs: 10,
                request_timeout_secs: 30,
                max_delta_rows: 1000,
                max_chunk_bytes: 64 * 1024,
                max_pull_chunks: 128,
                max_push_inflight: 4,
            },
            guard: GuardConfig {
                db_path: data_dir.join("claw_guard.db").display().to_string(),
                jwt_secret: "change-me".to_string(),
                policy_dir: data_dir.join("policies"),
                sensitive_resources: vec!["memory".to_string(), "branch".to_string()],
                audit_flush_interval_ms: 100,
                audit_batch_size: 500,
                tls_cert_path: data_dir.join("certs").join("server.crt"),
                tls_key_path: data_dir.join("certs").join("server.key"),
            },
            reflect: ReflectConfig {
                base_url: None,
                api_key: None,
            },
            server: ServerConfig {
                grpc_port: 50050,
                http_port: 8080,
            },
            plugins: PluginsConfig {
                plugins_dir: data_dir.join("plugins"),
                enabled: Vec::new(),
                sandbox_enabled: true,
            },
            telemetry: TelemetryConfig {
                metrics_port: 9090,
                otel_endpoint: None,
                service_name: "clawdb".to_string(),
            },
        }
    }

    /// Builds configuration from environment variables.
    pub fn from_env() -> ClawDBResult<Self> {
        let data_dir = std::env::var("CLAW_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".clawdb")
            });
        let mut config = Self::default_for_dir(&data_dir);
        apply_env_overrides(&mut config)?;
        if std::env::var("CLAW_GUARD_JWT_SECRET").is_err() && config.guard.jwt_secret == "change-me"
        {
            return Err(ClawDBError::Config(
                "CLAW_GUARD_JWT_SECRET is required".to_string(),
            ));
        }
        Ok(config)
    }

    /// Loads configuration from a TOML file and then applies environment overrides.
    pub fn from_file(path: &Path) -> ClawDBResult<Self> {
        let raw = std::fs::read_to_string(path)?;
        let mut config: Self =
            toml::from_str(&raw).map_err(|error| ClawDBError::Config(error.to_string()))?;
        if config.data_dir.as_os_str().is_empty() {
            config.data_dir = path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .to_path_buf();
        }
        apply_env_overrides(&mut config)?;
        if config.guard.jwt_secret.trim().is_empty() || config.guard.jwt_secret == "change-me" {
            return Err(ClawDBError::Config(
                "CLAW_GUARD_JWT_SECRET is required".to_string(),
            ));
        }
        Ok(config)
    }

    /// Loads configuration from disk or falls back to defaults.
    pub fn load_or_default(data_dir: &Path) -> ClawDBResult<Self> {
        let path = data_dir.join("config.toml");
        if path.exists() {
            Self::from_file(&path)
        } else {
            Ok(Self::default_for_dir(data_dir))
        }
    }

    /// Loads configuration from disk without environment overrides.
    pub fn load(path: &Path) -> ClawDBResult<Self> {
        let raw = std::fs::read_to_string(path)?;
        toml::from_str(&raw).map_err(|error| ClawDBError::Config(error.to_string()))
    }

    /// Saves this config as pretty TOML.
    pub fn save(&self, path: &Path) -> ClawDBResult<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let raw =
            toml::to_string_pretty(self).map_err(|error| ClawDBError::Config(error.to_string()))?;
        std::fs::write(path, raw)?;
        Ok(())
    }
}

impl Default for ClawDBConfig {
    fn default() -> Self {
        Self::default_for_dir(
            &dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".clawdb"),
        )
    }
}

impl Default for CoreConfig {
    fn default() -> Self {
        ClawDBConfig::default().core
    }
}

impl Default for VectorConfig {
    fn default() -> Self {
        ClawDBConfig::default().vector
    }
}

impl Default for BranchConfig {
    fn default() -> Self {
        ClawDBConfig::default().branch
    }
}

impl Default for SyncConfig {
    fn default() -> Self {
        ClawDBConfig::default().sync
    }
}

impl Default for GuardConfig {
    fn default() -> Self {
        ClawDBConfig::default().guard
    }
}

impl Default for ReflectConfig {
    fn default() -> Self {
        ClawDBConfig::default().reflect
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        ClawDBConfig::default().server
    }
}

impl Default for PluginsConfig {
    fn default() -> Self {
        ClawDBConfig::default().plugins
    }
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        ClawDBConfig::default().telemetry
    }
}

fn apply_env_overrides(config: &mut ClawDBConfig) -> ClawDBResult<()> {
    if let Ok(value) = std::env::var("CLAW_DATA_DIR") {
        config.data_dir = PathBuf::from(value);
    }
    if let Ok(value) = std::env::var("CLAW_WORKSPACE_ID") {
        config.workspace_id = parse_uuid(&value, "CLAW_WORKSPACE_ID")?;
    }
    if let Ok(value) = std::env::var("CLAW_AGENT_ID") {
        config.agent_id = parse_uuid(&value, "CLAW_AGENT_ID")?;
    }
    if let Ok(value) = std::env::var("CLAW_LOG_LEVEL") {
        config.log_level = value;
    }
    if let Ok(value) = std::env::var("CLAW_LOG_FORMAT") {
        config.log_format = value;
    }
    if let Ok(value) = std::env::var("CLAW_VECTOR_BASE_URL") {
        config.vector.embedding_service_url = value;
    }
    if let Ok(value) = std::env::var("CLAW_VECTOR_ENABLED") {
        config.vector.enabled = parse_bool(&value)?;
    }
    if let Ok(value) = std::env::var("CLAW_SYNC_HUB_URL") {
        config.sync.hub_url = Some(value);
    }
    if let Ok(value) = std::env::var("CLAW_GUARD_JWT_SECRET") {
        config.guard.jwt_secret = value;
    }
    if let Ok(value) = std::env::var("CLAW_GUARD_POLICY_DIR") {
        config.guard.policy_dir = PathBuf::from(value);
    }
    if let Ok(value) = std::env::var("CLAW_REFLECT_BASE_URL") {
        config.reflect.base_url = Some(value);
    }
    if let Ok(value) = std::env::var("CLAW_REFLECT_API_KEY") {
        config.reflect.api_key = Some(value);
    }
    Ok(())
}

fn parse_uuid(value: &str, name: &str) -> ClawDBResult<Uuid> {
    Uuid::parse_str(value).map_err(|error| ClawDBError::Config(format!("invalid {name}: {error}")))
}

fn parse_bool(value: &str) -> ClawDBResult<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => Err(ClawDBError::Config(format!("invalid boolean: {value}"))),
    }
}

fn host_identity() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("HOST"))
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "clawdb-local".to_string())
}
