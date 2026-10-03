use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use super::{CachedOutput, ResultEntry, ResultId, ResultStore, ResultStoreRegistry};
use crate::finalization::{ReadyResult, ResultObservationIntent};
use crate::plan::PlanOutputRef;
use crate::result::{ResultRunBasis, StoredResultSnapshot};
use crate::run_registry::RunId;

impl ResultStore {
    pub(crate) fn begin_run(
        &self,
        run: RunId,
        outputs: &[PlanOutputRef],
        basis: Option<&ResultRunBasis>,
        reused_inputs: &BTreeMap<PlanOutputRef, ResultId>,
    ) -> bool {
        self.update(|registry, retired| {
            if reused_inputs.iter().any(|(output, id)| {
                outputs.contains(output)
                    || !registry
                        .outputs
                        .get(output)
                        .is_some_and(|cached| cached.valid && cached.result == Some(*id))
            }) {
                return false;
            }
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
                            inputs: basis
                                .and_then(|basis| basis.inputs.outputs.get(output).cloned()),
                            source_results: basis
                                .and_then(|basis| basis.inputs.outputs.get(output))
                                .into_iter()
                                .flat_map(|inputs| inputs.sources())
                                .filter_map(|source| {
                                    reused_inputs.get(source).map(|id| (source.clone(), *id))
                                })
                                .collect(),
                            ..Default::default()
                        },
                    )
                    .and_then(|cached| cached.result);
                if let Some(previous) = previous {
                    registry.detach(previous, retired);
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
        })
    }

    /// Publishing is atomic; retained snapshots do not authorize obsolete runs to publish.
    pub(crate) fn publish(
        &self,
        results: &[ReadyResult],
        observations: &[ResultObservationIntent],
    ) -> bool {
        let published = results
            .iter()
            .map(|result| (result.output().clone(), result.result_id()))
            .collect::<BTreeMap<_, _>>();
        self.update(|registry, retired| {
            if results
                .iter()
                .any(|result| !registry.accepts_result(result, &published))
            {
                return false;
            }
            for result in results {
                registry.publish_result(result, &published, retired);
            }
            for observation in observations {
                registry.record_observation(observation, &published);
            }
            for graph in results
                .iter()
                .map(|result| result.output().graph().as_str())
                .collect::<BTreeSet<_>>()
            {
                registry.refresh_graph(graph);
            }
            true
        })
    }
}

impl ResultStoreRegistry {
    fn accepts_result(
        &self,
        result: &ReadyResult,
        published: &BTreeMap<PlanOutputRef, ResultId>,
    ) -> bool {
        let Some(cached) = self.outputs.get(result.output()) else {
            return false;
        };
        if cached.run != Some(result.pin().provenance().run_id()) || cached.result.is_some() {
            return false;
        }
        if cached.inputs.as_ref().is_some_and(|inputs| {
            inputs.sources().any(|source| {
                !published.contains_key(source)
                    && !cached.source_results.get(source).is_some_and(|expected| {
                        self.outputs.get(source).is_some_and(|current| {
                            current.valid && current.result == Some(*expected)
                        })
                    })
            })
        }) {
            return false;
        }
        self.values.get(&result.result_id()).is_none_or(|entry| {
            entry.snapshot.output() == result.output()
                && entry.snapshot.provenance() == result.pin().provenance()
        })
    }

    fn publish_result(
        &mut self,
        result: &ReadyResult,
        published: &BTreeMap<PlanOutputRef, ResultId>,
        retired: &mut Vec<ResultEntry>,
    ) {
        let id = result.result_id();
        let cached = self
            .outputs
            .get_mut(result.output())
            .expect("admitted output");
        let source_results = cached
            .inputs
            .as_ref()
            .into_iter()
            .flat_map(|inputs| inputs.sources())
            .filter_map(|source| {
                published
                    .get(source)
                    .or_else(|| cached.source_results.get(source))
                    .copied()
                    .map(|id| (source.clone(), id))
            })
            .collect();
        let previous = cached.result.replace(id);
        cached.source_results = source_results;
        if let Some(previous) = previous.filter(|previous| *previous != id) {
            self.detach(previous, retired);
        }
        self.values
            .entry(id)
            .or_insert_with(|| ResultEntry {
                snapshot: StoredResultSnapshot::new(
                    Arc::clone(result.value()),
                    result.output().clone(),
                    result.pin().provenance().clone(),
                ),
                cached: true,
                leases: BTreeSet::new(),
                observations: BTreeMap::new(),
            })
            .cached = true;
    }

    fn record_observation(
        &mut self,
        observation: &ResultObservationIntent,
        published: &BTreeMap<PlanOutputRef, ResultId>,
    ) {
        // The sealed observation identifies a consumer that actually ran. Keep its
        // binding with the shared result; it neither copies nor retains another value.
        let Some(node) = observation.requester.node() else {
            return;
        };
        let graph = observation.requester.graph().as_str();
        let Some(inputs) = self
            .graph_inputs
            .get(graph)
            .and_then(|current| current.inputs.observers.get(node))
            .cloned()
        else {
            return;
        };
        let Some(entry) = self.values.get_mut(&observation.result_id) else {
            return;
        };
        if entry.snapshot.output().graph().as_str() == graph
            && published.get(entry.snapshot.output()) == Some(&observation.result_id)
        {
            entry.observations.insert(node.clone(), inputs);
        }
    }
}
