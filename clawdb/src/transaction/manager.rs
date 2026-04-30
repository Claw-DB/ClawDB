//! `TransactionManager`: high-level API for beginning, committing, and rolling back transactions.

use std::sync::Arc;
use uuid::Uuid;

use crate::{
    error::{ClawDBError, ClawDBResult},
    transaction::{
        context::{IsolationLevel, TransactionContext, TransactionStatus},
        coordinator::TransactionCoordinator,
        log::{TransactionLog, TransactionLogEntry},
    },
};

/// Coordinates the full transaction lifecycle.
pub struct TransactionManager {
    coordinator: Arc<TransactionCoordinator>,
    log: Arc<TransactionLog>,
}

impl TransactionManager {
    /// Creates a new `TransactionManager`.
    pub fn new() -> Self {
        Self {
            coordinator: Arc::new(TransactionCoordinator::new()),
            log: Arc::new(TransactionLog::default()),
        }
    }

    /// Begins a new transaction for `session_id` at the given isolation level.
    pub fn begin(
        &self,
        session_id: Uuid,
        isolation: IsolationLevel,
    ) -> TransactionContext {
        let ctx = TransactionContext::new(session_id, isolation);
        self.coordinator.register(ctx.tx_id, vec![]);
        ctx
    }

    /// Commits a transaction, checking for conflicts first.
    pub fn commit(&self, ctx: &mut TransactionContext) -> ClawDBResult<()> {
        if !ctx.is_active() {
            return Err(ClawDBError::TransactionFailed {
                tx_id: ctx.tx_id,
                reason: "transaction is not active".to_string(),
            });
        }

        self.coordinator.check_conflicts(ctx.tx_id, &ctx.write_set)?;

        ctx.status = TransactionStatus::Committed;
        self.coordinator.deregister(ctx.tx_id);

        self.log.append(TransactionLogEntry {
            tx_id: ctx.tx_id,
            session_id: ctx.session_id,
            committed_at: chrono::Utc::now().timestamp(),
            write_set: ctx.write_set.clone(),
        });

        Ok(())
    }

    /// Rolls back a transaction.
    pub fn rollback(&self, ctx: &mut TransactionContext) {
        ctx.status = TransactionStatus::RolledBack;
        self.coordinator.deregister(ctx.tx_id);
    }

    /// Returns the number of currently active transactions.
    pub fn active_count(&self) -> usize {
        self.coordinator.active_count()
    }

    /// Returns a reference to the transaction log.
    pub fn log(&self) -> &TransactionLog {
        &self.log
    }
}

impl Default for TransactionManager {
    fn default() -> Self {
        Self::new()
    }
}
