//! `TransactionContext`: per-transaction state and isolation metadata.

use uuid::Uuid;
use std::time::Instant;

/// The isolation level of a transaction.
#[derive(Debug, Clone, PartialEq)]
pub enum IsolationLevel {
    /// Read committed: sees all committed writes at the time of each read.
    ReadCommitted,
    /// Snapshot isolation: sees a consistent snapshot taken at transaction start.
    Snapshot,
    /// Serialisable: prevents all anomalies at the cost of higher conflict rates.
    Serializable,
}

/// The current status of a transaction.
#[derive(Debug, Clone, PartialEq)]
pub enum TransactionStatus {
    Active,
    Committed,
    RolledBack,
    Conflicted,
}

/// Holds the mutable state of a single in-flight transaction.
#[derive(Debug)]
pub struct TransactionContext {
    pub tx_id: Uuid,
    pub session_id: Uuid,
    pub isolation: IsolationLevel,
    pub status: TransactionStatus,
    pub started_at: Instant,
    pub write_set: Vec<String>,
    pub read_set: Vec<String>,
}

impl TransactionContext {
    /// Creates a new active transaction context.
    pub fn new(session_id: Uuid, isolation: IsolationLevel) -> Self {
        Self {
            tx_id: Uuid::new_v4(),
            session_id,
            isolation,
            status: TransactionStatus::Active,
            started_at: Instant::now(),
            write_set: vec![],
            read_set: vec![],
        }
    }

    /// Returns `true` if the transaction is still active.
    pub fn is_active(&self) -> bool {
        self.status == TransactionStatus::Active
    }

    /// Records a key in the write set.
    pub fn record_write(&mut self, key: impl Into<String>) {
        self.write_set.push(key.into());
    }

    /// Records a key in the read set.
    pub fn record_read(&mut self, key: impl Into<String>) {
        self.read_set.push(key.into());
    }
}
