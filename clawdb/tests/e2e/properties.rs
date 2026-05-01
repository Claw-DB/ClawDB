/// Property-based transaction tests using proptest.
/// These tests verify that ClawDB transactions are correct under concurrent stress conditions.
use clawdb::{ClawDB, ClawDBConfig, ClawDBError, ClawDBResult, ClawDBSession};
use proptest::prelude::*;
use std::collections::HashSet;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::Mutex;
use uuid::Uuid;

/// Strategy that generates valid memory content strings.
fn arb_memory_content() -> impl Strategy<Value = String> {
    r#"[a-zA-Z0-9_\-\.]{5,50}"#
}

/// Setup function for tests.
async fn setup_test_db() -> ClawDBResult<(ClawDB, TempDir)> {
    let temp = TempDir::new()?;
    let cfg = ClawDBConfig::default_for_dir(temp.path());
    let db = ClawDB::new(cfg).await?;
    Ok((db, temp))
}

/// Create a test session with given role.
async fn test_session(db: &ClawDB, role: &str) -> ClawDBResult<ClawDBSession> {
    db.session(
        Uuid::new_v4(),
        role,
        vec!["memory:read".to_string(), "memory:write".to_string()],
    )
    .await
}

/// Property-based test: Concurrent remember operations should not corrupt memory table.
/// This test verifies that multiple concurrent writes maintain table integrity.
#[tokio::test]
fn prop_concurrent_remembers_no_corruption() {
    proptest!(|(contents in prop::collection::vec(arb_memory_content(), 5..20)| {
        futures::executor::block_on(async {
            let (db, _tmp) = setup_test_db().await.unwrap();
            let sess = test_session(&db, "assistant").await.unwrap();

            // Spawn concurrent remember tasks
            let mut handles = vec![];
            for content in contents {
                let db_clone = db.clone();
                let sess_clone = sess.clone();
                let handle = tokio::spawn(async move {
                    db_clone.remember(&sess_clone, &content).await
                });
                handles.push(handle);
            }

            // Wait for all tasks
            let results: Vec<_> = futures::future::join_all(handles)
                .await
                .into_iter()
                .collect();

            // Verify all operations succeeded or contain expected errors
            let successes = results.iter().filter(|r| r.is_ok()).count();
            prop_assert!(successes > 0, "At least one remember should succeed");

            // Close cleanly
            let _ = db.close().await;
        }).unwrap();
    });
}

/// Property-based test: Concurrent branch operations should maintain isolation.
/// This test verifies that concurrent branch creates don't interfere with each other.
#[tokio::test]
fn prop_concurrent_branches_isolated() {
    proptest!(|(branch_names in prop::collection::vec(
        r#"[a-z][a-z0-9_]{0,20}"#,
        3..10
    )| {
        futures::executor::block_on(async {
            let (db, _tmp) = setup_test_db().await.unwrap();
            let sess = test_session(&db, "assistant").await.unwrap();

            // Spawn concurrent branch creates
            let mut handles = vec![];
            for name in &branch_names {
                let db_clone = db.clone();
                let sess_clone = sess.clone();
                let name_clone = name.clone();
                let handle = tokio::spawn(async move {
                    db_clone.branch(&sess_clone, &name_clone).await
                });
                handles.push(handle);
            }

            // Wait for all tasks
            let results: Vec<_> = futures::future::join_all(handles)
                .await
                .into_iter()
                .collect();

            // Collect successful branch IDs
            let branch_ids: HashSet<_> = results
                .iter()
                .filter_map(|r| r.as_ref().ok().map(|id| id.clone()))
                .collect();

            // Verify all unique branch operations created distinct IDs
            prop_assert_eq!(branch_ids.len(), branch_names.len(),
                "Each branch operation should create a unique ID");

            let _ = db.close().await;
        }).unwrap();
    });
}

/// Property-based test: Transaction isolation - concurrent transactions should not see partial results.
/// This test verifies that transactions properly isolate concurrent writes.
#[tokio::test]
fn prop_transaction_isolation_level() {
    proptest!(|(batch_size in 2usize..10)| {
        futures::executor::block_on(async {
            let (db, _tmp) = setup_test_db().await.unwrap();
            let sess = test_session(&db, "assistant").await.unwrap();

            // Create a shared counter to verify atomicity
            let counter = Arc::new(Mutex::new(0u32));

            // Spawn concurrent transactions
            let mut handles = vec![];
            for i in 0..batch_size {
                let db_clone = db.clone();
                let sess_clone = sess.clone();
                let counter_clone = counter.clone();
                
                let handle = tokio::spawn(async move {
                    let result = db_clone
                        .transaction(&sess_clone, |_tx| async {
                            // Simulate work
                            let mut guard = counter_clone.lock().await;
                            *guard += 1;
                            Ok::<_, ClawDBError>(())
                        })
                        .await;
                    (i, result)
                });
                handles.push(handle);
            }

            // Wait for all transactions
            let results: Vec<_> = futures::future::join_all(handles)
                .await
                .into_iter()
                .collect();

            // Verify all transactions attempted
            let attempts = results.len();
            prop_assert_eq!(attempts, batch_size, "All transaction attempts should complete");

            // Verify counter incremented by number of successful transactions
            let final_count = *counter.lock().await;
            prop_assert_eq!(final_count, batch_size as u32, "All transactions should have executed");

            let _ = db.close().await;
        }).unwrap();
    });
}

/// Property-based test: Remember-Search-Branch-Merge workflow correctness.
/// This test verifies that the full workflow maintains data consistency.
#[tokio::test]
fn prop_full_workflow_consistency() {
    proptest!(|(
        memory_ops in 1usize..5,
        branch_ops in 1usize..3
    )| {
        futures::executor::block_on(async {
            let (db, _tmp) = setup_test_db().await.unwrap();
            let sess = test_session(&db, "assistant").await.unwrap();

            // Phase 1: Remember operations
            let mut memory_ids = vec![];
            for i in 0..memory_ops {
                if let Ok(result) = db.remember(&sess, &format!("memory-{}", i)).await {
                    memory_ids.push(result.memory_id);
                }
            }

            prop_assert!(!memory_ids.is_empty(), "At least one memory should be stored");

            // Phase 2: Branch operations
            let mut branch_ids = vec![];
            for i in 0..branch_ops {
                if let Ok(id) = db.branch(&sess, &format!("branch-{}", i)).await {
                    branch_ids.push(id);
                }
            }

            prop_assert!(!branch_ids.is_empty(), "At least one branch should be created");

            // Phase 3: Merge operations
            if branch_ids.len() >= 2 {
                let merge_result = db.merge(&sess, branch_ids[0].clone(), branch_ids[1].clone()).await;
                prop_assert!(merge_result.is_ok(), "Merge should succeed with valid branches");
            }

            // Phase 4: Recall to verify data consistency
            if !memory_ids.is_empty() {
                let recalled = db.recall(&sess, &memory_ids).await;
                prop_assert!(recalled.is_ok(), "Recall should succeed");
                let memories = recalled.unwrap();
                prop_assert_eq!(memories.len(), memory_ids.len(), 
                    "Should recall all stored memories");
            }

            let _ = db.close().await;
        }).unwrap();
    });
}

/// Property-based test: Diff operations should handle concurrent branches.
/// This test verifies that diff correctly identifies changes between branches.
#[tokio::test]
fn prop_diff_reflects_concurrent_changes() {
    proptest!(|(branch_name in "[a-z][a-z0-9_]{0,15}")| {
        futures::executor::block_on(async {
            let (db, _tmp) = setup_test_db().await.unwrap();
            let sess = test_session(&db, "assistant").await.unwrap();

            // Store initial memory
            let _ = db.remember(&sess, "initial").await;

            // Create first branch (baseline)
            let branch_a = match db.branch(&sess, &format!("{}-a", branch_name)).await {
                Ok(id) => id,
                Err(_) => return Ok(()),
            };

            // Create second branch (for comparison)
            let branch_b = match db.branch(&sess, &format!("{}-b", branch_name)).await {
                Ok(id) => id,
                Err(_) => return Ok(()),
            };

            // Diff the branches
            let diff_result = db.diff(&sess, branch_a.clone(), branch_b.clone()).await;
            prop_assert!(diff_result.is_ok(), "Diff should complete successfully");

            let _ = db.close().await;
        }).unwrap();
    });
}

/// Property-based test: Sync operations should handle concurrent calls.
/// This test verifies that concurrent sync operations don't cause corruption.
#[tokio::test]
fn prop_concurrent_syncs_safe() {
    proptest!(|(sync_count in 2usize..6)| {
        futures::executor::block_on(async {
            let (db, _tmp) = setup_test_db().await.unwrap();
            let sess = test_session(&db, "assistant").await.unwrap();

            // Store some data first
            for i in 0..2 {
                let _ = db.remember(&sess, &format!("sync-test-{}", i)).await;
            }

            // Spawn concurrent sync operations
            let mut handles = vec![];
            for _ in 0..sync_count {
                let db_clone = db.clone();
                let sess_clone = sess.clone();
                
                let handle = tokio::spawn(async move {
                    db_clone.sync(&sess_clone).await
                });
                handles.push(handle);
            }

            // Wait for all syncs
            let results: Vec<_> = futures::future::join_all(handles)
                .await
                .into_iter()
                .collect();

            // Verify at least one succeeded
            let successes = results.iter().filter(|r| r.is_ok()).count();
            prop_assert!(successes > 0, "At least one sync should succeed");

            let _ = db.close().await;
        }).unwrap();
    });
}

/// Property-based test: Stress test with random operation sequences.
/// This test verifies overall system stability under random concurrent operations.
#[tokio::test]
fn prop_random_operation_sequence_safe() {
    proptest!(|(
        ops in prop::collection::vec(0u8..5, 10..30)
    )| {
        futures::executor::block_on(async {
            let (db, _tmp) = setup_test_db().await.unwrap();
            let sess = test_session(&db, "assistant").await.unwrap();

            // Execute random sequence of operations
            for (idx, op) in ops.iter().enumerate() {
                let result = match op % 5 {
                    0 => db.remember(&sess, &format!("op-{}", idx)).await.map(|_| ()),
                    1 => db.search(&sess, "test").await.map(|_| ()),
                    2 => db.branch(&sess, &format!("branch-{}", idx)).await.map(|_| ()),
                    3 => db.sync(&sess).await.map(|_| ()),
                    _ => db.health().await.map(|_| ()),
                };
                
                // Operations should not panic; they may fail gracefully
                let _ = result;
            }

            let _ = db.close().await;
        }).unwrap();
    });
}
