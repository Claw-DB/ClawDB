//! `TransactionLog`: append-only log of committed transaction records.

use uuid::Uuid;
use std::collections::VecDeque;
use std::sync::Mutex;

/// A single entry in the transaction log.
#[derive(Debug, Clone)]
pub struct TransactionLogEntry {
    pub tx_id: Uuid,
    pub session_id: Uuid,
    pub committed_at: i64,
    pub write_set: Vec<String>,
}

/// Append-only in-memory transaction log (production would persist to WAL).
pub struct TransactionLog {
    entries: Mutex<VecDeque<TransactionLogEntry>>,
    max_entries: usize,
}

impl TransactionLog {
    /// Creates a new `TransactionLog` with a rolling window of `max_entries`.
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Mutex::new(VecDeque::new()),
            max_entries,
        }
    }

    /// Appends a committed transaction entry to the log.
    pub fn append(&self, entry: TransactionLogEntry) {
        let mut log = self.entries.lock().expect("transaction log lock poisoned");
        if log.len() >= self.max_entries {
            log.pop_front();
        }
        log.push_back(entry);
    }

    /// Returns a snapshot of all current log entries.
    pub fn snapshot(&self) -> Vec<TransactionLogEntry> {
        self.entries
            .lock()
            .expect("transaction log lock poisoned")
            .iter()
            .cloned()
            .collect()
    }

    /// Returns the number of entries currently in the log.
    pub fn len(&self) -> usize {
        self.entries.lock().expect("transaction log lock poisoned").len()
    }

    /// Returns `true` if the log is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for TransactionLog {
    fn default() -> Self {
        Self::new(10_000)
    }
}
