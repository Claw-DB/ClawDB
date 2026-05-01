//! HTTP server entry point.

use std::sync::Arc;

use tokio_util::sync::CancellationToken;

use crate::{config::ServerSubConfig, engine::ClawDB, error::ClawDBResult};

/// Starts the axum HTTP server.
///
/// Binds to `0.0.0.0:<config.http_port>` and shuts down cleanly when
/// `shutdown` is cancelled.
pub async fn serve(
    engine: Arc<ClawDB>,
    config: &ServerSubConfig,
    shutdown: CancellationToken,
) -> ClawDBResult<()> {
    let router = super::routes::build_router(engine);
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], config.http_port));

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| crate::error::ClawDBError::Io(e))?;

    tracing::info!(port = config.http_port, "HTTP server listening");

    axum::serve(listener, router)
        .with_graceful_shutdown(async move { shutdown.cancelled().await })
        .await
            .map_err(|e| crate::error::ClawDBError::ComponentFailed {
                component: "http".to_string(),
                reason: e.to_string(),
            })?;

    Ok(())
}
