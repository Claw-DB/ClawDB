use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
    time::Duration,
};

use anyhow::{Context, Result};
use clap::Parser;
use clawdb::ClawDBConfig;
use clawdb_server::{build_state, spawn_servers, ServerOptions, VERSION_TEXT};

#[derive(Parser, Debug)]
#[command(name = "clawdb-server", about = "ClawDB server", version = VERSION_TEXT)]
struct Args {
    #[arg(long)]
    config: Option<PathBuf>,

    #[arg(long, env = "CLAW_DATA_DIR")]
    data_dir: Option<PathBuf>,

    #[arg(long, env = "CLAW_GRPC_PORT")]
    grpc_port: Option<u16>,

    #[arg(long, env = "CLAW_HTTP_PORT")]
    http_port: Option<u16>,

    #[arg(long, env = "CLAW_METRICS_PORT")]
    metrics_port: Option<u16>,

    #[arg(long)]
    generate_self_signed_cert: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let mut config = load_config(&args).await?;

    if args.generate_self_signed_cert {
        generate_self_signed_cert(&config).await?;
        return Ok(());
    }

    config.server.grpc_port = args.grpc_port.unwrap_or(config.server.grpc_port);
    config.server.http_port = args.http_port.unwrap_or(config.server.http_port);
    config.telemetry.metrics_port = args.metrics_port.unwrap_or(config.telemetry.metrics_port);

    clawdb::telemetry::init_telemetry(&config.telemetry)?;

    let state = build_state(config.clone()).await?;
    let mut servers = spawn_servers(
        state,
        ServerOptions {
            grpc_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), config.server.grpc_port),
            http_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), config.server.http_port),
            metrics_addr: SocketAddr::new(
                IpAddr::V4(Ipv4Addr::UNSPECIFIED),
                config.telemetry.metrics_port,
            ),
        },
    )
    .await?;

    tracing::info!(
        grpc = %servers.addresses.grpc,
        http = %servers.addresses.http,
        metrics = %servers.addresses.metrics,
        "clawdb-server listening"
    );

    tokio::select! {
        _ = wait_for_signal() => {
            tracing::info!("graceful shutdown initiated");
            servers.shutdown(Duration::from_secs(30)).await?;
            Ok(())
        }
        result = &mut servers.grpc_task => task_exit("gRPC", result),
        result = &mut servers.http_task => task_exit("HTTP", result),
        result = &mut servers.metrics_task => task_exit("metrics", result),
    }
}

async fn load_config(args: &Args) -> Result<ClawDBConfig> {
    let mut config = if let Some(path) = &args.config {
        ClawDBConfig::from_file(path).context("failed to load config file")?
    } else {
        ClawDBConfig::from_env().context("failed to load environment configuration")?
    };

    if let Some(data_dir) = &args.data_dir {
        config.data_dir = data_dir.clone();
    }

    Ok(config)
}

async fn generate_self_signed_cert(config: &ClawDBConfig) -> Result<()> {
    let tls_dir = config.data_dir.join("tls");
    tokio::fs::create_dir_all(&tls_dir)
        .await
        .context("failed to create tls directory")?;
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".to_string()])
        .context("failed to generate self-signed certificate")?;
    let cert_pem = cert
        .serialize_pem()
        .context("failed to serialize certificate")?;
    let key_pem = cert.serialize_private_key_pem();

    let cert_path = tls_dir.join("server.crt");
    let key_path = tls_dir.join("server.key");
    tokio::fs::write(&cert_path, cert_pem)
        .await
        .context("failed to write self-signed certificate")?;
    tokio::fs::write(&key_path, key_pem)
        .await
        .context("failed to write self-signed key")?;

    println!(
        "generated development certificate at {} and {}",
        cert_path.display(),
        key_path.display()
    );
    Ok(())
}

async fn wait_for_signal() -> Result<()> {
    #[cfg(unix)]
    {
        let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .context("failed to install SIGTERM handler")?;
        tokio::select! {
            _ = tokio::signal::ctrl_c() => Ok(()),
            _ = term.recv() => Ok(()),
        }
    }
    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c()
            .await
            .context("failed to install signal handler")
    }
}

fn task_exit(name: &str, result: Result<Result<()>, tokio::task::JoinError>) -> Result<()> {
    match result {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) => Err(error).context(format!("{name} server task failed")),
        Err(error) => {
            tracing::error!(task = name, error = %error, "server task panicked");
            Err(error).context(format!("{name} server task panicked"))
        }
    }
}
