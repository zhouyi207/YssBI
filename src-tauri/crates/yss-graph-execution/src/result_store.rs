use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, RwLock};
use uuid::Uuid;

use crate::finalization::{ReadyResult, ResultObservationIntent};
use crate::plan::{PlanNodeId, PlanOutputRef};
use crate::result::{
    ConnectionCacheState, ConnectionResultState, GraphResultCacheState, GraphResultInputs,
    OutputResultInputs, ResultCacheState, ResultRetentionError, ResultRunBasis,
    StoredResultSnapshot,
};
pub use crate::result::{ResultId, StoredResult};
use crate::run_registry::RunId;

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
    result: Option<ResultId>,
    inputs: Option<OutputResultInputs>,
    source_results: BTreeMap<PlanOutputRef, ResultId>,
    valid: bool,
}

struct ObservedGraphInputs {
    revision: Uuid,
    inputs: GraphResultInputs,
}

#[derive(Default)]
struct ResultStoreRegistry {
    values: BTreeMap<ResultId, ResultEntry>,
    graph_inputs: BTreeMap<String, ObservedGraphInputs>,
    outputs: BTreeMap<PlanOutputRef, CachedOutput>,
    leases: BTreeMap<Uuid, ResultLease>,
    owner_leases: BTreeMap<Box<str>, BTreeSet<Uuid>>,
    closed_owners: BTreeSet<Box<str>>,
}

impl ResultStoreRegistry {
    fn collect(&mut self, id: ResultId) {
        if self
            .values
            .get(&id)
            .is_some_and(|entry| !entry.cached && entry.leases.is_empty())
        {
            self.values.remove(&id);
        }
    }

    fn detach(&mut self, id: ResultId) {
        if let Some(entry) = self.values.get_mut(&id) {
            entry.cached = false;
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
        self.outputs.retain(|output, cached| {
            if output.graph().as_str() != graph {
                return true;
            }
            if let Some(result) = cached.result {
                detached.push(result);
            }
            false
        });
        for id in detached {
            self.detach(id);
        }
    }

    fn observe_graph_inputs(&mut self, graph: &str, inputs: GraphResultInputs) {
        if self
            .graph_inputs
            .get(graph)
            .is_some_and(|current| current.inputs == inputs)
        {
            return;
        }
        let mut detached = Vec::new();
        self.outputs.retain(|output, cached| {
            if output.graph().as_str() != graph {
                return true;
            }
            // The cache may survive an edit; the old run's publication authority never does.
            cached.run = None;
            if inputs.outputs.contains_key(output) {
                return true;
            }
            if let Some(id) = cached.result {
                detached.push(id);
            }
            false
        });
        for id in detached {
            self.detach(id);
        }
        self.graph_inputs.insert(
            graph.to_owned(),
            ObservedGraphInputs {
                revision: Uuid::new_v4(),
                inputs,
            },
        );
        self.refresh_graph(graph);
    }

    fn refresh_graph(&mut self, graph: &str) {
        let observed = self.graph_inputs.get(graph);
        let mut pending = Vec::new();
        let mut remaining = BTreeMap::new();
        let mut dependents: BTreeMap<PlanOutputRef, Vec<PlanOutputRef>> = BTreeMap::new();
        for (output, cached) in &self.outputs {
            if output.graph().as_str() != graph || cached.result.is_none() {
                continue;
            }
            match (
                &cached.inputs,
                observed.and_then(|current| current.inputs.outputs.get(output)),
            ) {
                (Some(produced), Some(current)) if current.available && produced == current => {
                    let sources = produced.sources().collect::<BTreeSet<_>>();
                    if !sources.iter().all(|source| {
                        let actual = self.outputs.get(*source).and_then(|value| value.result);
                        actual.is_some() && actual == cached.source_results.get(*source).copied()
                    }) {
                        continue;
                    }
                    remaining.insert(output.clone(), sources.len());
                    for source in &sources {
                        dependents
                            .entry((*source).clone())
                            .or_default()
                            .push(output.clone());
                    }
                    if sources.is_empty() {
                        pending.push(output.clone());
                    }
                }
                // Standalone execution has no editor-provided semantic basis.
                (None, None) if observed.is_none() => pending.push(output.clone()),
                _ => {}
            }
        }
        for (output, cached) in &mut self.outputs {
            if output.graph().as_str() == graph {
                cached.valid = false;
            }
        }
        // One dependency pass also fails closed for cycles and missing upstream outputs.
        while let Some(output) = pending.pop() {
            self.outputs
                .get_mut(&output)
                .expect("indexed cached output")
                .valid = true;
            for dependent in dependents.get(&output).into_iter().flatten() {
                let count = remaining.get_mut(dependent).expect("indexed dependent");
                *count -= 1;
                if *count == 0 {
                    pending.push(dependent.clone());
                }
            }
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
            .filter(|(_, cached)| cached.valid)
            .filter_map(|(_, cached)| cached.result.and_then(|id| registry.values.get(&id)))
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

    pub(crate) fn begin_run(
        &self,
        run: RunId,
        outputs: &[PlanOutputRef],
        basis: Option<&ResultRunBasis>,
    ) -> bool {
        let mut registry = self
            .registry
            .write()
            .unwrap_or_else(|error| error.into_inner());
        if let Some(basis) = basis {
            if !registry
                .graph_inputs
                .get(basis.graph.as_ref())
                .is_some_and(|current| {
                    current.revision == basis.revision && current.inputs == basis.inputs
                })
                || outputs
                    .iter()
                    .any(|output| !basis.inputs.outputs.contains_key(output))
            {
                return false;
            }
        } else if outputs
            .iter()
            .any(|output| registry.graph_inputs.contains_key(output.graph().as_str()))
        {
            return false;
        }
        if outputs.iter().any(|output| {
            registry
                .outputs
                .get(output)
                .is_some_and(|cached| cached.run.is_some_and(|owner| owner > run))
        }) {
            return false;
        }
        for output in outputs {
            let previous = registry
                .outputs
                .insert(
                    output.clone(),
                    CachedOutput {
                        run: Some(run),
                        inputs: basis.and_then(|basis| basis.inputs.outputs.get(output).cloned()),
                        ..Default::default()
                    },
                )
                .and_then(|cached| cached.result);
            if let Some(previous) = previous {
                registry.detach(previous);
            }
        }
        for graph in outputs
            .iter()
            .map(|output| output.graph().as_str())
            .collect::<BTreeSet<_>>()
        {
            registry.refresh_graph(graph);
        }
        true
    }

    /// Publishing is atomic; retained snapshots do not authorize obsolete runs to publish.
    pub(crate) fn publish(
        &self,
        results: &[ReadyResult],
        observations: &[ResultObservationIntent],
    ) -> bool {
        let mut registry = self
            .registry
            .write()
            .unwrap_or_else(|error| error.into_inner());
        let published = results
            .iter()
            .map(|result| (result.output().clone(), result.result_id()))
            .collect::<BTreeMap<_, _>>();
        if results.iter().any(|result| {
            registry
                .outputs
                .get(result.output())
                .and_then(|cached| cached.run)
                != Some(result.pin().provenance().run_id())
                || registry.outputs.get(result.output()).is_some_and(|cached| {
                    cached.result.is_some()
                        || cached.inputs.as_ref().is_some_and(|inputs| {
                            inputs
                                .sources()
                                .any(|source| !published.contains_key(source))
                        })
                })
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
            let cached = registry
                .outputs
                .get(result.output())
                .expect("admitted output");
            let source_results = cached
                .inputs
                .as_ref()
                .into_iter()
                .flat_map(|inputs| inputs.sources())
                .filter_map(|source| {
                    published
                        .get(source)
                        .copied()
                        .map(|id| (source.clone(), id))
                })
                .collect();
            let cached = registry
                .outputs
                .get_mut(result.output())
                .expect("admitted output");
            let previous = cached.result.replace(id);
            cached.source_results = source_results;
            if let Some(previous) = previous.filter(|previous| *previous != id) {
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
                    cached: true,
                    leases: BTreeSet::new(),
                    observations: BTreeMap::new(),
                })
                .cached = true;
        }
        // The sealed observation identifies a consumer that actually ran. Keep its
        // binding with the shared result; it neither copies nor retains another value.
        for observation in observations {
            let Some(node) = observation.requester.node() else {
                continue;
            };
            let graph = observation.requester.graph().as_str();
            let Some(inputs) = registry
                .graph_inputs
                .get(graph)
                .and_then(|current| current.inputs.observers.get(node))
                .cloned()
            else {
                continue;
            };
            let Some(entry) = registry.values.get_mut(&observation.result_id) else {
                continue;
            };
            if entry.snapshot.output().graph().as_str() == graph
                && published.get(entry.snapshot.output()) == Some(&observation.result_id)
            {
                entry.observations.insert(node.clone(), inputs);
            }
        }
        for graph in results
            .iter()
            .map(|result| result.output().graph().as_str())
            .collect::<BTreeSet<_>>()
        {
            registry.refresh_graph(graph);
        }
        true
    }

    pub(crate) fn query_pin_result(&self, output: &PlanOutputRef) -> Option<StoredResultSnapshot> {
        let registry = self
            .registry
            .read()
            .unwrap_or_else(|error| error.into_inner());
        let cached = registry.outputs.get(output)?;
        if !cached.valid {
            return None;
        }
        registry
            .values
            .get(&cached.result?)
            .map(|entry| entry.snapshot.clone())
    }

    pub(crate) fn observe_graph_inputs(&self, graph: &str, inputs: GraphResultInputs) {
        let mut registry = self
            .registry
            .write()
            .unwrap_or_else(|error| error.into_inner());
        registry.observe_graph_inputs(graph, inputs);
    }

    pub(crate) fn capture_run_basis(
        &self,
        graph: &str,
        inputs: GraphResultInputs,
    ) -> Option<ResultRunBasis> {
        let mut registry = self
            .registry
            .write()
            .unwrap_or_else(|error| error.into_inner());
        if registry
            .graph_inputs
            .get(graph)
            .is_some_and(|current| current.inputs.semantic_input_hash != inputs.semantic_input_hash)
            || inputs
                .outputs
                .keys()
                .any(|output| output.graph().as_str() != graph)
        {
            return None;
        }
        registry.observe_graph_inputs(graph, inputs);
        let current = &registry.graph_inputs[graph];
        Some(ResultRunBasis {
            graph: graph.into(),
            revision: current.revision,
            inputs: current.inputs.clone(),
        })
    }

    pub(crate) fn query_cache_states(
        &self,
        graph: &str,
        semantic_input_hash: &[u8; 32],
    ) -> Option<GraphResultCacheState> {
        let registry = self
            .registry
            .read()
            .unwrap_or_else(|error| error.into_inner());
        let current = registry.graph_inputs.get(graph)?;
        if &current.inputs.semantic_input_hash != semantic_input_hash {
            return None;
        }
        let outputs = current
            .inputs
            .outputs
            .keys()
            .map(|output| {
                let state = match registry.outputs.get(output) {
                    Some(cached) if cached.valid => ResultCacheState::Valid {
                        result_id: cached.result.expect("valid result"),
                    },
                    Some(cached) if cached.result.is_some() => ResultCacheState::Stale,
                    _ => ResultCacheState::Missing,
                };
                (output.clone(), state)
            })
            .collect();
        let mut connections = BTreeMap::new();
        for (output, inputs) in &current.inputs.outputs {
            let cached = registry
                .outputs
                .get(output)
                .filter(|cached| cached.result.is_some());
            for (input, sources) in &inputs.bindings {
                for source in sources {
                    let state = match cached {
                        Some(cached) if cached.valid => ConnectionCacheState::Valid,
                        Some(cached)
                            if cached
                                .inputs
                                .as_ref()
                                .and_then(|inputs| inputs.bindings.get(input))
                                .is_some_and(|consumed| consumed.contains(source)) =>
                        {
                            ConnectionCacheState::Stale
                        }
                        _ => ConnectionCacheState::New,
                    };
                    // Any matching output proves the binding was consumed; node completeness is separate.
                    connections
                        .entry((source.clone(), input.clone()))
                        .and_modify(|current: &mut ConnectionCacheState| {
                            *current = (*current).max(state)
                        })
                        .or_insert(state);
                }
            }
        }
        for (node, inputs) in &current.inputs.observers {
            let observed = |source: &PlanOutputRef| {
                let cached = registry.outputs.get(source)?;
                let entry = registry.values.get(&cached.result?)?;
                Some((cached.valid, entry.observations.get(node)?))
            };
            let valid = inputs.available
                && inputs.sources().all(|source| {
                    observed(source).is_some_and(|(valid, consumed)| valid && consumed == inputs)
                });
            for (input, sources) in &inputs.bindings {
                for source in sources {
                    let state = if valid {
                        ConnectionCacheState::Valid
                    } else if observed(source).is_some_and(|(_, consumed)| {
                        consumed
                            .bindings
                            .get(input)
                            .is_some_and(|sources| sources.contains(source))
                    }) {
                        ConnectionCacheState::Stale
                    } else {
                        ConnectionCacheState::New
                    };
                    connections.insert((source.clone(), input.clone()), state);
                }
            }
        }
        Some(GraphResultCacheState {
            outputs,
            connections: connections
                .into_iter()
                .map(|((output, input), state)| ConnectionResultState {
                    output,
                    input,
                    state,
                })
                .collect(),
        })
    }

    pub(crate) fn resource_keys(&self, graph: &str) -> BTreeSet<Box<str>> {
        let registry = self
            .registry
            .read()
            .unwrap_or_else(|error| error.into_inner());
        registry
            .graph_inputs
            .get(graph)
            .into_iter()
            .flat_map(|current| {
                current
                    .inputs
                    .outputs
                    .values()
                    .chain(current.inputs.observers.values())
            })
            .flat_map(|output| output.resources.keys().cloned())
            .collect()
    }

    pub(crate) fn observe_resource_versions(
        &self,
        versions: &BTreeMap<Box<str>, Option<[u8; 32]>>,
    ) {
        let mut registry = self
            .registry
            .write()
            .unwrap_or_else(|error| error.into_inner());
        let mut changed = Vec::new();
        for (graph, current) in &mut registry.graph_inputs {
            let mut dirty = false;
            for output in current
                .inputs
                .outputs
                .values_mut()
                .chain(current.inputs.observers.values_mut())
            {
                for (resource, version) in &mut output.resources {
                    if let Some(actual) = versions.get(resource)
                        && actual != version
                    {
                        *version = *actual;
                        dirty = true;
                    }
                }
            }
            if dirty {
                current.revision = Uuid::new_v4();
                changed.push(graph.clone());
            }
        }
        for graph in changed {
            for (output, cached) in &mut registry.outputs {
                if output.graph().as_str() == graph {
                    cached.run = None;
                }
            }
            registry.refresh_graph(&graph);
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

    fn named_output(port: &str) -> PlanOutputRef {
        PlanOutputRef::new(
            output().graph().clone(),
            PlanPortAddress::from_existing(port.into()),
        )
    }

    fn cache_inputs(hash: u8, nodes: &[(&str, u8, &[&str])]) -> GraphResultInputs {
        GraphResultInputs {
            semantic_input_hash: [hash; 32],
            observers: BTreeMap::new(),
            outputs: nodes
                .iter()
                .map(|(port, fingerprint, sources)| {
                    (
                        named_output(port),
                        OutputResultInputs {
                            fingerprint: [*fingerprint; 32],
                            bindings: if sources.is_empty() {
                                BTreeMap::new()
                            } else {
                                BTreeMap::from([(
                                    PlanPortAddress::from_existing(format!("{port}:input").into()),
                                    sources.iter().map(|port| named_output(port)).collect(),
                                )])
                            },
                            resources: BTreeMap::new(),
                            available: true,
                        },
                    )
                })
                .collect(),
        }
    }

    fn cached_result(id: u64, run: RunId, output: PlanOutputRef) -> ReadyResult {
        let id = ResultId::from_existing(id);
        ReadyResult::from_scheduler(
            id,
            StoredResult::new(crate::value::RuntimeValue::Integer(id.get() as i64)),
            ResultCategory::Value,
            ReadyPinResult::new(
                output,
                ResultProvenance::produced(
                    crate::identity::ExecutionSessionId::new(Uuid::nil()),
                    id,
                    run,
                    10,
                ),
            ),
        )
    }

    fn publish_cached_graph(store: &ResultStore, inputs: &GraphResultInputs) {
        let graph = output();
        let basis = store
            .capture_run_basis(graph.graph().as_str(), inputs.clone())
            .unwrap();
        let outputs = inputs.outputs.keys().cloned().collect::<Vec<_>>();
        let run = RunId::from_existing(1);
        assert!(store.begin_run(run, &outputs, Some(&basis)));
        assert!(
            store.publish(
                &outputs
                    .into_iter()
                    .enumerate()
                    .map(|(index, output)| { cached_result(index as u64 + 1, run, output) })
                    .collect::<Vec<_>>(),
                &[],
            )
        );
    }

    #[test]
    fn observed_input_branches_require_consumption_and_follow_the_shared_result_lifetime() {
        use crate::plan::PlanSourceIdentity;

        let store = ResultStore::new();
        let output = named_output("species");
        let graph = output.graph().as_str();
        let node = |name: &str| PlanNodeId::from_existing(name.into());
        let input = |name: &str| PlanPortAddress::from_existing(format!("{name}:data").into());
        let observed_inputs = |name: &str| OutputResultInputs {
            fingerprint: [9; 32],
            bindings: BTreeMap::from([(input(name), Box::new([output.clone()]) as Box<[_]>)]),
            resources: BTreeMap::new(),
            available: true,
        };
        let mut original = cache_inputs(1, &[("species", 1, &[])]);
        original.observers = ["b", "c", "d"]
            .into_iter()
            .map(|name| (node(name), observed_inputs(name)))
            .collect();
        let observations = ["b", "c", "d"]
            .into_iter()
            .map(|name| ResultObservationIntent {
                result_id: ResultId::from_existing(1),
                requester: PlanSourceIdentity::new(output.graph().clone(), Some(node(name)), None),
            })
            .collect::<Vec<_>>();
        let basis = store.capture_run_basis(graph, original.clone()).unwrap();
        let run = RunId::from_existing(1);
        assert!(store.begin_run(run, std::slice::from_ref(&output), Some(&basis)));
        assert!(store.publish(&[cached_result(1, run, output.clone())], &observations));
        let states = store.query_cache_states(graph, &[1; 32]).unwrap();
        assert_eq!(states.connections.len(), 3);
        assert!(
            states
                .connections
                .iter()
                .all(|edge| edge.state == ConnectionCacheState::Valid)
        );
        assert_eq!(store.query_graph_results(graph, 10).len(), 1);

        let mut edited = original.clone();
        edited.semantic_input_hash = [2; 32];
        edited
            .observers
            .get_mut(&node("b"))
            .unwrap()
            .bindings
            .clear();
        edited.observers.get_mut(&node("b")).unwrap().available = false;
        edited.observers.insert(node("new"), observed_inputs("new"));
        store.observe_graph_inputs(graph, edited.clone());
        let states = store.query_cache_states(graph, &[2; 32]).unwrap();
        for edge in &states.connections {
            assert_eq!(
                edge.state,
                if edge.input == input("new") {
                    ConnectionCacheState::New
                } else {
                    ConnectionCacheState::Valid
                }
            );
        }
        assert!(store.query_pin_result(&output).is_some());
        edited.semantic_input_hash = [3; 32];
        edited.observers.get_mut(&node("c")).unwrap().fingerprint = [8; 32];
        store.observe_graph_inputs(graph, edited);
        let states = store.query_cache_states(graph, &[3; 32]).unwrap();
        assert_eq!(
            states
                .connections
                .iter()
                .find(|edge| edge.input == input("c"))
                .unwrap()
                .state,
            ConnectionCacheState::Stale
        );
        assert_eq!(
            states
                .connections
                .iter()
                .find(|edge| edge.input == input("d"))
                .unwrap()
                .state,
            ConnectionCacheState::Valid
        );

        store.observe_graph_inputs(graph, original.clone());
        assert!(
            store
                .query_cache_states(graph, &[1; 32])
                .unwrap()
                .connections
                .iter()
                .all(|edge| edge.state == ConnectionCacheState::Valid)
        );
        let basis = store.capture_run_basis(graph, original).unwrap();
        let run = RunId::from_existing(2);
        assert!(store.begin_run(run, std::slice::from_ref(&output), Some(&basis)));
        assert!(store.get(ResultId::from_existing(1)).is_none());
        assert!(store.publish(&[cached_result(2, run, output.clone())], &[]));
        assert!(
            store
                .query_cache_states(graph, &[1; 32])
                .unwrap()
                .connections
                .iter()
                .all(|edge| edge.state == ConnectionCacheState::New)
        );
        store.invalidate_graph(graph);
        assert!(store.get(ResultId::from_existing(2)).is_none());
    }

    #[test]
    fn edits_revalidate_only_dependent_caches_and_undo_cannot_resurrect_deleted_outputs() {
        let store = ResultStore::new();
        let original = cache_inputs(
            1,
            &[
                ("a", 1, &[]),
                ("b", 2, &["a"]),
                ("c", 3, &["b"]),
                ("d", 4, &["a"]),
            ],
        );
        publish_cached_graph(&store, &original);
        let b = store.query_pin_result(&named_output("b")).unwrap();
        let weak = Arc::downgrade(b.value());
        let b_id = b.provenance().result_id();
        drop(b);
        let graph = output();
        let graph = graph.graph().as_str();
        let edited = cache_inputs(
            2,
            &[
                ("a", 1, &[]),
                ("b", 8, &[]),
                ("c", 3, &["b"]),
                ("d", 4, &["a"]),
                ("e", 5, &["a"]),
            ],
        );
        store.observe_graph_inputs(graph, edited.clone());
        let states = store.query_cache_states(graph, &[2; 32]).unwrap();
        for port in ["a", "d"] {
            assert!(matches!(
                states.outputs[&named_output(port)],
                ResultCacheState::Valid { .. }
            ));
            assert!(store.query_pin_result(&named_output(port)).is_some());
        }
        for port in ["b", "c"] {
            assert_eq!(states.outputs[&named_output(port)], ResultCacheState::Stale);
            assert!(store.query_pin_result(&named_output(port)).is_none());
        }
        assert_eq!(
            states.outputs[&named_output("e")],
            ResultCacheState::Missing
        );
        let connection = |state: &GraphResultCacheState, source: &str, input: &str| {
            state
                .connections
                .iter()
                .find(|connection| {
                    connection.output == named_output(source) && connection.input.as_str() == input
                })
                .unwrap()
                .state
        };
        assert_eq!(
            connection(&states, "a", "d:input"),
            ConnectionCacheState::Valid
        );
        assert_eq!(
            connection(&states, "b", "c:input"),
            ConnectionCacheState::Stale
        );
        assert_eq!(
            connection(&states, "a", "e:input"),
            ConnectionCacheState::New
        );
        let rewired = cache_inputs(
            4,
            &[
                ("a", 1, &[]),
                ("b", 8, &["d"]),
                ("c", 3, &["b"]),
                ("d", 4, &["a"]),
            ],
        );
        store.observe_graph_inputs(graph, rewired);
        let states = store.query_cache_states(graph, &[4; 32]).unwrap();
        assert_eq!(
            connection(&states, "d", "b:input"),
            ConnectionCacheState::New
        );
        assert_eq!(
            connection(&states, "b", "c:input"),
            ConnectionCacheState::Stale
        );
        assert!(store.get(b_id).is_some());
        assert!(store.query_cache_states(graph, &[1; 32]).is_none());
        store.observe_graph_inputs(graph, original.clone());
        let states = store.query_cache_states(graph, &[1; 32]).unwrap();
        assert!(
            states
                .connections
                .iter()
                .all(|connection| connection.state == ConnectionCacheState::Valid)
        );
        assert_eq!(
            store
                .query_pin_result(&named_output("b"))
                .unwrap()
                .provenance()
                .result_id(),
            b_id
        );
        assert!(store.query_pin_result(&named_output("c")).is_some());
        store.observe_graph_inputs(graph, edited);
        assert!(store.query_pin_result(&named_output("c")).is_none());
        let mut deleted = original.clone();
        deleted.semantic_input_hash = [3; 32];
        deleted.outputs.remove(&named_output("b"));
        store.observe_graph_inputs(graph, deleted);
        assert!(weak.upgrade().is_none());
        store.observe_graph_inputs(graph, original);
        assert!(store.query_pin_result(&named_output("b")).is_none());
        assert!(store.query_pin_result(&named_output("c")).is_none());
        assert!(store.query_pin_result(&named_output("d")).is_some());
    }

    #[test]
    fn rerun_versions_and_edit_epochs_prevent_obsolete_cache_or_run_restoration() {
        let store = ResultStore::new();
        let original = cache_inputs(1, &[("a", 1, &[]), ("b", 2, &["a"])]);
        publish_cached_graph(&store, &original);
        let graph_output = output();
        let graph = graph_output.graph().as_str();
        let obsolete = store.capture_run_basis(graph, original.clone()).unwrap();
        let edited = cache_inputs(2, &[("a", 1, &[]), ("b", 3, &[])]);
        store.observe_graph_inputs(graph, edited.clone());
        store.observe_graph_inputs(graph, original.clone());
        let outputs = [named_output("a")];
        assert!(!store.begin_run(RunId::from_existing(2), &outputs, Some(&obsolete)));
        let basis = store.capture_run_basis(graph, original.clone()).unwrap();
        let run = RunId::from_existing(3);
        assert!(store.begin_run(run, &outputs, Some(&basis)));
        assert!(store.query_pin_result(&named_output("b")).is_none());
        assert!(store.publish(&[cached_result(3, run, named_output("a"))], &[]));
        assert!(store.query_pin_result(&named_output("a")).is_some());
        assert!(store.query_pin_result(&named_output("b")).is_none());
        store.observe_graph_inputs(graph, edited.clone());
        store.observe_graph_inputs(graph, original.clone());
        assert!(store.query_pin_result(&named_output("b")).is_none());
        let run = RunId::from_existing(4);
        let basis = store.capture_run_basis(graph, original.clone()).unwrap();
        assert!(store.begin_run(run, &outputs, Some(&basis)));
        store.observe_graph_inputs(graph, edited);
        store.observe_graph_inputs(graph, original.clone());
        assert!(!store.publish(&[cached_result(4, run, named_output("a"))], &[]));
        assert!(store.query_pin_result(&named_output("a")).is_none());
        let resources = ResultStore::new();
        let mut original = original;
        original
            .outputs
            .get_mut(&named_output("a"))
            .unwrap()
            .resources
            .insert("database:source".into(), Some([1; 32]));
        publish_cached_graph(&resources, &original);
        let old_admission = resources
            .capture_run_basis(graph, original.clone())
            .unwrap();
        resources.observe_resource_versions(&BTreeMap::from([(
            "database:source".into(),
            Some([7; 32]),
        )]));
        assert!(resources.query_pin_result(&named_output("a")).is_none());
        assert!(resources.query_pin_result(&named_output("b")).is_none());
        assert!(!resources.begin_run(RunId::from_existing(2), &outputs, Some(&old_admission)));
        // Undo restores graph semantics, while the independent data revision stays current.
        original
            .outputs
            .get_mut(&named_output("a"))
            .unwrap()
            .resources
            .insert("database:source".into(), Some([7; 32]));
        resources.observe_graph_inputs(graph, original);
        assert!(resources.query_pin_result(&named_output("b")).is_none());
        assert!(resources.get(ResultId::from_existing(1)).is_some());
    }

    fn result(id: u64, run: RunId) -> ReadyResult {
        let id = ResultId::from_existing(id);
        ReadyResult::from_scheduler(
            id,
            StoredResult::new(crate::value::RuntimeValue::Decimal(id.get() as f64)),
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
        store.begin_run(first, &[output()], None);
        assert!(store.publish(&[result(1, first)], &[]));
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
        store.begin_run(next, &[output()], None);
        assert!(store.publish(&[result(2, next)], &[]));
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
        store.begin_run(run, &[output()], None);
        store.publish(&[result(1, run)], &[]);
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
            store.begin_run(run, &[output()], None);
            store.publish(&[result(2, run)], &[]);
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
        store.begin_run(first, &[output()], None);
        assert!(store.publish(&[result(1, first)], &[]));
        let previous = Arc::downgrade(store.get(ResultId::from_existing(1)).unwrap().value());
        store.begin_run(second, &[output()], None);
        assert!(previous.upgrade().is_none());
        assert!(store.query_pin_result(&output()).is_none());
        assert!(!store.publish(&[result(2, first)], &[]));
        assert!(store.publish(&[result(3, second)], &[]));
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
        assert!(!store.publish(&[result(4, second)], &[]));
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
                StoredResult::new(crate::value::RuntimeValue::Decimal(1.0)),
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
        assert!(store.begin_run(first, &[output(), other.clone()], None));
        assert!(store.publish(&[result(1, first), other_result(2, first)], &[]));
        assert!(store.begin_run(second, &[output()], None));
        assert_eq!(
            store
                .query_pin_result(&other)
                .unwrap()
                .provenance()
                .result_id(),
            ResultId::from_existing(2)
        );
        assert!(store.begin_run(second, std::slice::from_ref(&other), None));
        assert!(store.begin_run(third, &[output()], None));
        assert!(!store.publish(&[result(3, second), other_result(4, second)], &[]));
        assert!(store.query_pin_result(&other).is_none());
        assert!(store.publish(&[result(5, third)], &[]));
        assert!(!store.begin_run(second, &[output()], None));
        assert!(store.get(ResultId::from_existing(5)).is_some());
        store.observe_graph_inputs(
            output().graph().as_str(),
            cache_inputs(1, &[("node:result", 1, &[])]),
        );
        store.observe_graph_inputs(
            output().graph().as_str(),
            cache_inputs(1, &[("node:result", 1, &[])]),
        );
        assert!(store.get(ResultId::from_existing(5)).is_some());
        store.observe_graph_inputs(
            output().graph().as_str(),
            cache_inputs(2, &[("node:result", 2, &[])]),
        );
        assert!(store.query_pin_result(&output()).is_none());
        assert!(!store.publish(&[result(6, third)], &[]));
    }
}
