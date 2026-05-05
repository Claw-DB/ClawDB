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
    state::{AppState, RequestId},
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

pub fn router(state: Arc<AppState>) -> Router {
    let public = Router::new()
        .route("/v1/health", get(health))
        .route("/v1/ready", get(ready))
        .route("/v1/sessions", post(create_session))
        .route("/v1/metrics", get(metrics));

    let protected = Router::new()
        .route("/v1/sessions/me", get(whoami))
        .route("/v1/sessions/:id", delete(revoke_session))
        .route("/v1/memories", post(remember).get(list_memories))
        .route("/v1/memories/search", get(search))
        .route("/v1/memories/:id", get(recall_one).delete(delete_memory))
        .route("/v1/branches", post(create_branch).get(list_branches))
        .route("/v1/branches/:id/merge", post(merge_branch))
        .route("/v1/branches/:id/diff", get(diff_branch))
        .route("/v1/branches/:id", delete(discard_branch))
        .route("/v1/sync", post(sync))
        .route("/v1/reflect", post(reflect))
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
