//! `TransactionCoordinator`: detects write-set conflicts between concurrent transactions.

use uuid::Uuid;
use std::collections::HashMap;
use std::sync::Mutex;

use crate::error::{ClawDBError, ClawDBResult};

/// Tracks active transaction write sets for conflict detection.
pub struct TransactionCoordinator {
    active: Mutex<HashMap<Uuid, Vec<String>>>,
}

impl TransactionCoordinator {
    /// Creates a new `TransactionCoordinator`.
    pub fn new() -> Self {
        Self {
            active: Mutex::new(HashMap::new()),
        }
    }

    /// Registers a transaction's write set for conflict tracking.
    pub fn register(&self, tx_id: Uuid, write_set: Vec<String>) {
        self.active
            .lock()
            .expect("coordinator lock poisoned")
            .insert(tx_id, write_set);
    }

    /// Checks whether `tx_id`'s write set conflicts with any other active transaction.
    ///
    /// Returns `Ok(())` if no conflict, or `Err(TransactionConflict)` otherwise.
    pub fn check_conflicts(
        &self,
        tx_id: Uuid,
        write_set: &[String],
    ) -> ClawDBResult<()> {
        let active = self.active.lock().expect("coordinator lock poisoned");
        for (other_id, other_writes) in active.iter() {
            if *other_id == tx_id {
                continue;
            }
            let conflict = write_set.iter().any(|k| other_writes.contains(k));
            if conflict {
                return Err(ClawDBError::TransactionConflict {
                    tx_id,
                    conflicting_tx: *other_id,
                });
            }
        }
        Ok(())
    }

    /// Deregisters a completed transaction.
    pub fn deregister(&self, tx_id: Uuid) {
        self.active
            .lock()
            .expect("coordinator lock poisoned")
            .remove(&tx_id);
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
