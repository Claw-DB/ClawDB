use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::{HeaderValue, StatusCode},
    middleware,
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Extension, Json, Router,
};
use clawdb::{prelude::MergeStrategy, ClawDBError};
use serde::Deserialize;
use tower_http::{
    limit::RequestBodyLimitLayer, normalize_path::NormalizePathLayer,
    set_header::SetResponseHeaderLayer,
};
use uuid::Uuid;

use crate::{
    http::auth::{self, AuthContext},
    state::{AppState, RequestId, StagedMemory},
};

#[derive(Deserialize)]
struct CreateSessionBody {
    agent_id: Uuid,
    role: String,
    scopes: Vec<String>,
    #[serde(default)]
    ttl_secs: Option<u64>,
}

#[derive(Deserialize)]
struct MemoryBody {
    content: String,
    #[serde(default)]
    r#type: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    metadata: serde_json::Value,
}

#[derive(Deserialize)]
struct UpdateMemoryBody {
    content: String,
}

#[derive(Deserialize)]
struct SearchQuery {
    q: String,
    #[serde(default = "default_top_k")]
    top_k: usize,
    #[serde(default)]
    semantic: bool,
}

#[derive(Deserialize)]
struct ListMemoriesQuery {
    #[serde(default)]
    r#type: Option<String>,
    #[serde(default)]
    limit: Option<usize>,
}

fn default_top_k() -> usize {
    10
}

#[derive(Deserialize)]
struct BranchBody {
    name: String,
    #[serde(default)]
    from: Option<Uuid>,
}

#[derive(Deserialize)]
struct MergeBody {
    #[serde(alias = "target_id")]
    target: Uuid,
    #[serde(default)]
    strategy: Option<String>,
}

#[derive(Deserialize)]
struct DiffQuery {
    target: Uuid,
}

#[derive(Deserialize)]
struct ReflectJobsQuery {
    #[serde(default)]
    agent_id: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    limit: Option<u32>,
    #[serde(default)]
    offset: Option<u32>,
}

#[derive(Deserialize)]
struct ResolveContradictionBody {
    strategy: String,
    #[serde(default)]
    merged_value: Option<serde_json::Value>,
}

pub fn router(state: Arc<AppState>) -> Router {
    let public = Router::new()
        .route("/v1/health", get(health))
        .route("/v1/ready", get(ready))
        .route("/v1/sessions", post(create_session))
        .route("/v1/metrics", get(metrics));

    let protected = Router::new()
        .route("/v1/sessions/me", get(whoami))
        .route("/v1/sessions/:id", delete(revoke_session))
        .route("/v1/sessions/active/count", get(active_session_count))
        .route("/v1/memories", post(remember).get(list_memories))
        .route("/v1/memories/search", get(search))
        .route(
            "/v1/memories/:id",
            get(recall_one).patch(update_memory).delete(delete_memory),
        )
        .route("/v1/branches", post(create_branch).get(list_branches))
        .route("/v1/branches/trunk", get(get_trunk_branch))
        .route("/v1/branches/by-name/:name", get(get_branch_by_name))
        .route("/v1/branches/:id/merge", post(merge_branch))
        .route("/v1/branches/:id/diff", get(diff_branch))
        .route("/v1/branches/:id/archive", post(archive_branch))
        .route("/v1/branches/:id", get(get_branch).delete(discard_branch))
        .route("/v1/sync", post(sync))
        .route("/v1/sync/push", post(push_sync))
        .route("/v1/sync/pull", post(pull_sync))
        .route("/v1/sync/reconcile", post(reconcile_sync))
        .route("/v1/sync/status", get(sync_status))
        .route("/v1/reflect", post(reflect))
        .route("/v1/reflect/jobs", get(list_reflect_jobs))
        .route("/v1/reflect/jobs/:job_id", get(get_reflect_job))
        .route("/v1/reflect/facts/:agent_id", get(get_reflect_facts))
        .route(
            "/v1/reflect/preferences/:agent_id",
            get(get_reflect_preferences),
        )
        .route(
            "/v1/reflect/contradictions/:agent_id",
            get(get_reflect_contradictions),
        )
        .route(
            "/v1/reflect/contradictions/:agent_id/:contradiction_id/resolve",
            post(resolve_reflect_contradiction),
        )
        .route("/v1/tx", post(begin_transaction))
        .route("/v1/tx/:id/memories", post(tx_remember))
        .route("/v1/tx/:id/memories/typed", post(tx_remember_typed))
        .route("/v1/tx/:id/commit", post(commit_transaction))
        .route("/v1/tx/:id/rollback", post(rollback_transaction))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::rate_limit_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::auth_middleware,
        ));

    public
        .merge(protected)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::metrics_middleware,
        ))
        .layer(middleware::from_fn(auth::request_id_middleware))
        .layer(SetResponseHeaderLayer::if_not_present(
            axum::http::header::HeaderName::from_static("content-security-policy"),
            HeaderValue::from_static("default-src 'none'; frame-ancestors 'none'; base-uri 'none'"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            axum::http::header::HeaderName::from_static("x-content-type-options"),
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            axum::http::header::HeaderName::from_static("x-frame-options"),
            HeaderValue::from_static("DENY"),
        ))
        .layer(RequestBodyLimitLayer::new(10 * 1024 * 1024))
        .layer(NormalizePathLayer::trim_trailing_slash())
        .with_state(state)
}

pub fn metrics_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(metrics))
        .route("/metrics", get(metrics))
        .route("/v1/metrics", get(metrics))
        .with_state(state)
}

async fn health(State(state): State<Arc<AppState>>) -> Response {
    match state.db.health().await {
        Ok(report) => Json(report).into_response(),
        Err(error) => map_error(error, None),
    }
}

async fn ready(State(state): State<Arc<AppState>>) -> Response {
    match state.db.health().await {
        Ok(report) if report.ok => StatusCode::OK.into_response(),
        Ok(_) => StatusCode::SERVICE_UNAVAILABLE.into_response(),
        Err(error) => map_error(error, None),
    }
}

async fn metrics(State(state): State<Arc<AppState>>) -> Response {
    if let Ok(count) = state.db.active_session_count().await {
        state.metrics.set_active_sessions(count);
    }
    let rendered = state.metrics.render(state.db.metrics_handle().render());
    (
        StatusCode::OK,
        [(
            axum::http::header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; version=0.0.4"),
        )],
        rendered,
    )
        .into_response()
}

async fn create_session(
    State(state): State<Arc<AppState>>,
    Extension(request_id): Extension<RequestId>,
    Json(body): Json<CreateSessionBody>,
) -> Response {
    match state
        .db
        .session_with_ttl(
            body.agent_id,
            &body.role,
            body.scopes,
            body.ttl_secs.unwrap_or(3600) as i64,
        )
        .await
    {
        Ok(session) => Json(serde_json::json!({
            "id": session.id,
            "session_id": session.id,
            "agent_id": session.agent_id,
            "role": session.role,
            "token": session.token,
            "expires_at": session.expires_at.to_rfc3339(),
            "scopes": session.scopes,
        }))
        .into_response(),
        Err(error) => map_error(error, Some(request_id.0)),
    }
}

async fn whoami(Extension(auth): Extension<AuthContext>) -> Response {
    Json(serde_json::json!({
        "id": auth.session.id,
        "session_id": auth.session.id,
        "agent_id": auth.session.agent_id,
        "role": auth.session.role,
        "token": auth.session.token,
        "expires_at": auth.session.expires_at.to_rfc3339(),
        "scopes": auth.session.scopes,
    }))
    .into_response()
}

async fn revoke_session(
    State(state): State<Arc<AppState>>,
    Extension(request_id): Extension<RequestId>,
    Path(id): Path<Uuid>,
) -> Response {
    match state.db.revoke_session(id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => map_error(error, Some(request_id.0)),
    }
}

async fn remember(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Extension(request_id): Extension<RequestId>,
    Json(body): Json<MemoryBody>,
) -> Response {
    let result = if let Some(memory_type) = body.r#type.as_deref() {
        state
            .db
            .remember_typed(
                &auth.session,
                &body.content,
                memory_type,
                &body.tags,
                body.metadata,
            )
            .await
    } else {
        state.db.remember(&auth.session, &body.content).await
    };

    match result {
        Ok(remembered) => Json(remembered).into_response(),
        Err(error) => map_error(error, Some(request_id.0)),
    }
}

async fn search(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Extension(request_id): Extension<RequestId>,
    Query(query): Query<SearchQuery>,
) -> Response {
    match state
        .db
        .search_with_options(&auth.session, &query.q, query.top_k, query.semantic, None)
        .await
    {
        Ok(hits) => Json(hits).into_response(),
        Err(error) => map_error(error, Some(request_id.0)),
    }
}

async fn recall_one(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Extension(request_id): Extension<RequestId>,
    Path(id): Path<Uuid>,
) -> Response {
    match state.db.recall(&auth.session, &[id]).await {
        Ok(mut memories) => match memories.pop() {
            Some(memory) => Json(memory).into_response(),
            None => auth::error_response(
                StatusCode::NOT_FOUND,
                "not_found",
                None,
                Some(request_id.0),
                None,
            ),
        },
        Err(error) => map_error(error, Some(request_id.0)),
    }
}

async fn list_memories(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Extension(request_id): Extension<RequestId>,
    Query(query): Query<ListMemoriesQuery>,
) -> Response {
    match state
        .db
        .list_memories(&auth.session, query.r#type.as_deref())
        .await
    {
        Ok(mut memories) => {
            if let Some(limit) = query.limit {
                memories.truncate(limit);
            }
            Json(memories).into_response()
        }
        Err(error) => map_error(error, Some(request_id.0)),
    }
}

async fn delete_memory(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Extension(request_id): Extension<RequestId>,
    Path(id): Path<Uuid>,
) -> Response {
    match state.db.delete_memory(&auth.session, id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => map_error(error, Some(request_id.0)),
    }
}

async fn update_memory(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Extension(request_id): Extension<RequestId>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateMemoryBody>,
) -> Response {
    match state.db.update_memory(&auth.session, id, &body.content).await {
        Ok(()) => Json(serde_json::json!({"updated": true, "memory_id": id})).into_response(),
        Err(error) => map_error(error, Some(request_id.0)),
    }
}

async fn create_branch(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Extension(request_id): Extension<RequestId>,
    Json(body): Json<BranchBody>,
) -> Response {
    let branch = if let Some(from) = body.from {
        state.db.fork_branch(&auth.session, from, &body.name).await
    } else {
        state.db.branch(&auth.session, &body.name).await
    };
    match branch {
        Ok(id) => Json(serde_json::json!({"id": id, "branch_id": id, "name": body.name})).into_response(),
        Err(error) => map_error(error, Some(request_id.0)),
    }
}

async fn list_branches(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Extension(request_id): Extension<RequestId>,
) -> Response {
    match state.db.list_branches(&auth.session).await {
        Ok(branches) => Json(branches).into_response(),
        Err(error) => map_error(error, Some(request_id.0)),
    }
}

async fn get_branch(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Extension(request_id): Extension<RequestId>,
    Path(id): Path<Uuid>,
) -> Response {
    match state.db.get_branch(&auth.session, id).await {
        Ok(branch) => Json(branch).into_response(),
        Err(error) => map_error(error, Some(request_id.0)),
    }
}

async fn get_branch_by_name(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Extension(request_id): Extension<RequestId>,
    Path(name): Path<String>,
) -> Response {
    match state.db.get_branch_by_name(&auth.session, &name).await {
        Ok(branch) => Json(branch).into_response(),
        Err(error) => map_error(error, Some(request_id.0)),
    }
}

async fn get_trunk_branch(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Extension(request_id): Extension<RequestId>,
) -> Response {
    match state.db.trunk_branch(&auth.session).await {
        Ok(branch) => Json(branch).into_response(),
        Err(error) => map_error(error, Some(request_id.0)),
    }
}

async fn merge_branch(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Extension(request_id): Extension<RequestId>,
    Path(id): Path<Uuid>,
    Json(body): Json<MergeBody>,
) -> Response {
    match state
        .db
        .merge_with_strategy(
            &auth.session,
            id,
            body.target,
            parse_strategy(body.strategy.as_deref()),
        )
        .await
    {
        Ok(result) => Json(result).into_response(),
        Err(error) => map_error(error, Some(request_id.0)),
    }
}

async fn diff_branch(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Extension(request_id): Extension<RequestId>,
    Path(id): Path<Uuid>,
    Query(query): Query<DiffQuery>,
) -> Response {
    match state.db.diff(&auth.session, id, query.target).await {
        Ok(result) => Json(result).into_response(),
        Err(error) => map_error(error, Some(request_id.0)),
    }
}

async fn discard_branch(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Extension(request_id): Extension<RequestId>,
    Path(id): Path<Uuid>,
) -> Response {
    match state.db.discard_branch(&auth.session, id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => map_error(error, Some(request_id.0)),
    }
}

async fn archive_branch(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Extension(request_id): Extension<RequestId>,
    Path(id): Path<Uuid>,
) -> Response {
    match state.db.archive_branch(&auth.session, id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => map_error(error, Some(request_id.0)),
    }
}

async fn sync(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Extension(request_id): Extension<RequestId>,
) -> Response {
    match state.db.sync(&auth.session).await {
        Ok(result) => Json(result).into_response(),
        Err(error) => map_error(error, Some(request_id.0)),
    }
}

async fn push_sync(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Extension(request_id): Extension<RequestId>,
) -> Response {
    match state.db.push_sync(&auth.session).await {
        Ok(result) => Json(result).into_response(),
        Err(error) => map_error(error, Some(request_id.0)),
    }
}

async fn pull_sync(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Extension(request_id): Extension<RequestId>,
) -> Response {
    match state.db.pull_sync(&auth.session).await {
        Ok(result) => Json(result).into_response(),
        Err(error) => map_error(error, Some(request_id.0)),
    }
}

async fn reconcile_sync(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Extension(request_id): Extension<RequestId>,
) -> Response {
    match state.db.reconcile_sync(&auth.session).await {
        Ok(result) => Json(result).into_response(),
        Err(error) => map_error(error, Some(request_id.0)),
    }
}

async fn sync_status(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Extension(request_id): Extension<RequestId>,
) -> Response {
    match state.db.sync_status(&auth.session).await {
        Ok(result) => Json(result).into_response(),
        Err(error) => map_error(error, Some(request_id.0)),
    }
}

async fn reflect(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Extension(request_id): Extension<RequestId>,
) -> Response {
    match state.db.reflect(&auth.session).await {
        Ok(result) => Json(result).into_response(),
        Err(error) => map_error(error, Some(request_id.0)),
    }
}

async fn get_reflect_facts(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Extension(request_id): Extension<RequestId>,
    Path(agent_id): Path<String>,
) -> Response {
    match state.db.reflect_get_facts(&auth.session, &agent_id).await {
        Ok(result) => Json(result).into_response(),
        Err(error) => map_error(error, Some(request_id.0)),
    }
}

async fn list_reflect_jobs(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Extension(request_id): Extension<RequestId>,
    Query(query): Query<ReflectJobsQuery>,
) -> Response {
    match state
        .db
        .reflect_list_jobs(
            &auth.session,
            query.agent_id.as_deref(),
            query.status.as_deref(),
            query.limit,
            query.offset,
        )
        .await
    {
        Ok(result) => Json(result).into_response(),
        Err(error) => map_error(error, Some(request_id.0)),
    }
}

async fn get_reflect_job(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Extension(request_id): Extension<RequestId>,
    Path(job_id): Path<String>,
) -> Response {
    match state.db.reflect_get_job(&auth.session, &job_id).await {
        Ok(result) => Json(result).into_response(),
        Err(error) => map_error(error, Some(request_id.0)),
    }
}

async fn get_reflect_preferences(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Extension(request_id): Extension<RequestId>,
    Path(agent_id): Path<String>,
) -> Response {
    match state.db.reflect_get_preferences(&auth.session, &agent_id).await {
        Ok(result) => Json(result).into_response(),
        Err(error) => map_error(error, Some(request_id.0)),
    }
}

async fn get_reflect_contradictions(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Extension(request_id): Extension<RequestId>,
    Path(agent_id): Path<String>,
) -> Response {
    match state
        .db
        .reflect_get_contradictions(&auth.session, &agent_id)
        .await
    {
        Ok(result) => Json(result).into_response(),
        Err(error) => map_error(error, Some(request_id.0)),
    }
}

async fn resolve_reflect_contradiction(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Extension(request_id): Extension<RequestId>,
    Path((agent_id, contradiction_id)): Path<(String, String)>,
    Json(body): Json<ResolveContradictionBody>,
) -> Response {
    match state
        .db
        .reflect_resolve_contradiction(
            &auth.session,
            &agent_id,
            &contradiction_id,
            &body.strategy,
            body.merged_value,
        )
        .await
    {
        Ok(result) => Json(result).into_response(),
        Err(error) => map_error(error, Some(request_id.0)),
    }
}

async fn active_session_count(
    State(state): State<Arc<AppState>>,
    Extension(request_id): Extension<RequestId>,
) -> Response {
    match state.db.active_session_count().await {
        Ok(count) => Json(serde_json::json!({"count": count})).into_response(),
        Err(error) => map_error(error, Some(request_id.0)),
    }
}

async fn begin_transaction(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
) -> Response {
    let tx_id = Uuid::new_v4();
    state.transactions.lock().await.insert(
        tx_id,
        crate::state::PendingTransaction {
            id: tx_id,
            session: auth.session,
            staged_memories: Vec::new(),
        },
    );
    Json(serde_json::json!({"tx_id": tx_id})).into_response()
}

async fn tx_remember(
    State(state): State<Arc<AppState>>,
    Extension(request_id): Extension<RequestId>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateMemoryBody>,
) -> Response {
    let mut transactions = state.transactions.lock().await;
    let Some(pending) = transactions.get_mut(&id) else {
        return auth::error_response(
            StatusCode::NOT_FOUND,
            "not_found",
            Some("transaction not found".to_string()),
            Some(request_id.0),
            None,
        );
    };
    pending.staged_memories.push(StagedMemory {
        content: body.content,
        memory_type: "semantic".to_string(),
        tags: Vec::new(),
        metadata: serde_json::Value::Null,
    });
    Json(serde_json::json!({"staged": true})).into_response()
}

async fn tx_remember_typed(
    State(state): State<Arc<AppState>>,
    Extension(request_id): Extension<RequestId>,
    Path(id): Path<Uuid>,
    Json(body): Json<MemoryBody>,
) -> Response {
    let memory_type = body.r#type.unwrap_or_else(|| "semantic".to_string());
    let mut transactions = state.transactions.lock().await;
    let Some(pending) = transactions.get_mut(&id) else {
        return auth::error_response(
            StatusCode::NOT_FOUND,
            "not_found",
            Some("transaction not found".to_string()),
            Some(request_id.0),
            None,
        );
    };
    pending.staged_memories.push(StagedMemory {
        content: body.content,
        memory_type,
        tags: body.tags,
        metadata: body.metadata,
    });
    Json(serde_json::json!({"staged": true})).into_response()
}

async fn commit_transaction(
    State(state): State<Arc<AppState>>,
    Extension(request_id): Extension<RequestId>,
    Path(id): Path<Uuid>,
) -> Response {
    let pending = match state.transactions.lock().await.remove(&id) {
        Some(pending) => pending,
        None => {
            return auth::error_response(
                StatusCode::NOT_FOUND,
                "not_found",
                Some("transaction not found".to_string()),
                Some(request_id.0),
                None,
            );
        }
    };

    match state.db.transaction(&pending.session).await {
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
                    return map_error(error, Some(request_id.0));
                }
            }
            match tx.commit().await {
                Ok(()) => Json(serde_json::json!({"committed": true})).into_response(),
                Err(error) => map_error(error, Some(request_id.0)),
            }
        }
        Err(error) => map_error(error, Some(request_id.0)),
    }
}

async fn rollback_transaction(
    State(state): State<Arc<AppState>>,
    Extension(request_id): Extension<RequestId>,
    Path(id): Path<Uuid>,
) -> Response {
    let pending = match state.transactions.lock().await.remove(&id) {
        Some(pending) => pending,
        None => {
            return auth::error_response(
                StatusCode::NOT_FOUND,
                "not_found",
                Some("transaction not found".to_string()),
                Some(request_id.0),
                None,
            );
        }
    };
    match state.db.transaction(&pending.session).await {
        Ok(tx) => match tx.rollback().await {
            Ok(()) => Json(serde_json::json!({"rolled_back": true})).into_response(),
            Err(error) => map_error(error, Some(request_id.0)),
        },
        Err(error) => map_error(error, Some(request_id.0)),
    }
}

fn map_error(error: ClawDBError, request_id: Option<String>) -> Response {
    match error {
        ClawDBError::PermissionDenied(reason) => auth::error_response(
            StatusCode::FORBIDDEN,
            "permission_denied",
            Some(reason),
            request_id,
            None,
        ),
        ClawDBError::SessionInvalid => auth::error_response(
            StatusCode::UNAUTHORIZED,
            "session_invalid",
            None,
            request_id,
            None,
        ),
        ClawDBError::ComponentDisabled(component) => auth::error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "component_disabled",
            None,
            request_id,
            Some(component.to_string()),
        ),
        other => {
            tracing::error!(request_id = ?request_id, error = %other, "HTTP handler failed");
            auth::error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal",
                None,
                request_id,
                None,
            )
        }
    }
}

fn parse_strategy(value: Option<&str>) -> MergeStrategy {
    match value.unwrap_or("theirs").to_ascii_lowercase().as_str() {
        "ours" => MergeStrategy::Ours,
        "union" => MergeStrategy::Union,
        "manual" => MergeStrategy::Manual,
        _ => MergeStrategy::Theirs,
    }
}
