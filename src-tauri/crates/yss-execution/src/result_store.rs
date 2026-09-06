use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

use crate::finalization::ReadyResult;
use crate::plan::PlanOutputRef;
use crate::result::StoredResultSnapshot;
pub use crate::result::{ResultId, StoredResult};
use crate::run_registry::RunId;

#[derive(Default)]
struct ResultStoreRegistry {
    values: BTreeMap<ResultId, StoredResultSnapshot>,
    graph_inputs: BTreeMap<String, [u8; 32]>,
    outputs: BTreeMap<PlanOutputRef, (RunId, Option<ResultId>)>,
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
            .cloned()
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
                registry.values.remove(&previous);
            }
        }
        true
    }

    /// Publish the run as one store change. An obsolete run must never restore a released value.
    pub(crate) fn publish(&self, results: &[ReadyResult]) -> bool {
        let mut registry = self
            .registry
            .write()
            .unwrap_or_else(|error| error.into_inner());
        if results.iter().any(|result| {
            registry.outputs.get(result.output()).map(|(run, _)| *run)
                != Some(result.pin().provenance().run_id())
        }) {
            return false;
        }
        for result in results {
            let provenance = result.pin().provenance();
            if let Some((_, Some(previous))) = registry.outputs.insert(
                result.output().clone(),
                (provenance.run_id(), Some(result.result_id())),
            ) {
                registry.values.remove(&previous);
            }
            registry.values.insert(
                result.result_id(),
                StoredResultSnapshot::new(
                    Arc::new(result.value().clone()),
                    result.output().clone(),
                    provenance.clone(),
                ),
            );
        }
        true
    }

    pub(crate) fn query_pin_result(&self, output: &PlanOutputRef) -> Option<StoredResultSnapshot> {
        let registry = self
            .registry
            .read()
            .unwrap_or_else(|error| error.into_inner());
        let (_, result) = registry.outputs.get(output)?;
        registry.values.get(&(*result)?).cloned()
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
            registry
                .outputs
                .retain(|output, _| output.graph().as_str() != graph);
            registry
                .values
                .retain(|_, result| result.output().graph().as_str() != graph);
        }
    }

    pub(crate) fn invalidate_graph(&self, graph: &str) {
        let mut registry = self
            .registry
            .write()
            .unwrap_or_else(|error| error.into_inner());
        registry.graph_inputs.remove(graph);
        registry
            .outputs
            .retain(|output, _| output.graph().as_str() != graph);
        registry
            .values
            .retain(|_, result| result.output().graph().as_str() != graph);
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
            ReadyPinResult::new(output(), ResultProvenance::produced(id, run, 10)),
        )
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
                    ResultProvenance::produced(ResultId::from_existing(id), run, 10),
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
        assert!(store.begin_run(second, &[other.clone()]));
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
