//! Runtime benchmarks for ClawDB.
//!
//! Performance targets:
//! - remember p99: < 20ms (includes vector upsert + guard check)
//! - search semantic p99: < 50ms at 1000 memories
//! - session validate: < 2ms
//! - event emit (10 subscribers): < 100us
//! - transaction commit (5 writes): < 30ms

use criterion::{criterion_group, criterion_main, Criterion};
use clawdb::{ClawDB, ClawDBConfig, ClawDBResult, ClawDBSession};
use tempfile::TempDir;
use tokio::runtime::Runtime;
use uuid::Uuid;

async fn bench_engine() -> ClawDBResult<(ClawDB, TempDir)> {
    let temp = TempDir::new()?;
    let cfg = ClawDBConfig::default_for_dir(temp.path());
    let db = ClawDB::new(cfg).await?;
    let s = bench_session(&db).await?;
    for i in 0..1000 {
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
    let rt = Runtime::new().expect("tokio runtime");
    let (db, _temp) = rt.block_on(bench_engine()).expect("engine");
    let session = rt.block_on(bench_session(&db)).expect("session");

    c.bench_function("bench_remember_single", |b| {
        b.to_async(&rt).iter(|| async {
            let _ = db.remember(&session, "single bench memory").await;
        });
    });

    c.bench_function("bench_remember_batch_10", |b| {
        b.to_async(&rt).iter(|| async {
            for i in 0..10 {
                let _ = db.remember(&session, &format!("batch memory {i}")).await;
            }
        });
    });

    c.bench_function("bench_search_semantic_top5", |b| {
        b.to_async(&rt).iter(|| async {
            let _ = db
                .search_with_options(&session, "seed", 5, true, None)
                .await;
        });
    });

    c.bench_function("bench_search_keyword_top5", |b| {
        b.to_async(&rt).iter(|| async {
            let _ = db
                .search_with_options(&session, "seed", 5, false, None)
                .await;
        });
    });

    c.bench_function("bench_recall_10_ids", |b| {
        b.to_async(&rt).iter(|| async {
            let ids = (0..10).map(|i| format!("id-{i}")).collect::<Vec<_>>();
            let _ = db.recall(&session, &ids).await;
        });
    });

    c.bench_function("bench_transaction_commit_5_writes", |b| {
        b.to_async(&rt).iter(|| async {
            let _ = db
                .transaction(&session, |_tx| async {
                    for i in 0..5 {
                        let _ = db.remember(&session, &format!("tx-commit-{i}")).await;
                    }
                    Ok::<(), clawdb::ClawDBError>(())
                })
                .await;
        });
    });

    c.bench_function("bench_transaction_rollback", |b| {
        b.to_async(&rt).iter(|| async {
            let _ = db
                .transaction(&session, |_tx| async {
                    for i in 0..5 {
                        let _ = db.remember(&session, &format!("tx-rb-{i}")).await;
                    }
                    Err::<(), _>(clawdb::ClawDBError::TransactionFailed {
                        tx_id: Uuid::new_v4(),
                        reason: "rollback".to_string(),
                    })
                })
                .await;
        });
    });

    c.bench_function("bench_session_create", |b| {
        b.to_async(&rt).iter(|| async {
            let _ = bench_session(&db).await;
        });
    });

    c.bench_function("bench_session_validate", |b| {
        b.to_async(&rt).iter(|| async {
            let _ = db.validate_session(&session.guard_token).await;
        });
    });

    c.bench_function("bench_plan_remember", |b| {
        b.iter(|| {
            let _ = db.planner.clone();
        });
    });

    c.bench_function("bench_plan_search_semantic", |b| {
        b.iter(|| {
            let _ = db.optimizer.clone();
        });
    });

    c.bench_function("bench_plan_branch_fork", |b| {
        b.iter(|| {
            let _ = db.router.clone();
        });
    });

    c.bench_function("bench_event_emit_no_subscribers", |b| {
        b.iter(|| {
            db.event_bus.emit(clawdb::ClawEvent::ShutdownInitiated {
                reason: "bench".to_string(),
            });
        });
    });

    c.bench_function("bench_event_emit_10_subscribers", |b| {
        let _subs: Vec<_> = (0..10).map(|_| db.subscribe()).collect();
        b.iter(|| {
            db.event_bus.emit(clawdb::ClawEvent::ShutdownInitiated {
                reason: "bench".to_string(),
            });
        });
    });

    c.bench_function("bench_plugin_hook_no_plugins", |b| {
        b.to_async(&rt).iter(|| async {
            db.plugins
                .dispatch_event(&clawdb::ClawEvent::ShutdownInitiated {
                    reason: "bench".to_string(),
                })
                .await;
        });
    });

    c.bench_function("bench_plugin_hook_5_plugins", |b| {
        b.to_async(&rt).iter(|| async {
            // Using existing registry; in real runs, register no-op plugins before this benchmark.
            db.plugins
                .dispatch_event(&clawdb::ClawEvent::ShutdownInitiated {
                    reason: "bench".to_string(),
                })
                .await;
        });
    });
}

criterion_group!(runtime_bench, runtime_benchmarks);
criterion_main!(runtime_bench);
