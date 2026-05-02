//! Runtime benchmarks for ClawDB.

use clawdb::{ClawDB, ClawDBConfig, ClawDBResult, ClawDBSession};
use criterion::{criterion_group, criterion_main, Criterion};
use tempfile::TempDir;
use tokio::runtime::Runtime;
use uuid::Uuid;

async fn bench_engine() -> ClawDBResult<(ClawDB, TempDir)> {
    let temp = TempDir::new()?;
    let mut cfg = ClawDBConfig::default_for_dir(temp.path());
    cfg.guard.jwt_secret = "bench-secret".to_string();
    cfg.vector.enabled = false;
    let db = ClawDB::new(cfg).await?;
    let s = bench_session(&db).await?;
    for i in 0..100 {
        let _ = db.remember(&s, &format!("seed memory {i}")).await?;
    }
    Ok((db, temp))
}

async fn bench_session(db: &ClawDB) -> ClawDBResult<ClawDBSession> {
    db.session(
        Uuid::new_v4(),
        "assistant",
        vec!["memory:read".to_string(), "memory:write".to_string()],
    )
    .await
}

fn runtime_benchmarks(c: &mut Criterion) {
    let rt = Runtime::new().unwrap_or_else(|_| panic!("failed to create tokio runtime"));
    let (db, _temp) = rt
        .block_on(bench_engine())
        .unwrap_or_else(|_| panic!("failed to start bench engine"));
    let session = rt
        .block_on(bench_session(&db))
        .unwrap_or_else(|_| panic!("failed to create bench session"));

    c.bench_function("remember_single", |b| {
        b.to_async(&rt).iter(|| async {
            let _ = db.remember(&session, "single bench memory").await;
        });
    });

    c.bench_function("search_fts_top5", |b| {
        b.to_async(&rt).iter(|| async {
            let _ = db
                .search_with_options(&session, "seed", 5, false, None)
                .await;
        });
    });

    c.bench_function("transaction_commit_5", |b| {
        b.to_async(&rt).iter(|| async {
            if let Ok(mut tx) = db.transaction(&session).await {
                for i in 0..5 {
                    let _ = tx.remember(&format!("txbench{i}")).await;
                }
                let _ = tx.commit().await;
            }
        });
    });

    c.bench_function("session_validate", |b| {
        b.to_async(&rt).iter(|| async {
            let _ = db.validate_session(&session.token).await;
        });
    });
}

criterion_group!(runtime_bench, runtime_benchmarks);
criterion_main!(runtime_bench);
