use std::collections::{BTreeMap, BTreeSet};
use std::sync::RwLock;
use uuid::Uuid;

use crate::plan::{PlanNodeId, PlanOutputRef};
use crate::result::{GraphResultInputs, OutputResultInputs, StoredResultSnapshot};
pub use crate::result::{ResultId, StoredResult};
use crate::run_registry::RunId;

mod cache;
mod projection;
mod publication;
mod retention;

struct ResultEntry {
    snapshot: StoredResultSnapshot,
    cached: bool,
    leases: BTreeSet<Uuid>,
    observations: BTreeMap<PlanNodeId, OutputResultInputs>,
}

struct ResultLease {
    result_id: ResultId,
    owner: Box<str>,
    handoff: Option<Box<str>>,
}

#[derive(Default)]
struct CachedOutput {
    run: Option<RunId>,
    pending: Option<PendingOutput>,
    result: Option<ResultId>,
    inputs: Option<OutputResultInputs>,
    source_results: BTreeMap<PlanOutputRef, ResultId>,
    valid: bool,
}

/// Publication inputs for the current run, separate from the last successful value's basis.
struct PendingOutput {
    inputs: Option<OutputResultInputs>,
    source_results: BTreeMap<PlanOutputRef, ResultId>,
}

struct ObservedGraphInputs {
    revision: Uuid,
    inputs: GraphResultInputs,
}

#[derive(Default)]
struct ResultStoreRegistry {
    revision: u64,
    values: BTreeMap<ResultId, ResultEntry>,
    graph_inputs: BTreeMap<String, ObservedGraphInputs>,
    outputs: BTreeMap<PlanOutputRef, CachedOutput>,
    leases: BTreeMap<Uuid, ResultLease>,
    owner_leases: BTreeMap<Box<str>, BTreeSet<Uuid>>,
    closed_owners: BTreeSet<Box<str>>,
}

#[derive(Default)]
pub struct ResultStore {
    registry: RwLock<ResultStoreRegistry>,
}

impl ResultStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn revision(&self) -> u64 {
        self.registry
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .revision
    }

    pub fn get(&self, result: ResultId) -> Option<StoredResultSnapshot> {
        self.registry
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .values
            .get(&result)
            .map(|entry| entry.snapshot.clone())
    }

    fn update<T>(
        &self,
        apply: impl FnOnce(&mut ResultStoreRegistry, &mut Vec<ResultEntry>) -> T,
    ) -> T {
        // Declare reclamation before the guard so values and relation handles drop
        // after the registry unlocks, including when a mutation unwinds.
        let mut retired = Vec::new();
        let mut registry = self
            .registry
            .write()
            .unwrap_or_else(|error| error.into_inner());
        apply(&mut registry, &mut retired)
    }
}

#[cfg(test)]
mod tests;
