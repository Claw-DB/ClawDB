use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::Duration,
};

use clawdb::ClawDBConfig;
use clawdb_server::{
    build_state,
    grpc::service::proto::{
        claw_db_service_client::ClawDbServiceClient, BeginTxRequest, BranchRequest,
        CommitTxRequest, DeleteMemoryRequest, DiscardBranchRequest, GetBranchRequest,
        HealthRequest, ListBranchesRequest, ListMemoriesRequest, RememberRequest, SearchRequest,
        TxRememberTypedRequest,
    },
    spawn_servers, ServerOptions,
};
use tempfile::tempdir;
use uuid::Uuid;

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
    let session = state
        .db
        .session(
            Uuid::new_v4(),
            "agent",
            vec![
                "memory:write".to_string(),
                "memory:read".to_string(),
                "branch:write".to_string(),
                "branch:read".to_string(),
            ],
        )
        .await?;
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

    let ready = get_with_retry(&client, &format!("{http_base}/v1/ready")).await?;
    assert!(ready.status().is_success());

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

    let unauth = grpc
        .remember(tonic::Request::new(RememberRequest {
            content: "unauthorized".to_string(),
        }))
        .await
        .expect_err("missing token should be rejected");
    assert_eq!(unauth.code(), tonic::Code::Unauthenticated);

    let mut remember_req = tonic::Request::new(RememberRequest {
        content: "grpc memory".to_string(),
    });
    add_session(&mut remember_req, &session.token)?;
    let remember_resp = grpc.remember(remember_req).await?;
    let memory_id = remember_resp.get_ref().memory_id.clone();
    assert!(!memory_id.is_empty());

    let mut search_req = tonic::Request::new(SearchRequest {
        query: "grpc memory".to_string(),
        top_k: 5,
        semantic: false,
        filter_json: String::new(),
    });
    add_session(&mut search_req, &session.token)?;
    let search_resp = grpc.search(search_req).await?;
    assert!(!search_resp.get_ref().hits.is_empty());

    let mut list_memories_req = tonic::Request::new(ListMemoriesRequest {
        r#type: String::new(),
        limit: 0,
    });
    add_session(&mut list_memories_req, &session.token)?;
    let list_memories_resp = grpc.list_memories(list_memories_req).await?;
    assert!(!list_memories_resp.get_ref().memories.is_empty());

    let mut begin_tx_req = tonic::Request::new(BeginTxRequest {});
    add_session(&mut begin_tx_req, &session.token)?;
    let tx = grpc.begin_tx(begin_tx_req).await?;
    let tx_id = tx.get_ref().tx_id.clone();
    assert!(!tx_id.is_empty());

    let mut tx_remember_req = tonic::Request::new(TxRememberTypedRequest {
        tx_id: tx_id.clone(),
        content: "tx memory".to_string(),
        r#type: "semantic".to_string(),
        tags: vec!["grpc".to_string()],
        metadata_json: "{\"source\":\"test\"}".to_string(),
    });
    add_session(&mut tx_remember_req, &session.token)?;
    let tx_memory = grpc.tx_remember_typed(tx_remember_req).await?;
    assert!(!tx_memory.get_ref().memory_id.is_empty());

    let mut commit_tx_req = tonic::Request::new(CommitTxRequest { tx_id });
    add_session(&mut commit_tx_req, &session.token)?;
    let commit_tx_resp = grpc.commit_tx(commit_tx_req).await?;
    assert!(commit_tx_resp.get_ref().committed);

    let mut branch_req = tonic::Request::new(BranchRequest {
        name: "grpc-test-branch".to_string(),
        from: String::new(),
    });
    add_session(&mut branch_req, &session.token)?;
    let branch_resp = grpc.branch(branch_req).await?;
    let branch_id = branch_resp.get_ref().branch_id.clone();
    assert!(!branch_id.is_empty());

    let mut list_branches_req = tonic::Request::new(ListBranchesRequest {});
    add_session(&mut list_branches_req, &session.token)?;
    let list_branches_resp = grpc.list_branches(list_branches_req).await?;
    assert!(!list_branches_resp.get_ref().branches.is_empty());

    let mut get_branch_req = tonic::Request::new(GetBranchRequest {
        branch_id: branch_id.clone(),
    });
    add_session(&mut get_branch_req, &session.token)?;
    let get_branch_resp = grpc.get_branch(get_branch_req).await?;
    assert!(get_branch_resp.get_ref().branch.is_some());

    let mut discard_branch_req = tonic::Request::new(DiscardBranchRequest { branch_id });
    add_session(&mut discard_branch_req, &session.token)?;
    let discard_branch_resp = grpc.discard_branch(discard_branch_req).await?;
    assert!(discard_branch_resp.get_ref().discarded);

    let mut delete_memory_req = tonic::Request::new(DeleteMemoryRequest { memory_id });
    add_session(&mut delete_memory_req, &session.token)?;
    let delete_memory_resp = grpc.delete_memory(delete_memory_req).await?;
    assert!(delete_memory_resp.get_ref().deleted);

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

fn add_session<T>(request: &mut tonic::Request<T>, token: &str) -> anyhow::Result<()> {
    request.metadata_mut().insert(
        "x-claw-session",
        tonic::metadata::MetadataValue::try_from(token)?,
    );
    Ok(())
}
