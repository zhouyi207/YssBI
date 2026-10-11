use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use super::{ResultEntry, ResultId, ResultStore, ResultStoreRegistry};
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
                let cached = registry.outputs.entry(output.clone()).or_default();
                let previous = cached.result.take();
                cached.run = Some(run);
                cached.inputs = basis.and_then(|basis| basis.inputs.outputs.get(output).cloned());
                cached.source_results = cached
                    .inputs
                    .as_ref()
                    .into_iter()
                    .flat_map(|inputs| inputs.sources())
                    .filter_map(|source| reused_inputs.get(source).map(|id| (source.clone(), *id)))
                    .collect();
                if let Some(previous) = previous {
                    registry.detach(previous, retired);
                }
            }
            for graph in outputs
                .iter()
                .map(|output| output.graph().as_str())
                .collect::<BTreeSet<_>>()
            {
                registry.refresh_graph(graph, retired);
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
        if published.len() != results.len() {
            return false;
        }
        self.update(|registry, retired| {
            if results
                .iter()
                .any(|result| !registry.accepts_result(result, &published))
                || observations
                    .iter()
                    .any(|observation| !registry.accepts_observation(observation, &published))
            {
                return false;
            }
            for result in results {
                registry.publish_result(result, &published);
            }
            for observation in observations {
                registry.record_observation(observation, &published);
            }
            for graph in results
                .iter()
                .map(|result| result.output().graph().as_str())
                .chain(
                    observations
                        .iter()
                        .map(|observation| observation.requester.graph().as_str()),
                )
                .collect::<BTreeSet<_>>()
            {
                registry.refresh_graph(graph, retired);
            }
            true
        })
    }
}

impl ResultStoreRegistry {
    fn accepts_observation(
        &self,
        observation: &ResultObservationIntent,
        published: &BTreeMap<PlanOutputRef, ResultId>,
    ) -> bool {
        let output = published
            .iter()
            .find_map(|(output, id)| (*id == observation.result_id).then_some(output))
            .or_else(|| {
                self.values
                    .get(&observation.result_id)
                    .map(|entry| entry.snapshot.output())
            });
        let Some(output) = output else {
            return false;
        };
        if output.graph() != observation.requester.graph() {
            return false;
        }
        let current = if let Some(id) = published.get(output) {
            *id == observation.result_id
        } else {
            self.outputs
                .get(output)
                .is_some_and(|cached| cached.valid && cached.result == Some(observation.result_id))
        };
        current
            && match &observation.input_basis {
                Some(basis) => {
                    basis.available
                        && basis.sources().any(|source| source == output)
                        && observation.requester.node().and_then(|node| {
                            self.graph_inputs
                                .get(output.graph().as_str())
                                .and_then(|graph| graph.inputs.observers.get(node))
                        }) == Some(basis)
                }
                None => published.get(output) == Some(&observation.result_id),
            }
    }

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
        cached.result = Some(id);
        cached.source_results = source_results;
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
            && (published.get(entry.snapshot.output()) == Some(&observation.result_id)
                || self
                    .outputs
                    .get(entry.snapshot.output())
                    .is_some_and(|cached| {
                        cached.valid && cached.result == Some(observation.result_id)
                    }))
        {
            entry.observations.insert(node.clone(), inputs);
        }
    }
}
