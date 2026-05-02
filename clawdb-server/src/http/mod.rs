pub mod auth;
pub mod router;

use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

use crate::{
    http::router::{metrics_router, router},
    state::AppState,
};

pub async fn serve(
    listener: TcpListener,
    state: Arc<AppState>,
    shutdown: CancellationToken,
) -> Result<()> {
    axum::serve(listener, router(state))
        .with_graceful_shutdown(async move {
            shutdown.cancelled().await;
        })
        .await
        .context("HTTP server failed")
}

pub async fn serve_metrics(
    listener: TcpListener,
    state: Arc<AppState>,
    shutdown: CancellationToken,
) -> Result<()> {
    axum::serve(listener, metrics_router(state))
        .with_graceful_shutdown(async move {
            shutdown.cancelled().await;
        })
        .await
        .context("metrics server failed")
}
