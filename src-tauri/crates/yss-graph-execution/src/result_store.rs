use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, RwLock};
use uuid::Uuid;

use crate::finalization::ReadyResult;
use crate::plan::PlanOutputRef;
pub use crate::result::{ResultId, StoredResult};
use crate::result::{ResultRetentionError, StoredResultSnapshot};
use crate::run_registry::RunId;

struct ResultEntry {
    snapshot: StoredResultSnapshot,
    current: bool,
    leases: BTreeSet<Uuid>,
}

struct ResultLease {
    result_id: ResultId,
    owner: Box<str>,
    handoff: Option<Box<str>>,
}

#[derive(Default)]
struct ResultStoreRegistry {
    values: BTreeMap<ResultId, ResultEntry>,
    graph_inputs: BTreeMap<String, [u8; 32]>,
    outputs: BTreeMap<PlanOutputRef, (RunId, Option<ResultId>)>,
    leases: BTreeMap<Uuid, ResultLease>,
    owner_leases: BTreeMap<Box<str>, BTreeSet<Uuid>>,
    closed_owners: BTreeSet<Box<str>>,
}

impl ResultStoreRegistry {
    fn collect(&mut self, id: ResultId) {
        if self
            .values
            .get(&id)
            .is_some_and(|entry| !entry.current && entry.leases.is_empty())
        {
            self.values.remove(&id);
        }
    }

    fn detach(&mut self, id: ResultId) {
        if let Some(entry) = self.values.get_mut(&id) {
            entry.current = false;
            self.collect(id);
        }
    }

    fn unindex_owner(&mut self, owner: &str, lease_id: Uuid) {
        if let Some(leases) = self.owner_leases.get_mut(owner) {
            leases.remove(&lease_id);
            if leases.is_empty() {
                self.owner_leases.remove(owner);
            }
        }
    }

    fn remove_lease(&mut self, lease_id: Uuid) {
        let Some(lease) = self.leases.remove(&lease_id) else {
            return;
        };
        self.unindex_owner(&lease.owner, lease_id);
        if let Some(target) = lease.handoff {
            self.unindex_owner(&target, lease_id);
        }
        if let Some(entry) = self.values.get_mut(&lease.result_id) {
            entry.leases.remove(&lease_id);
            self.collect(lease.result_id);
        }
    }

    fn detach_graph(&mut self, graph: &str) {
        let mut detached = Vec::new();
        self.outputs.retain(|output, (_, result)| {
            if output.graph().as_str() != graph {
                return true;
            }
            if let Some(result) = result {
                detached.push(*result);
            }
            false
        });
        for id in detached {
            self.detach(id);
        }
    }
}

#[derive(Default)]
pub struct ResultStore {
    registry: RwLock<ResultStoreRegistry>,
}

impl ResultStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, result: ResultId) -> Option<StoredResultSnapshot> {
        self.registry
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .values
            .get(&result)
            .map(|entry| entry.snapshot.clone())
    }

    pub(crate) fn query_graph_results(
        &self,
        graph: &str,
        limit: usize,
    ) -> Vec<StoredResultSnapshot> {
        let registry = self
            .registry
            .read()
            .unwrap_or_else(|error| error.into_inner());
        // Retained reports are snapshots, never candidates for the current-output search.
        registry
            .outputs
            .iter()
            .filter(|(output, _)| output.graph().as_str() == graph)
            .filter_map(|(_, (_, id))| id.and_then(|id| registry.values.get(&id)))
            .take(limit)
            .map(|entry| entry.snapshot.clone())
            .collect()
    }

    pub fn retain(
        &self,
        id: ResultId,
        lease_id: Uuid,
        owner: &str,
        handoff: Option<&str>,
    ) -> Result<StoredResultSnapshot, ResultRetentionError> {
        let mut registry = self
            .registry
            .write()
            .unwrap_or_else(|error| error.into_inner());
        if registry.closed_owners.contains(owner)
            || handoff.is_some_and(|target| registry.closed_owners.contains(target))
        {
            return Err(ResultRetentionError::OwnerClosed);
        }
        let snapshot = registry
            .values
            .get(&id)
            .ok_or(ResultRetentionError::Unavailable)?
            .snapshot
            .clone();
        if let Some(existing) = registry.leases.get(&lease_id) {
            return if existing.result_id == id
                && existing.owner.as_ref() == owner
                && existing.handoff.as_deref() == handoff
            {
                Ok(snapshot)
            } else {
                Err(ResultRetentionError::LeaseConflict)
            };
        }
        registry
            .values
            .get_mut(&id)
            .unwrap()
            .leases
            .insert(lease_id);
        registry
            .owner_leases
            .entry(owner.into())
            .or_default()
            .insert(lease_id);
        if let Some(target) = handoff {
            registry
                .owner_leases
                .entry(target.into())
                .or_default()
                .insert(lease_id);
        }
        registry.leases.insert(
            lease_id,
            ResultLease {
                result_id: id,
                owner: owner.into(),
                handoff: handoff.map(Into::into),
            },
        );
        Ok(snapshot)
    }

    pub fn claim(
        &self,
        lease_id: Uuid,
        owner: &str,
    ) -> Result<StoredResultSnapshot, ResultRetentionError> {
        let mut registry = self
            .registry
            .write()
            .unwrap_or_else(|error| error.into_inner());
        let lease = registry
            .leases
            .get(&lease_id)
            .ok_or(ResultRetentionError::Unavailable)?;
        let id = lease.result_id;
        if lease.owner.as_ref() == owner && lease.handoff.is_none() {
            return registry
                .values
                .get(&id)
                .map(|entry| entry.snapshot.clone())
                .ok_or(ResultRetentionError::Unavailable);
        }
        if lease.handoff.as_deref() != Some(owner) {
            return Err(ResultRetentionError::WrongOwner);
        }
        let old_owner = lease.owner.clone();
        registry.unindex_owner(&old_owner, lease_id);
        let lease = registry.leases.get_mut(&lease_id).unwrap();
        lease.owner = owner.into();
        lease.handoff = None;
        registry
            .owner_leases
            .entry(owner.into())
            .or_default()
            .insert(lease_id);
        Ok(registry.values[&id].snapshot.clone())
    }

    pub fn release(&self, lease_id: Uuid, owner: &str) -> Result<(), ResultRetentionError> {
        let mut registry = self
            .registry
            .write()
            .unwrap_or_else(|error| error.into_inner());
        if let Some(lease) = registry.leases.get(&lease_id) {
            if lease.owner.as_ref() != owner {
                return Err(ResultRetentionError::WrongOwner);
            }
            registry.remove_lease(lease_id);
        }
        Ok(())
    }

    pub fn reconcile(&self, owner: &str, active: &BTreeSet<Uuid>) {
        let mut registry = self
            .registry
            .write()
            .unwrap_or_else(|error| error.into_inner());
        let removed = registry
            .owner_leases
            .get(owner)
            .into_iter()
            .flatten()
            .copied()
            .filter(|id| {
                !active.contains(id)
                    && registry.leases.get(id).is_some_and(|lease| {
                        lease.owner.as_ref() == owner && lease.handoff.is_none()
                    })
            })
            .collect::<Vec<_>>();
        for id in removed {
            registry.remove_lease(id);
        }
    }

    pub fn close_owner(&self, owner: &str) {
        let mut registry = self
            .registry
            .write()
            .unwrap_or_else(|error| error.into_inner());
        registry.closed_owners.insert(owner.into());
        let ids = registry
            .owner_leases
            .get(owner)
            .cloned()
            .unwrap_or_default();
        // Includes unclaimed handoffs: destroying either endpoint must not leak a lease.
        for id in ids {
            registry.remove_lease(id);
        }
    }

    pub(crate) fn begin_run(&self, run: RunId, outputs: &[PlanOutputRef]) -> bool {
        let mut registry = self
            .registry
            .write()
            .unwrap_or_else(|error| error.into_inner());
        if outputs.iter().any(|output| {
            registry
                .outputs
                .get(output)
                .is_some_and(|(owner, _)| *owner > run)
        }) {
            return false;
        }
        for output in outputs {
            if let Some((_, Some(previous))) = registry.outputs.insert(output.clone(), (run, None))
            {
                registry.detach(previous);
            }
        }
        true
    }

    /// Publishing is atomic; retained snapshots do not authorize obsolete runs to publish.
    pub(crate) fn publish(&self, results: &[ReadyResult]) -> bool {
        let mut registry = self
            .registry
            .write()
            .unwrap_or_else(|error| error.into_inner());
        if results.iter().any(|result| {
            registry.outputs.get(result.output()).map(|(run, _)| *run)
                != Some(result.pin().provenance().run_id())
                || registry
                    .values
                    .get(&result.result_id())
                    .is_some_and(|entry| {
                        entry.snapshot.output() != result.output()
                            || entry.snapshot.provenance() != result.pin().provenance()
                    })
        }) {
            return false;
        }
        for result in results {
            let id = result.result_id();
            let provenance = result.pin().provenance();
            if let Some((_, Some(previous))) = registry
                .outputs
                .insert(result.output().clone(), (provenance.run_id(), Some(id)))
                && previous != id
            {
                registry.detach(previous);
            }
            registry
                .values
                .entry(id)
                .or_insert_with(|| ResultEntry {
                    snapshot: StoredResultSnapshot::new(
                        Arc::new(result.value().clone()),
                        result.output().clone(),
                        provenance.clone(),
                    ),
                    current: true,
                    leases: BTreeSet::new(),
                })
                .current = true;
        }
        true
    }

    pub(crate) fn query_pin_result(&self, output: &PlanOutputRef) -> Option<StoredResultSnapshot> {
        let registry = self
            .registry
            .read()
            .unwrap_or_else(|error| error.into_inner());
        let (_, id) = registry.outputs.get(output)?;
        registry
            .values
            .get(&(*id)?)
            .map(|entry| entry.snapshot.clone())
    }

    pub(crate) fn observe_graph_inputs(&self, graph: &str, inputs: [u8; 32]) {
        let mut registry = self
            .registry
            .write()
            .unwrap_or_else(|error| error.into_inner());
        if registry
            .graph_inputs
            .insert(graph.to_owned(), inputs)
            .is_some_and(|previous| previous != inputs)
        {
            registry.detach_graph(graph);
        }
    }

    pub(crate) fn invalidate_graph(&self, graph: &str) {
        let mut registry = self
            .registry
            .write()
            .unwrap_or_else(|error| error.into_inner());
        registry.graph_inputs.remove(graph);
        registry.detach_graph(graph);
    }

    pub fn clear(&self) {
        *self
            .registry
            .write()
            .unwrap_or_else(|error| error.into_inner()) = ResultStoreRegistry::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::finalization::ReadyPinResult;
    use crate::plan::{PlanGraphId, PlanPortAddress, ResultCategory};
    use crate::result::ResultProvenance;

    fn output() -> PlanOutputRef {
        PlanOutputRef::new(
            PlanGraphId::from_existing("events/main.yssbi-event".into()),
            PlanPortAddress::from_existing("node:result".into()),
        )
    }

    fn result(id: u64, run: RunId) -> ReadyResult {
        let id = ResultId::from_existing(id);
        ReadyResult::from_scheduler(
            id,
            StoredResult::Scalar(id.get() as f64),
            ResultCategory::Value,
            ReadyPinResult::new(
                output(),
                ResultProvenance::produced(
                    crate::identity::ExecutionSessionId::new(Uuid::nil()),
                    id,
                    run,
                    10,
                ),
            ),
        )
    }

    #[test]
    fn retained_snapshots_survive_output_changes_until_the_last_lease_is_released() {
        let store = ResultStore::new();
        let first = RunId::from_existing(1);
        store.begin_run(first, &[output()]);
        assert!(store.publish(&[result(1, first)]));
        let id = ResultId::from_existing(1);
        let weak = Arc::downgrade(store.get(id).unwrap().value());
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        store.retain(id, a, "main", None).unwrap();
        store.retain(id, a, "main", None).unwrap();
        store.retain(id, b, "report-window", None).unwrap();
        assert_eq!(store.registry.read().unwrap().values[&id].leases.len(), 2);
        store.invalidate_graph("events/main.yssbi-event");
        assert!(store.query_pin_result(&output()).is_none());
        assert!(
            store
                .query_graph_results("events/main.yssbi-event", 10)
                .is_empty()
        );
        assert!(store.get(id).is_some());
        let next = RunId::from_existing(2);
        store.begin_run(next, &[output()]);
        assert!(store.publish(&[result(2, next)]));
        assert_eq!(
            store
                .query_graph_results("events/main.yssbi-event", 10)
                .len(),
            1
        );
        assert_eq!(
            store
                .query_pin_result(&output())
                .unwrap()
                .provenance()
                .result_id(),
            ResultId::from_existing(2)
        );
        store.release(a, "main").unwrap();
        store.release(a, "main").unwrap();
        assert!(store.get(id).is_some());
        assert!(matches!(
            store.release(b, "main"),
            Err(ResultRetentionError::WrongOwner)
        ));
        store.release(b, "report-window").unwrap();
        assert!(store.get(id).is_none());
        assert!(weak.upgrade().is_none());
        assert!(store.get(ResultId::from_existing(2)).is_some());
    }

    #[test]
    fn window_handoffs_and_owner_reconciliation_do_not_leak_or_drop_claimed_results() {
        let store = ResultStore::new();
        let run = RunId::from_existing(1);
        store.begin_run(run, &[output()]);
        store.publish(&[result(1, run)]);
        let id = ResultId::from_existing(1);
        let lease = Uuid::new_v4();
        store.retain(id, lease, "main", Some("plot")).unwrap();
        store.invalidate_graph("events/main.yssbi-event");
        store.reconcile("main", &BTreeSet::new());
        assert!(store.get(id).is_some());
        assert!(matches!(
            store.claim(lease, "other"),
            Err(ResultRetentionError::WrongOwner)
        ));
        store.claim(lease, "plot").unwrap();
        store.claim(lease, "plot").unwrap();
        store.close_owner("main");
        assert!(store.get(id).is_some());
        assert!(matches!(
            store.retain(id, Uuid::new_v4(), "main", None),
            Err(ResultRetentionError::OwnerClosed)
        ));
        store.reconcile("plot", &BTreeSet::from([lease]));
        assert!(store.get(id).is_some());
        store.reconcile("plot", &BTreeSet::new());
        assert!(store.get(id).is_none());
        for closed in ["main", "plot"] {
            let store = ResultStore::new();
            let run = RunId::from_existing(2);
            store.begin_run(run, &[output()]);
            store.publish(&[result(2, run)]);
            let id = ResultId::from_existing(2);
            store
                .retain(id, Uuid::new_v4(), "main", Some("plot"))
                .unwrap();
            store.invalidate_graph("events/main.yssbi-event");
            store.close_owner(closed);
            assert!(store.get(id).is_none());
            assert!(store.registry.read().unwrap().leases.is_empty());
            assert!(store.registry.read().unwrap().owner_leases.is_empty());
        }
    }

    #[test]
    fn rerun_releases_previous_value_and_rejects_obsolete_publication() {
        let store = ResultStore::new();
        let first = RunId::from_existing(1);
        let second = RunId::from_existing(2);
        store.begin_run(first, &[output()]);
        assert!(store.publish(&[result(1, first)]));
        let previous = Arc::downgrade(store.get(ResultId::from_existing(1)).unwrap().value());
        store.begin_run(second, &[output()]);
        assert!(previous.upgrade().is_none());
        assert!(store.query_pin_result(&output()).is_none());
        assert!(!store.publish(&[result(2, first)]));
        assert!(store.publish(&[result(3, second)]));
        assert!(store.get(ResultId::from_existing(1)).is_none());
        assert_eq!(
            store
                .query_pin_result(&output())
                .unwrap()
                .provenance()
                .result_id(),
            ResultId::from_existing(3)
        );
        store.invalidate_graph("events/main.yssbi-event");
        assert!(store.query_pin_result(&output()).is_none());
        assert!(!store.publish(&[result(4, second)]));
    }
    #[test]
    fn superseded_batch_does_not_publish_any_output_or_remove_unrelated_results() {
        let store = ResultStore::new();
        let first = RunId::from_existing(1);
        let second = RunId::from_existing(2);
        let third = RunId::from_existing(3);
        let other = PlanOutputRef::new(
            output().graph().clone(),
            PlanPortAddress::from_existing("other:value".into()),
        );
        let other_result = |id, run| {
            ReadyResult::from_scheduler(
                ResultId::from_existing(id),
                StoredResult::Scalar(1.0),
                ResultCategory::Value,
                ReadyPinResult::new(
                    other.clone(),
                    ResultProvenance::produced(
                        crate::identity::ExecutionSessionId::new(Uuid::nil()),
                        ResultId::from_existing(id),
                        run,
                        10,
                    ),
                ),
            )
        };
        assert!(store.begin_run(first, &[output(), other.clone()]));
        assert!(store.publish(&[result(1, first), other_result(2, first)]));
        assert!(store.begin_run(second, &[output()]));
        assert_eq!(
            store
                .query_pin_result(&other)
                .unwrap()
                .provenance()
                .result_id(),
            ResultId::from_existing(2)
        );
        assert!(store.begin_run(second, std::slice::from_ref(&other)));
        assert!(store.begin_run(third, &[output()]));
        assert!(!store.publish(&[result(3, second), other_result(4, second)]));
        assert!(store.query_pin_result(&other).is_none());
        assert!(store.publish(&[result(5, third)]));
        assert!(!store.begin_run(second, &[output()]));
        assert!(store.get(ResultId::from_existing(5)).is_some());
        store.observe_graph_inputs(output().graph().as_str(), [1; 32]);
        store.observe_graph_inputs(output().graph().as_str(), [1; 32]);
        assert!(store.get(ResultId::from_existing(5)).is_some());
        store.observe_graph_inputs(output().graph().as_str(), [2; 32]);
        assert!(store.query_pin_result(&output()).is_none());
        assert!(!store.publish(&[result(6, third)]));
    }
}
