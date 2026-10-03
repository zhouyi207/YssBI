use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

use super::{ObservedGraphInputs, ResultEntry, ResultStore, ResultStoreRegistry};
use crate::plan::PlanOutputRef;
use crate::result::{GraphResultCacheState, GraphResultInputs, ResultRunBasis};

impl ResultStoreRegistry {
    fn detach_graph(&mut self, graph: &str, retired: &mut Vec<ResultEntry>) {
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
            self.detach(id, retired);
        }
    }

    fn observe_graph_inputs(
        &mut self,
        graph: &str,
        inputs: GraphResultInputs,
        retired: &mut Vec<ResultEntry>,
    ) {
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
            self.detach(id, retired);
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

    pub(super) fn refresh_graph(&mut self, graph: &str) {
        self.revision = self
            .revision
            .checked_add(1)
            .expect("result state revision exhausted");
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

impl ResultStore {
    pub(crate) fn observe_graph_inputs(
        &self,
        graph: &str,
        inputs: GraphResultInputs,
    ) -> GraphResultCacheState {
        let semantic_input_hash = inputs.semantic_input_hash;
        self.update(|registry, retired| {
            registry.observe_graph_inputs(graph, inputs, retired);
            registry
                .query_cache_states(graph, &semantic_input_hash)
                .expect("the result state is captured under the input publication lock")
        })
    }

    pub(crate) fn capture_run_basis(
        &self,
        graph: &str,
        inputs: GraphResultInputs,
    ) -> Option<ResultRunBasis> {
        self.update(|registry, retired| {
            if registry.graph_inputs.get(graph).is_some_and(|current| {
                current.inputs.semantic_input_hash != inputs.semantic_input_hash
            }) || inputs
                .outputs
                .keys()
                .any(|output| output.graph().as_str() != graph)
            {
                return None;
            }
            registry.observe_graph_inputs(graph, inputs, retired);
            let current = &registry.graph_inputs[graph];
            Some(ResultRunBasis {
                graph: graph.into(),
                revision: current.revision,
                inputs: current.inputs.clone(),
            })
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
        self.update(|registry, _retired| {
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
        })
    }

    pub(crate) fn invalidate_graph(&self, graph: &str) {
        self.update(|registry, retired| {
            registry.graph_inputs.remove(graph);
            registry.detach_graph(graph, retired);
        })
    }
}
