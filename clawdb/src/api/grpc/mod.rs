//! Minimal TCP-based gRPC placeholder server.

use std::sync::Arc;

use tokio_util::sync::CancellationToken;

use crate::{config::ServerConfig, engine::ClawDB, error::ClawDBResult};

/// Serves a lightweight TCP listener for the configured gRPC port until shutdown.
pub async fn serve(
    _db: Arc<ClawDB>,
    config: &ServerConfig,
    shutdown: CancellationToken,
) -> ClawDBResult<()> {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", config.grpc_port)).await?;
    loop {
        tokio::select! {
            _ = shutdown.cancelled() => break,
            accept = listener.accept() => {
                let (mut stream, _) = accept?;
                tokio::spawn(async move {
                    use tokio::io::AsyncWriteExt;
                    let _ = stream.write_all(b"clawdb grpc placeholder\n").await;
                });
            }
        }
    }
    Ok(())
}
