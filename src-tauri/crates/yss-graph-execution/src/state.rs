use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::time::{Instant, SystemTime, SystemTimeError, UNIX_EPOCH};
#[cfg(test)]
use yss_node_kernel::KernelId;

use thiserror::Error;

use crate::error::{RunFailure, RunFailureCode, RunPhase};
use crate::finalization::{
    ExecutionFinalizationHandoff, ReadyPinResult, ReadyResult, ResultObservationIntent,
    SuccessfulExecutionCandidate,
};
use crate::identity::{ExecutionSessionId, RuntimeGeneration};
use crate::kernel_invocation::parameter_value;
use crate::package_preparation::PreparedExecutionPlan;
use crate::resource_preparation::{
    PreparedRunResources, ResourcePreparationError, ResourceProviderFactory, RunResourceBindings,
    RunResourceRequest,
};
use crate::result::{
    GraphResultCacheState, GraphResultInputs, ResultId, ResultProvenance, ResultRunBasis,
    StoredResult, StoredResultSnapshot,
};
use crate::result_store::ResultStore;
use crate::run_registry::RunRegistry;
use crate::run_registry::{RunRegistryError, RunState};
use yss_node_kernel::RuntimeValue;
use yss_node_kernel::{KernelControl, KernelError, KernelRegistry};

#[derive(Clone)]
pub struct RunExecutionControl {
    pub(crate) cancellation: Arc<AtomicBool>,
    pub(crate) deadline: Instant,
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
    #[error("prepared execution belongs to another kernel capability version")]
    KernelCapabilitiesChanged,
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
    #[error("prepared execution kernel failed")]
    Kernel(#[source] OperationExecutionError),
    #[error("execution result identity space is exhausted")]
    ResultIdentityExhausted,
    #[error("execution result timestamp is unavailable")]
    ResultTimestamp(#[source] SystemTimeError),
}

#[derive(Debug, Error)]
pub enum OperationExecutionError {
    #[error(transparent)]
    Kernel(#[from] KernelError),
    #[error("prepared graph execution contains inconsistent values")]
    Failed,
    #[error("requested graph output is unavailable in the prepared plan")]
    DemandOutputUnavailable,
    #[error("node execution failed")]
    AtNode {
        source: crate::plan::PlanSourceIdentity,
        #[source]
        error: Box<OperationExecutionError>,
    },
}

impl OperationExecutionError {
    fn at_node(self, source: &crate::plan::PlanSourceIdentity) -> Self {
        match self {
            Self::Kernel(KernelError::Cancelled | KernelError::DeadlineExceeded)
            | Self::AtNode { .. } => self,
            error => Self::AtNode {
                source: source.clone(),
                error: Box::new(error),
            },
        }
    }

    fn failure(&self) -> RunFailure {
        let code = match self {
            Self::AtNode { source, error } => {
                return RunFailure {
                    source: Some(source.clone()),
                    ..error.failure()
                };
            }
            Self::Kernel(KernelError::ShapeMismatch) => RunFailureCode::ShapeMismatch,
            Self::Kernel(KernelError::InvalidParameter) => RunFailureCode::InvalidParameter,
            Self::Kernel(KernelError::UnalignedSeries) => RunFailureCode::UnalignedSeries,
            Self::Kernel(KernelError::BudgetExceeded) => RunFailureCode::BudgetExceeded,
            Self::Kernel(KernelError::InputLayoutMismatch) => RunFailureCode::InputLayoutMismatch,
            Self::Kernel(KernelError::OutputContractMismatch) => {
                RunFailureCode::OutputContractMismatch
            }
            Self::Kernel(KernelError::ScientificFailure) => RunFailureCode::ScientificFailure,
            Self::Kernel(KernelError::DivisionByZero) => RunFailureCode::DivisionByZero,
            Self::Kernel(KernelError::NonFiniteResult) => RunFailureCode::NonFiniteResult,
            Self::Kernel(KernelError::InvalidNumericInput) => RunFailureCode::InvalidNumericInput,
            Self::Kernel(KernelError::KernelNotFound) => RunFailureCode::KernelNotFound,
            Self::Kernel(KernelError::DeadlineExceeded) => RunFailureCode::DeadlineExceeded,
            _ => RunFailureCode::KernelFailed,
        };
        RunFailure {
            code,
            phase: RunPhase::Execution,
            source: None,
        }
    }
}

impl ExecutePreparedError {
    pub fn failure(&self) -> RunFailure {
        let (code, phase) = match self {
            Self::Kernel(error) => return error.failure(),
            Self::DeadlineExceeded { phase } => (RunFailureCode::DeadlineExceeded, *phase),
            Self::ResourcePreparation(_) => (
                RunFailureCode::ResourceUnavailable,
                RunPhase::ResourcePreparation,
            ),
            Self::ResultIdentityExhausted | Self::ResultTimestamp(_) => {
                (RunFailureCode::FinalizationFailed, RunPhase::Finalization)
            }
            _ => (RunFailureCode::KernelFailed, RunPhase::Execution),
        };
        RunFailure {
            code,
            phase,
            source: None,
        }
    }
}

#[derive(Debug)]
struct SchedulerOutput {
    results: Box<[SchedulerResult]>,
    observations: Box<[SchedulerObservation]>,
}

impl SchedulerOutput {
    fn new(results: Box<[SchedulerResult]>, observations: Box<[SchedulerObservation]>) -> Self {
        Self {
            results,
            observations,
        }
    }
}

#[derive(Debug)]
struct SchedulerResult {
    value: StoredResult,
    category: crate::plan::ResultCategory,
    output: crate::plan::PlanOutputRef,
}

#[derive(Debug)]
struct SchedulerObservation {
    output: crate::plan::PlanOutputRef,
    requester: crate::plan::PlanSourceIdentity,
}

struct PreparedPlanExecution<'a> {
    package: &'a crate::plan::ExecutionPlanPackage,
    bindings: &'a [crate::resource_preparation::RunResourceBinding],
    resources: &'a PreparedRunResources,
    control: &'a RunExecutionControl,
    run_id: crate::run_registry::RunId,
    producers: &'a [Option<usize>],
    selection: ExecutionSelection,
}

struct PreparedExecutionDispatch<'a> {
    demand: &'a crate::plan::PlanExecutionDemand,
    result_basis: Option<&'a ResultRunBasis>,
    executor: &'a dyn PreparedPlanExecutor,
    on_event: Option<&'a mut dyn FnMut(PreparedExecutionEvent)>,
}

trait PreparedPlanExecutor: Send + Sync {
    fn execute(
        &self,
        execution: PreparedPlanExecution<'_>,
    ) -> Result<SchedulerOutput, OperationExecutionError>;
}

struct NeutralPlanExecutor {
    kernels: Arc<KernelRegistry>,
    relations: Arc<dyn yss_relational_contract::RelationFactory>,
}

impl PreparedPlanExecutor for NeutralPlanExecutor {
    fn execute(
        &self,
        execution: PreparedPlanExecution<'_>,
    ) -> Result<SchedulerOutput, OperationExecutionError> {
        let PreparedPlanExecution {
            package,
            bindings: _bindings,
            resources,
            control,
            run_id: _run_id,
            producers,
            selection,
        } = execution;
        let operations = package.plan().operations();
        let mut values: Vec<Option<RuntimeValue>> = vec![None; producers.len()];
        let mut remaining_dependencies = vec![0usize; operations.len()];
        let mut dependents = vec![Vec::new(); operations.len()];
        for (operation_index, operation) in operations.iter().enumerate() {
            if !selection.required_operations[operation_index] {
                continue;
            }
            for binding in operation.inputs() {
                let crate::plan::PlanInputSource::Value(reference) = binding.source() else {
                    continue;
                };
                let Some(producer) = producers
                    .get(reference.index() as usize)
                    .and_then(|producer| *producer)
                else {
                    return Err(OperationExecutionError::Failed);
                };
                remaining_dependencies[operation_index] = remaining_dependencies[operation_index]
                    .checked_add(1)
                    .ok_or(OperationExecutionError::Failed)?;
                dependents[producer].push(operation_index);
            }
        }
        let mut ready = remaining_dependencies
            .iter()
            .enumerate()
            .filter_map(|(operation_index, remaining)| {
                (selection.required_operations[operation_index] && *remaining == 0)
                    .then_some(operation_index)
            })
            .collect::<VecDeque<_>>();
        let kernel_control =
            KernelControl::new(Arc::clone(&control.cancellation), control.deadline);
        let mut completed_count = 0usize;
        let mut results = Vec::new();
        while let Some(operation_index) = ready.pop_front() {
            let operation = &operations[operation_index];
            check_execution_control(control)?;
            let inputs = operation
                .inputs()
                .iter()
                .map(|binding| {
                    let value = match binding.source() {
                        crate::plan::PlanInputSource::Value(reference) => values
                            .get(reference.index() as usize)
                            .and_then(Option::as_ref)
                            .cloned()
                            .ok_or(OperationExecutionError::Failed),
                        crate::plan::PlanInputSource::Parameter(handle) => {
                            let Some(payload) = package.parameters().entries().get(handle) else {
                                return Err(OperationExecutionError::Failed);
                            };
                            parameter_value(payload.value(), resources)
                                .map(std::borrow::Cow::into_owned)
                                .map_err(OperationExecutionError::from)
                        }
                    }?;
                    apply_input_coercions(value, &binding.contract().coercions)
                })
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| error.at_node(operation.source()))?;

            let output_values = crate::kernel_invocation::invoke(
                &self.kernels,
                &self.relations,
                operation,
                &inputs,
                package.parameters(),
                resources,
                &kernel_control,
            )
            .map_err(|error| OperationExecutionError::from(error).at_node(operation.source()))?;
            if output_values.len() != operation.outputs().len() {
                return Err(OperationExecutionError::Failed);
            }
            for (output, value) in operation.outputs().iter().zip(output_values) {
                let Some(slot) = values.get_mut(output.value().index() as usize) else {
                    return Err(OperationExecutionError::Failed);
                };
                if slot.replace(value.clone()).is_some() {
                    return Err(OperationExecutionError::Failed);
                }
                results.push(SchedulerResult {
                    value: StoredResult::new(value).with_output_contract(output.contract().clone()),
                    category: output.contract().category,
                    output: output.output().clone(),
                });
            }
            completed_count = completed_count
                .checked_add(1)
                .ok_or(OperationExecutionError::Failed)?;
            for dependent in &dependents[operation_index] {
                let remaining = &mut remaining_dependencies[*dependent];
                *remaining = remaining
                    .checked_sub(1)
                    .ok_or(OperationExecutionError::Failed)?;
                if *remaining == 0 {
                    ready.push_back(*dependent);
                }
            }
        }
        if completed_count
            != selection
                .required_operations
                .iter()
                .filter(|required| **required)
                .count()
        {
            return Err(OperationExecutionError::Failed);
        }

        let observations = selection
            .observations
            .into_iter()
            .map(|selected| {
                let crate::plan::PlanInputSource::Value(value) = selected.source else {
                    return Err(OperationExecutionError::Failed);
                };
                let producer = producers
                    .get(value.index() as usize)
                    .and_then(|producer| *producer)
                    .ok_or(OperationExecutionError::Failed)?;
                let output = operations[producer]
                    .outputs()
                    .iter()
                    .find(|output| output.value() == value)
                    .map(|output| output.output().clone())
                    .ok_or(OperationExecutionError::Failed)?;
                Ok(SchedulerObservation {
                    output,
                    requester: selected.requester,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(SchedulerOutput::new(
            results.into_boxed_slice(),
            observations.into_boxed_slice(),
        ))
    }
}

struct SelectedObservation {
    source: crate::plan::PlanInputSource,
    requester: crate::plan::PlanSourceIdentity,
}

struct ExecutionSelection {
    required_operations: Vec<bool>,
    observations: Vec<SelectedObservation>,
}

fn execution_producers(
    package: &crate::plan::ExecutionPlanPackage,
) -> Result<Vec<Option<usize>>, OperationExecutionError> {
    let operations = package.plan().operations();
    let value_count = operations
        .iter()
        .flat_map(|operation| operation.outputs())
        .map(|output| output.value().index() as usize)
        .max()
        .map_or(0, |max| max + 1);
    let mut producers = vec![None; value_count];
    for (operation_index, operation) in operations.iter().enumerate() {
        for output in operation.outputs() {
            let Some(producer) = producers.get_mut(output.value().index() as usize) else {
                return Err(OperationExecutionError::Failed);
            };
            if producer.replace(operation_index).is_some() {
                return Err(OperationExecutionError::Failed);
            }
        }
    }
    Ok(producers)
}

fn select_execution(
    package: &crate::plan::ExecutionPlanPackage,
    demand: &crate::plan::PlanExecutionDemand,
    producers: &[Option<usize>],
) -> Result<ExecutionSelection, OperationExecutionError> {
    let operations = package.plan().operations();
    let consumed = operations
        .iter()
        .flat_map(|operation| operation.inputs())
        .filter_map(|binding| match binding.source() {
            crate::plan::PlanInputSource::Value(value) => Some(*value),
            crate::plan::PlanInputSource::Parameter(_) => None,
        })
        .collect::<std::collections::BTreeSet<_>>();
    let include_defaults = matches!(demand, crate::plan::PlanExecutionDemand::Default)
        || matches!(
            demand,
            crate::plan::PlanExecutionDemand::Outputs {
                include_default_results: true,
                ..
            }
        );
    let mut selected = BTreeMap::new();
    if include_defaults {
        for operation in operations {
            for output in operation.outputs() {
                if !consumed.contains(&output.value()) {
                    selected.insert(
                        output.output().clone(),
                        (output.value(), output.contract().category),
                    );
                }
            }
        }
    }
    if let crate::plan::PlanExecutionDemand::Outputs { outputs, .. } = demand {
        for requested in outputs {
            let Some((value, category)) = operations.iter().find_map(|operation| {
                operation
                    .outputs()
                    .iter()
                    .find(|output| output.output() == requested)
                    .map(|output| (output.value(), output.contract().category))
            }) else {
                return Err(OperationExecutionError::DemandOutputUnavailable);
            };
            selected.insert(requested.clone(), (value, category));
        }
    }
    let observations = if include_defaults {
        operations
            .iter()
            .flat_map(|operation| {
                operation
                    .observation_intents()
                    .iter()
                    .map(|intent| match intent {
                        crate::plan::PlanObservationIntent::InspectInput { source } => {
                            SelectedObservation {
                                source: source.clone(),
                                requester: operation.source().clone(),
                            }
                        }
                    })
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    if include_defaults && !operations.is_empty() && selected.is_empty() && observations.is_empty()
    {
        return Err(OperationExecutionError::Failed);
    }

    let mut required_operations = vec![false; operations.len()];
    let mut pending = selected
        .values()
        .map(|(value, _)| *value)
        .chain(
            observations
                .iter()
                .filter_map(|observation| match &observation.source {
                    crate::plan::PlanInputSource::Value(value) => Some(*value),
                    crate::plan::PlanInputSource::Parameter(_) => None,
                }),
        )
        .collect::<Vec<_>>();
    while let Some(value) = pending.pop() {
        let Some(producer) = producers
            .get(value.index() as usize)
            .and_then(|producer| *producer)
        else {
            return Err(OperationExecutionError::Failed);
        };
        if std::mem::replace(&mut required_operations[producer], true) {
            continue;
        }
        pending.extend(operations[producer].inputs().iter().filter_map(|binding| {
            match binding.source() {
                crate::plan::PlanInputSource::Value(value) => Some(*value),
                crate::plan::PlanInputSource::Parameter(_) => None,
            }
        }));
    }

    Ok(ExecutionSelection {
        required_operations,
        observations,
    })
}

pub(crate) fn check_execution_control(
    control: &RunExecutionControl,
) -> Result<(), OperationExecutionError> {
    if control.cancellation.load(Ordering::Acquire) {
        return Err(OperationExecutionError::Kernel(KernelError::Cancelled));
    }
    if Instant::now() >= control.deadline {
        return Err(OperationExecutionError::Kernel(
            KernelError::DeadlineExceeded,
        ));
    }
    Ok(())
}

fn apply_input_coercions(
    mut value: RuntimeValue,
    coercions: &[crate::plan::PlanInputCoercionKind],
) -> Result<RuntimeValue, OperationExecutionError> {
    for coercion in coercions {
        value = match coercion {
            // Broadcast is a kernel-owned shape operation. Keeping the scalar
            // value here makes the coercion explicit without fabricating a
            // DataSeries length in the scheduler.
            crate::plan::PlanInputCoercionKind::BroadcastScalarToSeries => value,
        };
    }
    Ok(value)
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
    pub(crate) graph_plans: crate::graph_preparation::GraphPlanCache,
    session_id: ExecutionSessionId,
    generation: RuntimeGeneration,
    admission: Arc<(Mutex<RuntimeAdmission>, Condvar)>,
    results: ResultStore,
    runs: RunRegistry,
    executor: NeutralPlanExecutor,
    active_controls: Mutex<BTreeMap<crate::run_registry::RunId, Arc<AtomicBool>>>,
    next_result_id: AtomicU64,
}

impl ExecutionRuntimeState {
    pub fn new(
        session_id: ExecutionSessionId,
        generation: RuntimeGeneration,
        kernels: Arc<KernelRegistry>,
        relations: Arc<dyn yss_relational_contract::RelationFactory>,
    ) -> Self {
        Self {
            graph_plans: crate::graph_preparation::GraphPlanCache::default(),
            session_id,
            generation,
            admission: Arc::new((Mutex::new(RuntimeAdmission::default()), Condvar::new())),
            results: ResultStore::new(),
            runs: RunRegistry::new(),
            executor: NeutralPlanExecutor { kernels, relations },
            active_controls: Mutex::new(BTreeMap::new()),
            next_result_id: AtomicU64::new(1),
        }
    }

    pub fn kernels(&self) -> &KernelRegistry {
        &self.executor.kernels
    }

    pub fn session_id(&self) -> ExecutionSessionId {
        self.session_id
    }

    pub fn generation(&self) -> RuntimeGeneration {
        self.generation
    }

    pub fn close_admission(&self) {
        let (state, _) = &*self.admission;
        state.lock().unwrap_or_else(PoisonError::into_inner).closed = true;
    }

    pub fn is_admission_closed(&self) -> bool {
        let (state, _) = &*self.admission;
        state.lock().unwrap_or_else(PoisonError::into_inner).closed
    }

    pub fn query_result(&self, result_id: ResultId) -> Option<StoredResultSnapshot> {
        self.results.get(result_id)
    }

    pub fn query_graph_results(&self, graph: &str, limit: usize) -> Vec<StoredResultSnapshot> {
        self.results.query_graph_results(graph, limit)
    }

    pub fn query_pin_result(
        &self,
        output: &crate::plan::PlanOutputRef,
    ) -> Option<StoredResultSnapshot> {
        self.results.query_pin_result(output)
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
        demand: &crate::plan::PlanExecutionDemand,
        result_basis: Option<&ResultRunBasis>,
        mut on_event: impl FnMut(PreparedExecutionEvent),
    ) -> Result<ExecutedPreparedRun, ExecutePreparedError> {
        self.execute_prepared_inner(
            plan,
            bindings,
            resources,
            control,
            PreparedExecutionDispatch {
                demand,
                result_basis,
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
        let selection = select_execution(plan.package(), demand, &producers)
            .map_err(ExecutePreparedError::Kernel)?;
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
            .runs
            .admit_next()
            .map_err(ExecutePreparedError::RunRegistry)?;
        let mut lifecycle = RunLifecycleGuard::start(&self.runs, run_id)
            .map_err(ExecutePreparedError::RunRegistry)?;
        self.active_controls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(run_id, Arc::clone(&control.cancellation));
        if !self.results.begin_run(run_id, &outputs, result_basis) {
            let result = terminate_run(
                &mut lifecycle,
                run_id,
                ExecutePreparedError::Cancelled {
                    phase: RunPhase::Admission,
                },
            );
            self.remove_active_control(run_id);
            return result;
        }
        if let Some(on_event) = on_event.as_mut() {
            on_event(PreparedExecutionEvent::RunStarted { run_id, outputs });
        }
        let request = RunResourceRequest::new(plan, &bindings);
        let prepared_resources = match resources.prepare(&request) {
            Ok(resources) => resources,
            Err(error) => {
                let result = terminate_run(
                    &mut lifecycle,
                    run_id,
                    ExecutePreparedError::ResourcePreparation(error),
                );
                self.remove_active_control(run_id);
                return result;
            }
        };
        if let Err(error) = control.check(RunPhase::Execution) {
            let result = terminate_run(&mut lifecycle, run_id, error);
            self.remove_active_control(run_id);
            return result;
        }

        let output = match executor.execute(PreparedPlanExecution {
            package: plan.package(),
            bindings: bindings.bindings(),
            resources: &prepared_resources,
            control,
            run_id,
            producers: &producers,
            selection,
        }) {
            Ok(output) => output,
            Err(OperationExecutionError::Kernel(KernelError::Cancelled)) => {
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
            Err(OperationExecutionError::Kernel(KernelError::DeadlineExceeded)) => {
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

        let mut results = Vec::with_capacity(output.results.len());
        let mut observation_intents = Vec::new();
        let mut result_ids_by_output = BTreeMap::new();
        for scheduled in output.results {
            let result_id = match self.allocate_result_id() {
                Ok(result_id) => result_id,
                Err(error) => {
                    let result = terminate_run(&mut lifecycle, run_id, error);
                    self.remove_active_control(run_id);
                    return result;
                }
            };
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
        for observation in output.observations {
            let Some(result_id) = result_ids_by_output.get(&observation.output).copied() else {
                let result = terminate_run(
                    &mut lifecycle,
                    run_id,
                    ExecutePreparedError::Kernel(OperationExecutionError::Failed),
                );
                self.remove_active_control(run_id);
                return result;
            };
            observation_intents.push(ResultObservationIntent {
                result_id,
                requester: observation.requester,
            });
        }
        let grants = prepared_resources.finish();
        let candidate = SuccessfulExecutionCandidate::from_scheduler(
            results.into_boxed_slice(),
            observation_intents.into_boxed_slice(),
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

    fn remove_active_control(&self, run_id: crate::run_registry::RunId) {
        self.active_controls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&run_id);
    }

    fn allocate_result_id(&self) -> Result<ResultId, ExecutePreparedError> {
        self.next_result_id
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                current.checked_add(1)
            })
            .map(ResultId::from_existing)
            .map_err(|_| ExecutePreparedError::ResultIdentityExhausted)
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

    pub fn observe_graph_result_inputs(&self, graph: &str, inputs: GraphResultInputs) {
        self.results.observe_graph_inputs(graph, inputs);
    }

    pub fn capture_result_run_basis(
        &self,
        graph: &str,
        inputs: GraphResultInputs,
    ) -> Option<ResultRunBasis> {
        self.results.capture_run_basis(graph, inputs)
    }

    pub fn query_result_cache_states(
        &self,
        graph: &str,
        semantic_input_hash: &[u8; 32],
    ) -> Option<GraphResultCacheState> {
        self.results.query_cache_states(graph, semantic_input_hash)
    }

    pub fn result_resource_keys(&self, graph: &str) -> std::collections::BTreeSet<Box<str>> {
        self.results.resource_keys(graph)
    }

    pub fn observe_result_resource_versions(
        &self,
        versions: &BTreeMap<Box<str>, Option<[u8; 32]>>,
    ) {
        self.results.observe_resource_versions(versions);
    }

    pub fn invalidate_graph_results(&self, graph: &str) {
        self.graph_plans.remove(graph);
        self.results.invalidate_graph(graph);
    }

    pub fn retain_result(
        &self,
        result_id: ResultId,
        lease_id: uuid::Uuid,
        owner: &str,
        handoff: Option<&str>,
    ) -> Result<StoredResultSnapshot, crate::result::ResultRetentionError> {
        self.results.retain(result_id, lease_id, owner, handoff)
    }

    pub fn claim_result_lease(
        &self,
        lease_id: uuid::Uuid,
        owner: &str,
    ) -> Result<StoredResultSnapshot, crate::result::ResultRetentionError> {
        self.results.claim(lease_id, owner)
    }

    pub fn release_result_lease(
        &self,
        lease_id: uuid::Uuid,
        owner: &str,
    ) -> Result<(), crate::result::ResultRetentionError> {
        self.results.release(lease_id, owner)
    }

    pub fn reconcile_result_leases(
        &self,
        owner: &str,
        active: &std::collections::BTreeSet<uuid::Uuid>,
    ) {
        self.results.reconcile(owner, active);
    }

    pub fn close_result_owner(&self, owner: &str) {
        self.results.close_owner(owner);
    }

    pub fn publish_committed_results(&self, handoff: &ExecutionFinalizationHandoff) -> bool {
        self.results
            .publish(handoff.results(), handoff.observation_intents())
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
        ExecutionPlan, ExecutionPlanPackage, PlanBasis, PlanExecutionDemand, PlanGraphId, PlanId,
        PlanInputBinding, PlanInputSource, PlanOperation, PlanOutputBinding, PlanOutputRef,
        PlanParameterBundleBuilder, PlanParameterHandle, PlanParameterPayload,
        PlanParameterSchemaId, PlanParameterValue, PlanPortAddress, PlanProjectSessionId,
        PlanProvenance, PlanRegistryFingerprint, PlanResourceId, PlanResourceObservedState,
        PlanResourceRequirement, PlanResourceVersion, PlanSourceIdentity, ResourceAccess,
        ResourceKind, ValueRef,
    };
    use crate::resource_preparation::{RunResourceBinding, RunResourceBindings};
    use crate::result_store::{ResultId, StoredResult};
    use crate::run_registry::{RunId, RunState};
    use std::collections::BTreeMap;
    use std::time::Duration;
    use yss_data_contract::TabularScalar;

    fn prepared_plan(state: &ExecutionRuntimeState) -> PreparedExecutionPlan {
        let resource = PlanResourceId::from_existing("databases/answer".into());
        let version = PlanResourceVersion::from_existing("v1".into());
        let basis = PlanBasis::new(
            PlanProjectSessionId::from_existing("session".into()),
            PlanRegistryFingerprint::from_bytes([4; 32]),
            yss_node_kernel::KernelRegistry::default().fingerprint(),
            BTreeMap::from([(resource.clone(), version.clone())]),
            BTreeMap::from([(resource, PlanResourceObservedState::Present(version))]),
        );
        let parameters = Arc::new(PlanParameterBundleBuilder::new(basis.clone()).freeze());
        let package = ExecutionPlanPackage::new(
            Arc::new(ExecutionPlan::empty()),
            parameters,
            PlanProvenance::new(
                PlanSourceIdentity::new(
                    PlanGraphId::from_existing("events/main".into()),
                    None,
                    None,
                ),
                basis,
                PlanId::from_existing(11),
            ),
        );
        state
            .prepare_package(package, RuntimeGeneration::INITIAL)
            .expect("test package is valid")
    }

    fn bindings() -> RunResourceBindings {
        let requirement = PlanResourceRequirement::new(
            PlanResourceId::from_existing("databases/answer".into()),
            ResourceKind::DataFrame,
            ResourceAccess::Shared,
            false,
        );
        RunResourceBindings::new(
            PlanProjectSessionId::from_existing("session".into()),
            [requirement.clone()],
            [RunResourceBinding::new(
                requirement,
                PlanResourceVersion::from_existing("v1".into()),
                yss_node_kernel::RuntimeValue::Scalar(TabularScalar::Integer(4)),
            )],
        )
    }

    fn empty_bindings() -> RunResourceBindings {
        RunResourceBindings::new(
            PlanProjectSessionId::from_existing("session".into()),
            Vec::<PlanResourceRequirement>::new(),
            Vec::<RunResourceBinding>::new(),
        )
    }

    fn prepared_operation_plan(
        state: &ExecutionRuntimeState,
        operations: impl IntoIterator<Item = PlanOperation>,
        parameter_entries: impl IntoIterator<Item = (PlanParameterHandle, PlanParameterPayload)>,
    ) -> PreparedExecutionPlan {
        let basis = PlanBasis::new(
            PlanProjectSessionId::from_existing("session".into()),
            PlanRegistryFingerprint::from_bytes([4; 32]),
            yss_node_kernel::KernelRegistry::default().fingerprint(),
            BTreeMap::new(),
            BTreeMap::new(),
        );
        let mut parameters = PlanParameterBundleBuilder::new(basis.clone());
        for (handle, payload) in parameter_entries {
            parameters
                .insert(handle, payload)
                .expect("test parameter handles are unique");
        }
        let package = ExecutionPlanPackage::new(
            Arc::new(ExecutionPlan::new(
                operations
                    .into_iter()
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            )),
            Arc::new(parameters.freeze()),
            PlanProvenance::new(
                PlanSourceIdentity::new(
                    PlanGraphId::from_existing("events/main".into()),
                    None,
                    None,
                ),
                basis,
                PlanId::from_existing(12),
            ),
        );
        state
            .prepare_package(package, RuntimeGeneration::INITIAL)
            .expect("test package is valid")
    }

    fn operation_source(node: &str) -> PlanSourceIdentity {
        PlanSourceIdentity::new(
            PlanGraphId::from_existing("events/main".into()),
            Some(crate::plan::PlanNodeId::from_existing(node.into())),
            None,
        )
    }

    fn operation_output(node: &str, value: ValueRef) -> PlanOutputBinding {
        PlanOutputBinding::new(
            PlanOutputRef::new(
                PlanGraphId::from_existing("events/main".into()),
                PlanPortAddress::from_existing(format!("{node}:result").into_boxed_str()),
            ),
            value,
            crate::plan::PlanOutputContract {
                data_type: yss_data_contract::ValueType::Scalar(
                    yss_data_contract::SemanticType::Numeric,
                ),
                schema: None,
                category: crate::plan::ResultCategory::Value,
                source: PlanSourceIdentity::new(
                    PlanGraphId::from_existing("events/main".into()),
                    Some(crate::plan::PlanNodeId::from_existing(node.into())),
                    Some(PlanPortAddress::from_existing(
                        format!("{node}:result").into_boxed_str(),
                    )),
                ),
            },
        )
    }

    fn operation_specialization(kind: &str, node: &str) -> crate::plan::PlanKernelSpecialization {
        crate::plan::PlanKernelSpecialization::new(
            KernelId::from_existing(kind.into()),
            Box::new([]),
            Box::new([crate::plan::PlanTypeBinding::new(
                PlanPortAddress::from_existing(format!("{node}:result").into_boxed_str()),
                yss_data_contract::ValueType::Scalar(yss_data_contract::SemanticType::Numeric),
            )]),
            Box::new([]),
        )
    }

    struct TestExecutor;

    impl PreparedPlanExecutor for TestExecutor {
        fn execute(
            &self,
            execution: PreparedPlanExecution<'_>,
        ) -> Result<SchedulerOutput, OperationExecutionError> {
            let bindings = execution.bindings;
            let resources = execution.resources;
            assert_eq!(bindings.len(), 1);
            assert_eq!(
                resources.value(&PlanResourceId::from_existing("databases/answer".into())),
                Some(&yss_node_kernel::RuntimeValue::Scalar(
                    TabularScalar::Integer(4)
                ))
            );
            Ok(SchedulerOutput::new(
                vec![SchedulerResult {
                    value: StoredResult::new(yss_node_kernel::RuntimeValue::Scalar(
                        TabularScalar::Integer(5),
                    )),
                    category: crate::plan::ResultCategory::Value,
                    output: operation_output("test-executor", ValueRef::new(0))
                        .output()
                        .clone(),
                }]
                .into_boxed_slice(),
                Box::new([]),
            ))
        }
    }

    fn state() -> ExecutionRuntimeState {
        ExecutionRuntimeState::new(
            ExecutionSessionId::new(uuid::Uuid::nil()),
            crate::identity::RuntimeGeneration::INITIAL,
            yss_node_kernel::KernelRegistry::default().into(),
            crate::test_relations(),
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
    fn neutral_executor_waits_for_an_upstream_value_later_in_the_plan() {
        let state = state();
        let parameter_handle = PlanParameterHandle::from_existing("constant/value".into());
        let consumer = PlanOperation::new(
            operation_source("consumer"),
            crate::plan::PlanNodeTypeId::from_existing("yssbi.numeric.square".into()),
            BTreeMap::new(),
            Box::new([PlanInputBinding::new(
                PlanPortAddress::from_existing("consumer:value".into()),
                PlanInputSource::Value(ValueRef::new(1)),
                crate::plan::PlanInputContract {
                    key: "input".into(),
                    group: None,
                    expected_type: yss_data_contract::ValueType::Scalar(
                        yss_data_contract::SemanticType::Numeric,
                    ),
                    coercions: Box::new([]),
                },
            )]),
            Box::new([]),
            Box::new([operation_output("consumer", ValueRef::new(0))]),
            crate::plan::PlanKernelSpecialization::new(
                KernelId::from_existing("yssbi.numeric.square".into()),
                Box::new([crate::plan::PlanTypeBinding::new(
                    PlanPortAddress::from_existing("consumer:value".into()),
                    yss_data_contract::ValueType::Scalar(yss_data_contract::SemanticType::Numeric),
                )]),
                operation_specialization("yssbi.numeric.square", "consumer")
                    .output_types()
                    .into(),
                Box::new([]),
            ),
        );
        let producer = PlanOperation::new(
            operation_source("producer"),
            crate::plan::PlanNodeTypeId::from_existing("yssbi.constant.get".into()),
            BTreeMap::from([(
                yss_node_kernel::KernelParameterKey::from_existing("value".into()),
                parameter_handle.clone(),
            )]),
            Box::new([]),
            Box::new([]),
            Box::new([operation_output("producer", ValueRef::new(1))]),
            operation_specialization("yssbi.constant.get", "producer"),
        );
        let plan = prepared_operation_plan(
            &state,
            [consumer, producer],
            [(
                parameter_handle,
                PlanParameterPayload::new(
                    PlanParameterSchemaId::from_existing("graph.constant".into()),
                    PlanParameterValue::Literal(std::sync::Arc::new(
                        yss_node_kernel::RuntimeValue::Scalar(TabularScalar::Integer(7)),
                    )),
                ),
            )],
        );

        let candidate = state
            .execute_prepared(
                &plan,
                empty_bindings(),
                &ResourceProviderFactory::new("session".into()),
                &RunExecutionControl::new(Instant::now() + Duration::from_secs(1)),
            )
            .expect("the executor must wait for the producer instead of reading by plan order");

        assert_eq!(candidate.results().len(), 2);
        assert!(candidate.results().iter().all(|result| {
            result.value().value()
                == &if result.output().port().as_str().starts_with("consumer:") {
                    RuntimeValue::float64(49.0).unwrap()
                } else {
                    RuntimeValue::Scalar(TabularScalar::Integer(7))
                }
        }));
        let outputs = candidate
            .results()
            .iter()
            .map(|result| result.output().clone())
            .collect::<Vec<_>>();
        let handoff = candidate.into_finalization_handoff();
        state.publish_committed_results(&handoff);
        assert!(
            outputs
                .iter()
                .all(|output| { state.query_pin_result(output).is_some() })
        );
        drop(handoff);
        for _ in 0..100 {
            let previous = outputs
                .iter()
                .map(|output| state.query_pin_result(output).unwrap())
                .collect::<Vec<_>>();
            let weak = previous
                .iter()
                .map(|result| Arc::downgrade(result.value()))
                .collect::<Vec<_>>();
            let ids = previous
                .iter()
                .map(|result| result.provenance().result_id())
                .collect::<Vec<_>>();
            drop(previous);
            let candidate = state
                .execute_prepared(
                    &plan,
                    empty_bindings(),
                    &ResourceProviderFactory::new("session".into()),
                    &RunExecutionControl::new(Instant::now() + Duration::from_secs(1)),
                )
                .unwrap();
            assert!(weak.iter().all(|result| result.upgrade().is_none()));
            assert!(ids.iter().all(|id| state.query_result(*id).is_none()));
            assert!(state.publish_committed_results(&candidate.into_finalization_handoff()));
        }
        let failed = state.execute_prepared(
            &plan,
            empty_bindings(),
            &ResourceProviderFactory::new("another-session".into()),
            &RunExecutionControl::new(Instant::now() + Duration::from_secs(1)),
        );
        assert!(failed.is_err());
        assert!(
            outputs
                .iter()
                .all(|output| state.query_pin_result(output).is_none())
        );
    }

    #[test]
    fn explicit_output_demand_skips_unrelated_graph_components() {
        let state = state();
        let parameter_handle = PlanParameterHandle::from_existing("constant/value".into());
        let selected_output = operation_output("selected", ValueRef::new(0));
        let requested = selected_output.output().clone();
        let selected = PlanOperation::new(
            operation_source("selected"),
            crate::plan::PlanNodeTypeId::from_existing("yssbi.constant.get".into()),
            BTreeMap::from([(
                yss_node_kernel::KernelParameterKey::from_existing("value".into()),
                parameter_handle.clone(),
            )]),
            Box::new([]),
            Box::new([]),
            Box::new([selected_output]),
            operation_specialization("yssbi.constant.get", "selected"),
        );
        let unrelated = PlanOperation::new(
            operation_source("unrelated"),
            crate::plan::PlanNodeTypeId::from_existing("yssbi.constant.get".into()),
            BTreeMap::new(),
            Box::new([]),
            Box::new([]),
            Box::new([operation_output("unrelated", ValueRef::new(1))]),
            operation_specialization("yssbi.unsupported", "unrelated"),
        );
        let plan = prepared_operation_plan(
            &state,
            [selected, unrelated],
            [(
                parameter_handle,
                PlanParameterPayload::new(
                    PlanParameterSchemaId::from_existing("graph.constant".into()),
                    PlanParameterValue::Literal(std::sync::Arc::new(
                        yss_node_kernel::RuntimeValue::Scalar(TabularScalar::Integer(7)),
                    )),
                ),
            )],
        );

        let executed = state
            .execute_prepared_handoff(
                &plan,
                empty_bindings(),
                &ResourceProviderFactory::new("session".into()),
                &RunExecutionControl::new(Instant::now() + Duration::from_secs(1)),
                &PlanExecutionDemand::Outputs {
                    outputs: vec![requested].into_boxed_slice(),
                    include_default_results: false,
                },
                None,
                |_| {},
            )
            .expect("an unrelated unsupported component must not be scheduled");

        assert_eq!(
            executed.handoff().results()[0].value().value(),
            &yss_node_kernel::RuntimeValue::Scalar(TabularScalar::Integer(7))
        );
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
            &yss_node_kernel::RuntimeValue::Scalar(TabularScalar::Integer(5))
        );
        assert_eq!(
            handoff.results()[0].category(),
            crate::plan::ResultCategory::Value
        );
        assert_eq!(
            state.runs().state(RunId::from_existing(1)),
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
        assert_eq!(state.runs().state(RunId::from_existing(1)), None);
    }
}
