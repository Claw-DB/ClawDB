use std::time::Duration;

use clawdb::{ClawDB, ClawDBConfig, ClawDBError, ClawDBResult};
use tempfile::TempDir;
use tokio::time::sleep;
use uuid::Uuid;

fn test_config(dir: &TempDir) -> ClawDBConfig {
    let mut config = ClawDBConfig::default_for_dir(dir.path());
    config.guard.jwt_secret = "test-secret".to_string();
    config.vector.enabled = false;
    config.sync.hub_url = None;
    config
}

async fn setup() -> ClawDBResult<(ClawDB, TempDir)> {
    let dir = TempDir::new()?;
    let db = ClawDB::new(test_config(&dir)).await?;
    Ok((db, dir))
}

#[tokio::test]
async fn open_default_equivalent_from_env() -> ClawDBResult<()> {
    let dir = TempDir::new()?;
    std::env::set_var("CLAW_DATA_DIR", dir.path());
    std::env::set_var("CLAW_GUARD_JWT_SECRET", "test-secret");
    std::env::set_var("CLAW_LOG_LEVEL", "debug");
    std::env::set_var("CLAW_LOG_FORMAT", "console");

    let config = ClawDBConfig::from_env()?;
    assert_eq!(config.data_dir, dir.path());
    assert_eq!(config.log_level, "debug");
    assert_eq!(config.log_format, "console");
    assert_eq!(config.guard.jwt_secret, "test-secret");

    std::env::remove_var("CLAW_DATA_DIR");
    std::env::remove_var("CLAW_GUARD_JWT_SECRET");
    std::env::remove_var("CLAW_LOG_LEVEL");
    std::env::remove_var("CLAW_LOG_FORMAT");
    Ok(())
}

#[tokio::test]
async fn test_remember_and_search() -> ClawDBResult<()> {
    let (db, _dir) = setup().await?;
    let session = db
        .session(
            Uuid::new_v4(),
            "assistant",
            vec!["memory:read".to_string(), "memory:write".to_string()],
        )
        .await?;

    db.remember(&session, "clawdb memory one").await?;
    db.remember(&session, "clawdb memory two").await?;
    db.remember(&session, "clawdb memory three").await?;

    let hits = db.search(&session, "clawdb").await?;
    assert!(!hits.is_empty());
    db.close().await
}

#[tokio::test]
async fn test_permission_denied() -> ClawDBResult<()> {
    let (db, _dir) = setup().await?;
    let session = db
        .session(Uuid::new_v4(), "assistant", vec!["memory:read".to_string()])
        .await?;

    let err = db.remember(&session, "blocked write").await.unwrap_err();
    assert!(matches!(err, ClawDBError::PermissionDenied(_)));
    db.close().await
}

#[tokio::test]
async fn test_branch_lifecycle() -> ClawDBResult<()> {
    let (db, _dir) = setup().await?;
    let session = db
        .session(
            Uuid::new_v4(),
            "assistant",
            vec!["branch:read".to_string(), "branch:write".to_string()],
        )
        .await?;

    let source = db.branch(&session, "source").await?;
    let target = db.branch(&session, "target").await?;
    let _ = db.merge(&session, source, target).await?;
    let _ = db.diff(&session, source, target).await?;
    db.close().await
}

#[tokio::test]
async fn test_transaction_commit() -> ClawDBResult<()> {
    let (db, _dir) = setup().await?;
    let session = db
        .session(
            Uuid::new_v4(),
            "assistant",
            vec!["memory:read".to_string(), "memory:write".to_string()],
        )
        .await?;

    let mut tx = db.transaction(&session).await?;
    let mut ids = Vec::new();
    for idx in 0..5 {
        let id = tx.remember(&format!("txcommit{idx}")).await?;
        ids.push(id);
    }
    tx.commit().await?;

    let records = db.recall(&session, &ids).await?;
    assert_eq!(records.len(), 5);
    db.close().await
}

#[tokio::test]
async fn test_transaction_rollback() -> ClawDBResult<()> {
    let (db, _dir) = setup().await?;
    let session = db
        .session(
            Uuid::new_v4(),
            "assistant",
            vec!["memory:read".to_string(), "memory:write".to_string()],
        )
        .await?;

    let mut tx = db.transaction(&session).await?;
    for idx in 0..5 {
        let _ = tx.remember(&format!("txrollback{idx}")).await?;
    }
    tx.rollback().await?;

    let hits = db.search(&session, "txrollback").await?;
    assert_eq!(hits.len(), 0);
    db.close().await
}

#[tokio::test]
async fn test_health_ok() -> ClawDBResult<()> {
    let (db, _dir) = setup().await?;
    let health = db.health().await?;
    assert!(health.ok);
    assert!(health.components.values().all(|v| *v));
    db.close().await
}

#[tokio::test]
async fn test_session_expiry() -> ClawDBResult<()> {
    let (db, _dir) = setup().await?;
    let session = db
        .session_with_ttl(
            Uuid::new_v4(),
            "assistant",
            vec!["memory:write".to_string()],
            1,
        )
        .await?;

    sleep(Duration::from_secs(2)).await;
    let err = db.remember(&session, "should fail").await.unwrap_err();
    assert!(matches!(err, ClawDBError::SessionInvalid));
    db.close().await
}
