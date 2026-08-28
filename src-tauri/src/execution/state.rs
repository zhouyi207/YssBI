use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::time::Instant;

use thiserror::Error;

use crate::execution::error::RunPhase;
use crate::execution::finalization::{
    CandidateEffectProjection, ExecutionFinalizationHandoff, ReadyResult, ResultObservationIntent,
    SuccessfulExecutionCandidate,
};
use crate::execution::identity::{ExecutionSessionId, RuntimeGeneration};
use crate::execution::package_preparation::PreparedExecutionPlan;
use crate::execution::resource_preparation::{
    PreparedRunResources, ResourcePreparationError, ResourceProviderFactory, RunResourceBindings,
    RunResourceRequest,
};
use crate::execution::result::{
    ExecutionResultQueryError, PinResultHistorySnapshot, ResultId, StoredResult,
};
use crate::execution::result_store::ResultStore;
use crate::execution::run_registry::RunRegistry;
use crate::execution::run_registry::{RunRegistryError, RunState};

#[derive(Clone)]
pub(crate) struct RunExecutionControl {
    cancellation: Arc<AtomicBool>,
    deadline: Instant,
}

impl RunExecutionControl {
    pub(crate) fn new(deadline: Instant) -> Self {
        Self {
            cancellation: Arc::new(AtomicBool::new(false)),
            deadline,
        }
    }

    pub(crate) fn with_cancellation(cancellation: Arc<AtomicBool>, deadline: Instant) -> Self {
        Self {
            cancellation,
            deadline,
        }
    }

    fn check(&self, phase: RunPhase) -> Result<(), ExecutePreparedError> {
        if self.cancellation.load(Ordering::Acquire) {
            return Err(ExecutePreparedError::Cancelled { phase });
        }
        if Instant::now() >= self.deadline {
            return Err(ExecutePreparedError::DeadlineExceeded { phase });
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
pub(crate) enum ExecutePreparedError {
    #[error("prepared execution belongs to another runtime generation")]
    RuntimeGenerationMismatch {
        expected: RuntimeGeneration,
        actual: RuntimeGeneration,
    },
    #[error("execution admission failed")]
    Admission(#[source] ExecutionAdmissionError),
    #[error("execution resource preparation failed")]
    ResourcePreparation(#[source] ResourcePreparationError),
    #[error("execution run lifecycle failed")]
    RunRegistry(#[source] RunRegistryError),
    #[error("execution was cancelled")]
    Cancelled { phase: RunPhase },
    #[error("execution deadline was exceeded")]
    DeadlineExceeded { phase: RunPhase },
    #[error("prepared execution kernel is unavailable")]
    KernelUnavailable,
    #[error("prepared execution kernel failed")]
    Kernel(#[source] KernelExecutionError),
}

#[derive(Debug, Error)]
pub(crate) enum KernelExecutionError {
    #[error("prepared execution kernel was cancelled")]
    Cancelled,
    #[error("prepared execution kernel deadline was exceeded")]
    DeadlineExceeded,
    #[error("prepared execution kernel failed")]
    Failed,
}

#[derive(Debug)]
struct SchedulerOutput {
    results: Box<[ReadyResult]>,
    effects: Box<[CandidateEffectProjection]>,
    observation_intents: Box<[ResultObservationIntent]>,
}

impl SchedulerOutput {
    fn new(
        results: Box<[ReadyResult]>,
        effects: Box<[CandidateEffectProjection]>,
        observation_intents: Box<[ResultObservationIntent]>,
    ) -> Self {
        Self {
            results,
            effects,
            observation_intents,
        }
    }
}

trait PreparedPlanExecutor {
    fn execute(
        &self,
        package: &crate::execution::plan::CompiledExecutionPackage,
        bindings: &[crate::execution::resource_preparation::RunResourceBinding],
        resources: &PreparedRunResources,
        control: &RunExecutionControl,
    ) -> Result<SchedulerOutput, KernelExecutionError>;
}

struct RunLifecycleGuard<'a> {
    registry: &'a RunRegistry,
    run_id: crate::execution::run_registry::RunId,
    terminal: bool,
}

impl<'a> RunLifecycleGuard<'a> {
    fn start(
        registry: &'a RunRegistry,
        run_id: crate::execution::run_registry::RunId,
    ) -> Result<Self, RunRegistryError> {
        registry.transition(run_id, RunState::Running)?;
        Ok(Self {
            registry,
            run_id,
            terminal: false,
        })
    }

    fn cancel(&mut self) -> Result<(), RunRegistryError> {
        self.registry.transition(self.run_id, RunState::Cancelled)?;
        self.terminal = true;
        Ok(())
    }

    fn fail(&mut self) -> Result<(), RunRegistryError> {
        self.registry.transition(self.run_id, RunState::Failed)?;
        self.terminal = true;
        Ok(())
    }

    fn succeed(&mut self) -> Result<(), RunRegistryError> {
        self.registry
            .transition(self.run_id, RunState::Finalizing)?;
        self.registry.transition(self.run_id, RunState::Succeeded)?;
        self.terminal = true;
        Ok(())
    }
}

impl Drop for RunLifecycleGuard<'_> {
    fn drop(&mut self) {
        if !self.terminal {
            let _ = self.registry.transition(self.run_id, RunState::Failed);
        }
    }
}

#[derive(Default)]
struct RuntimeAdmission {
    closed: bool,
    active: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
pub(crate) enum ExecutionAdmissionError {
    #[error("execution session admission is closed")]
    Closed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ExecutionDrainControl {
    deadline: Instant,
}

impl ExecutionDrainControl {
    pub(crate) const fn new(deadline: Instant) -> Self {
        Self { deadline }
    }

    pub(crate) const fn deadline(self) -> Instant {
        self.deadline
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct ExecutionOutstandingWork {
    pub(crate) active: usize,
}

impl ExecutionOutstandingWork {
    const fn is_empty(self) -> bool {
        self.active == 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExecutionDrainOutcome {
    Drained {
        outstanding: ExecutionOutstandingWork,
    },
    TimedOut {
        outstanding: ExecutionOutstandingWork,
    },
}

#[must_use = "an execution work lease releases session admission when dropped"]
pub(crate) struct ExecutionWorkLease {
    admission: Arc<(Mutex<RuntimeAdmission>, Condvar)>,
}

/// Session-local execution state. It is intentionally not installed by the
/// current composition root until the atomic runtime cutover.
pub struct ExecutionRuntimeState {
    session_id: ExecutionSessionId,
    generation: RuntimeGeneration,
    admission: Arc<(Mutex<RuntimeAdmission>, Condvar)>,
    results: ResultStore,
    runs: RunRegistry,
}

impl ExecutionRuntimeState {
    pub fn new(session_id: ExecutionSessionId, generation: RuntimeGeneration) -> Self {
        Self {
            session_id,
            generation,
            admission: Arc::new((Mutex::new(RuntimeAdmission::default()), Condvar::new())),
            results: ResultStore::new(),
            runs: RunRegistry::new(),
        }
    }

    pub fn session_id(&self) -> ExecutionSessionId {
        self.session_id
    }

    pub fn generation(&self) -> RuntimeGeneration {
        self.generation
    }

    pub(crate) fn close_admission(&self) {
        let (state, _) = &*self.admission;
        state.lock().unwrap_or_else(PoisonError::into_inner).closed = true;
    }

    pub fn is_admission_closed(&self) -> bool {
        let (state, _) = &*self.admission;
        state.lock().unwrap_or_else(PoisonError::into_inner).closed
    }

    pub(crate) fn results(&self) -> &ResultStore {
        &self.results
    }

    pub(crate) fn query_result(&self, result_id: ResultId) -> Option<Arc<StoredResult>> {
        self.results.get(result_id)
    }

    pub(crate) fn query_pin_result_history(
        &self,
        output: &crate::execution::plan::PlanOutputRef,
    ) -> Result<Box<[PinResultHistorySnapshot]>, ExecutionResultQueryError> {
        self.results.query_pin_result_history(output)
    }

    pub(crate) fn runs(&self) -> &RunRegistry {
        &self.runs
    }

    pub(crate) fn execute_prepared(
        &self,
        plan: &PreparedExecutionPlan,
        bindings: RunResourceBindings,
        resources: &ResourceProviderFactory,
        control: &RunExecutionControl,
    ) -> Result<SuccessfulExecutionCandidate, ExecutePreparedError> {
        self.execute_prepared_inner(plan, bindings, resources, control, None)
    }

    pub(crate) fn execute_prepared_handoff(
        &self,
        plan: &PreparedExecutionPlan,
        bindings: RunResourceBindings,
        resources: &ResourceProviderFactory,
        control: &RunExecutionControl,
    ) -> Result<ExecutionFinalizationHandoff, ExecutePreparedError> {
        self.execute_prepared(plan, bindings, resources, control)
            .map(SuccessfulExecutionCandidate::into_finalization_handoff)
    }

    fn execute_prepared_inner(
        &self,
        plan: &PreparedExecutionPlan,
        bindings: RunResourceBindings,
        resources: &ResourceProviderFactory,
        control: &RunExecutionControl,
        executor: Option<&dyn PreparedPlanExecutor>,
    ) -> Result<SuccessfulExecutionCandidate, ExecutePreparedError> {
        let actual_generation = self.generation();
        let plan_generation = plan.generation();
        if actual_generation != plan_generation {
            return Err(ExecutePreparedError::RuntimeGenerationMismatch {
                expected: actual_generation,
                actual: plan_generation,
            });
        }

        let _work = self.admit().map_err(ExecutePreparedError::Admission)?;
        control.check(RunPhase::Admission)?;

        let request = RunResourceRequest::new(plan, &bindings);
        let prepared_resources = resources
            .prepare(&request)
            .map_err(ExecutePreparedError::ResourcePreparation)?;
        control.check(RunPhase::ResourcePreparation)?;

        let run_id = self
            .runs
            .admit_next()
            .map_err(ExecutePreparedError::RunRegistry)?;
        let mut lifecycle = RunLifecycleGuard::start(&self.runs, run_id)
            .map_err(ExecutePreparedError::RunRegistry)?;
        if let Err(error) = control.check(RunPhase::Execution) {
            return terminate_run(&mut lifecycle, error);
        }

        let Some(executor) = executor else {
            return terminate_run(&mut lifecycle, ExecutePreparedError::KernelUnavailable);
        };
        let output = match executor.execute(
            plan.package(),
            bindings.bindings(),
            &prepared_resources,
            control,
        ) {
            Ok(output) => output,
            Err(KernelExecutionError::Cancelled) => {
                return terminate_run(
                    &mut lifecycle,
                    ExecutePreparedError::Cancelled {
                        phase: RunPhase::Execution,
                    },
                );
            }
            Err(KernelExecutionError::DeadlineExceeded) => {
                return terminate_run(
                    &mut lifecycle,
                    ExecutePreparedError::DeadlineExceeded {
                        phase: RunPhase::Execution,
                    },
                );
            }
            Err(error) => {
                return terminate_run(&mut lifecycle, ExecutePreparedError::Kernel(error));
            }
        };
        if let Err(error) = control.check(RunPhase::Finalization) {
            return terminate_run(&mut lifecycle, error);
        }

        let (effects, grants) = prepared_resources.finish();
        let candidate = SuccessfulExecutionCandidate::from_scheduler(
            output.results,
            effects,
            output.observation_intents,
            grants,
        );
        lifecycle
            .succeed()
            .map_err(ExecutePreparedError::RunRegistry)?;
        Ok(candidate)
    }

    #[cfg(test)]
    fn execute_prepared_with_executor(
        &self,
        plan: &PreparedExecutionPlan,
        bindings: RunResourceBindings,
        resources: &ResourceProviderFactory,
        control: &RunExecutionControl,
        executor: &dyn PreparedPlanExecutor,
    ) -> Result<SuccessfulExecutionCandidate, ExecutePreparedError> {
        self.execute_prepared_inner(plan, bindings, resources, control, Some(executor))
    }

    pub(crate) fn admit(&self) -> Result<ExecutionWorkLease, ExecutionAdmissionError> {
        let (state, _) = &*self.admission;
        let mut state = state.lock().unwrap_or_else(PoisonError::into_inner);
        if state.closed {
            return Err(ExecutionAdmissionError::Closed);
        }
        state.active += 1;
        drop(state);
        Ok(ExecutionWorkLease {
            admission: Arc::clone(&self.admission),
        })
    }

    pub(crate) fn drain(&self, control: &ExecutionDrainControl) -> ExecutionDrainOutcome {
        let (state, changed) = &*self.admission;
        let mut state = state.lock().unwrap_or_else(PoisonError::into_inner);
        loop {
            let outstanding = ExecutionOutstandingWork {
                active: state.active,
            };
            if outstanding.is_empty() {
                return ExecutionDrainOutcome::Drained { outstanding };
            }

            let Some(remaining) = control.deadline().checked_duration_since(Instant::now()) else {
                return ExecutionDrainOutcome::TimedOut { outstanding };
            };
            let (next_state, wait_result) = changed
                .wait_timeout(state, remaining)
                .unwrap_or_else(|error| error.into_inner());
            state = next_state;
            if wait_result.timed_out() {
                return ExecutionDrainOutcome::TimedOut {
                    outstanding: ExecutionOutstandingWork {
                        active: state.active,
                    },
                };
            }
        }
    }

    pub(crate) fn cancel_and_drain(
        &self,
        control: &ExecutionDrainControl,
    ) -> ExecutionDrainOutcome {
        self.close_admission();
        self.drain(control)
    }
}

fn terminate_run(
    lifecycle: &mut RunLifecycleGuard<'_>,
    error: ExecutePreparedError,
) -> Result<SuccessfulExecutionCandidate, ExecutePreparedError> {
    let transition = if matches!(&error, ExecutePreparedError::Cancelled { .. }) {
        lifecycle.cancel()
    } else {
        lifecycle.fail()
    };
    transition.map_err(ExecutePreparedError::RunRegistry)?;
    Err(error)
}

impl Drop for ExecutionWorkLease {
    fn drop(&mut self) {
        let (state, changed) = &*self.admission;
        let mut state = state.lock().unwrap_or_else(PoisonError::into_inner);
        debug_assert!(state.active > 0);
        state.active = state.active.saturating_sub(1);
        drop(state);
        changed.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::identity::ExecutionSessionId;
    use crate::execution::package_preparation::PreparedExecutionPlan;
    use crate::execution::plan::{
        CompiledExecutionPackage, CompiledFunctionBundle, CompiledParameterBundleBuilder,
        ExecutionPlan, PlanCompilationBasis, PlanCompileId, PlanGraphId, PlanGraphRevision,
        PlanProjectSessionId, PlanProvenance, PlanRegistryFingerprint, PlanResourceId,
        PlanResourceObservedState, PlanResourceRequirement, PlanResourceVersion,
        PlanSourceIdentity, ResourceAccess, ResourceKind,
    };
    use crate::execution::resource_preparation::{RunResourceBinding, RunResourceBindings};
    use crate::execution::result_store::{ResultId, StoredResult};
    use crate::execution::run_registry::{RunId, RunState};
    use std::collections::BTreeMap;
    use std::time::Duration;

    fn prepared_plan(state: &ExecutionRuntimeState) -> PreparedExecutionPlan {
        let resource = PlanResourceId::from_existing("variables/answer".into());
        let version = PlanResourceVersion::from_existing("v1".into());
        let basis = PlanCompilationBasis::new(
            PlanProjectSessionId::from_existing("session".into()),
            PlanGraphRevision::INITIAL,
            PlanRegistryFingerprint::from_bytes([4; 32]),
            BTreeMap::from([(resource.clone(), version.clone())]),
            BTreeMap::from([(resource, PlanResourceObservedState::Present(version))]),
        );
        let parameters = Arc::new(CompiledParameterBundleBuilder::new(basis.clone()).freeze());
        let functions = Arc::new(CompiledFunctionBundle::new(basis.clone(), Box::new([]), 0));
        let package = CompiledExecutionPackage::new(
            Arc::new(ExecutionPlan::empty()),
            functions,
            parameters,
            PlanProvenance::new(
                PlanSourceIdentity::new(
                    PlanGraphId::from_existing("events/main".into()),
                    None,
                    None,
                ),
                basis,
                PlanCompileId::from_existing(11),
            ),
        );
        state
            .prepare_compiled_package(package, RuntimeGeneration::INITIAL)
            .expect("test package is valid")
    }

    fn bindings() -> RunResourceBindings {
        let requirement = PlanResourceRequirement::new(
            PlanResourceId::from_existing("variables/answer".into()),
            ResourceKind::Variable,
            ResourceAccess::Shared,
            false,
        );
        RunResourceBindings::new(
            PlanProjectSessionId::from_existing("session".into()),
            [requirement.clone()],
            [RunResourceBinding::new(
                requirement,
                PlanResourceVersion::from_existing("v1".into()),
                crate::execution::value::RuntimeValue::Integer(4),
            )],
        )
    }

    struct TestExecutor;

    impl PreparedPlanExecutor for TestExecutor {
        fn execute(
            &self,
            _package: &CompiledExecutionPackage,
            bindings: &[RunResourceBinding],
            resources: &PreparedRunResources,
            _control: &RunExecutionControl,
        ) -> Result<SchedulerOutput, KernelExecutionError> {
            assert_eq!(bindings.len(), 1);
            assert_eq!(
                resources.value(&PlanResourceId::from_existing("variables/answer".into())),
                Some(&crate::execution::value::RuntimeValue::Integer(4))
            );
            Ok(SchedulerOutput::new(
                vec![ReadyResult::from_scheduler(
                    ResultId::from_existing(1),
                    StoredResult::Runtime(crate::execution::value::RuntimeValue::Integer(5)),
                )]
                .into_boxed_slice(),
                Box::new([]),
                Box::new([]),
            ))
        }
    }

    fn state() -> ExecutionRuntimeState {
        ExecutionRuntimeState::new(
            ExecutionSessionId::new(uuid::Uuid::nil()),
            crate::execution::identity::RuntimeGeneration::INITIAL,
        )
    }

    #[test]
    fn closed_session_drains_an_active_lease_and_rejects_new_work() {
        let state = state();
        let lease = state.admit().expect("test admission must open");
        assert_eq!(
            state.cancel_and_drain(&ExecutionDrainControl::new(Instant::now())),
            ExecutionDrainOutcome::TimedOut {
                outstanding: ExecutionOutstandingWork { active: 1 },
            }
        );
        assert!(matches!(
            state.admit(),
            Err(ExecutionAdmissionError::Closed)
        ));

        drop(lease);
        assert_eq!(
            state.drain(&ExecutionDrainControl::new(
                Instant::now() + Duration::from_secs(1),
            )),
            ExecutionDrainOutcome::Drained {
                outstanding: ExecutionOutstandingWork { active: 0 },
            }
        );
    }

    #[test]
    fn execute_prepared_reports_kernel_unavailable_without_publishing_a_candidate() {
        let state = state();
        let plan = prepared_plan(&state);
        let result = state.execute_prepared(
            &plan,
            bindings(),
            &ResourceProviderFactory::new("session".into()),
            &RunExecutionControl::new(Instant::now() + Duration::from_secs(1)),
        );

        assert!(matches!(
            result,
            Err(ExecutePreparedError::KernelUnavailable)
        ));
        assert_eq!(
            state.runs().state(RunId::from_existing(0)),
            Some(RunState::Failed)
        );
        assert_eq!(state.results().get(ResultId::from_existing(1)), None);
    }

    #[test]
    fn execute_prepared_success_uses_the_candidate_to_create_the_only_handoff() {
        let state = state();
        let plan = prepared_plan(&state);
        let candidate = state
            .execute_prepared_with_executor(
                &plan,
                bindings(),
                &ResourceProviderFactory::new("session".into()),
                &RunExecutionControl::new(Instant::now() + Duration::from_secs(1)),
                &TestExecutor,
            )
            .expect("test executor produces a neutral scheduler output");
        let handoff = candidate.into_finalization_handoff();

        assert_eq!(handoff.results().len(), 1);
        assert_eq!(handoff.results()[0].result_id(), ResultId::from_existing(1));
        assert_eq!(
            handoff.results()[0].value(),
            &StoredResult::Runtime(crate::execution::value::RuntimeValue::Integer(5))
        );
        assert_eq!(
            state.runs().state(RunId::from_existing(0)),
            Some(RunState::Succeeded)
        );
    }

    #[test]
    fn execute_prepared_cancellation_happens_before_run_registration() {
        let state = state();
        let plan = prepared_plan(&state);
        let cancellation = Arc::new(AtomicBool::new(true));
        let result = state.execute_prepared(
            &plan,
            bindings(),
            &ResourceProviderFactory::new("session".into()),
            &RunExecutionControl::with_cancellation(
                cancellation,
                Instant::now() + Duration::from_secs(1),
            ),
        );

        assert!(matches!(
            result,
            Err(ExecutePreparedError::Cancelled {
                phase: RunPhase::Admission
            })
        ));
        assert_eq!(state.runs().state(RunId::from_existing(0)), None);
    }
}
