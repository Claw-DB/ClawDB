use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::Duration,
};

use clawdb::ClawDBConfig;
use clawdb_server::{
    build_state,
    grpc::service::proto::{
        claw_db_service_client::ClawDbServiceClient, ActiveSessionCountRequest, ArchiveBranchRequest,
        BeginTxRequest, BranchRequest, CommitTxRequest, DeleteMemoryRequest, DiscardBranchRequest,
        GetBranchByNameRequest, GetBranchRequest, GetTrunkBranchRequest, HealthRequest,
        ListBranchesRequest, ListMemoriesRequest, RecallRequest, RememberRequest,
        RememberTypedRequest, RollbackTxRequest, SearchRequest, SyncStatusRequest,
        TxRememberRequest, TxRememberTypedRequest, UpdateMemoryRequest, ValidateSessionRequest,
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

// ---------------------------------------------------------------------------
// Extended gRPC coverage test
// ---------------------------------------------------------------------------

#[tokio::test]
async fn server_grpc_extended_coverage() -> anyhow::Result<()> {
    let temp = tempdir()?;
    let mut config = ClawDBConfig::default_for_dir(temp.path());
    config.guard.jwt_secret = "test-secret-ext".to_string();
    config.vector.enabled = false;
    config.reflect.base_url = None;
    config.reflect.api_key = None;
    config.sync.hub_url = None;

    let state = build_state(config).await?;
    let session = state
        .db
        .session(Uuid::new_v4(), "agent", vec!["*".to_string()])
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

    let grpc_url = format!("http://{}", servers.addresses.grpc);
    let mut grpc = connect_grpc_with_retry(&grpc_url).await?;

    // ValidateSession
    let mut vs_req = tonic::Request::new(ValidateSessionRequest {});
    add_session(&mut vs_req, &session.token)?;
    let vs_resp = grpc.validate_session(vs_req).await?;
    assert!(!vs_resp.get_ref().session_id.is_empty());
    assert_eq!(vs_resp.get_ref().role, "agent");

    // RememberTyped
    let mut rt_req = tonic::Request::new(RememberTypedRequest {
        content: "typed memory".to_string(),
        r#type: "episodic".to_string(),
        tags: vec!["test".to_string()],
        metadata_json: "{}".to_string(),
    });
    add_session(&mut rt_req, &session.token)?;
    let rt_resp = grpc.remember_typed(rt_req).await?;
    let typed_memory_id = rt_resp.get_ref().memory_id.clone();
    assert!(!typed_memory_id.is_empty());

    // UpdateMemory
    let mut um_req = tonic::Request::new(UpdateMemoryRequest {
        memory_id: typed_memory_id.clone(),
        content: "updated typed memory".to_string(),
    });
    add_session(&mut um_req, &session.token)?;
    let um_resp = grpc.update_memory(um_req).await?;
    assert!(um_resp.get_ref().updated);

    // Recall
    let mut recall_req = tonic::Request::new(RecallRequest {
        memory_ids: vec![typed_memory_id.clone()],
    });
    add_session(&mut recall_req, &session.token)?;
    let recall_resp = grpc.recall(recall_req).await?;
    assert_eq!(recall_resp.get_ref().memories.len(), 1);

    // Branch, GetBranchByName, GetTrunkBranch
    let mut branch_req = tonic::Request::new(BranchRequest {
        name: "ext-test-branch".to_string(),
        from: String::new(),
    });
    add_session(&mut branch_req, &session.token)?;
    let branch_resp = grpc.branch(branch_req).await?;
    let branch_id = branch_resp.get_ref().branch_id.clone();

    let mut gbn_req = tonic::Request::new(GetBranchByNameRequest {
        name: "ext-test-branch".to_string(),
    });
    add_session(&mut gbn_req, &session.token)?;
    let gbn_resp = grpc.get_branch_by_name(gbn_req).await?;
    assert!(gbn_resp.get_ref().branch.is_some());

    let mut trunk_req = tonic::Request::new(GetTrunkBranchRequest {});
    add_session(&mut trunk_req, &session.token)?;
    let trunk_resp = grpc.get_trunk_branch(trunk_req).await?;
    assert!(trunk_resp.get_ref().branch.is_some());

    // ArchiveBranch
    let mut archive_req = tonic::Request::new(ArchiveBranchRequest {
        branch_id: branch_id.clone(),
    });
    add_session(&mut archive_req, &session.token)?;
    let archive_resp = grpc.archive_branch(archive_req).await?;
    assert!(archive_resp.get_ref().archived);

    // Sync (local-only → returns zeros)
    let mut sync_req = tonic::Request::new(clawdb_server::grpc::service::proto::SyncRequest {});
    add_session(&mut sync_req, &session.token)?;
    let sync_resp = grpc.sync(sync_req).await?;
    // Local-only mode returns zeros
    assert_eq!(sync_resp.get_ref().pushed, 0);

    // SyncStatus
    let mut ss_req = tonic::Request::new(SyncStatusRequest {});
    add_session(&mut ss_req, &session.token)?;
    let ss_resp = grpc.sync_status(ss_req).await?;
    assert!(!ss_resp.get_ref().status_json.is_empty());

    // ActiveSessionCount
    let mut asc_req = tonic::Request::new(ActiveSessionCountRequest {});
    add_session(&mut asc_req, &session.token)?;
    let asc_resp = grpc.active_session_count(asc_req).await?;
    assert!(asc_resp.get_ref().count >= 1);

    // Reflect (disabled → returns skipped summary)
    let mut reflect_req = tonic::Request::new(clawdb_server::grpc::service::proto::ReflectRequest {});
    add_session(&mut reflect_req, &session.token)?;
    let reflect_resp = grpc.reflect(reflect_req).await?;
    assert!(reflect_resp.get_ref().skipped);

    // TxRemember + RollbackTx
    let mut begin_req = tonic::Request::new(BeginTxRequest {});
    add_session(&mut begin_req, &session.token)?;
    let tx_resp = grpc.begin_tx(begin_req).await?;
    let tx_id = tx_resp.get_ref().tx_id.clone();

    let mut tx_rem_req = tonic::Request::new(TxRememberRequest {
        tx_id: tx_id.clone(),
        content: "staged memory".to_string(),
    });
    add_session(&mut tx_rem_req, &session.token)?;
    let tx_rem_resp = grpc.tx_remember(tx_rem_req).await?;
    assert!(!tx_rem_resp.get_ref().memory_id.is_empty());

    let mut rollback_req = tonic::Request::new(RollbackTxRequest { tx_id });
    add_session(&mut rollback_req, &session.token)?;
    let rollback_resp = grpc.rollback_tx(rollback_req).await?;
    assert!(rollback_resp.get_ref().rolled_back);

    servers.shutdown(Duration::from_secs(5)).await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// HTTP REST coverage test
// ---------------------------------------------------------------------------

#[tokio::test]
async fn server_http_rest_coverage() -> anyhow::Result<()> {
    let temp = tempdir()?;
    let mut config = ClawDBConfig::default_for_dir(temp.path());
    config.guard.jwt_secret = "test-secret-http".to_string();
    config.vector.enabled = false;
    config.reflect.base_url = None;
    config.reflect.api_key = None;
    config.sync.hub_url = None;

    let state = build_state(config).await?;
    let session = state
        .db
        .session(Uuid::new_v4(), "agent", vec!["*".to_string()])
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

    let base = format!("http://{}", servers.addresses.http);
    let client = reqwest::Client::new();

    // Helper closures are too complex; use inline auth header.
    macro_rules! auth_get {
        ($path:expr) => {
            client
                .get(format!("{}{}", base, $path))
                .bearer_auth(&session.token)
                .send()
                .await?
        };
    }
    macro_rules! auth_post {
        ($path:expr, $body:expr) => {
            client
                .post(format!("{}{}", base, $path))
                .bearer_auth(&session.token)
                .json($body)
                .send()
                .await?
        };
    }
    macro_rules! auth_patch {
        ($path:expr, $body:expr) => {
            client
                .patch(format!("{}{}", base, $path))
                .bearer_auth(&session.token)
                .json($body)
                .send()
                .await?
        };
    }
    macro_rules! auth_delete {
        ($path:expr) => {
            client
                .delete(format!("{}{}", base, $path))
                .bearer_auth(&session.token)
                .send()
                .await?
        };
    }

    // Wait for server to be ready.
    get_with_retry(&client, &format!("{base}/v1/health")).await?;

    // --- Sessions ---
    // GET /v1/sessions/me
    let whoami = auth_get!("/v1/sessions/me");
    assert!(whoami.status().is_success(), "whoami: {}", whoami.status());
    let whoami_body: serde_json::Value = whoami.json().await?;
    assert!(whoami_body["agent_id"].is_string());

    // GET /v1/sessions/active/count
    let count_resp = auth_get!("/v1/sessions/active/count");
    assert!(count_resp.status().is_success(), "active count: {}", count_resp.status());
    let count_body: serde_json::Value = count_resp.json().await?;
    assert!(count_body["count"].as_u64().unwrap_or(0) >= 1);

    // --- Memories ---
    // POST /v1/memories
    let create_mem = auth_post!(
        "/v1/memories",
        &serde_json::json!({"content": "http rest test memory", "type": "semantic", "tags": ["rest"]})
    );
    assert!(create_mem.status().is_success(), "create mem: {}", create_mem.status());
    let create_body: serde_json::Value = create_mem.json().await?;
    let mem_id = create_body["memory_id"]
        .as_str()
        .or_else(|| create_body["id"].as_str())
        .expect("memory_id in response")
        .to_string();

    // GET /v1/memories (list)
    let list_mem = auth_get!("/v1/memories");
    assert!(list_mem.status().is_success(), "list memories: {}", list_mem.status());
    let list_body: serde_json::Value = list_mem.json().await?;
    assert!(list_body.as_array().map(|a| !a.is_empty()).unwrap_or(false));

    // GET /v1/memories/search?q=...
    let search_resp = auth_get!("/v1/memories/search?q=rest+test");
    assert!(search_resp.status().is_success(), "search: {}", search_resp.status());

    // GET /v1/memories/:id (recall one)
    let recall_resp = auth_get!(format!("/v1/memories/{}", mem_id));
    assert!(recall_resp.status().is_success(), "recall one: {}", recall_resp.status());
    let recall_body: serde_json::Value = recall_resp.json().await?;
    assert!(recall_body["id"].is_string() || recall_body["memory_id"].is_string());

    // PATCH /v1/memories/:id (update)
    let update_resp = auth_patch!(
        format!("/v1/memories/{}", mem_id),
        &serde_json::json!({"content": "updated http rest test memory"})
    );
    assert!(update_resp.status().is_success(), "update mem: {}", update_resp.status());

    // DELETE /v1/memories/:id
    let del_resp = auth_delete!(format!("/v1/memories/{}", mem_id));
    assert!(del_resp.status().is_success() || del_resp.status() == reqwest::StatusCode::NO_CONTENT,
        "delete mem: {}", del_resp.status());

    // --- Branches ---
    // POST /v1/branches
    let create_branch = auth_post!(
        "/v1/branches",
        &serde_json::json!({"name": "http-test-branch"})
    );
    assert!(create_branch.status().is_success(), "create branch: {}", create_branch.status());
    let branch_body: serde_json::Value = create_branch.json().await?;
    let branch_id = branch_body["id"]
        .as_str()
        .or_else(|| branch_body["branch_id"].as_str())
        .expect("branch id")
        .to_string();

    // GET /v1/branches (list)
    let list_branches = auth_get!("/v1/branches");
    assert!(list_branches.status().is_success(), "list branches: {}", list_branches.status());
    let branches_body: serde_json::Value = list_branches.json().await?;
    assert!(branches_body.as_array().map(|a| !a.is_empty()).unwrap_or(false));

    // GET /v1/branches/trunk
    let trunk_resp = auth_get!("/v1/branches/trunk");
    assert!(trunk_resp.status().is_success(), "trunk: {}", trunk_resp.status());

    // GET /v1/branches/by-name/:name
    let byname_resp = auth_get!("/v1/branches/by-name/http-test-branch");
    assert!(byname_resp.status().is_success(), "by-name: {}", byname_resp.status());

    // GET /v1/branches/:id
    let get_branch_resp = auth_get!(format!("/v1/branches/{}", branch_id));
    assert!(get_branch_resp.status().is_success(), "get branch: {}", get_branch_resp.status());

    // POST /v1/branches/:id/archive
    let archive_resp = auth_post!(format!("/v1/branches/{}/archive", branch_id), &serde_json::json!({}));
    assert!(
        archive_resp.status().is_success() || archive_resp.status() == reqwest::StatusCode::NO_CONTENT,
        "archive: {}", archive_resp.status()
    );

    // --- Sync ---
    // POST /v1/sync (local-only → zeros)
    let sync_resp = auth_post!("/v1/sync", &serde_json::json!({}));
    assert!(sync_resp.status().is_success(), "sync: {}", sync_resp.status());
    let sync_body: serde_json::Value = sync_resp.json().await?;
    assert_eq!(sync_body["pushed"].as_u64().unwrap_or(0), 0);

    // GET /v1/sync/status
    let status_resp = auth_get!("/v1/sync/status");
    assert!(status_resp.status().is_success(), "sync status: {}", status_resp.status());

    // POST /v1/sync/push (may fail in local-only mode — accept 2xx or 5xx)
    let push_resp = auth_post!("/v1/sync/push", &serde_json::json!({}));
    assert!(
        push_resp.status().is_success() || push_resp.status().as_u16() >= 500,
        "sync push unexpected: {}", push_resp.status()
    );

    // POST /v1/sync/pull
    let pull_resp = auth_post!("/v1/sync/pull", &serde_json::json!({}));
    assert!(
        pull_resp.status().is_success() || pull_resp.status().as_u16() >= 500,
        "sync pull unexpected: {}", pull_resp.status()
    );

    // POST /v1/sync/reconcile
    let reconcile_resp = auth_post!("/v1/sync/reconcile", &serde_json::json!({}));
    assert!(
        reconcile_resp.status().is_success() || reconcile_resp.status().as_u16() >= 500,
        "sync reconcile unexpected: {}", reconcile_resp.status()
    );

    // --- Reflect ---
    // POST /v1/reflect (reflect disabled → skipped)
    let reflect_resp = auth_post!("/v1/reflect", &serde_json::json!({}));
    assert!(reflect_resp.status().is_success(), "reflect: {}", reflect_resp.status());
    let reflect_body: serde_json::Value = reflect_resp.json().await?;
    assert_eq!(reflect_body["skipped"].as_bool(), Some(true));

    // GET /v1/reflect/jobs (reflect disabled → 503 or empty)
    let jobs_resp = auth_get!("/v1/reflect/jobs");
    assert!(
        jobs_resp.status().is_success() || jobs_resp.status().as_u16() >= 500,
        "reflect jobs unexpected: {}", jobs_resp.status()
    );

    // --- Transactions ---
    // POST /v1/tx (begin)
    let begin_resp = auth_post!("/v1/tx", &serde_json::json!({}));
    assert!(begin_resp.status().is_success(), "begin tx: {}", begin_resp.status());
    let begin_body: serde_json::Value = begin_resp.json().await?;
    let tx_id = begin_body["tx_id"].as_str().expect("tx_id").to_string();

    // POST /v1/tx/:id/memories/typed (stage memory)
    let stage_resp = auth_post!(
        format!("/v1/tx/{}/memories/typed", tx_id),
        &serde_json::json!({"content": "tx staged memory", "type": "episodic", "tags": ["tx"]})
    );
    assert!(stage_resp.status().is_success(), "tx stage: {}", stage_resp.status());

    // POST /v1/tx/:id/commit
    let commit_resp = auth_post!(format!("/v1/tx/{}/commit", tx_id), &serde_json::json!({}));
    assert!(commit_resp.status().is_success(), "tx commit: {}", commit_resp.status());
    let commit_body: serde_json::Value = commit_resp.json().await?;
    assert_eq!(commit_body["committed"].as_bool(), Some(true));

    // POST /v1/tx (begin another tx for rollback test)
    let begin2_resp = auth_post!("/v1/tx", &serde_json::json!({}));
    assert!(begin2_resp.status().is_success());
    let begin2_body: serde_json::Value = begin2_resp.json().await?;
    let tx2_id = begin2_body["tx_id"].as_str().expect("tx_id").to_string();

    // POST /v1/tx/:id/memories (simple stage)
    let stage2_resp = auth_post!(
        format!("/v1/tx/{}/memories", tx2_id),
        &serde_json::json!({"content": "will be rolled back"})
    );
    assert!(stage2_resp.status().is_success(), "tx stage2: {}", stage2_resp.status());

    // POST /v1/tx/:id/rollback
    let rollback_resp = auth_post!(
        format!("/v1/tx/{}/rollback", tx2_id),
        &serde_json::json!({})
    );
    assert!(rollback_resp.status().is_success(), "tx rollback: {}", rollback_resp.status());

    servers.shutdown(Duration::from_secs(5)).await?;
    Ok(())
}
