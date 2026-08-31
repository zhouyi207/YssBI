use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::time::Instant;

use thiserror::Error;

use crate::error::RunPhase;
use crate::finalization::{
    ExecutionFinalizationHandoff, ReadyResult, ResultObservationIntent,
    SuccessfulExecutionCandidate,
};
use crate::identity::{ExecutionSessionId, RuntimeGeneration};
use crate::package_preparation::PreparedExecutionPlan;
use crate::ports::scientific::ScientificBackend;
use crate::resource_preparation::{
    PreparedRunResources, ResourcePreparationError, ResourceProviderFactory, RunResourceBindings,
    RunResourceRequest,
};
use crate::result::{ExecutionResultQueryError, PinResultHistorySnapshot, ResultId, StoredResult};
use crate::result_store::ResultStore;
use crate::run_registry::RunRegistry;
use crate::run_registry::{RunRegistryError, RunState};
use crate::value::RuntimeValue;

#[derive(Clone)]
pub struct RunExecutionControl {
    cancellation: Arc<AtomicBool>,
    deadline: Instant,
}

impl RunExecutionControl {
    #[cfg(test)]
    pub(crate) fn new(deadline: Instant) -> Self {
        Self {
            cancellation: Arc::new(AtomicBool::new(false)),
            deadline,
        }
    }

    pub fn with_cancellation(cancellation: Arc<AtomicBool>, deadline: Instant) -> Self {
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
pub enum ExecutePreparedError {
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
pub enum KernelExecutionError {
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
    observation_intents: Box<[ResultObservationIntent]>,
}

impl SchedulerOutput {
    fn new(
        results: Box<[ReadyResult]>,
        observation_intents: Box<[ResultObservationIntent]>,
    ) -> Self {
        Self {
            results,
            observation_intents,
        }
    }
}

trait PreparedPlanExecutor: Send + Sync {
    fn execute(
        &self,
        package: &crate::plan::CompiledExecutionPackage,
        bindings: &[crate::resource_preparation::RunResourceBinding],
        resources: &PreparedRunResources,
        control: &RunExecutionControl,
        run_id: crate::run_registry::RunId,
    ) -> Result<SchedulerOutput, KernelExecutionError>;
}

#[cfg(any(test, feature = "test-support"))]
struct UnavailableScientificBackend;

#[cfg(any(test, feature = "test-support"))]
impl ScientificBackend for UnavailableScientificBackend {
    fn acf_pacf(
        &self,
        _request: crate::ports::scientific::AcfPacfRequest,
        _control: &crate::ports::scientific::BackendExecutionControl,
    ) -> Result<
        crate::ports::scientific::AcfPacfResult,
        crate::ports::scientific::ScientificBackendError,
    > {
        Err(crate::ports::scientific::ScientificBackendError::Unavailable)
    }
}

#[derive(Default)]
struct NeutralPlanExecutor;

impl PreparedPlanExecutor for NeutralPlanExecutor {
    fn execute(
        &self,
        package: &crate::plan::CompiledExecutionPackage,
        _bindings: &[crate::resource_preparation::RunResourceBinding],
        resources: &PreparedRunResources,
        control: &RunExecutionControl,
        run_id: crate::run_registry::RunId,
    ) -> Result<SchedulerOutput, KernelExecutionError> {
        let mut values = Vec::new();
        let mut inspected = None;
        let mut last_output = None;

        for operation in package.plan().operations() {
            check_kernel_control(control)?;
            let inputs = operation
                .inputs()
                .iter()
                .map(|binding| match binding.source() {
                    crate::plan::PlanInputSource::Value(reference) => values
                        .get(reference.index() as usize)
                        .cloned()
                        .ok_or(KernelExecutionError::Failed),
                    crate::plan::PlanInputSource::Parameter(handle) => {
                        let Some(payload) = package.parameters().entries().get(handle) else {
                            return Err(KernelExecutionError::Failed);
                        };
                        parameter_value(payload.value(), resources)
                    }
                })
                .collect::<Result<Vec<_>, _>>()?;

            let value = execute_operation(
                operation.kind().as_str(),
                &inputs,
                operation
                    .parameter_handles()
                    .iter()
                    .find_map(|handle| package.parameters().entries().get(handle))
                    .map(|payload| payload.value()),
                resources,
            )?;
            if operation.kind().as_str() == "yssbi.debug.view" {
                inspected = Some(value.clone());
            }
            if operation.output().is_some() {
                last_output = Some(value.clone());
            }
            values.push(value);
        }

        let Some(value) = inspected.or(last_output) else {
            return Ok(SchedulerOutput::new(Box::new([]), Box::new([])));
        };
        let result_id = ResultId::from_existing(
            run_id
                .get()
                .checked_add(1)
                .ok_or(KernelExecutionError::Failed)?,
        );
        let result = ReadyResult::from_scheduler(
            result_id,
            StoredResult::Runtime(value),
            package
                .plan()
                .operations()
                .iter()
                .rev()
                .find(|operation| operation.output().is_some())
                .map(|operation| operation.result_category())
                .unwrap_or(crate::plan::ResultCategory::Value),
        );
        let observation_intents = if package
            .plan()
            .operations()
            .iter()
            .any(|operation| operation.kind().as_str() == "yssbi.debug.view")
        {
            package
                .plan()
                .operations()
                .iter()
                .find(|operation| operation.kind().as_str() == "yssbi.debug.view")
                .map(|operation| {
                    vec![ResultObservationIntent {
                        result_id,
                        requester: operation.source().clone(),
                    }]
                    .into_boxed_slice()
                })
                .unwrap_or_default()
        } else {
            Box::new([])
        };
        Ok(SchedulerOutput::new(
            vec![result].into_boxed_slice(),
            observation_intents,
        ))
    }
}

fn check_kernel_control(control: &RunExecutionControl) -> Result<(), KernelExecutionError> {
    if control.cancellation.load(Ordering::Acquire) {
        return Err(KernelExecutionError::Cancelled);
    }
    if Instant::now() >= control.deadline {
        return Err(KernelExecutionError::DeadlineExceeded);
    }
    Ok(())
}

fn parameter_value(
    value: &crate::plan::PlanParameterValue,
    resources: &PreparedRunResources,
) -> Result<RuntimeValue, KernelExecutionError> {
    match value {
        crate::plan::PlanParameterValue::Scalar(scalar) => Ok(match scalar {
            crate::plan::PlanParameterScalar::Null => RuntimeValue::Null,
            crate::plan::PlanParameterScalar::Bool(value) => RuntimeValue::Bool(*value),
            crate::plan::PlanParameterScalar::Integer(value) => RuntimeValue::Integer(*value),
            crate::plan::PlanParameterScalar::Unsigned(value) => RuntimeValue::Unsigned(*value),
            crate::plan::PlanParameterScalar::Decimal(value) => {
                RuntimeValue::Decimal(value.value())
            }
            crate::plan::PlanParameterScalar::String(value) => RuntimeValue::String(value.clone()),
        }),
        crate::plan::PlanParameterValue::Resource(resource) => resources
            .value(resource)
            .cloned()
            .ok_or(KernelExecutionError::Failed),
        crate::plan::PlanParameterValue::List(values) => values
            .iter()
            .map(|value| parameter_value(value, resources))
            .collect::<Result<Vec<_>, _>>()
            .map(|values| RuntimeValue::List(values.into_boxed_slice())),
        crate::plan::PlanParameterValue::Record(fields) => fields
            .iter()
            .map(|(field, value)| {
                Ok((
                    field.as_str().to_owned().into_boxed_str(),
                    parameter_value(value, resources)?,
                ))
            })
            .collect::<Result<std::collections::BTreeMap<_, _>, KernelExecutionError>>()
            .map(RuntimeValue::Record),
    }
}

fn execute_operation(
    kind: &str,
    inputs: &[RuntimeValue],
    parameter: Option<&crate::plan::PlanParameterValue>,
    resources: &PreparedRunResources,
) -> Result<RuntimeValue, KernelExecutionError> {
    match kind {
        "yssbi.constant.bool"
        | "yssbi.constant.int64"
        | "yssbi.constant.float64"
        | "yssbi.constant.string" => parameter
            .map(|value| parameter_value(value, resources))
            .transpose()?
            .ok_or(KernelExecutionError::Failed),
        "yssbi.project.variable.get" => {
            let Some(crate::plan::PlanParameterValue::Resource(resource)) = parameter else {
                return Err(KernelExecutionError::Failed);
            };
            resources
                .value(resource)
                .cloned()
                .ok_or(KernelExecutionError::Failed)
        }
        "yssbi.numeric.add.int64" | "yssbi.numeric.add.float64" => {
            binary_numeric(inputs, |left, right| left + right)
        }
        "yssbi.numeric.subtract.int64" | "yssbi.numeric.subtract.float64" => {
            binary_numeric(inputs, |left, right| left - right)
        }
        "yssbi.numeric.multiply.int64" | "yssbi.numeric.multiply.float64" => {
            binary_numeric(inputs, |left, right| left * right)
        }
        "yssbi.numeric.divide.int64" | "yssbi.numeric.divide.float64" => {
            binary_numeric(inputs, |left, right| left / right)
        }
        "yssbi.logic.and" => binary_bool(inputs, |left, right| left && right),
        "yssbi.logic.or" => binary_bool(inputs, |left, right| left || right),
        "yssbi.logic.not" => unary_bool(inputs, |value| !value),
        "yssbi.compare.equal" => Ok(RuntimeValue::Bool(inputs.first() == inputs.get(1))),
        "yssbi.compare.not_equal" => Ok(RuntimeValue::Bool(inputs.first() != inputs.get(1))),
        "yssbi.compare.less"
        | "yssbi.compare.less_equal"
        | "yssbi.compare.greater"
        | "yssbi.compare.greater_equal" => compare_numeric(kind, inputs),
        "yssbi.value.convert" | "yssbi.debug.print" => {
            inputs.first().cloned().ok_or(KernelExecutionError::Failed)
        }
        "yssbi.debug.view"
        | "yssbi.project.event.begin"
        | "yssbi.project.function.entry"
        | "yssbi.project.function.return"
        | "yssbi.project.function.call"
        | "yssbi.project.variable.set"
        | "yssbi.control.branch"
        | "yssbi.control.sequence"
        | "yssbi.control.loop"
        | "yssbi.control.do"
        | "yssbi.control.merge"
        | "yssbi.control.sleep"
        | "yssbi.reroute.data"
        | "yssbi.reroute.control"
        | "yssbi.reroute.effect" => Ok(inputs.first().cloned().unwrap_or(RuntimeValue::Null)),
        _ => Err(KernelExecutionError::Failed),
    }
}

fn numeric_input(value: Option<&RuntimeValue>) -> Result<f64, KernelExecutionError> {
    match value {
        Some(RuntimeValue::Integer(value)) => Ok(*value as f64),
        Some(RuntimeValue::Unsigned(value)) => Ok(*value as f64),
        Some(RuntimeValue::Decimal(value)) if value.is_finite() => Ok(*value),
        _ => Err(KernelExecutionError::Failed),
    }
}

fn binary_numeric(
    inputs: &[RuntimeValue],
    operation: impl FnOnce(f64, f64) -> f64,
) -> Result<RuntimeValue, KernelExecutionError> {
    let value = operation(
        numeric_input(inputs.first())?,
        numeric_input(inputs.get(1))?,
    );
    value
        .is_finite()
        .then_some(RuntimeValue::Decimal(value))
        .ok_or(KernelExecutionError::Failed)
}

fn binary_bool(
    inputs: &[RuntimeValue],
    operation: impl FnOnce(bool, bool) -> bool,
) -> Result<RuntimeValue, KernelExecutionError> {
    let Some(RuntimeValue::Bool(left)) = inputs.first() else {
        return Err(KernelExecutionError::Failed);
    };
    let Some(RuntimeValue::Bool(right)) = inputs.get(1) else {
        return Err(KernelExecutionError::Failed);
    };
    Ok(RuntimeValue::Bool(operation(*left, *right)))
}

fn unary_bool(
    inputs: &[RuntimeValue],
    operation: impl FnOnce(bool) -> bool,
) -> Result<RuntimeValue, KernelExecutionError> {
    let Some(RuntimeValue::Bool(value)) = inputs.first() else {
        return Err(KernelExecutionError::Failed);
    };
    Ok(RuntimeValue::Bool(operation(*value)))
}

fn compare_numeric(
    kind: &str,
    inputs: &[RuntimeValue],
) -> Result<RuntimeValue, KernelExecutionError> {
    let left = numeric_input(inputs.first())?;
    let right = numeric_input(inputs.get(1))?;
    let value = match kind {
        "yssbi.compare.less" => left < right,
        "yssbi.compare.less_equal" => left <= right,
        "yssbi.compare.greater" => left > right,
        "yssbi.compare.greater_equal" => left >= right,
        _ => return Err(KernelExecutionError::Failed),
    };
    Ok(RuntimeValue::Bool(value))
}

struct RunLifecycleGuard<'a> {
    registry: &'a RunRegistry,
    run_id: crate::run_registry::RunId,
    terminal: bool,
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

impl<'a> RunLifecycleGuard<'a> {
    fn start(
        registry: &'a RunRegistry,
        run_id: crate::run_registry::RunId,
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

    fn begin_finalization(&mut self) -> Result<(), RunRegistryError> {
        self.registry
            .transition(self.run_id, RunState::Finalizing)?;
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
pub enum ExecutionAdmissionError {
    #[error("execution session admission is closed")]
    Closed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionDrainControl {
    deadline: Instant,
}

impl ExecutionDrainControl {
    pub const fn new(deadline: Instant) -> Self {
        Self { deadline }
    }

    pub(crate) const fn deadline(self) -> Instant {
        self.deadline
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ExecutionOutstandingWork {
    active: usize,
}

impl ExecutionOutstandingWork {
    const fn is_empty(self) -> bool {
        self.active == 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionDrainOutcome {
    Drained {
        outstanding: ExecutionOutstandingWork,
    },
    TimedOut {
        outstanding: ExecutionOutstandingWork,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionCancelOutcome {
    NotFound,
    AlreadyCancelled,
    AlreadyTerminal,
    Requested,
}

#[must_use = "an execution work lease releases session admission when dropped"]
pub struct ExecutionWorkLease {
    admission: Arc<(Mutex<RuntimeAdmission>, Condvar)>,
}

/// Session-local execution state. Composition installs one instance per
/// Application session and replaces it atomically with that session.
pub struct ExecutionRuntimeState {
    session_id: ExecutionSessionId,
    generation: RuntimeGeneration,
    admission: Arc<(Mutex<RuntimeAdmission>, Condvar)>,
    results: ResultStore,
    runs: RunRegistry,
    scientific_backend: Arc<dyn ScientificBackend>,
    executor: Arc<dyn PreparedPlanExecutor>,
    active_controls: Mutex<BTreeMap<crate::run_registry::RunId, Arc<AtomicBool>>>,
}

impl ExecutionRuntimeState {
    #[cfg(any(test, feature = "test-support"))]
    pub fn new(session_id: ExecutionSessionId, generation: RuntimeGeneration) -> Self {
        Self::from_composition(
            session_id,
            generation,
            Arc::new(UnavailableScientificBackend),
        )
    }

    pub fn from_composition(
        session_id: ExecutionSessionId,
        generation: RuntimeGeneration,
        scientific_backend: Arc<dyn ScientificBackend>,
    ) -> Self {
        Self {
            session_id,
            generation,
            admission: Arc::new((Mutex::new(RuntimeAdmission::default()), Condvar::new())),
            results: ResultStore::new(),
            runs: RunRegistry::new(),
            scientific_backend,
            executor: Arc::new(NeutralPlanExecutor),
            active_controls: Mutex::new(BTreeMap::new()),
        }
    }

    pub fn session_id(&self) -> ExecutionSessionId {
        self.session_id
    }

    pub fn generation(&self) -> RuntimeGeneration {
        self.generation
    }

    pub fn scientific_backend(&self) -> &dyn ScientificBackend {
        self.scientific_backend.as_ref()
    }

    pub fn close_admission(&self) {
        let (state, _) = &*self.admission;
        state.lock().unwrap_or_else(PoisonError::into_inner).closed = true;
    }

    pub fn is_admission_closed(&self) -> bool {
        let (state, _) = &*self.admission;
        state.lock().unwrap_or_else(PoisonError::into_inner).closed
    }

    #[cfg(test)]
    pub(crate) fn results(&self) -> &ResultStore {
        &self.results
    }

    pub fn query_result(&self, result_id: ResultId) -> Option<Arc<StoredResult>> {
        self.results.get(result_id)
    }

    pub fn query_pin_result_history(
        &self,
        output: &crate::plan::PlanOutputRef,
    ) -> Result<Box<[PinResultHistorySnapshot]>, ExecutionResultQueryError> {
        self.results.query_pin_result_history(output)
    }

    pub fn runs(&self) -> &RunRegistry {
        &self.runs
    }

    #[cfg(test)]
    fn execute_prepared(
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
            Some(self.executor.as_ref()),
            None,
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
        mut on_run_started: impl FnMut(crate::run_registry::RunId),
    ) -> Result<ExecutedPreparedRun, ExecutePreparedError> {
        self.execute_prepared_inner(
            plan,
            bindings,
            resources,
            control,
            Some(self.executor.as_ref()),
            Some(&mut on_run_started),
        )
        .map(ExecutedPreparedCandidate::into_executed_run)
    }

    fn execute_prepared_inner(
        &self,
        plan: &PreparedExecutionPlan,
        bindings: RunResourceBindings,
        resources: &ResourceProviderFactory,
        control: &RunExecutionControl,
        executor: Option<&dyn PreparedPlanExecutor>,
        mut on_run_started: Option<&mut dyn FnMut(crate::run_registry::RunId)>,
    ) -> Result<ExecutedPreparedCandidate, ExecutePreparedError> {
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
        self.active_controls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(run_id, Arc::clone(&control.cancellation));
        if let Some(on_run_started) = on_run_started.as_deref_mut() {
            on_run_started(run_id);
        }
        if let Err(error) = control.check(RunPhase::Execution) {
            let result = terminate_run(&mut lifecycle, run_id, error);
            self.remove_active_control(run_id);
            return result;
        }

        let Some(executor) = executor else {
            let result = terminate_run(
                &mut lifecycle,
                run_id,
                ExecutePreparedError::KernelUnavailable,
            );
            self.remove_active_control(run_id);
            return result;
        };
        let output = match executor.execute(
            plan.package(),
            bindings.bindings(),
            &prepared_resources,
            control,
            run_id,
        ) {
            Ok(output) => output,
            Err(KernelExecutionError::Cancelled) => {
                let result = terminate_run(
                    &mut lifecycle,
                    run_id,
                    ExecutePreparedError::Cancelled {
                        phase: RunPhase::Execution,
                    },
                );
                self.remove_active_control(run_id);
                return result;
            }
            Err(KernelExecutionError::DeadlineExceeded) => {
                let result = terminate_run(
                    &mut lifecycle,
                    run_id,
                    ExecutePreparedError::DeadlineExceeded {
                        phase: RunPhase::Execution,
                    },
                );
                self.remove_active_control(run_id);
                return result;
            }
            Err(error) => {
                let result =
                    terminate_run(&mut lifecycle, run_id, ExecutePreparedError::Kernel(error));
                self.remove_active_control(run_id);
                return result;
            }
        };
        if let Err(error) = control.check(RunPhase::Finalization) {
            let result = terminate_run(&mut lifecycle, run_id, error);
            self.remove_active_control(run_id);
            return result;
        }

        let grants = prepared_resources.finish();
        let candidate = SuccessfulExecutionCandidate::from_scheduler(
            output.results,
            output.observation_intents,
            grants,
        );
        lifecycle
            .begin_finalization()
            .map_err(ExecutePreparedError::RunRegistry)?;
        self.remove_active_control(run_id);
        Ok(ExecutedPreparedCandidate { run_id, candidate })
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
        let executed =
            self.execute_prepared_inner(plan, bindings, resources, control, Some(executor), None)?;
        self.finalize_run_success(executed.run_id)
            .map_err(ExecutePreparedError::RunRegistry)?;
        Ok(executed.candidate())
    }

    fn remove_active_control(&self, run_id: crate::run_registry::RunId) {
        self.active_controls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&run_id);
    }

    pub fn cancel_run(&self, run_id: crate::run_registry::RunId) -> ExecutionCancelOutcome {
        match self.runs.state(run_id) {
            None => ExecutionCancelOutcome::NotFound,
            Some(RunState::Cancelled) => ExecutionCancelOutcome::AlreadyCancelled,
            Some(RunState::Succeeded | RunState::Failed) => ExecutionCancelOutcome::AlreadyTerminal,
            Some(RunState::Admitted | RunState::Running | RunState::Finalizing) => {
                if let Some(control) = self
                    .active_controls
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .get(&run_id)
                    .cloned()
                {
                    control.store(true, Ordering::Release);
                }
                ExecutionCancelOutcome::Requested
            }
        }
    }

    pub fn publish_committed_results(&self, handoff: &ExecutionFinalizationHandoff) {
        for result in handoff.results() {
            self.results
                .publish(result.result_id(), result.value().clone());
        }
    }

    pub fn finalize_run_success(
        &self,
        run_id: crate::run_registry::RunId,
    ) -> Result<(), RunRegistryError> {
        self.runs.transition(run_id, RunState::Succeeded)
    }

    pub fn finalize_run_failure(
        &self,
        run_id: crate::run_registry::RunId,
    ) -> Result<(), RunRegistryError> {
        self.runs.transition(run_id, RunState::Failed)
    }

    pub fn finalize_run_cancelled(
        &self,
        run_id: crate::run_registry::RunId,
    ) -> Result<(), RunRegistryError> {
        self.runs.transition(run_id, RunState::Cancelled)
    }

    pub fn admit(&self) -> Result<ExecutionWorkLease, ExecutionAdmissionError> {
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

    pub fn drain(&self, control: &ExecutionDrainControl) -> ExecutionDrainOutcome {
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

    pub fn cancel_and_drain(&self, control: &ExecutionDrainControl) -> ExecutionDrainOutcome {
        self.close_admission();
        self.drain(control)
    }
}

fn terminate_run(
    lifecycle: &mut RunLifecycleGuard<'_>,
    run_id: crate::run_registry::RunId,
    error: ExecutePreparedError,
) -> Result<ExecutedPreparedCandidate, ExecutePreparedError> {
    let transition = if matches!(&error, ExecutePreparedError::Cancelled { .. }) {
        lifecycle.cancel()
    } else {
        lifecycle.fail()
    };
    transition.map_err(ExecutePreparedError::RunRegistry)?;
    let _ = run_id;
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
    use crate::identity::ExecutionSessionId;
    use crate::package_preparation::PreparedExecutionPlan;
    use crate::plan::{
        CompiledExecutionPackage, CompiledFunctionBundle, CompiledParameterBundleBuilder,
        ExecutionPlan, PlanCompilationBasis, PlanCompileId, PlanGraphId, PlanGraphRevision,
        PlanProjectSessionId, PlanProvenance, PlanRegistryFingerprint, PlanResourceId,
        PlanResourceObservedState, PlanResourceRequirement, PlanResourceVersion,
        PlanSourceIdentity, ResourceAccess, ResourceKind,
    };
    use crate::resource_preparation::{RunResourceBinding, RunResourceBindings};
    use crate::result_store::{ResultId, StoredResult};
    use crate::run_registry::{RunId, RunState};
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
                crate::value::RuntimeValue::Integer(4),
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
            _run_id: RunId,
        ) -> Result<SchedulerOutput, KernelExecutionError> {
            assert_eq!(bindings.len(), 1);
            assert_eq!(
                resources.value(&PlanResourceId::from_existing("variables/answer".into())),
                Some(&crate::value::RuntimeValue::Integer(4))
            );
            Ok(SchedulerOutput::new(
                vec![ReadyResult::from_scheduler(
                    ResultId::from_existing(1),
                    StoredResult::Runtime(crate::value::RuntimeValue::Integer(5)),
                    crate::plan::ResultCategory::Value,
                )]
                .into_boxed_slice(),
                Box::new([]),
            ))
        }
    }

    fn state() -> ExecutionRuntimeState {
        ExecutionRuntimeState::new(
            ExecutionSessionId::new(uuid::Uuid::nil()),
            crate::identity::RuntimeGeneration::INITIAL,
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
    fn execute_prepared_uses_neutral_executor_without_publishing_empty_candidate() {
        let state = state();
        let plan = prepared_plan(&state);
        let candidate = state
            .execute_prepared(
                &plan,
                bindings(),
                &ResourceProviderFactory::new("session".into()),
                &RunExecutionControl::new(Instant::now() + Duration::from_secs(1)),
            )
            .expect("neutral executor accepts an empty plan");

        assert!(candidate.results().is_empty());
        assert_eq!(
            state.runs().state(RunId::from_existing(0)),
            Some(RunState::Succeeded)
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
            handoff.results()[0].value().value(),
            &StoredResult::Runtime(crate::value::RuntimeValue::Integer(5))
        );
        assert_eq!(
            handoff.results()[0].category(),
            crate::plan::ResultCategory::Value
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
