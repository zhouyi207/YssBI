//! Coordinate admission, resource preparation, execution and the finalization handoff.
use super::scheduler::{
    PreparedPlanExecution, PreparedPlanExecutor, SchedulerOutput, execution_producers,
    select_execution,
};
use super::{ActiveExecutionRun, ExecutionRuntimeState, RunExecutionControl};
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
use std::collections::BTreeMap;
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
        let mut run = self.start_run(control)?;
        let candidate = self
            .execute_stage_inner(plan, bindings, resources, &mut run, dispatch)
            .map_err(|error| run.lifecycle.terminate(error))?;
        run.begin_finalization()
            .map_err(ExecutePreparedError::RunRegistry)?;
        Ok(ExecutedPreparedCandidate {
            run_id: run.id,
            candidate,
        })
    }

    fn execute_stage_inner(
        &self,
        plan: &PreparedExecutionPlan,
        bindings: RunResourceBindings,
        resources: &ResourceProviderFactory,
        run: &mut ActiveExecutionRun<'_>,
        dispatch: PreparedExecutionDispatch<'_>,
    ) -> Result<SuccessfulExecutionCandidate, ExecutePreparedError> {
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

        let control = run.control;
        run.check_completed()?;
        if self.runs.state(run.id) != Some(crate::run_registry::RunState::Running) {
            return Err(ExecutePreparedError::RunRegistry(
                crate::run_registry::RunRegistryError::InvalidTransition,
            ));
        }

        let producers =
            execution_producers(plan.package()).map_err(ExecutePreparedError::Kernel)?;
        let reuse_inputs = result_basis.is_some()
            && matches!(
                demand,
                crate::plan::PlanExecutionDemand::Outputs {
                    reuse_inputs: true,
                    ..
                } | crate::plan::PlanExecutionDemand::Node { .. }
            );
        let retained = self
            .results
            .retained_boundaries(plan.package().provenance().source().graph().as_str());
        let completed = run.completed.keys().cloned().collect();
        let selection = select_execution(
            plan.package(),
            demand,
            &producers,
            &retained,
            &completed,
            |output| {
                (reuse_inputs || run.completed.contains_key(output))
                    .then(|| self.results.query_pin_result(output))
                    .flatten()
            },
        )
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
        let run_id = run.id;
        {
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
                result_basis,
                run_id,
                run.created_at_ms,
            )
        }
    }

    fn prepare_result_candidate(
        &self,
        output: SchedulerOutput,
        resources: PreparedRunResources,
        mut result_ids_by_output: BTreeMap<crate::plan::PlanOutputRef, ResultId>,
        result_basis: Option<&crate::result::ResultRunBasis>,
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
                    input_basis: result_basis
                        .and_then(|basis| {
                            observation
                                .requester
                                .node()
                                .and_then(|node| basis.inputs.observers.get(node))
                        })
                        .cloned(),
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

impl ActiveExecutionRun<'_> {
    pub fn execute_stage(
        &mut self,
        plan: &PreparedExecutionPlan,
        bindings: RunResourceBindings,
        resources: &ResourceProviderFactory,
        results: ExecutionResultRequest<'_>,
        mut on_event: impl FnMut(PreparedExecutionEvent),
    ) -> Result<ExecutionFinalizationHandoff, ExecutePreparedError> {
        self.runtime
            .execute_stage_inner(
                plan,
                bindings,
                resources,
                self,
                PreparedExecutionDispatch {
                    demand: results.demand,
                    result_basis: results.basis,
                    executor: &self.runtime.executor,
                    on_event: Some(&mut on_event),
                },
            )
            .map_err(|error| self.lifecycle.terminate(error))
            .map(SuccessfulExecutionCandidate::into_finalization_handoff)
    }
}
