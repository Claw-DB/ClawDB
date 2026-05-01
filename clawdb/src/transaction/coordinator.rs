//! `TransactionCoordinator`: cross-subsystem two-phase commit coordination.
//!
//! The coordinator is the "brain" of the 2PC protocol.  It does not hold any
//! subsystem handles itself; those are owned by [`super::manager::TransactionManager`].
//! The coordinator's job is to:
//!
//! 1. Track which transactions are in-flight and their write sets.
//! 2. Detect write-set conflicts between concurrent transactions.
//! 3. Provide the phase-1 (prepare) gate: reject a commit if another active
//!    transaction has a conflicting write set.
//!
//! Conflict detection uses a simple optimistic approach: a transaction's write
//! set is registered when it begins, updated as writes are made, and checked
//! for overlap with all other active write sets at commit-prepare time.

use std::collections::HashMap;
use std::sync::Mutex;

use uuid::Uuid;

use crate::error::{ClawDBError, ClawDBResult};

// ── Internal state ──────────────────────────────────────────────────────────

/// Internal per-transaction record tracked by the coordinator.
#[derive(Debug, Clone)]
struct TxRecord {
    session_id: Uuid,
    write_set: Vec<String>,
    started_at: std::time::Instant,
}

// ── Coordinator ─────────────────────────────────────────────────────────────

/// Tracks in-flight transactions and arbitrates commits via optimistic conflict detection.
///
/// All public methods are synchronous and use a `Mutex` internally so they can be
/// called from non-async contexts without spawning tasks.  The critical section is
/// intentionally kept short (no I/O inside the lock).
pub struct TransactionCoordinator {
    active: Mutex<HashMap<Uuid, TxRecord>>,
}

impl TransactionCoordinator {
    /// Creates a new, empty `TransactionCoordinator`.
    pub fn new() -> Self {
        Self {
            active: Mutex::new(HashMap::new()),
        }
    }

    /// Registers a newly-begun transaction with an empty write set.
    ///
    /// Must be called exactly once per transaction, before any writes.
    pub fn register(&self, tx_id: Uuid, session_id: Uuid) {
        let mut map = self.active.lock().expect("coordinator lock poisoned");
        map.insert(
            tx_id,
            TxRecord {
                session_id,
                write_set: Vec::new(),
                started_at: std::time::Instant::now(),
            },
        );
    }

    /// Appends `keys` to the write set of `tx_id`.
    ///
    /// No-ops if the transaction is no longer active (e.g., already committed or
    /// rolled back).
    pub fn extend_write_set(&self, tx_id: Uuid, keys: impl IntoIterator<Item = String>) {
        let mut map = self.active.lock().expect("coordinator lock poisoned");
        if let Some(record) = map.get_mut(&tx_id) {
            record.write_set.extend(keys);
        }
    }

    /// Phase-1 gate: checks `tx_id`'s write set for conflicts with all other active
    /// transactions.
    ///
    /// Returns `Ok(())` if no conflict is detected, or
    /// `Err(`[`ClawDBError::TransactionConflict`]`)` naming the conflicting peer.
    pub fn prepare(&self, tx_id: Uuid, additional_writes: &[String]) -> ClawDBResult<()> {
        let map = self.active.lock().expect("coordinator lock poisoned");

        let my_writes: Vec<&str> = map
            .get(&tx_id)
            .map(|r| r.write_set.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default();

        for (other_id, other) in map.iter() {
            if *other_id == tx_id {
                continue;
            }
            let conflict = my_writes
                .iter()
                .chain(additional_writes.iter().map(|s| s.as_str()).collect::<Vec<_>>().iter())
                .any(|k| other.write_set.iter().any(|w| w == k));
            if conflict {
                return Err(ClawDBError::TransactionConflict {
                    tx_id,
                    conflicting_tx: *other_id,
                });
            }
        }
        Ok(())
    }

    /// Legacy alias kept for backward compatibility with existing call sites.
    pub fn check_conflicts(&self, tx_id: Uuid, write_set: &[String]) -> ClawDBResult<()> {
        self.prepare(tx_id, write_set)
    }

    /// Deregisters a completed (committed or rolled-back) transaction.
    pub fn deregister(&self, tx_id: Uuid) {
        self.active
            .lock()
            .expect("coordinator lock poisoned")
            .remove(&tx_id);
    }

    /// Returns the number of transactions currently tracked as active.
    pub fn active_count(&self) -> usize {
        self.active.lock().expect("coordinator lock poisoned").len()
    }

    /// Returns IDs of transactions that have been open longer than `threshold`.
    pub fn stale_transactions(
        &self,
        threshold: std::time::Duration,
    ) -> Vec<(Uuid, Uuid)> {
        let map = self.active.lock().expect("coordinator lock poisoned");
        map.iter()
            .filter(|(_, r)| r.started_at.elapsed() > threshold)
            .map(|(id, r)| (*id, r.session_id))
            .collect()
    }
}

impl Default for TransactionCoordinator {
    fn default() -> Self {
        Self::new()
    }
}


    /// Returns the number of currently tracked transactions.
    pub fn active_count(&self) -> usize {
        self.active.lock().expect("coordinator lock poisoned").len()
    }
}

impl Default for TransactionCoordinator {
    fn default() -> Self {
        Self::new()
    }
}
