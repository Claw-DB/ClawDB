use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::Duration,
};

use clawdb::ClawDBConfig;
use clawdb_server::{
    build_state,
    grpc::service::proto::{claw_db_service_client::ClawDbServiceClient, HealthRequest},
    spawn_servers, ServerOptions,
};
use tempfile::tempdir;

#[tokio::test]
async fn server_exposes_http_grpc_and_metrics() -> anyhow::Result<()> {
    let temp = tempdir()?;
    let mut config = ClawDBConfig::default_for_dir(temp.path());
    config.guard.jwt_secret = "test-secret".to_string();
    config.vector.enabled = false;
    config.reflect.base_url = None;
    config.reflect.api_key = None;
    config.sync.hub_url = None;

    let state = build_state(config).await?;
    let servers = spawn_servers(
        state,
        ServerOptions {
            grpc_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
            http_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
            metrics_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
        },
    )
    .await?;

    let http_base = format!("http://{}", servers.addresses.http);
    let metrics_url = format!("http://{}/metrics", servers.addresses.metrics);
    let grpc_url = format!("http://{}", servers.addresses.grpc);

    let client = reqwest::Client::new();
    let health = get_with_retry(&client, &format!("{http_base}/v1/health")).await?;
    assert!(health.status().is_success());

    let unauthorized = client
        .post(format!("{http_base}/v1/memories"))
        .json(&serde_json::json!({ "content": "blocked" }))
        .send()
        .await?;
    assert_eq!(unauthorized.status(), reqwest::StatusCode::UNAUTHORIZED);

    let mut grpc = connect_grpc_with_retry(&grpc_url).await?;
    let grpc_health = grpc.health(tonic::Request::new(HealthRequest {})).await?;
    assert!(grpc_health.get_ref().ok);
    assert!(!grpc_health.get_ref().request_id.is_empty());

    let metrics = get_with_retry(&client, &metrics_url).await?;
    assert!(metrics.status().is_success());
    let body = metrics.text().await?;
    assert!(body.contains("clawdb_http_requests_total"));
    assert!(body.contains("clawdb_grpc_requests_total"));

    servers.shutdown(Duration::from_secs(5)).await?;
    Ok(())
}

async fn get_with_retry(client: &reqwest::Client, url: &str) -> anyhow::Result<reqwest::Response> {
    let mut last_error = None;
    for _ in 0..20 {
        match client.get(url).send().await {
            Ok(response) => return Ok(response),
            Err(error) => last_error = Some(error),
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    Err(last_error
        .expect("retry loop should capture an error")
        .into())
}

async fn connect_grpc_with_retry(
    url: &str,
) -> anyhow::Result<ClawDbServiceClient<tonic::transport::Channel>> {
    let mut last_error = None;
    for _ in 0..20 {
        match ClawDbServiceClient::connect(url.to_string()).await {
            Ok(client) => return Ok(client),
            Err(error) => last_error = Some(error),
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    Err(last_error
        .expect("retry loop should capture an error")
        .into())
}
