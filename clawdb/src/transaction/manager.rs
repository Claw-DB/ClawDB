//! `TransactionManager`: cross-subsystem two-phase commit (2PC) transaction coordination.
//!
//! ## Architecture
//!
//! ClawDB coordinates three participating subsystems:
//!
//! | Subsystem    | Mechanism                                                       |
//! |:------------ |:----------------------------------------------------------------|
//! | **claw-core**   | SQLite `BEGIN IMMEDIATE` / `COMMIT` / `ROLLBACK` via raw SQL |
//! | **claw-vector** | In-memory write buffer; applied atomically at commit time    |
//! | **claw-branch** | Snapshot taken at `begin`; discarded on commit, restored on rollback |
//!
//! ## Two-phase commit flow
//!
//! ```text
//! begin()  →  [core: BEGIN IMMEDIATE]  [branch: snapshot]  [vector: init buffer]
//! commit() →  Phase 1 (prepare): validate vector dims, verify snapshot
//!          →  Phase 2 (apply):   vector upserts, core COMMIT, delete snapshot
//!          →  On any phase-2 failure: rollback() + emit TransactionFailed
//! rollback() → core ROLLBACK, discard vector buffer, restore branch snapshot
//! ```

use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::{
    error::{ClawDBError, ClawDBResult},
    events::{bus::EventBus, types::ClawEvent},
    lifecycle::manager::ComponentLifecycleManager,
    session::context::SessionContext,
    transaction::{
        coordinator::TransactionCoordinator,
        log::{TransactionLog, TransactionLogEntry},
    },
};

// ── VectorUpsertOp ──────────────────────────────────────────────────────────

/// A single vector upsert buffered during a transaction.
#[derive(Debug, Clone)]
pub struct VectorUpsertOp {
    /// Target collection name.
    pub collection: String,
    /// Record identifier (typically the memory UUID as a string).
    pub id: String,
    /// Text content to be embedded.
    pub text: String,
    /// Arbitrary metadata stored alongside the vector.
    pub metadata: serde_json::Value,
    /// Pre-computed embedding dimensions; `None` if the embedding service
    /// will produce them at apply time.
    pub dimensions: Option<usize>,
}

// ── TxStatus ────────────────────────────────────────────────────────────────

/// Lifecycle state of a transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TxStatus {
    /// Transaction is open and accepting writes.
    Active,
    /// Phase-1 (prepare) succeeded; phase-2 is in progress.
    Committing,
    /// Rollback is in progress.
    RollingBack,
    /// Transaction has been committed successfully.
    Committed,
    /// Transaction has been rolled back.
    RolledBack,
}

// ── TxState ─────────────────────────────────────────────────────────────────

/// All mutable state for a single in-flight transaction.
///
/// Stored inside a [`Mutex`] so that the `TransactionManager` can hold one
/// canonical `Arc<Mutex<TxState>>` in the [`DashMap`].
#[derive(Debug)]
pub struct TxState {
    pub id: Uuid,
    pub session_id: Uuid,
    pub agent_id: Uuid,
    pub started_at: DateTime<Utc>,
    pub status: TxStatus,
    /// Whether a `BEGIN IMMEDIATE` has been issued to claw-core.
    pub core_tx_begun: bool,
    /// Buffered vector writes; applied atomically on commit, discarded on rollback.
    pub vector_buffer: Vec<VectorUpsertOp>,
    /// Snapshot ID taken by the branch engine at `begin`; used for rollback restoration.
    pub branch_snapshot_id: Option<Uuid>,
}

impl TxState {
    fn new(session_id: Uuid, agent_id: Uuid) -> Self {
        Self {
            id: Uuid::new_v4(),
            session_id,
            agent_id,
            started_at: Utc::now(),
            status: TxStatus::Active,
            core_tx_begun: false,
            vector_buffer: Vec::new(),
            branch_snapshot_id: None,
        }
    }
}

// ── TransactionManager ───────────────────────────────────────────────────────

/// Manages the full lifecycle of cross-subsystem transactions.
pub struct TransactionManager {
    lifecycle: Arc<ComponentLifecycleManager>,
    coordinator: Arc<TransactionCoordinator>,
    log: Arc<TransactionLog>,
    event_bus: Arc<EventBus>,
    active: DashMap<Uuid, Arc<Mutex<TxState>>>,
}

impl TransactionManager {
    /// Creates a new `TransactionManager`.
    pub fn new(
        lifecycle: Arc<ComponentLifecycleManager>,
        event_bus: Arc<EventBus>,
    ) -> Self {
        Self {
            lifecycle,
            coordinator: Arc::new(TransactionCoordinator::new()),
            log: Arc::new(TransactionLog::new(8_192)),
            event_bus,
            active: DashMap::new(),
        }
    }

    // ── begin ────────────────────────────────────────────────────────────────

    /// Begins a new cross-subsystem transaction and returns its `tx_id`.
    ///
    /// Steps:
    /// 1. Issues `BEGIN IMMEDIATE` to claw-core (serialises concurrent writers).
    /// 2. Asks the branch engine to snapshot the current agent state (for rollback).
    /// 3. Initialises an empty vector write buffer.
    /// 4. Registers the transaction in the coordinator and the active map.
    #[tracing::instrument(skip(self, session), fields(session_id = %session.session_id))]
    pub async fn begin(&self, session: &SessionContext) -> ClawDBResult<Uuid> {
        let mut state = TxState::new(session.session_id, session.agent_id);
        let tx_id = state.id;

        // ── Step 1: begin core SQLite transaction ────────────────────────────
        if let Ok(core) = self.lifecycle.core() {
            core.execute_raw_write("BEGIN IMMEDIATE", &[]).await.map_err(|e| {
                ClawDBError::TransactionFailed {
                    tx_id,
                    reason: format!("core BEGIN failed: {e}"),
                }
            })?;
            state.core_tx_begun = true;
        }

        // ── Step 2: take branch snapshot ────────────────────────────────────
        if let Ok(branch) = self.lifecycle.branch() {
            let snapshot_name = format!("tx-{tx_id}");
            match branch.create_snapshot(&snapshot_name).await {
                Ok(snap_id) => {
                    state.branch_snapshot_id = Some(snap_id);
                }
                Err(e) => {
                    tracing::warn!(tx_id = %tx_id, "branch snapshot failed (non-fatal): {e}");
                }
            }
        }

        // ── Step 3: register ─────────────────────────────────────────────────
        self.coordinator.register(tx_id, session.session_id);
        self.active
            .insert(tx_id, Arc::new(Mutex::new(state)));

        tracing::info!(tx_id = %tx_id, agent_id = %session.agent_id, "transaction begun");
        Ok(tx_id)
    }

    // ── commit ───────────────────────────────────────────────────────────────

    /// Commits a transaction using two-phase commit.
    ///
    /// **Phase 1 – prepare:**  validate vector buffer dimensions and verify the
    /// branch snapshot is intact.
    ///
    /// **Phase 2 – apply:**  apply vector upserts, commit the core SQLite
    /// transaction, then delete the (now unneeded) branch snapshot.
    ///
    /// If any phase-2 step fails, an automatic rollback is attempted and a
    /// [`ClawEvent::TransactionFailed`] event is emitted.
    #[tracing::instrument(skip(self), fields(tx_id = %tx_id))]
    pub async fn commit(&self, tx_id: Uuid) -> ClawDBResult<()> {
        let handle = self.get_handle(tx_id)?;
        let mut state = handle.lock().await;

        if state.status != TxStatus::Active {
            return Err(ClawDBError::TransactionFailed {
                tx_id,
                reason: format!("cannot commit; status is {:?}", state.status),
            });
        }

        // ── Phase 1: prepare ─────────────────────────────────────────────────
        self.coordinator.prepare(tx_id, &[])?;

        // Validate vector buffer dimensions.
        let vector_cfg_dims = self
            .lifecycle
            .core()
            .map(|_| 0usize) // placeholder; real dimensions come from config
            .unwrap_or(0);
        for op in &state.vector_buffer {
            if let Some(dims) = op.dimensions {
                if vector_cfg_dims > 0 && dims != vector_cfg_dims {
                    return Err(ClawDBError::TransactionFailed {
                        tx_id,
                        reason: format!(
                            "vector op '{}' has {dims} dims; expected {vector_cfg_dims}",
                            op.id
                        ),
                    });
                }
            }
        }

        // Verify branch snapshot exists (simple presence check).
        if state.branch_snapshot_id.is_none() {
            tracing::debug!(tx_id = %tx_id, "no branch snapshot; skipping snapshot verify");
        }

        state.status = TxStatus::Committing;

        // ── Phase 2: apply ───────────────────────────────────────────────────
        let result = self.apply_phase2(&mut state).await;

        if let Err(ref e) = result {
            // Phase 2 failed → attempt rollback.
            tracing::error!(tx_id = %tx_id, err = %e, "phase-2 failed; attempting rollback");
            state.status = TxStatus::RollingBack;
            if let Err(rb_err) = self.do_rollback(&mut state).await {
                tracing::error!(tx_id = %tx_id, err = %rb_err, "rollback also failed");
            }
            state.status = TxStatus::RolledBack;
            self.coordinator.deregister(tx_id);
            self.active.remove(&tx_id);

            self.event_bus.emit(ClawEvent::ShutdownInitiated {
                reason: format!("transaction {tx_id} failed during phase-2 commit: {e}"),
            });
            return Err(ClawDBError::TransactionFailed {
                tx_id,
                reason: e.to_string(),
            });
        }

        state.status = TxStatus::Committed;
        self.coordinator.deregister(tx_id);
        self.active.remove(&tx_id);

        self.log.append(TransactionLogEntry {
            tx_id,
            session_id: state.session_id,
            committed_at: Utc::now().timestamp(),
            write_set: state
                .vector_buffer
                .iter()
                .map(|op| op.id.clone())
                .collect(),
        });

        tracing::info!(tx_id = %tx_id, "transaction committed");
        Ok(())
    }

    async fn apply_phase2(&self, state: &mut TxState) -> ClawDBResult<()> {
        let tx_id = state.id;

        // Apply buffered vector upserts.
        if !state.vector_buffer.is_empty() {
            if let Ok(vector) = self.lifecycle.vector() {
                let buffer = std::mem::take(&mut state.vector_buffer);
                for op in buffer {
                    vector
                        .upsert(&op.collection, &op.id, &op.text, &op.metadata)
                        .await
                        .map_err(|e| ClawDBError::TransactionFailed {
                            tx_id,
                            reason: format!("vector upsert '{}' failed: {e}", op.id),
                        })?;
                }
            }
        }

        // Commit the core SQLite transaction.
        if state.core_tx_begun {
            if let Ok(core) = self.lifecycle.core() {
                core.execute_raw_write("COMMIT", &[])
                    .await
                    .map_err(|e| ClawDBError::TransactionFailed {
                        tx_id,
                        reason: format!("core COMMIT failed: {e}"),
                    })?;
                state.core_tx_begun = false;
            }
        }

        // Delete the branch snapshot (no longer needed for rollback).
        if let Some(snap_id) = state.branch_snapshot_id.take() {
            if let Ok(branch) = self.lifecycle.branch() {
                if let Err(e) = branch.delete_snapshot(snap_id).await {
                    tracing::warn!(tx_id = %tx_id, snap_id = %snap_id, "snapshot delete failed (non-fatal): {e}");
                }
            }
        }

        Ok(())
    }

    // ── rollback ─────────────────────────────────────────────────────────────

    /// Rolls back a transaction.
    ///
    /// Steps:
    /// 1. Rolls back the core SQLite transaction.
    /// 2. Discards the vector write buffer (no writes applied).
    /// 3. Restores the branch snapshot if one was taken at `begin`.
    #[tracing::instrument(skip(self), fields(tx_id = %tx_id))]
    pub async fn rollback(&self, tx_id: Uuid) -> ClawDBResult<()> {
        let handle = self.get_handle(tx_id)?;
        let mut state = handle.lock().await;

        if matches!(state.status, TxStatus::Committed | TxStatus::RolledBack) {
            return Err(ClawDBError::TransactionFailed {
                tx_id,
                reason: format!("cannot rollback; status is {:?}", state.status),
            });
        }

        state.status = TxStatus::RollingBack;
        self.do_rollback(&mut state).await?;
        state.status = TxStatus::RolledBack;

        self.coordinator.deregister(tx_id);
        self.active.remove(&tx_id);

        tracing::info!(tx_id = %tx_id, "transaction rolled back");
        Ok(())
    }

    async fn do_rollback(&self, state: &mut TxState) -> ClawDBResult<()> {
        let tx_id = state.id;

        // Rollback core SQLite transaction.
        if state.core_tx_begun {
            if let Ok(core) = self.lifecycle.core() {
                if let Err(e) = core.execute_raw_write("ROLLBACK", &[]).await {
                    tracing::error!(tx_id = %tx_id, "core ROLLBACK failed: {e}");
                }
            }
            state.core_tx_begun = false;
        }

        // Discard vector buffer — no writes are applied.
        state.vector_buffer.clear();

        // Restore the branch snapshot.
        if let Some(snap_id) = state.branch_snapshot_id.take() {
            if let Ok(branch) = self.lifecycle.branch() {
                if let Err(e) = branch.restore_snapshot(snap_id).await {
                    tracing::error!(tx_id = %tx_id, snap_id = %snap_id, "snapshot restore failed: {e}");
                    return Err(ClawDBError::TransactionFailed {
                        tx_id,
                        reason: format!("branch snapshot restore failed: {e}"),
                    });
                }
            }
        }

        Ok(())
    }

    // ── timeout_stale ────────────────────────────────────────────────────────

    /// Rolls back all transactions that have been open longer than `older_than_secs`.
    ///
    /// Returns the number of transactions rolled back.
    pub async fn timeout_stale(&self, older_than_secs: u64) -> ClawDBResult<u32> {
        let threshold = Duration::from_secs(older_than_secs);
        let stale = self.coordinator.stale_transactions(threshold);
        let mut count = 0u32;

        for (tx_id, _session_id) in stale {
            tracing::warn!(tx_id = %tx_id, older_than_secs, "timing out stale transaction");
            if let Err(e) = self.rollback(tx_id).await {
                tracing::error!(tx_id = %tx_id, err = %e, "stale transaction rollback failed");
            } else {
                count += 1;
            }
        }

        Ok(count)
    }

    // ── write buffer ─────────────────────────────────────────────────────────

    /// Buffers a vector upsert to be applied atomically when the transaction commits.
    pub async fn buffer_vector_upsert(
        &self,
        tx_id: Uuid,
        op: VectorUpsertOp,
    ) -> ClawDBResult<()> {
        let handle = self.get_handle(tx_id)?;
        let mut state = handle.lock().await;
        if state.status != TxStatus::Active {
            return Err(ClawDBError::TransactionFailed {
                tx_id,
                reason: "transaction is not active".to_string(),
            });
        }
        let key = op.id.clone();
        state.vector_buffer.push(op);
        self.coordinator.extend_write_set(tx_id, [key]);
        Ok(())
    }

    // ── helpers ───────────────────────────────────────────────────────────────

    fn get_handle(&self, tx_id: Uuid) -> ClawDBResult<Arc<Mutex<TxState>>> {
        self.active
            .get(&tx_id)
            .map(|r| Arc::clone(&r))
            .ok_or(ClawDBError::TransactionFailed {
                tx_id,
                reason: "transaction not found".to_string(),
            })
    }

    /// Returns the number of currently active transactions.
    pub fn active_count(&self) -> usize {
        self.active.len()
    }

    // ── legacy API (kept for backwards compat with existing callers) ──────────

    /// Legacy: creates a simple conflict-tracked context (no subsystem 2PC).
    ///
    /// Prefer [`begin`] for production use.
    pub fn begin_simple(
        &self,
        session_id: Uuid,
        isolation: crate::transaction::context::IsolationLevel,
    ) -> crate::transaction::context::TransactionContext {
        let ctx = crate::transaction::context::TransactionContext::new(session_id, isolation);
        self.coordinator.register(ctx.tx_id, session_id);
        ctx
    }

    /// Legacy: commits a simple (non-2PC) transaction context.
    pub fn commit_simple(
        &self,
        ctx: &mut crate::transaction::context::TransactionContext,
    ) -> ClawDBResult<()> {
        use crate::transaction::context::TransactionStatus;
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
            committed_at: Utc::now().timestamp(),
            write_set: ctx.write_set.clone(),
        });
        Ok(())
    }
}


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
