use std::{sync::Arc, time::Instant};

use axum::{
    extract::{MatchedPath, Request, State},
    http::{header, HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

use crate::state::{AppState, RequestId};

#[derive(Clone)]
pub struct AuthContext {
    pub token: String,
    pub session: clawdb::ClawDBSession,
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    request_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    component: Option<String>,
}

pub async fn request_id_middleware(mut request: Request, next: Next) -> Response {
    let request_id = request
        .headers()
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    request
        .extensions_mut()
        .insert(RequestId(request_id.clone()));
    let mut response = next.run(request).await;
    if let Ok(value) = HeaderValue::from_str(&request_id) {
        response.headers_mut().insert("x-request-id", value);
    }
    response
}

pub async fn metrics_middleware(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Response {
    let method = request.method().clone();
    let path = request
        .extensions()
        .get::<MatchedPath>()
        .map(|matched| matched.as_str().to_string())
        .unwrap_or_else(|| request.uri().path().to_string());
    let started = Instant::now();
    let response = next.run(request).await;
    state.metrics.observe_http(
        method.as_str(),
        &path,
        response.status().as_u16(),
        started.elapsed(),
    );
    if let Ok(count) = state.db.active_session_count().await {
        state.metrics.set_active_sessions(count);
    }
    response
}

pub async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    mut request: Request,
    next: Next,
) -> Response {
    let token = match bearer_token(request.headers()) {
        Some(token) => token,
        None => return unauthorized(),
    };

    match state.db.validate_session(&token).await {
        Ok(session) => {
            request
                .extensions_mut()
                .insert(AuthContext { token, session });
            next.run(request).await
        }
        Err(_) => unauthorized(),
    }
}

pub async fn rate_limit_middleware(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Response {
    let Some(auth) = request.extensions().get::<AuthContext>().cloned() else {
        return unauthorized();
    };

    let limiter = if request.method() == axum::http::Method::GET {
        &state.http_read_limiter
    } else {
        &state.http_write_limiter
    };

    if let Err(not_until) = limiter.check_key(&auth.token) {
        let retry_after = AppState::retry_after_seconds(&not_until);
        let request_id = request
            .extensions()
            .get::<RequestId>()
            .map(|value| value.0.clone());
        let mut response = (
            StatusCode::TOO_MANY_REQUESTS,
            Json(ErrorBody {
                error: "rate_limited".to_string(),
                detail: None,
                request_id,
                component: None,
            }),
        )
            .into_response();
        if let Ok(value) = HeaderValue::from_str(&retry_after.to_string()) {
            response.headers_mut().insert(header::RETRY_AFTER, value);
        }
        return response;
    }

    next.run(request).await
}

fn bearer_token(headers: &axum::http::HeaderMap) -> Option<String> {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(ToOwned::to_owned)
}

fn unauthorized() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(ErrorBody {
            error: "unauthorized".to_string(),
            detail: None,
            request_id: None,
            component: None,
        }),
    )
        .into_response()
}

pub fn error_response(
    status: StatusCode,
    error: &str,
    detail: Option<String>,
    request_id: Option<String>,
    component: Option<String>,
) -> Response {
    (
        status,
        Json(ErrorBody {
            error: error.to_string(),
            detail,
            request_id,
            component,
        }),
    )
        .into_response()
}
