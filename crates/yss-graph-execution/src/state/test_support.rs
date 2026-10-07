//! Result-query fixtures avoid executing their deliberately controlled reader adapters.
use super::ExecutionRuntimeState;
use crate::finalization::{ReadyPinResult, ReadyResult};
use crate::plan::PlanOutputRef;
use crate::result::{ResultId, ResultProvenance, StoredResult};
use crate::run_registry::RunId;
use std::collections::BTreeMap;

impl ExecutionRuntimeState {
    pub fn publish_fixture_result(&self, output: PlanOutputRef, value: StoredResult) -> ResultId {
        let id = self.allocate_result_id().expect("fixture identity");
        let run = RunId::from_existing(id.get());
        assert!(
            self.results
                .begin_run(run, std::slice::from_ref(&output), None, &BTreeMap::new())
        );
        let category = value.category();
        let result = ReadyResult::from_scheduler(
            id,
            value.with_evaluated_boundary(true),
            category,
            ReadyPinResult::new(
                output,
                ResultProvenance::produced(self.session_id, id, run, 0),
            ),
        );
        assert!(self.results.publish(&[result], &[]));
        id
    }
}
