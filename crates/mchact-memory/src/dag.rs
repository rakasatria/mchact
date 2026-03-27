use std::collections::{HashSet, VecDeque};

use crate::{ObservationStore, Result};
use crate::types::Observation;

// ---------------------------------------------------------------------------
// DAG traversal
// ---------------------------------------------------------------------------

/// Perform a BFS traversal starting from `root_id`, following `source_ids`
/// (i.e. "parent" direction) on each observation.
///
/// Returns all reachable observations including the root, in BFS order.
/// A visited set prevents infinite loops if the source graph contains cycles.
pub async fn trace_reasoning(
    store: &dyn ObservationStore,
    root_id: i64,
) -> Result<Vec<Observation>> {
    let mut visited: HashSet<i64> = HashSet::new();
    let mut queue: VecDeque<i64> = VecDeque::new();
    let mut result: Vec<Observation> = Vec::new();

    queue.push_back(root_id);
    visited.insert(root_id);

    while let Some(current_id) = queue.pop_front() {
        let obs = match store.get_observation(current_id).await? {
            Some(o) => o,
            None => continue, // Dangling reference; skip gracefully.
        };

        // Enqueue parents (source_ids) not yet visited.
        for &parent_id in &obs.source_ids {
            if visited.insert(parent_id) {
                queue.push_back(parent_id);
            }
        }

        result.push(obs);
    }

    Ok(result)
}

// ---------------------------------------------------------------------------
// Unit tests (pure logic, no async driver needed)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    // Integration-level tests for trace_reasoning live in
    // tests/sqlite_integration.rs since they require a live store.
    // Here we only test helper logic if any is extracted.
}
