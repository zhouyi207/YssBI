use std::collections::{BTreeMap, BTreeSet};

use super::{ResultStore, ResultStoreRegistry};
use crate::plan::PlanOutputRef;
use crate::result::{
    ConnectionCacheState, ConnectionResultState, GraphResultCacheState, ResultCacheState,
    StoredResultSnapshot,
};

impl ResultStoreRegistry {
    pub(super) fn query_cache_states(
        &self,
        graph: &str,
        semantic_input_hash: &[u8; 32],
    ) -> Option<GraphResultCacheState> {
        let current = self.graph_inputs.get(graph)?;
        if &current.inputs.semantic_input_hash != semantic_input_hash {
            return None;
        }
        let outputs = current
            .inputs
            .outputs
            .keys()
            .map(|output| {
                let state = match self.outputs.get(output) {
                    Some(cached)
                        if cached
                            .result
                            .and_then(|id| self.values.get(&id))
                            .is_some_and(|entry| !entry.snapshot.value().is_evaluated()) =>
                    {
                        ResultCacheState::Missing
                    }
                    Some(cached) if cached.valid => ResultCacheState::Valid {
                        result_id: cached.result.expect("valid result"),
                    },
                    Some(cached) if cached.result.is_some() => ResultCacheState::Stale {
                        result_id: cached.result.expect("retained stale result"),
                    },
                    _ => ResultCacheState::Missing,
                };
                (output.clone(), state)
            })
            .collect();
        let mut connections = BTreeMap::new();
        for (output, inputs) in &current.inputs.outputs {
            let cached = self
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
                let cached = self.outputs.get(source)?;
                let entry = self.values.get(&cached.result?)?;
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
            revision: self.revision,
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
}

impl ResultStore {
    pub(crate) fn query_result_with_validity(
        &self,
        id: crate::result::ResultId,
    ) -> Option<crate::result::ResultReadSnapshot> {
        let registry = self
            .registry
            .read()
            .unwrap_or_else(|error| error.into_inner());
        registry.read_snapshot(id)
    }

    /// Without a run filter, return current pins including retained stale values.
    /// A run filter reads only still-owned immutable results, never a run archive.
    pub(crate) fn query_graph_result_entries(
        &self,
        graph: &str,
        run: Option<crate::run_registry::RunId>,
    ) -> Vec<crate::result::ResultReadSnapshot> {
        let registry = self
            .registry
            .read()
            .unwrap_or_else(|error| error.into_inner());
        if let Some(run) = run {
            registry
                .values
                .iter()
                .filter(|(_, entry)| {
                    entry.snapshot.output().graph().as_str() == graph
                        && entry.snapshot.provenance().run_id() == run
                })
                .filter_map(|(id, _)| registry.read_snapshot(*id))
                .collect()
        } else {
            registry
                .outputs
                .iter()
                .filter(|(output, _)| output.graph().as_str() == graph)
                .filter_map(|(_, cached)| registry.read_snapshot(cached.result?))
                .collect()
        }
    }

    pub(crate) fn schema_candidates(&self, graph: &str) -> Vec<StoredResultSnapshot> {
        let registry = self
            .registry
            .read()
            .unwrap_or_else(|error| error.into_inner());
        registry
            .outputs
            .iter()
            .filter(|(output, _)| output.graph().as_str() == graph)
            .filter_map(|(_, cached)| cached.result.and_then(|id| registry.values.get(&id)))
            .filter(|entry| entry.snapshot.value().is_evaluated())
            .map(|entry| entry.snapshot.clone())
            .collect()
    }

    pub(crate) fn matching_schema_results(
        &self,
        graph: &str,
        inputs: &crate::result::GraphResultInputs,
    ) -> BTreeMap<PlanOutputRef, crate::result::ResultId> {
        let registry = self
            .registry
            .read()
            .unwrap_or_else(|error| error.into_inner());
        // Last-success Schema remains useful during a same-input rerun, while the
        // execution cache remains stale. Edits and changed source IDs still invalidate it.
        registry
            .matching_outputs(graph, Some(inputs), true)
            .into_iter()
            .filter_map(|output| Some((output.clone(), registry.outputs.get(&output)?.result?)))
            .collect()
    }

    pub(crate) fn retained_boundaries(&self, graph: &str) -> BTreeSet<PlanOutputRef> {
        let registry = self
            .registry
            .read()
            .unwrap_or_else(|error| error.into_inner());
        registry
            .outputs
            .iter()
            .filter(|(output, _)| output.graph().as_str() == graph)
            .filter(|(_, cached)| {
                cached
                    .result
                    .and_then(|id| registry.values.get(&id))
                    .is_some_and(|entry| entry.snapshot.value().is_evaluated())
            })
            .map(|(output, _)| output.clone())
            .collect()
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
            .filter(|entry| entry.snapshot.value().is_evaluated())
            .take(limit)
            .map(|entry| entry.snapshot.clone())
            .collect()
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
            .filter(|entry| entry.snapshot.value().is_evaluated())
            .map(|entry| entry.snapshot.clone())
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
        registry.query_cache_states(graph, semantic_input_hash)
    }
}

impl ResultStoreRegistry {
    fn read_snapshot(
        &self,
        id: crate::result::ResultId,
    ) -> Option<crate::result::ResultReadSnapshot> {
        use crate::result::{ResultReadSnapshot, ResultValidity};
        let entry = self.values.get(&id)?;
        if !entry.snapshot.value().is_evaluated() {
            return None;
        }
        let validity = match self.outputs.get(entry.snapshot.output()) {
            Some(cached) if cached.result == Some(id) && cached.valid => {
                ResultValidity::CurrentValid
            }
            Some(cached) if cached.result == Some(id) => ResultValidity::CurrentStale,
            _ => ResultValidity::Retained,
        };
        Some(ResultReadSnapshot {
            result: entry.snapshot.clone(),
            validity,
        })
    }
}
