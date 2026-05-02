use std::sync::Arc;

use anyhow::Context as _;
use clawdb::{prelude::MergeStrategy, ClawDBError, ClawDBSession};
use tonic::{metadata::MetadataValue, Code, Request, Response, Status};
use uuid::Uuid;

use crate::state::{AppState, PendingTransaction};

pub mod proto {
    tonic::include_proto!("clawdb.v1");
    pub const FILE_DESCRIPTOR_SET: &[u8] = tonic::include_file_descriptor_set!("clawdb_descriptor");
}

use proto::claw_db_service_server::ClawDbService;

pub struct ClawDbServiceImpl {
    state: Arc<AppState>,
}

impl ClawDbServiceImpl {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }

    async fn session_from_request<T>(
        &self,
        request: &Request<T>,
    ) -> Result<(String, ClawDBSession), Status> {
        let token = request
            .metadata()
            .get("x-claw-session")
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| Status::unauthenticated("missing session token"))?
            .to_string();
        let session = self
            .state
            .db
            .validate_session(&token)
            .await
            .map_err(|_| Status::unauthenticated("invalid session token"))?;
        Ok((token, session))
    }

    fn request_id<T>(request: &Request<T>) -> String {
        request
            .metadata()
            .get("x-request-id")
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| Uuid::new_v4().to_string())
    }

    fn set_request_id<T>(response: &mut Response<T>, request_id: &str) {
        if let Ok(value) = MetadataValue::try_from(request_id) {
            response.metadata_mut().insert("x-request-id", value);
        }
    }

    fn observe(&self, method: &str, status: Code) {
        let status_name = match status {
            Code::Ok => "OK",
            Code::Unauthenticated => "UNAUTHENTICATED",
            Code::PermissionDenied => "PERMISSION_DENIED",
            Code::FailedPrecondition => "FAILED_PRECONDITION",
            Code::ResourceExhausted => "RESOURCE_EXHAUSTED",
            Code::InvalidArgument => "INVALID_ARGUMENT",
            Code::NotFound => "NOT_FOUND",
            _ => "INTERNAL",
        };
        self.state.metrics.observe_grpc(method, status_name);
    }

    fn status_with_request_id(mut status: Status, request_id: &str) -> Status {
        if let Ok(value) = MetadataValue::try_from(request_id) {
            status.metadata_mut().insert("x-request-id", value);
        }
        status
    }

    fn map_error(&self, error: ClawDBError, request_id: &str) -> Status {
        let status = match error {
            ClawDBError::PermissionDenied(reason) => Status::permission_denied(reason),
            ClawDBError::SessionInvalid => Status::unauthenticated("session_invalid"),
            ClawDBError::ComponentDisabled(name) => {
                Status::failed_precondition(format!("component_disabled:{name}"))
            }
            ClawDBError::Config(_) | ClawDBError::ComponentInit(_, _) => {
                Status::internal(format!("internal error; request_id={request_id}"))
            }
            other => {
                tracing::error!(request_id, error = %other, "gRPC handler failed");
                Status::internal(format!("internal error; request_id={request_id}"))
            }
        };
        Self::status_with_request_id(status, request_id)
    }

    fn response_with_request_id<T>(
        &self,
        method: &str,
        mut response: Response<T>,
        request_id: &str,
    ) -> Response<T> {
        Self::set_request_id(&mut response, request_id);
        self.observe(method, Code::Ok);
        response
    }

    fn parse_merge_strategy(strategy: &str) -> MergeStrategy {
        match strategy.to_ascii_lowercase().as_str() {
            "ours" => MergeStrategy::Ours,
            "union" => MergeStrategy::Union,
            "manual" => MergeStrategy::Manual,
            _ => MergeStrategy::Theirs,
        }
    }
}

#[tonic::async_trait]
impl ClawDbService for ClawDbServiceImpl {
    async fn health(
        &self,
        request: Request<proto::HealthRequest>,
    ) -> Result<Response<proto::HealthResponse>, Status> {
        let request_id = Self::request_id(&request);
        match self.state.db.health().await {
            Ok(health) => Ok(self.response_with_request_id(
                "Health",
                Response::new(proto::HealthResponse {
                    ok: health.ok,
                    components: health.components,
                    uptime_secs: self.state.db.uptime_secs(),
                    request_id: request_id.clone(),
                }),
                &request_id,
            )),
            Err(error) => {
                let status = self.map_error(error, &request_id);
                self.observe("Health", status.code());
                Err(status)
            }
        }
    }

    async fn create_session(
        &self,
        request: Request<proto::CreateSessionRequest>,
    ) -> Result<Response<proto::CreateSessionResponse>, Status> {
        let request_id = Self::request_id(&request);
        if let Err(status) = self.session_from_request(&request).await {
            self.observe("CreateSession", status.code());
            return Err(Self::status_with_request_id(status, &request_id));
        }
        let inner = request.into_inner();
        let agent_id = match Uuid::parse_str(&inner.agent_id) {
            Ok(agent_id) => agent_id,
            Err(_) => {
                let status = Self::status_with_request_id(
                    Status::invalid_argument("invalid agent_id"),
                    &request_id,
                );
                self.observe("CreateSession", status.code());
                return Err(status);
            }
        };
        match self
            .state
            .db
            .session(agent_id, &inner.role, inner.scopes)
            .await
        {
            Ok(session) => Ok(self.response_with_request_id(
                "CreateSession",
                Response::new(proto::CreateSessionResponse {
                    id: session.id.to_string(),
                    token: session.token,
                    expires_at: session.expires_at.to_rfc3339(),
                    scopes: session.scopes,
                    request_id: request_id.clone(),
                }),
                &request_id,
            )),
            Err(error) => {
                let status = self.map_error(error, &request_id);
                self.observe("CreateSession", status.code());
                Err(status)
            }
        }
    }

    async fn validate_session(
        &self,
        request: Request<proto::ValidateSessionRequest>,
    ) -> Result<Response<proto::ValidateSessionResponse>, Status> {
        let request_id = Self::request_id(&request);
        match self.session_from_request(&request).await {
            Ok((_, session)) => Ok(self.response_with_request_id(
                "ValidateSession",
                Response::new(proto::ValidateSessionResponse {
                    session_id: session.id.to_string(),
                    agent_id: session.agent_id.to_string(),
                    workspace_id: session.workspace_id.to_string(),
                    role: session.role,
                    scopes: session.scopes,
                    expires_at: session.expires_at.to_rfc3339(),
                    request_id: request_id.clone(),
                }),
                &request_id,
            )),
            Err(status) => {
                let status = Self::status_with_request_id(status, &request_id);
                self.observe("ValidateSession", status.code());
                Err(status)
            }
        }
    }

    async fn revoke_session(
        &self,
        request: Request<proto::RevokeSessionRequest>,
    ) -> Result<Response<proto::RevokeSessionResponse>, Status> {
        let request_id = Self::request_id(&request);
        if let Err(status) = self.session_from_request(&request).await {
            self.observe("RevokeSession", status.code());
            return Err(Self::status_with_request_id(status, &request_id));
        }
        let session_id = match Uuid::parse_str(&request.get_ref().session_id) {
            Ok(session_id) => session_id,
            Err(_) => {
                let status = Self::status_with_request_id(
                    Status::invalid_argument("invalid session_id"),
                    &request_id,
                );
                self.observe("RevokeSession", status.code());
                return Err(status);
            }
        };
        match self.state.db.revoke_session(session_id).await {
            Ok(()) => Ok(self.response_with_request_id(
                "RevokeSession",
                Response::new(proto::RevokeSessionResponse {
                    revoked: true,
                    request_id: request_id.clone(),
                }),
                &request_id,
            )),
            Err(error) => {
                let status = self.map_error(error, &request_id);
                self.observe("RevokeSession", status.code());
                Err(status)
            }
        }
    }

    async fn remember(
        &self,
        request: Request<proto::RememberRequest>,
    ) -> Result<Response<proto::RememberResponse>, Status> {
        let request_id = Self::request_id(&request);
        let session = match self.session_from_request(&request).await {
            Ok((_, session)) => session,
            Err(status) => {
                let status = Self::status_with_request_id(status, &request_id);
                self.observe("Remember", status.code());
                return Err(status);
            }
        };
        match self
            .state
            .db
            .remember(&session, &request.get_ref().content)
            .await
        {
            Ok(remembered) => Ok(self.response_with_request_id(
                "Remember",
                Response::new(proto::RememberResponse {
                    memory_id: remembered.memory_id.to_string(),
                    indexed: remembered.indexed,
                    request_id: request_id.clone(),
                }),
                &request_id,
            )),
            Err(error) => {
                let status = self.map_error(error, &request_id);
                self.observe("Remember", status.code());
                Err(status)
            }
        }
    }

    async fn remember_typed(
        &self,
        request: Request<proto::RememberTypedRequest>,
    ) -> Result<Response<proto::RememberResponse>, Status> {
        let request_id = Self::request_id(&request);
        let session = match self.session_from_request(&request).await {
            Ok((_, session)) => session,
            Err(status) => {
                let status = Self::status_with_request_id(status, &request_id);
                self.observe("RememberTyped", status.code());
                return Err(status);
            }
        };
        let inner = request.into_inner();
        let metadata = if inner.metadata_json.trim().is_empty() {
            serde_json::Value::Null
        } else {
            match serde_json::from_str(&inner.metadata_json) {
                Ok(metadata) => metadata,
                Err(_) => {
                    let status = Self::status_with_request_id(
                        Status::invalid_argument("invalid metadata_json"),
                        &request_id,
                    );
                    self.observe("RememberTyped", status.code());
                    return Err(status);
                }
            }
        };
        match self
            .state
            .db
            .remember_typed(
                &session,
                &inner.content,
                &inner.r#type,
                &inner.tags,
                metadata,
            )
            .await
        {
            Ok(remembered) => Ok(self.response_with_request_id(
                "RememberTyped",
                Response::new(proto::RememberResponse {
                    memory_id: remembered.memory_id.to_string(),
                    indexed: remembered.indexed,
                    request_id: request_id.clone(),
                }),
                &request_id,
            )),
            Err(error) => {
                let status = self.map_error(error, &request_id);
                self.observe("RememberTyped", status.code());
                Err(status)
            }
        }
    }

    async fn search(
        &self,
        request: Request<proto::SearchRequest>,
    ) -> Result<Response<proto::SearchResponse>, Status> {
        let request_id = Self::request_id(&request);
        let session = match self.session_from_request(&request).await {
            Ok((_, session)) => session,
            Err(status) => {
                let status = Self::status_with_request_id(status, &request_id);
                self.observe("Search", status.code());
                return Err(status);
            }
        };
        let inner = request.into_inner();
        let filter = if inner.filter_json.trim().is_empty() {
            None
        } else {
            match serde_json::from_str(&inner.filter_json) {
                Ok(filter) => Some(filter),
                Err(_) => {
                    let status = Self::status_with_request_id(
                        Status::invalid_argument("invalid filter_json"),
                        &request_id,
                    );
                    self.observe("Search", status.code());
                    return Err(status);
                }
            }
        };
        match self
            .state
            .db
            .search_with_options(
                &session,
                &inner.query,
                inner.top_k.max(1) as usize,
                inner.semantic,
                filter,
            )
            .await
        {
            Ok(hits) => Ok(self.response_with_request_id(
                "Search",
                Response::new(proto::SearchResponse {
                    hits: hits
                        .into_iter()
                        .map(|hit| proto::SearchHit {
                            id: hit.id.to_string(),
                            score: hit.score,
                            content: hit.content,
                            memory_type: hit.memory_type,
                            tags: hit.tags,
                            metadata_json: hit.metadata.to_string(),
                        })
                        .collect(),
                    request_id: request_id.clone(),
                }),
                &request_id,
            )),
            Err(error) => {
                let status = self.map_error(error, &request_id);
                self.observe("Search", status.code());
                Err(status)
            }
        }
    }

    async fn recall(
        &self,
        request: Request<proto::RecallRequest>,
    ) -> Result<Response<proto::RecallResponse>, Status> {
        let request_id = Self::request_id(&request);
        let session = match self.session_from_request(&request).await {
            Ok((_, session)) => session,
            Err(status) => {
                let status = Self::status_with_request_id(status, &request_id);
                self.observe("Recall", status.code());
                return Err(status);
            }
        };
        let mut ids = Vec::with_capacity(request.get_ref().memory_ids.len());
        for id in &request.get_ref().memory_ids {
            match Uuid::parse_str(id) {
                Ok(parsed) => ids.push(parsed),
                Err(_) => {
                    let status = Self::status_with_request_id(
                        Status::invalid_argument("invalid memory_id"),
                        &request_id,
                    );
                    self.observe("Recall", status.code());
                    return Err(status);
                }
            }
        }
        match self.state.db.recall(&session, &ids).await {
            Ok(memories) => Ok(self.response_with_request_id(
                "Recall",
                Response::new(proto::RecallResponse {
                    memories: memories
                        .into_iter()
                        .map(|memory| proto::MemoryRecord {
                            id: memory.id.to_string(),
                            content: memory.content,
                            memory_type: memory.memory_type.as_str().to_string(),
                            tags: memory.tags,
                        })
                        .collect(),
                    request_id: request_id.clone(),
                }),
                &request_id,
            )),
            Err(error) => {
                let status = self.map_error(error, &request_id);
                self.observe("Recall", status.code());
                Err(status)
            }
        }
    }

    async fn branch(
        &self,
        request: Request<proto::BranchRequest>,
    ) -> Result<Response<proto::BranchResponse>, Status> {
        let request_id = Self::request_id(&request);
        let session = match self.session_from_request(&request).await {
            Ok((_, session)) => session,
            Err(status) => {
                let status = Self::status_with_request_id(status, &request_id);
                self.observe("Branch", status.code());
                return Err(status);
            }
        };
        let inner = request.into_inner();
        let branch_id = if inner.from.is_empty() {
            self.state.db.branch(&session, &inner.name).await
        } else {
            match Uuid::parse_str(&inner.from) {
                Ok(parent) => {
                    self.state
                        .db
                        .fork_branch(&session, parent, &inner.name)
                        .await
                }
                Err(_) => {
                    let status = Self::status_with_request_id(
                        Status::invalid_argument("invalid from branch"),
                        &request_id,
                    );
                    self.observe("Branch", status.code());
                    return Err(status);
                }
            }
        };
        match branch_id {
            Ok(branch_id) => Ok(self.response_with_request_id(
                "Branch",
                Response::new(proto::BranchResponse {
                    branch_id: branch_id.to_string(),
                    name: inner.name,
                    request_id: request_id.clone(),
                }),
                &request_id,
            )),
            Err(error) => {
                let status = self.map_error(error, &request_id);
                self.observe("Branch", status.code());
                Err(status)
            }
        }
    }

    async fn merge(
        &self,
        request: Request<proto::MergeRequest>,
    ) -> Result<Response<proto::MergeResponse>, Status> {
        let request_id = Self::request_id(&request);
        let session = match self.session_from_request(&request).await {
            Ok((_, session)) => session,
            Err(status) => {
                let status = Self::status_with_request_id(status, &request_id);
                self.observe("Merge", status.code());
                return Err(status);
            }
        };
        let source = match Uuid::parse_str(&request.get_ref().source) {
            Ok(source) => source,
            Err(_) => {
                let status = Self::status_with_request_id(
                    Status::invalid_argument("invalid source"),
                    &request_id,
                );
                self.observe("Merge", status.code());
                return Err(status);
            }
        };
        let target = match Uuid::parse_str(&request.get_ref().target) {
            Ok(target) => target,
            Err(_) => {
                let status = Self::status_with_request_id(
                    Status::invalid_argument("invalid target"),
                    &request_id,
                );
                self.observe("Merge", status.code());
                return Err(status);
            }
        };
        match self
            .state
            .db
            .merge_with_strategy(
                &session,
                source,
                target,
                Self::parse_merge_strategy(&request.get_ref().strategy),
            )
            .await
        {
            Ok(result) => Ok(self.response_with_request_id(
                "Merge",
                Response::new(proto::MergeResponse {
                    success: result.success,
                    applied: result.applied,
                    skipped: result.skipped,
                    conflicts: result.conflicts.len() as u32,
                    duration_ms: result.duration_ms,
                    request_id: request_id.clone(),
                }),
                &request_id,
            )),
            Err(error) => {
                let status = self.map_error(error, &request_id);
                self.observe("Merge", status.code());
                Err(status)
            }
        }
    }

    async fn diff(
        &self,
        request: Request<proto::DiffRequest>,
    ) -> Result<Response<proto::DiffResponse>, Status> {
        let request_id = Self::request_id(&request);
        let session = match self.session_from_request(&request).await {
            Ok((_, session)) => session,
            Err(status) => {
                let status = Self::status_with_request_id(status, &request_id);
                self.observe("Diff", status.code());
                return Err(status);
            }
        };
        let branch_id = match Uuid::parse_str(&request.get_ref().branch_id) {
            Ok(branch_id) => branch_id,
            Err(_) => {
                let status = Self::status_with_request_id(
                    Status::invalid_argument("invalid branch_id"),
                    &request_id,
                );
                self.observe("Diff", status.code());
                return Err(status);
            }
        };
        let target = match Uuid::parse_str(&request.get_ref().target) {
            Ok(target) => target,
            Err(_) => {
                let status = Self::status_with_request_id(
                    Status::invalid_argument("invalid target"),
                    &request_id,
                );
                self.observe("Diff", status.code());
                return Err(status);
            }
        };
        match self.state.db.diff(&session, branch_id, target).await {
            Ok(diff) => {
                let diff_json = match serde_json::to_string(&diff)
                    .context("failed to serialize diff")
                {
                    Ok(diff_json) => diff_json,
                    Err(_) => {
                        let status = Self::status_with_request_id(
                            Status::internal(format!("internal error; request_id={request_id}")),
                            &request_id,
                        );
                        self.observe("Diff", status.code());
                        return Err(status);
                    }
                };
                Ok(self.response_with_request_id(
                    "Diff",
                    Response::new(proto::DiffResponse {
                        added: diff.stats.added,
                        removed: diff.stats.removed,
                        modified: diff.stats.modified,
                        unchanged: diff.stats.unchanged,
                        divergence_score: diff.divergence_score as f32,
                        diff_json,
                        request_id: request_id.clone(),
                    }),
                    &request_id,
                ))
            }
            Err(error) => {
                let status = self.map_error(error, &request_id);
                self.observe("Diff", status.code());
                Err(status)
            }
        }
    }

    async fn sync(
        &self,
        request: Request<proto::SyncRequest>,
    ) -> Result<Response<proto::SyncResponse>, Status> {
        let request_id = Self::request_id(&request);
        let session = match self.session_from_request(&request).await {
            Ok((_, session)) => session,
            Err(status) => {
                let status = Self::status_with_request_id(status, &request_id);
                self.observe("Sync", status.code());
                return Err(status);
            }
        };
        match self.state.db.sync(&session).await {
            Ok(result) => Ok(self.response_with_request_id(
                "Sync",
                Response::new(proto::SyncResponse {
                    pushed: result.pushed,
                    pulled: result.pulled,
                    conflicts: result.conflicts,
                    duration_ms: result.duration_ms,
                    request_id: request_id.clone(),
                }),
                &request_id,
            )),
            Err(error) => {
                let status = self.map_error(error, &request_id);
                self.observe("Sync", status.code());
                Err(status)
            }
        }
    }

    async fn reflect(
        &self,
        request: Request<proto::ReflectRequest>,
    ) -> Result<Response<proto::ReflectResponse>, Status> {
        let request_id = Self::request_id(&request);
        let session = match self.session_from_request(&request).await {
            Ok((_, session)) => session,
            Err(status) => {
                let status = Self::status_with_request_id(status, &request_id);
                self.observe("Reflect", status.code());
                return Err(status);
            }
        };
        match self.state.db.reflect(&session).await {
            Ok(result) => Ok(self.response_with_request_id(
                "Reflect",
                Response::new(proto::ReflectResponse {
                    job_id: result.job_id.unwrap_or_default(),
                    status: result.status,
                    message: result.message,
                    skipped: result.skipped,
                    request_id: request_id.clone(),
                }),
                &request_id,
            )),
            Err(error) => {
                let status = self.map_error(error, &request_id);
                self.observe("Reflect", status.code());
                Err(status)
            }
        }
    }

    async fn begin_tx(
        &self,
        request: Request<proto::BeginTxRequest>,
    ) -> Result<Response<proto::BeginTxResponse>, Status> {
        let request_id = Self::request_id(&request);
        let session = match self.session_from_request(&request).await {
            Ok((_, session)) => session,
            Err(status) => {
                let status = Self::status_with_request_id(status, &request_id);
                self.observe("BeginTx", status.code());
                return Err(status);
            }
        };
        let tx_id = Uuid::new_v4();
        self.state
            .transactions
            .lock()
            .await
            .insert(tx_id, PendingTransaction { id: tx_id, session });
        Ok(self.response_with_request_id(
            "BeginTx",
            Response::new(proto::BeginTxResponse {
                tx_id: tx_id.to_string(),
                request_id: request_id.clone(),
            }),
            &request_id,
        ))
    }

    async fn commit_tx(
        &self,
        request: Request<proto::CommitTxRequest>,
    ) -> Result<Response<proto::CommitTxResponse>, Status> {
        let request_id = Self::request_id(&request);
        let tx_id = match Uuid::parse_str(&request.get_ref().tx_id) {
            Ok(tx_id) => tx_id,
            Err(_) => {
                let status = Self::status_with_request_id(
                    Status::invalid_argument("invalid tx_id"),
                    &request_id,
                );
                self.observe("CommitTx", status.code());
                return Err(status);
            }
        };
        let pending = match self.state.transactions.lock().await.remove(&tx_id) {
            Some(pending) => pending,
            None => {
                let status = Self::status_with_request_id(
                    Status::not_found("transaction not found"),
                    &request_id,
                );
                self.observe("CommitTx", status.code());
                return Err(status);
            }
        };
        match self.state.db.transaction(&pending.session).await {
            Ok(tx) => match tx.commit().await {
                Ok(()) => Ok(self.response_with_request_id(
                    "CommitTx",
                    Response::new(proto::CommitTxResponse {
                        committed: true,
                        request_id: request_id.clone(),
                    }),
                    &request_id,
                )),
                Err(error) => {
                    let status = self.map_error(error, &request_id);
                    self.observe("CommitTx", status.code());
                    Err(status)
                }
            },
            Err(error) => {
                let status = self.map_error(error, &request_id);
                self.observe("CommitTx", status.code());
                Err(status)
            }
        }
    }

    async fn rollback_tx(
        &self,
        request: Request<proto::RollbackTxRequest>,
    ) -> Result<Response<proto::RollbackTxResponse>, Status> {
        let request_id = Self::request_id(&request);
        let tx_id = match Uuid::parse_str(&request.get_ref().tx_id) {
            Ok(tx_id) => tx_id,
            Err(_) => {
                let status = Self::status_with_request_id(
                    Status::invalid_argument("invalid tx_id"),
                    &request_id,
                );
                self.observe("RollbackTx", status.code());
                return Err(status);
            }
        };
        let pending = match self.state.transactions.lock().await.remove(&tx_id) {
            Some(pending) => pending,
            None => {
                let status = Self::status_with_request_id(
                    Status::not_found("transaction not found"),
                    &request_id,
                );
                self.observe("RollbackTx", status.code());
                return Err(status);
            }
        };
        match self.state.db.transaction(&pending.session).await {
            Ok(tx) => match tx.rollback().await {
                Ok(()) => Ok(self.response_with_request_id(
                    "RollbackTx",
                    Response::new(proto::RollbackTxResponse {
                        rolled_back: true,
                        request_id: request_id.clone(),
                    }),
                    &request_id,
                )),
                Err(error) => {
                    let status = self.map_error(error, &request_id);
                    self.observe("RollbackTx", status.code());
                    Err(status)
                }
            },
            Err(error) => {
                let status = self.map_error(error, &request_id);
                self.observe("RollbackTx", status.code());
                Err(status)
            }
        }
    }
}
