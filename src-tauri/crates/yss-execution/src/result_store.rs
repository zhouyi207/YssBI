use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

use crate::plan::PlanOutputRef;

use crate::result::{
    ExecutionResultQueryError, PinResultEntry, PinResultHistorySnapshot, StoredResultSnapshot,
};
pub use crate::result::{ResultId, StoredResult};

#[derive(Default)]
struct ResultStoreRegistry {
    values: BTreeMap<ResultId, Arc<StoredResult>>,
    sources: BTreeMap<ResultId, (PlanOutputRef, PinResultEntry)>,
    pin_history: BTreeMap<PlanOutputRef, Vec<PinResultEntry>>,
}

pub struct ResultStore {
    registry: RwLock<ResultStoreRegistry>,
}

impl ResultStore {
    pub fn new() -> Self {
        Self {
            registry: RwLock::new(ResultStoreRegistry::default()),
        }
    }

    pub fn get(&self, result: ResultId) -> Option<StoredResultSnapshot> {
        let registry = self
            .registry
            .read()
            .unwrap_or_else(|error| error.into_inner());
        let value = registry.values.get(&result).cloned()?;
        let (output, entry) = registry.sources.get(&result).cloned()?;
        Some(StoredResultSnapshot::new(value, output, entry))
    }

    pub(crate) fn publish_for_output(
        &self,
        output: PlanOutputRef,
        entry: PinResultEntry,
        value: StoredResult,
    ) -> Arc<StoredResult> {
        let value = Arc::new(value);
        let mut registry = self
            .registry
            .write()
            .unwrap_or_else(|error| error.into_inner());
        registry
            .values
            .insert(entry.result_id(), Arc::clone(&value));
        registry
            .sources
            .insert(entry.result_id(), (output.clone(), entry.clone()));
        registry.pin_history.entry(output).or_default().push(entry);
        value
    }

    pub(crate) fn query_pin_result_history(
        &self,
        output: &PlanOutputRef,
    ) -> Result<Box<[PinResultHistorySnapshot]>, ExecutionResultQueryError> {
        let registry = self
            .registry
            .read()
            .unwrap_or_else(|error| error.into_inner());
        let Some(entries) = registry.pin_history.get(output) else {
            return Ok(Box::new([]));
        };
        entries
            .iter()
            .map(|entry| {
                let result = registry.values.get(&entry.result_id()).cloned().ok_or(
                    ExecutionResultQueryError::ResultSourceReadFailed {
                        result_id: entry.result_id(),
                    },
                )?;
                Ok(PinResultHistorySnapshot::new(entry.clone(), result))
            })
            .collect()
    }

    pub fn clear(&self) {
        let mut registry = self
            .registry
            .write()
            .unwrap_or_else(|error| error.into_inner());
        registry.values.clear();
        registry.sources.clear();
        registry.pin_history.clear();
    }
}

impl Default for ResultStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::{PlanGraphId, PlanGraphRevision, PlanOutputRef, PlanPortAddress};
    use crate::result::{ActivationId, PinResultEntry};
    use crate::run_registry::RunId;

    fn output() -> PlanOutputRef {
        PlanOutputRef::new(
            PlanGraphId::from_existing("events/main.yssbi-event".into()),
            PlanPortAddress::from_existing("node:result".into()),
        )
    }

    #[test]
    fn result_query_pin_history_pairs_oldest_entries_with_results_under_one_store_view() {
        let store = ResultStore::new();
        let output = output();
        store.publish_for_output(
            output.clone(),
            PinResultEntry::produced(
                ResultId::from_existing(1),
                RunId::from_existing(7),
                ActivationId::from_existing(1),
                PlanGraphRevision::INITIAL,
                10,
            ),
            StoredResult::Scalar(1.0),
        );
        store.publish_for_output(
            output.clone(),
            PinResultEntry::produced(
                ResultId::from_existing(2),
                RunId::from_existing(8),
                ActivationId::from_existing(2),
                PlanGraphRevision::from_existing(1),
                20,
            ),
            StoredResult::Scalar(2.0),
        );

        let history = store
            .query_pin_result_history(&output)
            .expect("all history entries have a stored result");
        let current = store
            .get(ResultId::from_existing(2))
            .expect("cached output remains directly queryable");
        assert_eq!(current.output(), &output);
        assert_eq!(current.entry().result_id(), ResultId::from_existing(2));
        let parts = history
            .into_vec()
            .into_iter()
            .map(PinResultHistorySnapshot::into_parts)
            .collect::<Vec<_>>();
        assert_eq!(parts[0].0.result_id(), ResultId::from_existing(1));
        assert_eq!(parts[1].0.result_id(), ResultId::from_existing(2));
        assert_eq!(*parts[0].1, StoredResult::Scalar(1.0));
        assert_eq!(*parts[1].1, StoredResult::Scalar(2.0));
    }
}
