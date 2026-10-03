//! Coordinate admission, resource preparation, execution and the finalization handoff.
use super::run_lifecycle::RunLifecycleGuard;
use super::scheduler::{
    PreparedPlanExecution, PreparedPlanExecutor, SchedulerOutput, execution_producers,
    select_execution,
};
use super::{ExecutionRuntimeState, RunExecutionControl};
use crate::error::{ExecutePreparedError, OperationExecutionError, RunPhase};
use crate::finalization::{
    ExecutionFinalizationHandoff, ReadyPinResult, ReadyResult, ResultObservationIntent,
    SuccessfulExecutionCandidate,
};
use crate::package_preparation::PreparedExecutionPlan;
use crate::resource_preparation::{
    PreparedRunResources, ResourceProviderFactory, RunResourceBindings, RunResourceRequest,
};
use crate::result::{ResultId, ResultProvenance, ResultRunBasis};
use std::{
    collections::BTreeMap,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use yss_node_kernel::KernelError;

pub struct ExecutionResultRequest<'a> {
    pub demand: &'a crate::plan::PlanExecutionDemand,
    pub basis: Option<&'a ResultRunBasis>,
}

struct PreparedExecutionDispatch<'a> {
    demand: &'a crate::plan::PlanExecutionDemand,
    result_basis: Option<&'a ResultRunBasis>,
    executor: &'a dyn PreparedPlanExecutor,
    on_event: Option<&'a mut dyn FnMut(PreparedExecutionEvent)>,
}

struct ExecutedPreparedCandidate {
    run_id: crate::run_registry::RunId,
    candidate: SuccessfulExecutionCandidate,
}

impl ExecutedPreparedCandidate {
    #[cfg(test)]
    fn candidate(self) -> SuccessfulExecutionCandidate {
        self.candidate
    }

    fn into_executed_run(self) -> ExecutedPreparedRun {
        ExecutedPreparedRun {
            run_id: self.run_id,
            handoff: self.candidate.into_finalization_handoff(),
        }
    }
}

pub struct ExecutedPreparedRun {
    run_id: crate::run_registry::RunId,
    handoff: ExecutionFinalizationHandoff,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PreparedExecutionEvent {
    RunStarted {
        run_id: crate::run_registry::RunId,
        outputs: Box<[crate::plan::PlanOutputRef]>,
    },
}

impl ExecutedPreparedRun {
    pub const fn run_id(&self) -> crate::run_registry::RunId {
        self.run_id
    }

    pub fn handoff(&self) -> &ExecutionFinalizationHandoff {
        &self.handoff
    }

    pub fn into_handoff(self) -> ExecutionFinalizationHandoff {
        self.handoff
    }
}

impl ExecutionRuntimeState {
    #[cfg(test)]
    pub(super) fn execute_prepared(
        &self,
        plan: &PreparedExecutionPlan,
        bindings: RunResourceBindings,
        resources: &ResourceProviderFactory,
        control: &RunExecutionControl,
    ) -> Result<SuccessfulExecutionCandidate, ExecutePreparedError> {
        let executed = self.execute_prepared_inner(
            plan,
            bindings,
            resources,
            control,
            PreparedExecutionDispatch {
                demand: &crate::plan::PlanExecutionDemand::Default,
                result_basis: None,
                executor: &self.executor,
                on_event: None,
            },
        )?;
        self.finalize_run_success(executed.run_id)
            .map_err(ExecutePreparedError::RunRegistry)?;
        Ok(executed.candidate())
    }

    pub fn execute_prepared_handoff(
        &self,
        plan: &PreparedExecutionPlan,
        bindings: RunResourceBindings,
        resources: &ResourceProviderFactory,
        control: &RunExecutionControl,
        results: ExecutionResultRequest<'_>,
        mut on_event: impl FnMut(PreparedExecutionEvent),
    ) -> Result<ExecutedPreparedRun, ExecutePreparedError> {
        self.execute_prepared_inner(
            plan,
            bindings,
            resources,
            control,
            PreparedExecutionDispatch {
                demand: results.demand,
                result_basis: results.basis,
                executor: &self.executor,
                on_event: Some(&mut on_event),
            },
        )
        .map(ExecutedPreparedCandidate::into_executed_run)
    }

    fn execute_prepared_inner(
        &self,
        plan: &PreparedExecutionPlan,
        bindings: RunResourceBindings,
        resources: &ResourceProviderFactory,
        control: &RunExecutionControl,
        dispatch: PreparedExecutionDispatch<'_>,
    ) -> Result<ExecutedPreparedCandidate, ExecutePreparedError> {
        let PreparedExecutionDispatch {
            demand,
            result_basis,
            executor,
            mut on_event,
        } = dispatch;
        let actual_generation = self.generation();
        if plan.package().provenance().basis().kernel_fingerprint() != self.kernels().fingerprint()
        {
            return Err(ExecutePreparedError::KernelCapabilitiesChanged);
        }
        let plan_generation = plan.generation();
        if actual_generation != plan_generation {
            return Err(ExecutePreparedError::RuntimeGenerationMismatch {
                expected: actual_generation,
                actual: plan_generation,
            });
        }

        let created_at_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(ExecutePreparedError::ResultTimestamp)?
            .as_millis()
            .try_into()
            .map_err(|_| ExecutePreparedError::ResultIdentityExhausted)?;

        let _work = self.admit().map_err(ExecutePreparedError::Admission)?;
        control.check(RunPhase::Admission)?;

        let producers =
            execution_producers(plan.package()).map_err(ExecutePreparedError::Kernel)?;
        let reuse_inputs = result_basis.is_some()
            && matches!(
                demand,
                crate::plan::PlanExecutionDemand::Outputs {
                    reuse_inputs: true,
                    ..
                }
            );
        let selection = select_execution(plan.package(), demand, &producers, |output| {
            reuse_inputs
                .then(|| self.results.query_pin_result(output))
                .flatten()
        })
        .map_err(ExecutePreparedError::Kernel)?;
        let reused_inputs = selection
            .cached_values
            .values()
            .map(|snapshot| (snapshot.output().clone(), snapshot.provenance().result_id()))
            .collect::<BTreeMap<_, _>>();
        let outputs = plan
            .package()
            .plan()
            .operations()
            .iter()
            .enumerate()
            .filter(|(index, _)| selection.required_operations[*index])
            .flat_map(|(_, operation)| {
                operation
                    .outputs()
                    .iter()
                    .map(|output| output.output().clone())
            })
            .collect::<Box<[_]>>();
        let run_id = self
            .admission
            .register_run(&self.runs, Arc::clone(&control.cancellation))?;
        let mut lifecycle = RunLifecycleGuard::start(&self.runs, run_id)
            .map_err(ExecutePreparedError::RunRegistry)?;
        let candidate = (|| {
            if !self
                .results
                .begin_run(run_id, &outputs, result_basis, &reused_inputs)
            {
                return Err(ExecutePreparedError::Cancelled {
                    phase: RunPhase::Admission,
                });
            }
            if let Some(on_event) = on_event.as_mut() {
                on_event(PreparedExecutionEvent::RunStarted { run_id, outputs });
            }
            let request = RunResourceRequest::new(plan, &bindings);
            let prepared_resources = resources
                .prepare(&request)
                .map_err(ExecutePreparedError::ResourcePreparation)?;
            control.check(RunPhase::Execution)?;
            let output = executor
                .execute(PreparedPlanExecution {
                    package: plan.package(),
                    resources: &prepared_resources,
                    control,
                    producers: &producers,
                    selection,
                })
                .map_err(|error| match error {
                    OperationExecutionError::Kernel(KernelError::Cancelled) => {
                        ExecutePreparedError::Cancelled {
                            phase: RunPhase::Execution,
                        }
                    }
                    OperationExecutionError::Kernel(KernelError::DeadlineExceeded) => {
                        ExecutePreparedError::DeadlineExceeded {
                            phase: RunPhase::Execution,
                        }
                    }
                    error => ExecutePreparedError::Kernel(error),
                })?;
            control.check(RunPhase::Finalization)?;
            self.prepare_result_candidate(
                output,
                prepared_resources,
                reused_inputs,
                run_id,
                created_at_ms,
            )
        })();
        let candidate = candidate.map_err(|error| lifecycle.terminate(error))?;
        lifecycle
            .begin_finalization()
            .map_err(ExecutePreparedError::RunRegistry)?;
        Ok(ExecutedPreparedCandidate { run_id, candidate })
    }

    fn prepare_result_candidate(
        &self,
        output: SchedulerOutput,
        resources: PreparedRunResources,
        mut result_ids_by_output: BTreeMap<crate::plan::PlanOutputRef, ResultId>,
        run_id: crate::run_registry::RunId,
        created_at_ms: u64,
    ) -> Result<SuccessfulExecutionCandidate, ExecutePreparedError> {
        let mut results = Vec::with_capacity(output.results.len());
        for scheduled in output.results {
            let result_id = self.allocate_result_id()?;
            result_ids_by_output.insert(scheduled.output.clone(), result_id);
            let pin = ReadyPinResult::new(
                scheduled.output,
                ResultProvenance::produced(self.session_id, result_id, run_id, created_at_ms),
            );
            results.push(ReadyResult::from_scheduler(
                result_id,
                scheduled.value,
                scheduled.category,
                pin,
            ));
        }
        let observation_intents = output
            .observations
            .into_iter()
            .map(|observation| {
                let result_id = result_ids_by_output
                    .get(&observation.output)
                    .copied()
                    .ok_or(ExecutePreparedError::Kernel(
                        OperationExecutionError::Failed,
                    ))?;
                Ok(ResultObservationIntent {
                    result_id,
                    requester: observation.requester,
                })
            })
            .collect::<Result<Box<[_]>, ExecutePreparedError>>()?;
        Ok(SuccessfulExecutionCandidate::from_scheduler(
            results.into_boxed_slice(),
            observation_intents,
            resources.finish(),
        ))
    }

    #[cfg(test)]
    pub(super) fn execute_prepared_with_executor(
        &self,
        plan: &PreparedExecutionPlan,
        bindings: RunResourceBindings,
        resources: &ResourceProviderFactory,
        control: &RunExecutionControl,
        executor: &dyn PreparedPlanExecutor,
    ) -> Result<SuccessfulExecutionCandidate, ExecutePreparedError> {
        let executed = self.execute_prepared_inner(
            plan,
            bindings,
            resources,
            control,
            PreparedExecutionDispatch {
                demand: &crate::plan::PlanExecutionDemand::Default,
                result_basis: None,
                executor,
                on_event: None,
            },
        )?;
        self.finalize_run_success(executed.run_id)
            .map_err(ExecutePreparedError::RunRegistry)?;
        Ok(executed.candidate())
    }
}
