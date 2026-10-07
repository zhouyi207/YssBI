//! One cancellation/lifecycle owner across schema-dependent execution stages.
use super::run_lifecycle::RunLifecycleGuard;
use super::{ExecutionRuntimeState, ExecutionWorkLease, RunExecutionControl};
use crate::error::{ExecutePreparedError, RunPhase};
use crate::finalization::ExecutionFinalizationHandoff;
use crate::plan::PlanOutputRef;
use crate::result::ResultId;
use crate::run_registry::{RunId, RunRegistryError};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct ActiveExecutionRun<'a> {
    pub(super) runtime: &'a ExecutionRuntimeState,
    pub(super) control: &'a RunExecutionControl,
    pub(super) lifecycle: RunLifecycleGuard<'a>,
    pub(super) id: RunId,
    pub(super) created_at_ms: u64,
    pub(super) completed: BTreeMap<PlanOutputRef, ResultId>,
    _work: ExecutionWorkLease,
}

impl ExecutionRuntimeState {
    pub fn start_run<'a>(
        &'a self,
        control: &'a RunExecutionControl,
    ) -> Result<ActiveExecutionRun<'a>, ExecutePreparedError> {
        let work = self.admit().map_err(ExecutePreparedError::Admission)?;
        control.check(RunPhase::Admission)?;
        let created_at_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(ExecutePreparedError::ResultTimestamp)?
            .as_millis()
            .try_into()
            .map_err(|_| ExecutePreparedError::ResultIdentityExhausted)?;
        let id = self
            .admission
            .register_run(&self.runs, Arc::clone(&control.cancellation))?;
        let lifecycle =
            RunLifecycleGuard::start(&self.runs, id).map_err(ExecutePreparedError::RunRegistry)?;
        Ok(ActiveExecutionRun {
            runtime: self,
            control,
            lifecycle,
            id,
            created_at_ms,
            completed: BTreeMap::new(),
            _work: work,
        })
    }
}

impl ActiveExecutionRun<'_> {
    pub fn run_id(&self) -> RunId {
        self.id
    }

    pub fn completed_output(&self, output: &PlanOutputRef) -> bool {
        self.completed.contains_key(output)
    }

    /// Only a successfully published stage can supply values to the next stage.
    pub fn record_published_stage(
        &mut self,
        handoff: &ExecutionFinalizationHandoff,
    ) -> Result<(), ExecutePreparedError> {
        for result in handoff
            .results()
            .iter()
            .filter(|result| result.value().is_evaluated())
        {
            let current = self.runtime.query_pin_result(result.output());
            if !current.is_some_and(|current| {
                current.provenance().run_id() == self.id
                    && current.provenance().result_id() == result.result_id()
            }) {
                return Err(ExecutePreparedError::Cancelled {
                    phase: RunPhase::Finalization,
                });
            }
            self.completed
                .insert(result.output().clone(), result.result_id());
        }
        Ok(())
    }

    pub(super) fn check_completed(&self) -> Result<(), ExecutePreparedError> {
        self.control.check(RunPhase::Admission)?;
        if self.completed.iter().any(|(output, id)| {
            self.runtime
                .query_pin_result(output)
                .is_none_or(|current| current.provenance().result_id() != *id)
        }) {
            return Err(ExecutePreparedError::Cancelled {
                phase: RunPhase::Admission,
            });
        }
        Ok(())
    }

    pub fn begin_finalization(&mut self) -> Result<(), RunRegistryError> {
        self.lifecycle.begin_finalization()
    }
}
