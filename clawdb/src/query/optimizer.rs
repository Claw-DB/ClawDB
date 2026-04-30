//! `QueryOptimizer`: rewrites and optimises `QueryPlan` instances before execution.

use crate::query::types::{QueryPlan, QueryStep};

/// Rewrites a `QueryPlan` to reduce redundant steps and increase parallelism.
pub struct QueryOptimizer;

impl QueryOptimizer {
    /// Creates a new `QueryOptimizer`.
    pub fn new() -> Self {
        Self
    }

    /// Applies all optimisation passes to `plan` in-place.
    pub fn optimise(&self, plan: &mut QueryPlan) {
        self.merge_adjacent_core_reads(&mut plan.steps);
        self.promote_parallel(&mut plan.steps);
    }

    /// Merges consecutive `CoreRead` steps into a single batched step.
    fn merge_adjacent_core_reads(&self, steps: &mut Vec<QueryStep>) {
        let mut i = 0;
        while i + 1 < steps.len() {
            if let (QueryStep::CoreRead { sql: a }, QueryStep::CoreRead { sql: b }) =
                (&steps[i], &steps[i + 1])
            {
                let merged = format!("{a}; {b}");
                steps[i] = QueryStep::CoreRead { sql: merged };
                steps.remove(i + 1);
            } else {
                i += 1;
            }
        }
    }

    /// Promotes independent adjacent non-guard steps into a `Parallel` wrapper.
    fn promote_parallel(&self, steps: &mut Vec<QueryStep>) {
        let mut i = 0;
        while i + 1 < steps.len() {
            let a_independent = Self::is_independent(&steps[i]);
            let b_independent = Self::is_independent(&steps[i + 1]);
            if a_independent && b_independent {
                let a = steps.remove(i);
                let b = steps.remove(i);
                steps.insert(i, QueryStep::Parallel(vec![a, b]));
            }
            i += 1;
        }
    }

    fn is_independent(step: &QueryStep) -> bool {
        matches!(
            step,
            QueryStep::VectorSearch { .. }
                | QueryStep::VectorUpsert { .. }
                | QueryStep::SyncPush
                | QueryStep::SyncPull
        )
    }
}

impl Default for QueryOptimizer {
    fn default() -> Self {
        Self::new()
    }
}
