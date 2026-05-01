//! Axum route definitions for the ClawDB HTTP REST API.
//!
//! All routes validate the caller's session token before delegating to the
//! engine.  The token must appear in:
//! - `Authorization: Bearer <token>` header, **or**
//! - `?token=<token>` query parameter.
//!
//! # Endpoints
//!
//! | Method | Path                       | Description                |
//! |:------ |:-------------------------- |:-------------------------- |
//! | POST   | /v1/session                | Create a session           |
//! | POST   | /v1/remember               | Store a memory             |
//! | POST   | /v1/search                 | Search memories            |
//! | POST   | /v1/recall                 | Recall specific memories   |
//! | POST   | /v1/branch                 | Create a branch            |
//! | POST   | /v1/merge                  | Merge branches             |
//! | POST   | /v1/diff                   | Diff two branches          |
//! | POST   | /v1/sync                   | Trigger sync               |
//! | POST   | /v1/reflect                | Start reflect job          |
//! | GET    | /v1/health                 | Health check               |
//! | GET    | /v1/events/stream          | SSE event stream           |
//! | GET    | /metrics                   | Prometheus metrics         |

use std::sync::Arc;

use axum::{
    extract::{Query as AxumQuery, State},
    http::{HeaderMap, StatusCode},
    response::{sse, IntoResponse, Response, Sse},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tokio_stream::StreamExt;
use uuid::Uuid;

use crate::{engine::ClawDB, error::ClawDBError};

// ── Shared state ──────────────────────────────────────────────────────────────

pub type AppState = Arc<ClawDB>;

// ── Error response ────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct ErrorBody {
    error: String,
    code: String,
}

fn err_response(status: StatusCode, code: &str, message: impl ToString) -> Response {
    let body = Json(ErrorBody {
        error: message.to_string(),
        code: code.to_string(),
    });
    (status, body).into_response()
}

fn engine_err(e: ClawDBError) -> Response {
    let code = e.http_status_code();
    let status = StatusCode::from_u16(code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let error_code = match code {
        400 => "BAD_REQUEST",
        401 => "UNAUTHORIZED",
        403 => "FORBIDDEN",
        404 => "NOT_FOUND",
        503 => "SERVICE_UNAVAILABLE",
        502 => "BAD_GATEWAY",
        _ => "INTERNAL_ERROR",
    };
    err_response(status, error_code, e)
}

// ── Token extraction ──────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct TokenQuery {
    token: Option<String>,
}

fn extract_token(headers: &HeaderMap, query_token: Option<&str>) -> Result<String, Response> {
    if let Some(val) = headers.get("authorization") {
        let s = val
            .to_str()
            .map_err(|_| err_response(StatusCode::BAD_REQUEST, "BAD_REQUEST", "invalid authorization header"))?;
        let tok = s
            .strip_prefix("Bearer ")
            .ok_or_else(|| err_response(StatusCode::UNAUTHORIZED, "UNAUTHORIZED", "expected Bearer token"))?;
        return Ok(tok.to_string());
    }
    if let Some(t) = query_token {
        return Ok(t.to_string());
    }
    Err(err_response(StatusCode::UNAUTHORIZED, "UNAUTHORIZED", "no session token"))
}

// ── Request / response bodies ─────────────────────────────────────────────────

#[derive(Deserialize)]
struct CreateSessionBody {
    agent_id: Uuid,
    role: String,
    #[serde(default)]
    scopes: Vec<String>,
    task_type: Option<String>,
}

#[derive(Serialize)]
struct CreateSessionResponse {
    session_token: String,
    expires_at: i64,
    scopes: Vec<String>,
}

#[derive(Deserialize)]
struct RememberBody {
    content: String,
    #[serde(default = "default_memory_type")]
    memory_type: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    metadata: serde_json::Value,
}
fn default_memory_type() -> String { "general".to_string() }

#[derive(Deserialize)]
struct SearchBody {
    query: String,
    #[serde(default = "default_top_k")]
    top_k: usize,
    #[serde(default = "default_true")]
    semantic: bool,
    filter: Option<serde_json::Value>,
}
fn default_top_k() -> usize { 10 }
fn default_true() -> bool { true }

#[derive(Deserialize)]
struct RecallBody {
    memory_ids: Vec<String>,
}

#[derive(Deserialize)]
struct BranchBody {
    name: String,
}

#[derive(Deserialize)]
struct MergeBody {
    source: Uuid,
    target: Uuid,
}

#[derive(Deserialize)]
struct DiffBody {
    branch_a: Uuid,
    branch_b: Uuid,
}

// ── Handlers ──────────────────────────────────────────────────────────────────

async fn create_session(
    State(db): State<AppState>,
    Json(body): Json<CreateSessionBody>,
) -> Response {
    let result = if let Some(task) = body.task_type {
        db.session_with_task(body.agent_id, &body.role, body.scopes.clone(), &task)
            .await
    } else {
        db.session(body.agent_id, &body.role, body.scopes.clone())
            .await
    };
    match result {
        Ok(sess) => Json(CreateSessionResponse {
            session_token: sess.guard_token.clone(),
            expires_at: sess.expires_at.timestamp(),
            scopes: sess.scopes,
        })
        .into_response(),
        Err(e) => engine_err(e),
    }
}

async fn remember(
    State(db): State<AppState>,
    headers: HeaderMap,
    AxumQuery(q): AxumQuery<TokenQuery>,
    Json(body): Json<RememberBody>,
) -> Response {
    let token = match extract_token(&headers, q.token.as_deref()) {
        Ok(t) => t,
        Err(r) => return r,
    };
    let ctx = match db.session_manager.validate(&token).await {
        Ok(c) => c,
        Err(e) => return engine_err(e),
    };
    let sess = crate::session::manager::ClawDBSession::from_context(ctx);
    match db.remember_typed(&sess, &body.content, &body.memory_type, &body.tags, body.metadata).await {
        Ok(r) => Json(r).into_response(),
        Err(e) => engine_err(e),
    }
}

async fn search(
    State(db): State<AppState>,
    headers: HeaderMap,
    AxumQuery(q): AxumQuery<TokenQuery>,
    Json(body): Json<SearchBody>,
) -> Response {
    let token = match extract_token(&headers, q.token.as_deref()) {
        Ok(t) => t,
        Err(r) => return r,
    };
    let ctx = match db.session_manager.validate(&token).await {
        Ok(c) => c,
        Err(e) => return engine_err(e),
    };
    let sess = crate::session::manager::ClawDBSession::from_context(ctx);
    match db.search_with_options(&sess, &body.query, body.top_k, body.semantic, body.filter).await {
        Ok(results) => Json(results).into_response(),
        Err(e) => engine_err(e),
    }
}

async fn recall(
    State(db): State<AppState>,
    headers: HeaderMap,
    AxumQuery(q): AxumQuery<TokenQuery>,
    Json(body): Json<RecallBody>,
) -> Response {
    let token = match extract_token(&headers, q.token.as_deref()) {
        Ok(t) => t,
        Err(r) => return r,
    };
    let ctx = match db.session_manager.validate(&token).await {
        Ok(c) => c,
        Err(e) => return engine_err(e),
    };
    let sess = crate::session::manager::ClawDBSession::from_context(ctx);
    match db.recall(&sess, &body.memory_ids).await {
        Ok(mems) => Json(mems).into_response(),
        Err(e) => engine_err(e),
    }
}

async fn branch_handler(
    State(db): State<AppState>,
    headers: HeaderMap,
    AxumQuery(q): AxumQuery<TokenQuery>,
    Json(body): Json<BranchBody>,
) -> Response {
    let token = match extract_token(&headers, q.token.as_deref()) {
        Ok(t) => t,
        Err(r) => return r,
    };
    let ctx = match db.session_manager.validate(&token).await {
        Ok(c) => c,
        Err(e) => return engine_err(e),
    };
    let sess = crate::session::manager::ClawDBSession::from_context(ctx);
    match db.branch(&sess, &body.name).await {
        Ok(id) => Json(serde_json::json!({ "branch_id": id })).into_response(),
        Err(e) => engine_err(e),
    }
}

async fn merge_handler(
    State(db): State<AppState>,
    headers: HeaderMap,
    AxumQuery(q): AxumQuery<TokenQuery>,
    Json(body): Json<MergeBody>,
) -> Response {
    let token = match extract_token(&headers, q.token.as_deref()) {
        Ok(t) => t,
        Err(r) => return r,
    };
    let ctx = match db.session_manager.validate(&token).await {
        Ok(c) => c,
        Err(e) => return engine_err(e),
    };
    let sess = crate::session::manager::ClawDBSession::from_context(ctx);
    match db.merge(&sess, body.source, body.target).await {
        Ok(result) => Json(result).into_response(),
        Err(e) => engine_err(e),
    }
}

async fn diff_handler(
    State(db): State<AppState>,
    headers: HeaderMap,
    AxumQuery(q): AxumQuery<TokenQuery>,
    Json(body): Json<DiffBody>,
) -> Response {
    let token = match extract_token(&headers, q.token.as_deref()) {
        Ok(t) => t,
        Err(r) => return r,
    };
    let ctx = match db.session_manager.validate(&token).await {
        Ok(c) => c,
        Err(e) => return engine_err(e),
    };
    let sess = crate::session::manager::ClawDBSession::from_context(ctx);
    match db.diff(&sess, body.branch_a, body.branch_b).await {
        Ok(d) => Json(d).into_response(),
        Err(e) => engine_err(e),
    }
}

async fn sync_handler(
    State(db): State<AppState>,
    headers: HeaderMap,
    AxumQuery(q): AxumQuery<TokenQuery>,
) -> Response {
    let token = match extract_token(&headers, q.token.as_deref()) {
        Ok(t) => t,
        Err(r) => return r,
    };
    let ctx = match db.session_manager.validate(&token).await {
        Ok(c) => c,
        Err(e) => return engine_err(e),
    };
    let sess = crate::session::manager::ClawDBSession::from_context(ctx);
    match db.sync(&sess).await {
        Ok(r) => Json(r).into_response(),
        Err(e) => engine_err(e),
    }
}

async fn reflect_handler(
    State(db): State<AppState>,
    headers: HeaderMap,
    AxumQuery(q): AxumQuery<TokenQuery>,
) -> Response {
    let token = match extract_token(&headers, q.token.as_deref()) {
        Ok(t) => t,
        Err(r) => return r,
    };
    let ctx = match db.session_manager.validate(&token).await {
        Ok(c) => c,
        Err(e) => return engine_err(e),
    };
    let sess = crate::session::manager::ClawDBSession::from_context(ctx);
    match db.reflect(&sess).await {
        Ok(job_id) => Json(serde_json::json!({ "job_id": job_id })).into_response(),
        Err(e) => engine_err(e),
    }
}

async fn health_handler(State(db): State<AppState>) -> Response {
    match db.health().await {
        Ok(report) => Json(report).into_response(),
        Err(e) => engine_err(e),
    }
}

async fn metrics_handler(State(db): State<AppState>) -> Response {
    let text = db.telemetry.render();
    (
        StatusCode::OK,
        [("content-type", "text/plain; version=0.0.4")],
        text,
    )
        .into_response()
}

async fn events_stream(
    State(db): State<AppState>,
    headers: HeaderMap,
    AxumQuery(q): AxumQuery<TokenQuery>,
) -> Response {
    let token = match extract_token(&headers, q.token.as_deref()) {
        Ok(t) => t,
        Err(r) => return r,
    };
    if let Err(e) = db.session_manager.validate(&token).await {
        return engine_err(e);
    }

    let rx = db.event_bus.subscribe();
    let stream = tokio_stream::wrappers::BroadcastStream::new(rx)
        .filter_map(|res| res.ok())
        .map(|ev| {
            let data = serde_json::to_string(&*ev).unwrap_or_default();
            Ok::<_, std::convert::Infallible>(sse::Event::default().data(data))
        });

    Sse::new(stream)
        .keep_alive(sse::KeepAlive::default())
        .into_response()
}

// ── Router builder ────────────────────────────────────────────────────────────

/// Builds the full axum `Router` for the ClawDB HTTP API.
pub fn build_router(db: Arc<ClawDB>) -> Router {
    use tower_http::{
        cors::CorsLayer,
        request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
        trace::TraceLayer,
    };

    Router::new()
        .route("/v1/session", post(create_session))
        .route("/v1/remember", post(remember))
        .route("/v1/search", post(search))
        .route("/v1/recall", post(recall))
        .route("/v1/branch", post(branch_handler))
        .route("/v1/merge", post(merge_handler))
        .route("/v1/diff", post(diff_handler))
        .route("/v1/sync", post(sync_handler))
        .route("/v1/reflect", post(reflect_handler))
        .route("/v1/health", get(health_handler))
        .route("/v1/events/stream", get(events_stream))
        .route("/metrics", get(metrics_handler))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
        .with_state(db)
}
