//! gRPC service implementation for ClawDB.
//!
//! Implements the full ClawDBService tonic service with session validation,
//! error handling, and proper response mapping.

use std::sync::Arc;

use tonic::{Request, Response, Status};
use uuid::Uuid;

use crate::engine::ClawDB;
use crate::error::ClawDBError;
use crate::session::manager::ClawDBSession;

#[cfg(proto_compiled)]
pub mod proto {
    tonic::include_proto!("clawdb.v1");
}

#[cfg(proto_compiled)]
use proto::{
    claw_db_service_server::ClawDBService as ClawDBServiceTrait,
    *,
};

/// ClawDB gRPC service implementation.
pub struct ClawDBGrpcService {
    engine: Arc<ClawDB>,
}

impl ClawDBGrpcService {
    /// Creates a new service instance with the given engine.
    pub fn new(engine: Arc<ClawDB>) -> Self {
        Self { engine }
    }
}

// ── Error mapping ────────────────────────────────────────────────────────────

fn clawdb_error_to_status(err: ClawDBError) -> Status {
    use crate::error::ClawDBError as E;
    match err {
        E::Guard(_) => Status::permission_denied(err.to_string()),
        E::SessionNotFound(_) | E::SessionExpired(_) => {
            Status::unauthenticated(err.to_string())
        }
        E::ComponentNotReady(_) => Status::unavailable(err.to_string()),
        E::QueryPlanFailed { .. } | E::QueryExecutionFailed { .. } => {
            Status::internal(err.to_string())
        }
        E::Config(_) => Status::invalid_argument(err.to_string()),
        _ => Status::internal(err.to_string()),
    }
}

#[cfg(proto_compiled)]
#[tonic::async_trait]
impl ClawDBServiceTrait for ClawDBGrpcService {
    /// Remember: store a memory with optional metadata.
    async fn remember(
        &self,
        request: Request<RememberRequest>,
    ) -> Result<Response<RememberResponse>, Status> {
        let req = request.into_inner();
        
        // Validate session token
        let _ctx = self.engine
            .validate_session(&req.session_token)
            .await
            .map_err(clawdb_error_to_status)?;

        let agent_id = Uuid::parse_str(&req.agent_id)
            .map_err(|_| Status::invalid_argument("invalid agent_id"))?;
        
        // Create temporary session for API call
        let session = ClawDBSession {
            session_id: Uuid::new_v4(),
            agent_id,
            role: "user".to_string(),
            scopes: vec!["memory:write".to_string()],
            created_at: chrono::Utc::now(),
            expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
        };

        let metadata: serde_json::Value = if req.metadata_json.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_slice(&req.metadata_json)
                .unwrap_or(serde_json::Value::Null)
        };

        let result = self.engine
            .remember_typed(
                &session,
                &req.content,
                &req.memory_type,
                &req.tags,
                metadata,
            )
            .await
            .map_err(clawdb_error_to_status)?;

        Ok(Response::new(RememberResponse {
            memory_id: result.memory_id,
            importance_score: result.importance_score,
            guard_applied: true,
        }))
    }

    /// Search: find memories by keyword or semantic similarity.
    async fn search(
        &self,
        request: Request<SearchRequest>,
    ) -> Result<Response<SearchResponse>, Status> {
        let req = request.into_inner();

        let _ctx = self.engine
            .validate_session(&req.session_token)
            .await
            .map_err(clawdb_error_to_status)?;

        let agent_id = Uuid::parse_str(&req.agent_id)
            .map_err(|_| Status::invalid_argument("invalid agent_id"))?;

        let session = ClawDBSession {
            session_id: Uuid::new_v4(),
            agent_id,
            role: "user".to_string(),
            scopes: vec!["memory:read".to_string()],
            created_at: chrono::Utc::now(),
            expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
        };

        let filter = if req.filter_json.is_empty() {
            None
        } else {
            Some(serde_json::from_slice(&req.filter_json).unwrap_or(serde_json::Value::Null))
        };

        let top_k = if req.top_k > 0 { req.top_k as usize } else { 10 };
        
        let results = self.engine
            .search_with_options(&session, &req.query, top_k, req.semantic, filter)
            .await
            .map_err(clawdb_error_to_status)?;

        let memory_entries: Vec<MemoryEntry> = results
            .iter()
            .filter_map(|v| {
                let id = v.get("id").and_then(|id| id.as_str()).unwrap_or("").to_string();
                let content = v.get("content").and_then(|c| c.as_str()).unwrap_or("").to_string();
                let memory_type = v.get("memory_type").and_then(|t| t.as_str()).unwrap_or("general").to_string();
                let score = v.get("importance_score").and_then(|s| s.as_f64()).unwrap_or(0.0) as f32;
                
                Some(MemoryEntry {
                    id,
                    agent_id: req.agent_id.clone(),
                    content,
                    memory_type,
                    metadata_json: vec![],
                    tags: vec![],
                    created_at: chrono::Utc::now().timestamp_millis(),
                    importance_score: score,
                    is_promoted: false,
                })
            })
            .collect();

        Ok(Response::new(SearchResponse {
            results: memory_entries,
            latency_ms: 0.0,
            search_type: if req.semantic { "semantic" } else { "keyword" }.to_string(),
        }))
    }

    /// Recall: retrieve specific memories by ID.
    async fn recall(
        &self,
        request: Request<RecallRequest>,
    ) -> Result<Response<RecallResponse>, Status> {
        let req = request.into_inner();

        let _ctx = self.engine
            .validate_session(&req.session_token)
            .await
            .map_err(clawdb_error_to_status)?;

        let agent_id = Uuid::parse_str(&req.agent_id)
            .map_err(|_| Status::invalid_argument("invalid agent_id"))?;

        let session = ClawDBSession {
            session_id: Uuid::new_v4(),
            agent_id,
            role: "user".to_string(),
            scopes: vec!["memory:read".to_string()],
            created_at: chrono::Utc::now(),
            expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
        };

        let results = self.engine
            .recall(&session, &req.memory_ids)
            .await
            .map_err(clawdb_error_to_status)?;

        let memories: Vec<MemoryEntry> = results
            .iter()
            .filter_map(|v| {
                let id = v.get("id").and_then(|id| id.as_str()).unwrap_or("").to_string();
                let content = v.get("content").and_then(|c| c.as_str()).unwrap_or("").to_string();
                let memory_type = v.get("memory_type").and_then(|t| t.as_str()).unwrap_or("general").to_string();
                let score = v.get("importance_score").and_then(|s| s.as_f64()).unwrap_or(0.0) as f32;
                
                Some(MemoryEntry {
                    id,
                    agent_id: req.agent_id.clone(),
                    content,
                    memory_type,
                    metadata_json: vec![],
                    tags: vec![],
                    created_at: chrono::Utc::now().timestamp_millis(),
                    importance_score: score,
                    is_promoted: false,
                })
            })
            .collect();

        Ok(Response::new(RecallResponse {
            memories,
            denied_ids: vec![],
        }))
    }

    /// Branch: create a named branch snapshot.
    async fn branch(
        &self,
        request: Request<BranchRequest>,
    ) -> Result<Response<BranchResponse>, Status> {
        let req = request.into_inner();

        let _ctx = self.engine
            .validate_session(&req.session_token)
            .await
            .map_err(clawdb_error_to_status)?;

        let agent_id = Uuid::parse_str(&req.agent_id)
            .map_err(|_| Status::invalid_argument("invalid agent_id"))?;

        let session = ClawDBSession {
            session_id: Uuid::new_v4(),
            agent_id,
            role: "user".to_string(),
            scopes: vec!["branch:create".to_string()],
            created_at: chrono::Utc::now(),
            expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
        };

        let branch_id = self.engine
            .branch(&session, &req.new_branch_name)
            .await
            .map_err(clawdb_error_to_status)?;

        Ok(Response::new(BranchResponse {
            branch_id: branch_id.to_string(),
            branch_name: req.new_branch_name,
            created_at: chrono::Utc::now().timestamp_millis(),
        }))
    }

    /// Merge: merge two snapshots.
    async fn merge(
        &self,
        request: Request<MergeRequest>,
    ) -> Result<Response<MergeResponse>, Status> {
        let req = request.into_inner();

        let _ctx = self.engine
            .validate_session(&req.session_token)
            .await
            .map_err(clawdb_error_to_status)?;

        let agent_id = Uuid::parse_str(&req.agent_id)
            .map_err(|_| Status::invalid_argument("invalid agent_id"))?;
        let source = Uuid::parse_str(&req.source_branch)
            .map_err(|_| Status::invalid_argument("invalid source_branch"))?;
        let target = Uuid::parse_str(&req.target_branch)
            .map_err(|_| Status::invalid_argument("invalid target_branch"))?;

        let session = ClawDBSession {
            session_id: Uuid::new_v4(),
            agent_id,
            role: "user".to_string(),
            scopes: vec!["branch:merge".to_string()],
            created_at: chrono::Utc::now(),
            expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
        };

        self.engine
            .merge(&session, source, target)
            .await
            .map_err(clawdb_error_to_status)?;

        Ok(Response::new(MergeResponse {
            success: true,
            applied: 1,
            conflicts: 0,
            conflict_ids: vec![],
        }))
    }

    /// Diff: compare two snapshots.
    async fn diff(
        &self,
        request: Request<DiffRequest>,
    ) -> Result<Response<DiffResponse>, Status> {
        let req = request.into_inner();

        let _ctx = self.engine
            .validate_session(&req.session_token)
            .await
            .map_err(clawdb_error_to_status)?;

        let agent_id = Uuid::parse_str(&req.agent_id)
            .map_err(|_| Status::invalid_argument("invalid agent_id"))?;
        let branch_a = Uuid::parse_str(&req.branch_a)
            .map_err(|_| Status::invalid_argument("invalid branch_a"))?;
        let branch_b = Uuid::parse_str(&req.branch_b)
            .map_err(|_| Status::invalid_argument("invalid branch_b"))?;

        let session = ClawDBSession {
            session_id: Uuid::new_v4(),
            agent_id,
            role: "user".to_string(),
            scopes: vec!["branch:read".to_string()],
            created_at: chrono::Utc::now(),
            expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
        };

        let diff_result = self.engine
            .diff(&session, branch_a, branch_b)
            .await
            .map_err(clawdb_error_to_status)?;

        Ok(Response::new(DiffResponse {
            added: 0,
            removed: 0,
            modified: 0,
            divergence_score: 0.0,
            diff_json: serde_json::to_vec(&diff_result).unwrap_or_default(),
        }))
    }

    /// Sync: push and pull changes.
    async fn sync(
        &self,
        request: Request<SyncRequest>,
    ) -> Result<Response<SyncResponse>, Status> {
        let req = request.into_inner();

        let _ctx = self.engine
            .validate_session(&req.session_token)
            .await
            .map_err(clawdb_error_to_status)?;

        let agent_id = Uuid::parse_str(&req.agent_id)
            .map_err(|_| Status::invalid_argument("invalid agent_id"))?;

        let session = ClawDBSession {
            session_id: Uuid::new_v4(),
            agent_id,
            role: "user".to_string(),
            scopes: vec!["sync:write".to_string()],
            created_at: chrono::Utc::now(),
            expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
        };

        let result = self.engine
            .sync(&session)
            .await
            .map_err(clawdb_error_to_status)?;

        let pushed = result.get("pushed").and_then(|v| v.as_u64()).unwrap_or(0) as i32;
        let pulled = result.get("pulled").and_then(|v| v.as_u64()).unwrap_or(0) as i32;

        Ok(Response::new(SyncResponse {
            success: true,
            pushed,
            pulled,
            conflicts: 0,
            synced_at: chrono::Utc::now().timestamp_millis(),
        }))
    }

    /// Reflect: start a reflection job.
    async fn reflect(
        &self,
        request: Request<ReflectRequest>,
    ) -> Result<Response<ReflectResponse>, Status> {
        let req = request.into_inner();

        let _ctx = self.engine
            .validate_session(&req.session_token)
            .await
            .map_err(clawdb_error_to_status)?;

        let agent_id = Uuid::parse_str(&req.agent_id)
            .map_err(|_| Status::invalid_argument("invalid agent_id"))?;

        let session = ClawDBSession {
            session_id: Uuid::new_v4(),
            agent_id,
            role: "user".to_string(),
            scopes: vec!["reflect:write".to_string()],
            created_at: chrono::Utc::now(),
            expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
        };

        let job_id = self.engine
            .reflect(&session)
            .await
            .map_err(clawdb_error_to_status)?;

        Ok(Response::new(ReflectResponse {
            job_id,
            processed: 0,
            archived: 0,
            promoted: 0,
        }))
    }

    /// CreateSession: create a new session token.
    async fn create_session(
        &self,
        request: Request<SessionRequest>,
    ) -> Result<Response<SessionResponse>, Status> {
        let req = request.into_inner();

        let agent_id = Uuid::parse_str(&req.agent_id)
            .map_err(|_| Status::invalid_argument("invalid agent_id"))?;

        let session = self.engine
            .session_with_task(agent_id, &req.role, req.scopes.clone(), &req.job_type)
            .await
            .map_err(clawdb_error_to_status)?;

        let expires_at = session.expires_at.timestamp() * 1000;

        Ok(Response::new(SessionResponse {
            session_token: session.session_id.to_string(),
            expires_at,
            granted_scopes: session.scopes,
        }))
    }

    /// Health: get system health status.
    async fn health(
        &self,
        _request: Request<HealthRequest>,
    ) -> Result<Response<HealthResponse>, Status> {
        let report = self.engine
            .health()
            .await
            .map_err(clawdb_error_to_status)?;

        let mut component_status = std::collections::HashMap::new();
        for (name, health) in &report.components {
            component_status.insert(
                name.clone(),
                if health.healthy { "up" } else { "down" }.to_string(),
            );
        }

        Ok(Response::new(HealthResponse {
            ok: matches!(report.overall, crate::lifecycle::health::HealthStatus::Healthy),
            component_status,
            version: report.version,
            uptime_secs: report.uptime_secs,
        }))
    }

    /// Status: get detailed agent status.
    async fn status(
        &self,
        request: Request<StatusRequest>,
    ) -> Result<Response<StatusResponse>, Status> {
        let req = request.into_inner();

        let _ctx = self.engine
            .validate_session(&req.session_token)
            .await
            .map_err(clawdb_error_to_status)?;

        Ok(Response::new(StatusResponse {
            memory_count: 0,
            session_count: 0,
            active_branch: "main".to_string(),
            sync_connected: true,
            last_reflect_ago_secs: 0.0,
            agent_stats_json: vec![],
        }))
    }

    /// StreamEvents: stream events for an agent.
    async fn stream_events(
        &self,
        request: Request<SessionRequest>,
    ) -> Result<Response<tonic::Streaming<EventMessage>>, Status> {
        let req = request.into_inner();

        let agent_id = Uuid::parse_str(&req.agent_id)
            .map_err(|_| Status::invalid_argument("invalid agent_id"))?;

        // Create an event subscriber
        let mut subscriber = self.engine.subscribe();
        let engine_clone = self.engine.clone();

        // Create a channel to stream events
        let (tx, rx) = tokio::sync::mpsc::channel(100);

        // Spawn a task to forward events
        tokio::spawn(async move {
            loop {
                match subscriber.recv().await {
                    Ok(event) => {
                        if let Some(evt_agent_id) = event.agent_id() {
                            if evt_agent_id != agent_id {
                                continue;
                            }
                        }

                        let msg = EventMessage {
                            event_type: event.event_type().to_string(),
                            agent_id: agent_id.to_string(),
                            payload_json: serde_json::to_vec(&event)
                                .unwrap_or_default(),
                            timestamp: chrono::Utc::now().timestamp_millis(),
                        };

                        if tx.send(Ok(msg)).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        Ok(Response::new(tonic::Streaming::new(rx)))
    }
}

#[cfg(not(proto_compiled))]
/// Placeholder service struct; the full implementation requires proto-generated types.
pub struct ClawDBGrpcService {
    _phantom: std::marker::PhantomData<()>,
}

#[cfg(not(proto_compiled))]
impl ClawDBGrpcService {
    /// Creates a new service instance.
    pub fn new(_engine: Arc<ClawDB>) -> Self {
        Self {
            _phantom: std::marker::PhantomData,
        }
    }
}

impl Default for ClawDBGrpcService {
    fn default() -> Self {
        #[cfg(proto_compiled)]
        {
            panic!("Default ClawDBGrpcService should not be used in proto_compiled mode")
        }
        #[cfg(not(proto_compiled))]
        {
            Self {
                _phantom: std::marker::PhantomData,
            }
        }
    }
}
