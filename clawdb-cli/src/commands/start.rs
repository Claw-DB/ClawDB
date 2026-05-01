//! `clawdb start` — run the full ClawDB runtime servers.

use std::{path::PathBuf, sync::Arc};

use clap::Args;
use clawdb::{api, ClawDB, ClawDBResult};
use tokio_util::sync::CancellationToken;

use super::load_config;

#[derive(Debug, Clone, Args)]
pub struct StartArgs {
    #[arg(long, default_value_t = 50050)]
    pub grpc_port: u16,
    #[arg(long, default_value_t = 8080)]
    pub http_port: u16,
    #[arg(long, default_value_t = 9090)]
    pub metrics_port: u16,
    #[arg(long)]
    pub no_http: bool,
    #[arg(long)]
    pub no_metrics: bool,
}

pub async fn run(args: StartArgs, data_dir: PathBuf) -> ClawDBResult<()> {
    let mut cfg = load_config(&data_dir)?;
    cfg.data_dir = data_dir;
    cfg.server.grpc_port = args.grpc_port;
    cfg.server.http_port = args.http_port;
    cfg.telemetry.metrics_port = if args.no_metrics { 0 } else { args.metrics_port };

    let db = Arc::new(ClawDB::new(cfg.clone()).await?);

    println!("ClawDB {} running", env!("CARGO_PKG_VERSION"));
    println!("  gRPC: localhost:{}", cfg.server.grpc_port);
    if !args.no_http {
        println!("  HTTP: localhost:{}", cfg.server.http_port);
    }
    if !args.no_metrics {
        println!("  Metrics: localhost:{}", cfg.telemetry.metrics_port);
    }

    let shutdown = CancellationToken::new();
    let grpc_shutdown = shutdown.child_token();
    let http_shutdown = shutdown.child_token();

    let grpc_db = db.clone();
    let grpc_cfg = cfg.server.clone();
    let grpc_task = tokio::spawn(async move { api::grpc::serve(grpc_db, &grpc_cfg, grpc_shutdown).await });

    let http_task = if args.no_http {
        None
    } else {
        let http_db = db.clone();
        let http_cfg = cfg.server.clone();
        Some(tokio::spawn(async move {
            api::http::serve(http_db, &http_cfg, http_shutdown).await
        }))
    };

    #[cfg(unix)]
    let wait_signal = async {
        let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to bind SIGTERM handler");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = term.recv() => {}
        }
    };

    #[cfg(not(unix))]
    let wait_signal = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    wait_signal.await;
    shutdown.cancel();

    let _ = grpc_task.await;
    if let Some(task) = http_task {
        let _ = task.await;
    }

    db.close().await
}
