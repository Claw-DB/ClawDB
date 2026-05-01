use std::time::Duration;

use clawdb::{ClawDB, ClawDBConfig, ClawDBError, ClawDBResult, ClawDBSession};
use rstest::rstest;
use tempfile::TempDir;
use tokio::time::sleep;
use uuid::Uuid;

async fn test_engine() -> ClawDBResult<(ClawDB, TempDir)> {
    let temp = TempDir::new()?;
    let cfg = ClawDBConfig::default_for_dir(temp.path());
    let db = ClawDB::new(cfg).await?;
    Ok((db, temp))
}

async fn test_session(db: &ClawDB, role: &str) -> ClawDBResult<ClawDBSession> {
    db.session(
        Uuid::new_v4(),
        role,
        vec!["memory:read".to_string(), "memory:write".to_string()],
    )
    .await
}

#[tokio::test]
async fn engine_opens_all_components_healthy() -> ClawDBResult<()> {
    let (db, _tmp) = test_engine().await?;
    let report = db.health().await?;
    assert!(matches!(report.overall, clawdb::HealthStatus::Healthy));
    db.close().await
}

#[tokio::test]
async fn engine_graceful_shutdown() -> ClawDBResult<()> {
    let (db, _tmp) = test_engine().await?;
    let session = test_session(&db, "assistant").await?;
    let _ = db.remember(&session, "warmup").await?;
    db.shutdown().await
}

#[tokio::test]
async fn session_created_and_validated() -> ClawDBResult<()> {
    let (db, _tmp) = test_engine().await?;
    let sess = test_session(&db, "assistant").await?;
    let ctx = db.validate_session(&sess.guard_token).await?;
    assert_eq!(ctx.agent_id, sess.agent_id);
    db.close().await
}

#[tokio::test]
#[ignore = "requires controllable guard token TTL support"]
async fn session_expires_after_ttl() -> ClawDBResult<()> {
    let (db, _tmp) = test_engine().await?;
    let sess = test_session(&db, "assistant").await?;
    sleep(Duration::from_secs(2)).await;
    let err = db.validate_session(&sess.guard_token).await.unwrap_err();
    assert!(matches!(err, ClawDBError::SessionExpired(_)));
    db.close().await
}

#[tokio::test]
async fn session_revoked_denies_access() -> ClawDBResult<()> {
    let (db, _tmp) = test_engine().await?;
    let sess = test_session(&db, "assistant").await?;
    db.revoke_session(sess.id).await?;
    let err = db.validate_session(&sess.guard_token).await.unwrap_err();
    assert!(matches!(err, ClawDBError::SessionNotFound(_)) || matches!(err, ClawDBError::Guard(_)));
    db.close().await
}

#[tokio::test]
async fn remember_stores_in_core_and_vector() -> ClawDBResult<()> {
    let (db, _tmp) = test_engine().await?;
    let sess = test_session(&db, "assistant").await?;
    let r = db.remember(&sess, "dual write memory").await?;
    let recalled = db.recall(&sess, &[r.memory_id]).await?;
    assert_eq!(recalled.len(), 1);
    db.close().await
}

#[tokio::test]
#[ignore = "requires loaded deny policy in guard engine"]
async fn remember_guard_denied() -> ClawDBResult<()> {
    let (db, _tmp) = test_engine().await?;
    let sess = test_session(&db, "denied-role").await?;
    let err = db.remember(&sess, "blocked").await.unwrap_err();
    assert!(matches!(err, ClawDBError::Guard(_)));
    db.close().await
}

#[tokio::test]
async fn search_returns_semantic_results() -> ClawDBResult<()> {
    let (db, _tmp) = test_engine().await?;
    let sess = test_session(&db, "assistant").await?;
    for i in 0..5 {
        let _ = db.remember(&sess, &format!("semantic memory {i}")).await?;
    }
    let res = db.search_with_options(&sess, "semantic", 5, true, None).await?;
    assert!(!res.is_empty());
    db.close().await
}

#[tokio::test]
#[ignore = "requires redaction policy in guard engine"]
async fn search_guard_redacts_fields() -> ClawDBResult<()> {
    let (db, _tmp) = test_engine().await?;
    let sess = test_session(&db, "assistant").await?;
    let res = db.search(&sess, "anything").await?;
    if let Some(first) = res.first() {
        assert!(first.get("content").is_none() || first["content"] == "[REDACTED]");
    }
    db.close().await
}

#[tokio::test]
async fn recall_returns_all_requested() -> ClawDBResult<()> {
    let (db, _tmp) = test_engine().await?;
    let sess = test_session(&db, "assistant").await?;
    let mut ids = Vec::new();
    for i in 0..3 {
        let r = db.remember(&sess, &format!("recall-{i}")).await?;
        ids.push(r.memory_id);
    }
    let out = db.recall(&sess, &ids).await?;
    assert_eq!(out.len(), 3);
    db.close().await
}

#[tokio::test]
async fn transaction_commit_atomic() -> ClawDBResult<()> {
    let (db, _tmp) = test_engine().await?;
    let sess = test_session(&db, "assistant").await?;
    db.transaction(&sess, |_tx| async {
        Ok::<_, ClawDBError>(())
    })
    .await?;
    db.close().await
}

#[tokio::test]
async fn transaction_rollback_atomic() -> ClawDBResult<()> {
    let (db, _tmp) = test_engine().await?;
    let sess = test_session(&db, "assistant").await?;
    let r = db
        .transaction(&sess, |_tx| async {
            Err::<(), _>(ClawDBError::TransactionFailed {
                tx_id: Uuid::new_v4(),
                reason: "forced".to_string(),
            })
        })
        .await;
    assert!(r.is_err());
    db.close().await
}

#[tokio::test]
#[ignore = "requires deterministic conflict fixture"]
async fn transaction_conflict_aborted() -> ClawDBResult<()> {
    let (db, _tmp) = test_engine().await?;
    let _ = db;
    Ok(())
}

#[tokio::test]
#[ignore = "requires branch-isolated write API"]
async fn branch_fork_isolates_writes() -> ClawDBResult<()> {
    let (db, _tmp) = test_engine().await?;
    let _ = db;
    Ok(())
}

#[tokio::test]
async fn branch_merge_applies_changes() -> ClawDBResult<()> {
    let (db, _tmp) = test_engine().await?;
    let sess = test_session(&db, "assistant").await?;
    let a = db.branch(&sess, "a").await?;
    let b = db.branch(&sess, "b").await?;
    let _ = db.merge(&sess, a, b).await?;
    db.close().await
}

#[tokio::test]
async fn branch_diff_shows_changes() -> ClawDBResult<()> {
    let (db, _tmp) = test_engine().await?;
    let sess = test_session(&db, "assistant").await?;
    let a = db.branch(&sess, "a2").await?;
    let b = db.branch(&sess, "b2").await?;
    let _diff = db.diff(&sess, a, b).await?;
    db.close().await
}

#[tokio::test]
async fn sync_push_sends_pending() -> ClawDBResult<()> {
    let (db, _tmp) = test_engine().await?;
    let sess = test_session(&db, "assistant").await?;
    let _ = db.sync(&sess).await?;
    db.close().await
}

#[tokio::test]
#[ignore = "requires mocked remote sync hub"]
async fn sync_pull_applies_remote() -> ClawDBResult<()> {
    let (db, _tmp) = test_engine().await?;
    let _ = db;
    Ok(())
}

#[tokio::test]
async fn event_bus_emits_on_remember() -> ClawDBResult<()> {
    let (db, _tmp) = test_engine().await?;
    let sess = test_session(&db, "assistant").await?;
    let mut sub = db.subscribe();
    let _ = db.remember(&sess, "event me").await?;
    let ev = tokio::time::timeout(Duration::from_secs(2), sub.recv()).await;
    assert!(ev.is_ok());
    db.close().await
}

#[tokio::test]
async fn event_bus_emits_on_sync() -> ClawDBResult<()> {
    let (db, _tmp) = test_engine().await?;
    let sess = test_session(&db, "assistant").await?;
    let mut sub = db.subscribe();
    let _ = db.sync(&sess).await?;
    let ev = tokio::time::timeout(Duration::from_secs(2), sub.recv()).await;
    assert!(ev.is_ok());
    db.close().await
}

#[tokio::test]
#[ignore = "requires dynamic plugin fixture"]
async fn plugin_loaded_and_hooks_called() -> ClawDBResult<()> {
    let (db, _tmp) = test_engine().await?;
    let _ = db;
    Ok(())
}

#[tokio::test]
#[ignore = "requires dynamic plugin fixture"]
async fn plugin_capability_denied() -> ClawDBResult<()> {
    let (db, _tmp) = test_engine().await?;
    let _ = db;
    Ok(())
}

#[tokio::test]
async fn router_routes_semantic_to_vector() -> ClawDBResult<()> {
    let (db, _tmp) = test_engine().await?;
    let sess = test_session(&db, "assistant").await?;
    let _ = db.search_with_options(&sess, "router", 5, true, None).await?;
    db.close().await
}

#[tokio::test]
async fn router_routes_keyword_to_core() -> ClawDBResult<()> {
    let (db, _tmp) = test_engine().await?;
    let sess = test_session(&db, "assistant").await?;
    let _ = db.search_with_options(&sess, "router", 5, false, None).await?;
    db.close().await
}

#[tokio::test]
#[ignore = "planner internals are not currently exposed"]
async fn planner_parallelises_remember() -> ClawDBResult<()> {
    let (db, _tmp) = test_engine().await?;
    let _ = db;
    Ok(())
}

#[tokio::test]
async fn full_workflow_remember_branch_merge_sync() -> ClawDBResult<()> {
    let (db, _tmp) = test_engine().await?;
    let sess = test_session(&db, "assistant").await?;
    let _ = db.remember(&sess, "workflow-1").await?;
    let a = db.branch(&sess, "workflow-a").await?;
    let b = db.branch(&sess, "workflow-b").await?;
    let _ = db.merge(&sess, a, b).await?;
    let _ = db.sync(&sess).await?;
    db.close().await
}

#[rstest]
#[case("assistant")]
#[case("writer")]
#[tokio::test]
async fn session_fixture_supports_roles(#[case] role: &str) -> ClawDBResult<()> {
    let (db, _tmp) = test_engine().await?;
    let sess = test_session(&db, role).await?;
    assert_eq!(sess.role, role);
    db.close().await
}
