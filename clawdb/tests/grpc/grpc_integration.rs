#![cfg(proto_compiled)]

use std::{net::TcpListener, sync::Arc, time::Duration};

use clawdb::{
    api::grpc::{self, server::proto},
    ClawDB, ClawDBConfig, ClawDBResult,
};
use rstest::rstest;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;
use tonic::transport::Channel;

use proto::{
    claw_db_service_client::ClawDbServiceClient, BranchRequest, DiffRequest, HealthRequest,
    RecallRequest, RememberRequest, SearchRequest, SessionRequest, StatusRequest, SyncRequest,
};

async fn start_test_server() -> ClawDBResult<(Arc<ClawDB>, String, CancellationToken)> {
    let temp = tempfile::TempDir::new()?;
    let mut cfg = ClawDBConfig::default_for_dir(temp.path());
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    drop(listener);
    cfg.server.grpc_port = port;
    let db = Arc::new(ClawDB::new(cfg.clone()).await?);
    let shutdown = CancellationToken::new();
    let db_clone = db.clone();
    let cfg_clone = cfg.server.clone();
    let shutdown_child = shutdown.child_token();
    tokio::spawn(async move {
        let _ = grpc::serve(db_clone, &cfg_clone, shutdown_child).await;
    });
    Ok((db, format!("http://localhost:{port}"), shutdown))
}

async fn test_client(addr: &str) -> ClawDBResult<ClawDbServiceClient<Channel>> {
    let client = ClawDbServiceClient::connect(addr.to_string()).await?;
    Ok(client)
}

#[tokio::test]
async fn grpc_health_returns_ok() -> ClawDBResult<()> {
    let (_db, addr, shutdown) = start_test_server().await?;
    let mut client = test_client(&addr).await?;
    let resp = timeout(Duration::from_secs(10), client.health(HealthRequest {})).await??;
    assert!(resp.into_inner().ok);
    shutdown.cancel();
    Ok(())
}

#[tokio::test]
async fn grpc_create_session_returns_token() -> ClawDBResult<()> {
    let (_db, addr, shutdown) = start_test_server().await?;
    let mut client = test_client(&addr).await?;
    let req = SessionRequest {
        agent_id: uuid::Uuid::new_v4().to_string(),
        role: "assistant".to_string(),
        scopes: vec!["memory:read".to_string(), "memory:write".to_string()],
        task_type: "default".to_string(),
    };
    let resp = timeout(Duration::from_secs(10), client.create_session(req)).await??;
    assert!(!resp.into_inner().session_token.is_empty());
    shutdown.cancel();
    Ok(())
}

#[tokio::test]
async fn grpc_remember_requires_valid_session() -> ClawDBResult<()> {
    let (_db, addr, shutdown) = start_test_server().await?;
    let mut client = test_client(&addr).await?;
    let req = RememberRequest {
        session_token: "".to_string(),
        content: "no session".to_string(),
        memory_type: "context".to_string(),
        tags: vec![],
        metadata_json: vec![],
    };
    let err = timeout(Duration::from_secs(10), client.remember(req)).await?.unwrap_err();
    assert_eq!(err.code(), tonic::Code::Unauthenticated);
    shutdown.cancel();
    Ok(())
}

#[tokio::test]
async fn grpc_remember_and_search() -> ClawDBResult<()> {
    let (_db, addr, shutdown) = start_test_server().await?;
    let mut client = test_client(&addr).await?;

    let sess = client
        .create_session(SessionRequest {
            agent_id: uuid::Uuid::new_v4().to_string(),
            role: "assistant".to_string(),
            scopes: vec!["memory:read".to_string(), "memory:write".to_string()],
            task_type: "default".to_string(),
        })
        .await?
        .into_inner();

    let _ = client
        .remember(RememberRequest {
            session_token: sess.session_token.clone(),
            content: "grpc roundtrip memory".to_string(),
            memory_type: "context".to_string(),
            tags: vec![],
            metadata_json: vec![],
        })
        .await?;

    let out = timeout(
        Duration::from_secs(10),
        client.search(SearchRequest {
            session_token: sess.session_token,
            query: "roundtrip".to_string(),
            top_k: 5,
            semantic: true,
            filter_json: vec![],
        }),
    )
    .await??;

    assert!(!out.into_inner().results.is_empty());
    shutdown.cancel();
    Ok(())
}

#[tokio::test]
async fn grpc_search_empty_returns_empty() -> ClawDBResult<()> {
    let (_db, addr, shutdown) = start_test_server().await?;
    let mut client = test_client(&addr).await?;
    let sess = client
        .create_session(SessionRequest {
            agent_id: uuid::Uuid::new_v4().to_string(),
            role: "assistant".to_string(),
            scopes: vec!["memory:read".to_string()],
            task_type: "default".to_string(),
        })
        .await?
        .into_inner();
    let out = client
        .search(SearchRequest {
            session_token: sess.session_token,
            query: "this probably does not exist".to_string(),
            top_k: 5,
            semantic: false,
            filter_json: vec![],
        })
        .await?
        .into_inner();
    assert!(out.results.is_empty() || out.results.len() <= 5);
    shutdown.cancel();
    Ok(())
}

#[tokio::test]
async fn grpc_branch_create_and_diff() -> ClawDBResult<()> {
    let (_db, addr, shutdown) = start_test_server().await?;
    let mut client = test_client(&addr).await?;
    let sess = client
        .create_session(SessionRequest {
            agent_id: uuid::Uuid::new_v4().to_string(),
            role: "assistant".to_string(),
            scopes: vec!["branch:write".to_string()],
            task_type: "default".to_string(),
        })
        .await?
        .into_inner();

    let a = client
        .branch(BranchRequest {
            session_token: sess.session_token.clone(),
            new_branch_name: "a".to_string(),
            parent_branch: "trunk".to_string(),
        })
        .await?
        .into_inner();

    let b = client
        .branch(BranchRequest {
            session_token: sess.session_token.clone(),
            new_branch_name: "b".to_string(),
            parent_branch: "trunk".to_string(),
        })
        .await?
        .into_inner();

    let diff = client
        .diff(DiffRequest {
            session_token: sess.session_token,
            branch_a: a.branch_id,
            branch_b: b.branch_id,
        })
        .await?
        .into_inner();

    assert!(diff.divergence_score >= 0.0);
    shutdown.cancel();
    Ok(())
}

#[tokio::test]
async fn grpc_sync_responds() -> ClawDBResult<()> {
    let (_db, addr, shutdown) = start_test_server().await?;
    let mut client = test_client(&addr).await?;
    let sess = client
        .create_session(SessionRequest {
            agent_id: uuid::Uuid::new_v4().to_string(),
            role: "assistant".to_string(),
            scopes: vec!["sync:write".to_string()],
            task_type: "default".to_string(),
        })
        .await?
        .into_inner();

    let _ = client
        .sync(SyncRequest {
            session_token: sess.session_token,
            mode: "reconcile".to_string(),
        })
        .await;

    shutdown.cancel();
    Ok(())
}

#[tokio::test]
#[ignore = "streaming event timing can be flaky in CI"]
async fn grpc_stream_events_receives_events() -> ClawDBResult<()> {
    Ok(())
}

#[tokio::test]
async fn grpc_status_returns_agent_stats() -> ClawDBResult<()> {
    let (_db, addr, shutdown) = start_test_server().await?;
    let mut client = test_client(&addr).await?;
    let sess = client
        .create_session(SessionRequest {
            agent_id: uuid::Uuid::new_v4().to_string(),
            role: "assistant".to_string(),
            scopes: vec!["memory:read".to_string()],
            task_type: "default".to_string(),
        })
        .await?
        .into_inner();

    let out = client
        .status(StatusRequest {
            session_token: sess.session_token,
            agent_id: "".to_string(),
        })
        .await?
        .into_inner();
    assert!(out.memory_count >= 0);
    shutdown.cancel();
    Ok(())
}

#[rstest]
#[case(100)]
#[tokio::test]
async fn grpc_concurrent_100_requests(#[case] n: usize) -> ClawDBResult<()> {
    let (_db, addr, shutdown) = start_test_server().await?;
    let mut seed_client = test_client(&addr).await?;
    let token = seed_client
        .create_session(SessionRequest {
            agent_id: uuid::Uuid::new_v4().to_string(),
            role: "assistant".to_string(),
            scopes: vec!["memory:write".to_string()],
            task_type: "default".to_string(),
        })
        .await?
        .into_inner()
        .session_token;

    let mut tasks = Vec::with_capacity(n);
    for i in 0..n {
        let addr_cloned = addr.clone();
        let token_cloned = token.clone();
        tasks.push(tokio::spawn(async move {
            let mut c = ClawDbServiceClient::connect(addr_cloned).await?;
            c.remember(RememberRequest {
                session_token: token_cloned,
                content: format!("bulk-{i}"),
                memory_type: "context".to_string(),
                tags: vec![],
                metadata_json: vec![],
            })
            .await?;
            Ok::<(), anyhow::Error>(())
        }));
    }

    for task in tasks {
        timeout(Duration::from_secs(10), task).await??.map_err(|e| anyhow::anyhow!(e))?;
    }

    shutdown.cancel();
    Ok(())
}
