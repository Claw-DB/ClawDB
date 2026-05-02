//! Minimal HTTP server for health and metrics.

use std::{net::SocketAddr, sync::Arc};

use axum::{extract::State, response::IntoResponse, routing::get, Router};
use tokio_util::sync::CancellationToken;

use crate::{config::ServerConfig, engine::ClawDB, error::ClawDBResult};

/// Serves a small HTTP API with `/health` and `/metrics`.
pub async fn serve(
    db: Arc<ClawDB>,
    config: &ServerConfig,
    shutdown: CancellationToken,
) -> ClawDBResult<()> {
    let app = Router::new()
        .route("/health", get(health))
        .route("/metrics", get(metrics))
        .with_state(db);
    let address = SocketAddr::from(([127, 0, 0, 1], config.http_port));
    let listener = tokio::net::TcpListener::bind(address).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            shutdown.cancelled().await;
        })
        .await
        .map_err(|error| crate::error::ClawDBError::Config(error.to_string()))
}

async fn health(State(db): State<Arc<ClawDB>>) -> impl IntoResponse {
    match db.health().await {
        Ok(report) => axum::Json(
            serde_json::to_value(report).unwrap_or_else(|_| serde_json::json!({"ok": false})),
        ),
        Err(error) => axum::Json(serde_json::json!({"ok": false, "error": error.to_string()})),
    }
}

async fn metrics(State(db): State<Arc<ClawDB>>) -> impl IntoResponse {
    db.metrics_handle().render()
}
