pub mod grpc;
pub mod http;
pub mod state;

use std::{net::SocketAddr, sync::Arc, time::Duration};

use anyhow::{Context, Result};
use clawdb::{ClawDB, ClawDBConfig};
use tokio::{net::TcpListener, task::JoinHandle, time::timeout};
use tokio_util::sync::CancellationToken;

use crate::state::AppState;

pub const VERSION_TEXT: &str = concat!(
    "clawdb-server ",
    env!("CARGO_PKG_VERSION"),
    "\ncomponents: claw-core/0.1.0 claw-vector/0.1.0 claw-branch/0.1.0\n            claw-sync/0.1.0 claw-guard/0.1.0 claw-reflect-client/0.1.0"
);

#[derive(Clone, Debug)]
pub struct ServerOptions {
    pub grpc_addr: SocketAddr,
    pub http_addr: SocketAddr,
    pub metrics_addr: SocketAddr,
}

#[derive(Clone, Debug)]
pub struct BoundAddresses {
    pub grpc: SocketAddr,
    pub http: SocketAddr,
    pub metrics: SocketAddr,
}

pub struct RunningServers {
    pub addresses: BoundAddresses,
    pub shutdown: CancellationToken,
    pub db: Arc<ClawDB>,
    pub grpc_task: JoinHandle<Result<()>>,
    pub http_task: JoinHandle<Result<()>>,
    pub metrics_task: JoinHandle<Result<()>>,
}

impl RunningServers {
    pub async fn shutdown(self, grace: Duration) -> Result<()> {
        self.shutdown.cancel();

        let joined = timeout(grace, async {
            let grpc = self.grpc_task.await.context("gRPC task join failed")?;
            let http = self.http_task.await.context("HTTP task join failed")?;
            let metrics = self
                .metrics_task
                .await
                .context("metrics task join failed")?;
            grpc?;
            http?;
            metrics?;
            Result::<()>::Ok(())
        })
        .await;

        self.db.close().await.context("failed to close clawdb")?;
        joined.context("timed out waiting for server tasks")??;
        Ok(())
    }
}

pub async fn build_state(config: ClawDBConfig) -> Result<Arc<AppState>> {
    let db = Arc::new(
        ClawDB::new(config)
            .await
            .context("failed to initialize ClawDB")?,
    );
    Ok(Arc::new(AppState::new(db)))
}

pub async fn spawn_servers(state: Arc<AppState>, options: ServerOptions) -> Result<RunningServers> {
    let shutdown = CancellationToken::new();

    let grpc_listener = TcpListener::bind(options.grpc_addr)
        .await
        .context("failed to bind gRPC listener")?;
    let http_listener = TcpListener::bind(options.http_addr)
        .await
        .context("failed to bind HTTP listener")?;
    let metrics_listener = TcpListener::bind(options.metrics_addr)
        .await
        .context("failed to bind metrics listener")?;

    let addresses = BoundAddresses {
        grpc: grpc_listener
            .local_addr()
            .context("missing gRPC local address")?,
        http: http_listener
            .local_addr()
            .context("missing HTTP local address")?,
        metrics: metrics_listener
            .local_addr()
            .context("missing metrics local address")?,
    };

    let grpc_task = tokio::spawn(grpc::serve(grpc_listener, state.clone(), shutdown.clone()));
    let http_task = tokio::spawn(http::serve(http_listener, state.clone(), shutdown.clone()));
    let metrics_task = tokio::spawn(http::serve_metrics(
        metrics_listener,
        state.clone(),
        shutdown.clone(),
    ));

    Ok(RunningServers {
        addresses,
        shutdown,
        db: state.db.clone(),
        grpc_task,
        http_task,
        metrics_task,
    })
}
