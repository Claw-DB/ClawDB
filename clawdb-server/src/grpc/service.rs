use std::sync::Arc;

use anyhow::Context as _;
use clawdb::{prelude::MergeStrategy, ClawDBError, ClawDBSession};
use tonic::{metadata::MetadataValue, Code, Request, Response, Status};
use uuid::Uuid;

use crate::state::{AppState, PendingTransaction, StagedMemory};

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

    fn map_memory_record(memory: clawdb::types::MemoryRecord) -> proto::MemoryRecord {
        proto::MemoryRecord {
            id: memory.id.to_string(),
            content: memory.content,
            memory_type: memory.memory_type.as_str().to_string(),
            tags: memory.tags,
        }
    }

    fn map_branch_record<T: serde::Serialize>(branch: T) -> Result<proto::BranchRecord, Status> {
        let branch_json = serde_json::to_string(&branch)
            .map_err(|_| Status::internal("failed to serialize branch"))?;
        let id = serde_json::to_value(&branch)
            .ok()
            .and_then(|value| {
                value
                    .get("id")
                    .cloned()
                    .or_else(|| value.get("branch_id").cloned())
            })
            .and_then(|value| value.as_str().map(ToOwned::to_owned))
            .unwrap_or_default();
        Ok(proto::BranchRecord {
            branch_id: id,
            branch_json,
        })
    }

    fn json_response<T: serde::Serialize>(
        value: &T,
        request_id: &str,
    ) -> Result<proto::JsonResponse, Status> {
        let json = serde_json::to_string(value).map_err(|_| {
            Self::status_with_request_id(
                Status::internal("failed to serialize response"),
                request_id,
            )
        })?;
        Ok(proto::JsonResponse {
            json,
            request_id: request_id.to_string(),
        })
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
            .session_with_ttl(
                agent_id,
                &inner.role,
                inner.scopes,
                if inner.ttl_secs == 0 {
                    3600
                } else {
                    inner.ttl_secs as i64
                },
            )
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

    async fn update_memory(
        &self,
        request: Request<proto::UpdateMemoryRequest>,
    ) -> Result<Response<proto::UpdateMemoryResponse>, Status> {
        let request_id = Self::request_id(&request);
        let session = match self.session_from_request(&request).await {
            Ok((_, session)) => session,
            Err(status) => {
                let status = Self::status_with_request_id(status, &request_id);
                self.observe("UpdateMemory", status.code());
                return Err(status);
            }
        };
        let memory_id = match Uuid::parse_str(&request.get_ref().memory_id) {
            Ok(memory_id) => memory_id,
            Err(_) => {
                let status = Self::status_with_request_id(
                    Status::invalid_argument("invalid memory_id"),
                    &request_id,
                );
                self.observe("UpdateMemory", status.code());
                return Err(status);
            }
        };
        match self
            .state
            .db
            .update_memory(&session, memory_id, &request.get_ref().content)
            .await
        {
            Ok(()) => Ok(self.response_with_request_id(
                "UpdateMemory",
                Response::new(proto::UpdateMemoryResponse {
                    updated: true,
                    request_id: request_id.clone(),
                }),
                &request_id,
            )),
            Err(error) => {
                let status = self.map_error(error, &request_id);
                self.observe("UpdateMemory", status.code());
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
                    memories: memories.into_iter().map(Self::map_memory_record).collect(),
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

    async fn list_memories(
        &self,
        request: Request<proto::ListMemoriesRequest>,
    ) -> Result<Response<proto::ListMemoriesResponse>, Status> {
        let request_id = Self::request_id(&request);
        let session = match self.session_from_request(&request).await {
            Ok((_, session)) => session,
            Err(status) => {
                let status = Self::status_with_request_id(status, &request_id);
                self.observe("ListMemories", status.code());
                return Err(status);
            }
        };
        let inner = request.into_inner();
        let memory_type = if inner.r#type.trim().is_empty() {
            None
        } else {
            Some(inner.r#type.as_str())
        };
        match self.state.db.list_memories(&session, memory_type).await {
            Ok(mut memories) => {
                if inner.limit > 0 {
                    memories.truncate(inner.limit as usize);
                }
                Ok(self.response_with_request_id(
                    "ListMemories",
                    Response::new(proto::ListMemoriesResponse {
                        memories: memories.into_iter().map(Self::map_memory_record).collect(),
                        request_id: request_id.clone(),
                    }),
                    &request_id,
                ))
            }
            Err(error) => {
                let status = self.map_error(error, &request_id);
                self.observe("ListMemories", status.code());
                Err(status)
            }
        }
    }

    async fn delete_memory(
        &self,
        request: Request<proto::DeleteMemoryRequest>,
    ) -> Result<Response<proto::DeleteMemoryResponse>, Status> {
        let request_id = Self::request_id(&request);
        let session = match self.session_from_request(&request).await {
            Ok((_, session)) => session,
            Err(status) => {
                let status = Self::status_with_request_id(status, &request_id);
                self.observe("DeleteMemory", status.code());
                return Err(status);
            }
        };
        let memory_id = match Uuid::parse_str(&request.get_ref().memory_id) {
            Ok(memory_id) => memory_id,
            Err(_) => {
                let status = Self::status_with_request_id(
                    Status::invalid_argument("invalid memory_id"),
                    &request_id,
                );
                self.observe("DeleteMemory", status.code());
                return Err(status);
            }
        };
        match self.state.db.delete_memory(&session, memory_id).await {
            Ok(()) => Ok(self.response_with_request_id(
                "DeleteMemory",
                Response::new(proto::DeleteMemoryResponse {
                    deleted: true,
                    request_id: request_id.clone(),
                }),
                &request_id,
            )),
            Err(error) => {
                let status = self.map_error(error, &request_id);
                self.observe("DeleteMemory", status.code());
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

    async fn get_branch(
        &self,
        request: Request<proto::GetBranchRequest>,
    ) -> Result<Response<proto::GetBranchResponse>, Status> {
        let request_id = Self::request_id(&request);
        let session = match self.session_from_request(&request).await {
            Ok((_, session)) => session,
            Err(status) => {
                let status = Self::status_with_request_id(status, &request_id);
                self.observe("GetBranch", status.code());
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
                self.observe("GetBranch", status.code());
                return Err(status);
            }
        };
        match self.state.db.get_branch(&session, branch_id).await {
            Ok(branch) => {
                let branch = match Self::map_branch_record(branch) {
                    Ok(branch) => branch,
                    Err(status) => {
                        let status = Self::status_with_request_id(status, &request_id);
                        self.observe("GetBranch", status.code());
                        return Err(status);
                    }
                };
                Ok(self.response_with_request_id(
                    "GetBranch",
                    Response::new(proto::GetBranchResponse {
                        branch: Some(branch),
                        request_id: request_id.clone(),
                    }),
                    &request_id,
                ))
            }
            Err(error) => {
                let status = self.map_error(error, &request_id);
                self.observe("GetBranch", status.code());
                Err(status)
            }
        }
    }

    async fn get_branch_by_name(
        &self,
        request: Request<proto::GetBranchByNameRequest>,
    ) -> Result<Response<proto::GetBranchResponse>, Status> {
        let request_id = Self::request_id(&request);
        let session = match self.session_from_request(&request).await {
            Ok((_, session)) => session,
            Err(status) => {
                let status = Self::status_with_request_id(status, &request_id);
                self.observe("GetBranchByName", status.code());
                return Err(status);
            }
        };
        match self
            .state
            .db
            .get_branch_by_name(&session, request.get_ref().name.as_str())
            .await
        {
            Ok(branch) => {
                let branch = match Self::map_branch_record(branch) {
                    Ok(branch) => branch,
                    Err(status) => {
                        let status = Self::status_with_request_id(status, &request_id);
                        self.observe("GetBranchByName", status.code());
                        return Err(status);
                    }
                };
                Ok(self.response_with_request_id(
                    "GetBranchByName",
                    Response::new(proto::GetBranchResponse {
                        branch: Some(branch),
                        request_id: request_id.clone(),
                    }),
                    &request_id,
                ))
            }
            Err(error) => {
                let status = self.map_error(error, &request_id);
                self.observe("GetBranchByName", status.code());
                Err(status)
            }
        }
    }

    async fn get_trunk_branch(
        &self,
        request: Request<proto::GetTrunkBranchRequest>,
    ) -> Result<Response<proto::GetBranchResponse>, Status> {
        let request_id = Self::request_id(&request);
        let session = match self.session_from_request(&request).await {
            Ok((_, session)) => session,
            Err(status) => {
                let status = Self::status_with_request_id(status, &request_id);
                self.observe("GetTrunkBranch", status.code());
                return Err(status);
            }
        };
        match self.state.db.trunk_branch(&session).await {
            Ok(branch) => {
                let branch = match Self::map_branch_record(branch) {
                    Ok(branch) => branch,
                    Err(status) => {
                        let status = Self::status_with_request_id(status, &request_id);
                        self.observe("GetTrunkBranch", status.code());
                        return Err(status);
                    }
                };
                Ok(self.response_with_request_id(
                    "GetTrunkBranch",
                    Response::new(proto::GetBranchResponse {
                        branch: Some(branch),
                        request_id: request_id.clone(),
                    }),
                    &request_id,
                ))
            }
            Err(error) => {
                let status = self.map_error(error, &request_id);
                self.observe("GetTrunkBranch", status.code());
                Err(status)
            }
        }
    }

    async fn list_branches(
        &self,
        request: Request<proto::ListBranchesRequest>,
    ) -> Result<Response<proto::ListBranchesResponse>, Status> {
        let request_id = Self::request_id(&request);
        let session = match self.session_from_request(&request).await {
            Ok((_, session)) => session,
            Err(status) => {
                let status = Self::status_with_request_id(status, &request_id);
                self.observe("ListBranches", status.code());
                return Err(status);
            }
        };
        match self.state.db.list_branches(&session).await {
            Ok(branches) => {
                let mut items = Vec::with_capacity(branches.len());
                for branch in branches {
                    items.push(match Self::map_branch_record(branch) {
                        Ok(item) => item,
                        Err(status) => {
                            let status = Self::status_with_request_id(status, &request_id);
                            self.observe("ListBranches", status.code());
                            return Err(status);
                        }
                    });
                }
                Ok(self.response_with_request_id(
                    "ListBranches",
                    Response::new(proto::ListBranchesResponse {
                        branches: items,
                        request_id: request_id.clone(),
                    }),
                    &request_id,
                ))
            }
            Err(error) => {
                let status = self.map_error(error, &request_id);
                self.observe("ListBranches", status.code());
                Err(status)
            }
        }
    }

    async fn discard_branch(
        &self,
        request: Request<proto::DiscardBranchRequest>,
    ) -> Result<Response<proto::DiscardBranchResponse>, Status> {
        let request_id = Self::request_id(&request);
        let session = match self.session_from_request(&request).await {
            Ok((_, session)) => session,
            Err(status) => {
                let status = Self::status_with_request_id(status, &request_id);
                self.observe("DiscardBranch", status.code());
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
                self.observe("DiscardBranch", status.code());
                return Err(status);
            }
        };
        match self.state.db.discard_branch(&session, branch_id).await {
            Ok(()) => Ok(self.response_with_request_id(
                "DiscardBranch",
                Response::new(proto::DiscardBranchResponse {
                    discarded: true,
                    request_id: request_id.clone(),
                }),
                &request_id,
            )),
            Err(error) => {
                let status = self.map_error(error, &request_id);
                self.observe("DiscardBranch", status.code());
                Err(status)
            }
        }
    }

    async fn archive_branch(
        &self,
        request: Request<proto::ArchiveBranchRequest>,
    ) -> Result<Response<proto::ArchiveBranchResponse>, Status> {
        let request_id = Self::request_id(&request);
        let session = match self.session_from_request(&request).await {
            Ok((_, session)) => session,
            Err(status) => {
                let status = Self::status_with_request_id(status, &request_id);
                self.observe("ArchiveBranch", status.code());
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
                self.observe("ArchiveBranch", status.code());
                return Err(status);
            }
        };
        match self.state.db.archive_branch(&session, branch_id).await {
            Ok(()) => Ok(self.response_with_request_id(
                "ArchiveBranch",
                Response::new(proto::ArchiveBranchResponse {
                    archived: true,
                    request_id: request_id.clone(),
                }),
                &request_id,
            )),
            Err(error) => {
                let status = self.map_error(error, &request_id);
                self.observe("ArchiveBranch", status.code());
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

    async fn push_sync(
        &self,
        request: Request<proto::PushSyncRequest>,
    ) -> Result<Response<proto::SyncActionResponse>, Status> {
        let request_id = Self::request_id(&request);
        let session = match self.session_from_request(&request).await {
            Ok((_, session)) => session,
            Err(status) => {
                let status = Self::status_with_request_id(status, &request_id);
                self.observe("PushSync", status.code());
                return Err(status);
            }
        };
        match self.state.db.push_sync(&session).await {
            Ok(stats) => {
                let summary_json = match serde_json::to_string(&stats) {
                    Ok(summary_json) => summary_json,
                    Err(_) => {
                        let status = Self::status_with_request_id(
                            Status::internal("failed to serialize push stats"),
                            &request_id,
                        );
                        self.observe("PushSync", status.code());
                        return Err(status);
                    }
                };
                Ok(self.response_with_request_id(
                    "PushSync",
                    Response::new(proto::SyncActionResponse {
                        summary_json,
                        request_id: request_id.clone(),
                    }),
                    &request_id,
                ))
            }
            Err(error) => {
                let status = self.map_error(error, &request_id);
                self.observe("PushSync", status.code());
                Err(status)
            }
        }
    }

    async fn pull_sync(
        &self,
        request: Request<proto::PullSyncRequest>,
    ) -> Result<Response<proto::SyncActionResponse>, Status> {
        let request_id = Self::request_id(&request);
        let session = match self.session_from_request(&request).await {
            Ok((_, session)) => session,
            Err(status) => {
                let status = Self::status_with_request_id(status, &request_id);
                self.observe("PullSync", status.code());
                return Err(status);
            }
        };
        match self.state.db.pull_sync(&session).await {
            Ok(stats) => {
                let summary_json = match serde_json::to_string(&stats) {
                    Ok(summary_json) => summary_json,
                    Err(_) => {
                        let status = Self::status_with_request_id(
                            Status::internal("failed to serialize pull stats"),
                            &request_id,
                        );
                        self.observe("PullSync", status.code());
                        return Err(status);
                    }
                };
                Ok(self.response_with_request_id(
                    "PullSync",
                    Response::new(proto::SyncActionResponse {
                        summary_json,
                        request_id: request_id.clone(),
                    }),
                    &request_id,
                ))
            }
            Err(error) => {
                let status = self.map_error(error, &request_id);
                self.observe("PullSync", status.code());
                Err(status)
            }
        }
    }

    async fn reconcile_sync(
        &self,
        request: Request<proto::ReconcileSyncRequest>,
    ) -> Result<Response<proto::SyncActionResponse>, Status> {
        let request_id = Self::request_id(&request);
        let session = match self.session_from_request(&request).await {
            Ok((_, session)) => session,
            Err(status) => {
                let status = Self::status_with_request_id(status, &request_id);
                self.observe("ReconcileSync", status.code());
                return Err(status);
            }
        };
        match self.state.db.reconcile_sync(&session).await {
            Ok(stats) => {
                let summary_json = match serde_json::to_string(&stats) {
                    Ok(summary_json) => summary_json,
                    Err(_) => {
                        let status = Self::status_with_request_id(
                            Status::internal("failed to serialize reconcile stats"),
                            &request_id,
                        );
                        self.observe("ReconcileSync", status.code());
                        return Err(status);
                    }
                };
                Ok(self.response_with_request_id(
                    "ReconcileSync",
                    Response::new(proto::SyncActionResponse {
                        summary_json,
                        request_id: request_id.clone(),
                    }),
                    &request_id,
                ))
            }
            Err(error) => {
                let status = self.map_error(error, &request_id);
                self.observe("ReconcileSync", status.code());
                Err(status)
            }
        }
    }

    async fn sync_status(
        &self,
        request: Request<proto::SyncStatusRequest>,
    ) -> Result<Response<proto::SyncStatusResponse>, Status> {
        let request_id = Self::request_id(&request);
        let session = match self.session_from_request(&request).await {
            Ok((_, session)) => session,
            Err(status) => {
                let status = Self::status_with_request_id(status, &request_id);
                self.observe("SyncStatus", status.code());
                return Err(status);
            }
        };
        match self.state.db.sync_status(&session).await {
            Ok(status_payload) => {
                let status_json = match serde_json::to_string(&status_payload) {
                    Ok(status_json) => status_json,
                    Err(_) => {
                        let status = Self::status_with_request_id(
                            Status::internal("failed to serialize sync status"),
                            &request_id,
                        );
                        self.observe("SyncStatus", status.code());
                        return Err(status);
                    }
                };
                Ok(self.response_with_request_id(
                    "SyncStatus",
                    Response::new(proto::SyncStatusResponse {
                        status_json,
                        request_id: request_id.clone(),
                    }),
                    &request_id,
                ))
            }
            Err(error) => {
                let status = self.map_error(error, &request_id);
                self.observe("SyncStatus", status.code());
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

    async fn reflect_get_facts(
        &self,
        request: Request<proto::ReflectGetFactsRequest>,
    ) -> Result<Response<proto::JsonResponse>, Status> {
        let request_id = Self::request_id(&request);
        let session = match self.session_from_request(&request).await {
            Ok((_, session)) => session,
            Err(status) => {
                let status = Self::status_with_request_id(status, &request_id);
                self.observe("ReflectGetFacts", status.code());
                return Err(status);
            }
        };
        match self
            .state
            .db
            .reflect_get_facts(&session, &request.get_ref().agent_id)
            .await
        {
            Ok(facts) => {
                let payload = match Self::json_response(&facts, &request_id) {
                    Ok(payload) => payload,
                    Err(status) => {
                        self.observe("ReflectGetFacts", status.code());
                        return Err(status);
                    }
                };
                Ok(self.response_with_request_id(
                    "ReflectGetFacts",
                    Response::new(payload),
                    &request_id,
                ))
            }
            Err(error) => {
                let status = self.map_error(error, &request_id);
                self.observe("ReflectGetFacts", status.code());
                Err(status)
            }
        }
    }

    async fn reflect_list_jobs(
        &self,
        request: Request<proto::ReflectListJobsRequest>,
    ) -> Result<Response<proto::JsonResponse>, Status> {
        let request_id = Self::request_id(&request);
        let session = match self.session_from_request(&request).await {
            Ok((_, session)) => session,
            Err(status) => {
                let status = Self::status_with_request_id(status, &request_id);
                self.observe("ReflectListJobs", status.code());
                return Err(status);
            }
        };
        let inner = request.into_inner();
        let agent_id = if inner.agent_id.trim().is_empty() {
            None
        } else {
            Some(inner.agent_id.as_str())
        };
        let status_filter = if inner.status.trim().is_empty() {
            None
        } else {
            Some(inner.status.as_str())
        };
        let limit = if inner.limit == 0 {
            None
        } else {
            Some(inner.limit)
        };
        let offset = if inner.offset == 0 {
            None
        } else {
            Some(inner.offset)
        };
        match self
            .state
            .db
            .reflect_list_jobs(&session, agent_id, status_filter, limit, offset)
            .await
        {
            Ok(jobs) => {
                let payload = match Self::json_response(&jobs, &request_id) {
                    Ok(payload) => payload,
                    Err(status) => {
                        self.observe("ReflectListJobs", status.code());
                        return Err(status);
                    }
                };
                Ok(self.response_with_request_id(
                    "ReflectListJobs",
                    Response::new(payload),
                    &request_id,
                ))
            }
            Err(error) => {
                let status = self.map_error(error, &request_id);
                self.observe("ReflectListJobs", status.code());
                Err(status)
            }
        }
    }

    async fn reflect_get_job(
        &self,
        request: Request<proto::ReflectGetJobRequest>,
    ) -> Result<Response<proto::JsonResponse>, Status> {
        let request_id = Self::request_id(&request);
        let session = match self.session_from_request(&request).await {
            Ok((_, session)) => session,
            Err(status) => {
                let status = Self::status_with_request_id(status, &request_id);
                self.observe("ReflectGetJob", status.code());
                return Err(status);
            }
        };
        match self
            .state
            .db
            .reflect_get_job(&session, &request.get_ref().job_id)
            .await
        {
            Ok(job) => {
                let payload = match Self::json_response(&job, &request_id) {
                    Ok(payload) => payload,
                    Err(status) => {
                        self.observe("ReflectGetJob", status.code());
                        return Err(status);
                    }
                };
                Ok(self.response_with_request_id(
                    "ReflectGetJob",
                    Response::new(payload),
                    &request_id,
                ))
            }
            Err(error) => {
                let status = self.map_error(error, &request_id);
                self.observe("ReflectGetJob", status.code());
                Err(status)
            }
        }
    }

    async fn reflect_get_preferences(
        &self,
        request: Request<proto::ReflectGetPreferencesRequest>,
    ) -> Result<Response<proto::JsonResponse>, Status> {
        let request_id = Self::request_id(&request);
        let session = match self.session_from_request(&request).await {
            Ok((_, session)) => session,
            Err(status) => {
                let status = Self::status_with_request_id(status, &request_id);
                self.observe("ReflectGetPreferences", status.code());
                return Err(status);
            }
        };
        match self
            .state
            .db
            .reflect_get_preferences(&session, &request.get_ref().agent_id)
            .await
        {
            Ok(preferences) => {
                let payload = match Self::json_response(&preferences, &request_id) {
                    Ok(payload) => payload,
                    Err(status) => {
                        self.observe("ReflectGetPreferences", status.code());
                        return Err(status);
                    }
                };
                Ok(self.response_with_request_id(
                    "ReflectGetPreferences",
                    Response::new(payload),
                    &request_id,
                ))
            }
            Err(error) => {
                let status = self.map_error(error, &request_id);
                self.observe("ReflectGetPreferences", status.code());
                Err(status)
            }
        }
    }

    async fn reflect_get_contradictions(
        &self,
        request: Request<proto::ReflectGetContradictionsRequest>,
    ) -> Result<Response<proto::JsonResponse>, Status> {
        let request_id = Self::request_id(&request);
        let session = match self.session_from_request(&request).await {
            Ok((_, session)) => session,
            Err(status) => {
                let status = Self::status_with_request_id(status, &request_id);
                self.observe("ReflectGetContradictions", status.code());
                return Err(status);
            }
        };
        match self
            .state
            .db
            .reflect_get_contradictions(&session, &request.get_ref().agent_id)
            .await
        {
            Ok(contradictions) => {
                let payload = match Self::json_response(&contradictions, &request_id) {
                    Ok(payload) => payload,
                    Err(status) => {
                        self.observe("ReflectGetContradictions", status.code());
                        return Err(status);
                    }
                };
                Ok(self.response_with_request_id(
                    "ReflectGetContradictions",
                    Response::new(payload),
                    &request_id,
                ))
            }
            Err(error) => {
                let status = self.map_error(error, &request_id);
                self.observe("ReflectGetContradictions", status.code());
                Err(status)
            }
        }
    }

    async fn reflect_resolve_contradiction(
        &self,
        request: Request<proto::ReflectResolveContradictionRequest>,
    ) -> Result<Response<proto::JsonResponse>, Status> {
        let request_id = Self::request_id(&request);
        let session = match self.session_from_request(&request).await {
            Ok((_, session)) => session,
            Err(status) => {
                let status = Self::status_with_request_id(status, &request_id);
                self.observe("ReflectResolveContradiction", status.code());
                return Err(status);
            }
        };
        let inner = request.into_inner();
        let merged_value = if inner.merged_value_json.trim().is_empty() {
            None
        } else {
            match serde_json::from_str(&inner.merged_value_json) {
                Ok(value) => Some(value),
                Err(_) => {
                    let status = Self::status_with_request_id(
                        Status::invalid_argument("invalid merged_value_json"),
                        &request_id,
                    );
                    self.observe("ReflectResolveContradiction", status.code());
                    return Err(status);
                }
            }
        };
        match self
            .state
            .db
            .reflect_resolve_contradiction(
                &session,
                &inner.agent_id,
                &inner.contradiction_id,
                &inner.strategy,
                merged_value,
            )
            .await
        {
            Ok(contradiction) => {
                let payload = match Self::json_response(&contradiction, &request_id) {
                    Ok(payload) => payload,
                    Err(status) => {
                        self.observe("ReflectResolveContradiction", status.code());
                        return Err(status);
                    }
                };
                Ok(self.response_with_request_id(
                    "ReflectResolveContradiction",
                    Response::new(payload),
                    &request_id,
                ))
            }
            Err(error) => {
                let status = self.map_error(error, &request_id);
                self.observe("ReflectResolveContradiction", status.code());
                Err(status)
            }
        }
    }

    async fn active_session_count(
        &self,
        request: Request<proto::ActiveSessionCountRequest>,
    ) -> Result<Response<proto::ActiveSessionCountResponse>, Status> {
        let request_id = Self::request_id(&request);
        if let Err(status) = self.session_from_request(&request).await {
            let status = Self::status_with_request_id(status, &request_id);
            self.observe("ActiveSessionCount", status.code());
            return Err(status);
        }
        match self.state.db.active_session_count().await {
            Ok(count) => Ok(self.response_with_request_id(
                "ActiveSessionCount",
                Response::new(proto::ActiveSessionCountResponse {
                    count,
                    request_id: request_id.clone(),
                }),
                &request_id,
            )),
            Err(error) => {
                let status = self.map_error(error, &request_id);
                self.observe("ActiveSessionCount", status.code());
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
        self.state.transactions.lock().await.insert(
            tx_id,
            PendingTransaction {
                id: tx_id,
                session,
                staged_memories: Vec::new(),
            },
        );
        Ok(self.response_with_request_id(
            "BeginTx",
            Response::new(proto::BeginTxResponse {
                tx_id: tx_id.to_string(),
                request_id: request_id.clone(),
            }),
            &request_id,
        ))
    }

    async fn tx_remember(
        &self,
        request: Request<proto::TxRememberRequest>,
    ) -> Result<Response<proto::TxRememberResponse>, Status> {
        let request_id = Self::request_id(&request);
        let inner = request.into_inner();
        let tx_id = match Uuid::parse_str(&inner.tx_id) {
            Ok(tx_id) => tx_id,
            Err(_) => {
                let status = Self::status_with_request_id(
                    Status::invalid_argument("invalid tx_id"),
                    &request_id,
                );
                self.observe("TxRemember", status.code());
                return Err(status);
            }
        };

        let staged_id = Uuid::new_v4();
        let mut transactions = self.state.transactions.lock().await;
        let Some(pending) = transactions.get_mut(&tx_id) else {
            let status = Self::status_with_request_id(
                Status::not_found("transaction not found"),
                &request_id,
            );
            self.observe("TxRemember", status.code());
            return Err(status);
        };
        pending.staged_memories.push(StagedMemory {
            content: inner.content,
            memory_type: "semantic".to_string(),
            tags: Vec::new(),
            metadata: serde_json::Value::Null,
        });

        Ok(self.response_with_request_id(
            "TxRemember",
            Response::new(proto::TxRememberResponse {
                memory_id: staged_id.to_string(),
                request_id: request_id.clone(),
            }),
            &request_id,
        ))
    }

    async fn tx_remember_typed(
        &self,
        request: Request<proto::TxRememberTypedRequest>,
    ) -> Result<Response<proto::TxRememberResponse>, Status> {
        let request_id = Self::request_id(&request);
        let inner = request.into_inner();
        let tx_id = match Uuid::parse_str(&inner.tx_id) {
            Ok(tx_id) => tx_id,
            Err(_) => {
                let status = Self::status_with_request_id(
                    Status::invalid_argument("invalid tx_id"),
                    &request_id,
                );
                self.observe("TxRememberTyped", status.code());
                return Err(status);
            }
        };
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
                    self.observe("TxRememberTyped", status.code());
                    return Err(status);
                }
            }
        };

        let staged_id = Uuid::new_v4();
        let mut transactions = self.state.transactions.lock().await;
        let Some(pending) = transactions.get_mut(&tx_id) else {
            let status = Self::status_with_request_id(
                Status::not_found("transaction not found"),
                &request_id,
            );
            self.observe("TxRememberTyped", status.code());
            return Err(status);
        };
        pending.staged_memories.push(StagedMemory {
            content: inner.content,
            memory_type: inner.r#type,
            tags: inner.tags,
            metadata,
        });

        Ok(self.response_with_request_id(
            "TxRememberTyped",
            Response::new(proto::TxRememberResponse {
                memory_id: staged_id.to_string(),
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
            Ok(mut tx) => {
                for staged in pending.staged_memories {
                    if let Err(error) = tx
                        .remember_typed(
                            &staged.content,
                            &staged.memory_type,
                            &staged.tags,
                            staged.metadata,
                        )
                        .await
                    {
                        let status = self.map_error(error, &request_id);
                        self.observe("CommitTx", status.code());
                        return Err(status);
                    }
                }
                match tx.commit().await {
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
                }
            }
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
