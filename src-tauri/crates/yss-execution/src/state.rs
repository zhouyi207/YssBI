use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::time::{Instant, SystemTime, SystemTimeError, UNIX_EPOCH};

use thiserror::Error;

use crate::error::RunPhase;
use crate::finalization::{
    ExecutionFinalizationHandoff, ReadyPinResult, ReadyResult, ResultObservationIntent,
    SuccessfulExecutionCandidate,
};
use crate::identity::{ExecutionSessionId, RuntimeGeneration};
use crate::package_preparation::PreparedExecutionPlan;
use crate::ports::scientific::ScientificBackend;
use crate::resource_preparation::{
    PreparedRunResources, ResourcePreparationError, ResourceProviderFactory, RunResourceBindings,
    RunResourceRequest,
};
use crate::result::{
    ActivationId, ExecutionResultQueryError, PinResultEntry, PinResultHistorySnapshot, ResultId,
    StoredResult, StoredResultSnapshot,
};
use crate::result_store::ResultStore;
use crate::run_output::RunOutputMessage;
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
    #[error("prepared execution kernel failed")]
    Kernel(#[source] KernelExecutionError),
    #[error("execution result identity space is exhausted")]
    ResultIdentityExhausted,
    #[error("execution result timestamp is unavailable")]
    ResultTimestamp(#[source] SystemTimeError),
}

#[derive(Debug, Error)]
pub enum KernelExecutionError {
    #[error("prepared execution kernel was cancelled")]
    Cancelled,
    #[error("prepared execution kernel deadline was exceeded")]
    DeadlineExceeded,
    #[error("prepared execution kernel failed")]
    Failed,
    #[error("requested graph output is unavailable in the compiled plan")]
    DemandOutputUnavailable,
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
    package: &'a crate::plan::CompiledExecutionPackage,
    bindings: &'a [crate::resource_preparation::RunResourceBinding],
    resources: &'a PreparedRunResources,
    control: &'a RunExecutionControl,
    run_id: crate::run_registry::RunId,
    demand: &'a crate::plan::PlanExecutionDemand,
}

struct PreparedExecutionDispatch<'a> {
    demand: &'a crate::plan::PlanExecutionDemand,
    executor: &'a dyn PreparedPlanExecutor,
    on_event: Option<&'a mut dyn FnMut(PreparedExecutionEvent)>,
}

trait PreparedPlanExecutor: Send + Sync {
    fn execute(
        &self,
        execution: PreparedPlanExecution<'_>,
        _on_output: &mut dyn FnMut(RunOutputMessage),
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
        execution: PreparedPlanExecution<'_>,
        _on_output: &mut dyn FnMut(RunOutputMessage),
    ) -> Result<SchedulerOutput, KernelExecutionError> {
        let PreparedPlanExecution {
            package,
            bindings: _bindings,
            resources,
            control,
            run_id: _run_id,
            demand,
        } = execution;
        let operations = package.plan().operations();
        let value_count = operations
            .iter()
            .flat_map(|operation| operation.outputs())
            .map(|output| output.value().index() as usize)
            .max()
            .map_or(0, |maximum| maximum.saturating_add(1));
        let mut values: Vec<Option<RuntimeValue>> = vec![None; value_count];
        let mut producers = vec![None; value_count];
        for (operation_index, operation) in operations.iter().enumerate() {
            for output in operation.outputs() {
                let Some(producer) = producers.get_mut(output.value().index() as usize) else {
                    return Err(KernelExecutionError::Failed);
                };
                if producer.replace(operation_index).is_some() {
                    return Err(KernelExecutionError::Failed);
                }
            }
        }
        let selection = select_execution(package, demand, &producers)?;
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
                    return Err(KernelExecutionError::Failed);
                };
                remaining_dependencies[operation_index] = remaining_dependencies[operation_index]
                    .checked_add(1)
                    .ok_or(KernelExecutionError::Failed)?;
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
        let mut completed_count = 0usize;
        let mut results = Vec::new();
        while let Some(operation_index) = ready.pop_front() {
            let operation = &operations[operation_index];
            check_kernel_control(control)?;
            let inputs = operation
                .inputs()
                .iter()
                .map(|binding| {
                    let value = match binding.source() {
                        crate::plan::PlanInputSource::Value(reference) => values
                            .get(reference.index() as usize)
                            .and_then(Option::as_ref)
                            .cloned()
                            .ok_or(KernelExecutionError::Failed),
                        crate::plan::PlanInputSource::Parameter(handle) => {
                            let Some(payload) = package.parameters().entries().get(handle) else {
                                return Err(KernelExecutionError::Failed);
                            };
                            parameter_value(payload.value(), resources)
                        }
                    }?;
                    apply_input_coercions(
                        value,
                        binding.port(),
                        operation.specialization().coercions(),
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;

            let mut output_values = execute_node(
                operation.kind().as_str(),
                &inputs,
                operation
                    .parameter_handles()
                    .iter()
                    .find_map(|handle| package.parameters().entries().get(handle))
                    .map(|payload| payload.value()),
                resources,
                operation.outputs(),
                operation.specialization(),
            )?;
            for output in operation.outputs() {
                let value = output_values
                    .remove(output.output())
                    .ok_or(KernelExecutionError::Failed)?;
                let Some(slot) = values.get_mut(output.value().index() as usize) else {
                    return Err(KernelExecutionError::Failed);
                };
                if slot.replace(value.clone()).is_some() {
                    return Err(KernelExecutionError::Failed);
                }
                results.push(SchedulerResult {
                    value: StoredResult::Runtime(value.clone()),
                    category: operation.result_category(),
                    output: output.output().clone(),
                });
            }
            if !output_values.is_empty() {
                return Err(KernelExecutionError::Failed);
            }
            completed_count = completed_count
                .checked_add(1)
                .ok_or(KernelExecutionError::Failed)?;
            for dependent in &dependents[operation_index] {
                let remaining = &mut remaining_dependencies[*dependent];
                *remaining = remaining
                    .checked_sub(1)
                    .ok_or(KernelExecutionError::Failed)?;
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
            return Err(KernelExecutionError::Failed);
        }

        let observations = selection
            .observations
            .into_iter()
            .map(|selected| {
                let crate::plan::PlanInputSource::Value(value) = selected.source else {
                    return Err(KernelExecutionError::Failed);
                };
                let output = operations
                    .iter()
                    .flat_map(|operation| operation.outputs())
                    .find(|output| output.value() == value)
                    .map(|output| output.output().clone())
                    .ok_or(KernelExecutionError::Failed)?;
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

fn select_execution(
    package: &crate::plan::CompiledExecutionPackage,
    demand: &crate::plan::PlanExecutionDemand,
    producers: &[Option<usize>],
) -> Result<ExecutionSelection, KernelExecutionError> {
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
                        (output.value(), operation.result_category()),
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
                    .map(|output| (output.value(), operation.result_category()))
            }) else {
                return Err(KernelExecutionError::DemandOutputUnavailable);
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
        return Err(KernelExecutionError::Failed);
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
            return Err(KernelExecutionError::Failed);
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

fn apply_input_coercions(
    mut value: RuntimeValue,
    port: &crate::plan::PlanPortAddress,
    coercions: &[crate::plan::PlanInputCoercion],
) -> Result<RuntimeValue, KernelExecutionError> {
    for coercion in coercions.iter().filter(|coercion| coercion.port() == port) {
        value = match coercion.kind() {
            crate::plan::PlanInputCoercionKind::WidenInt64ToFloat64 => value
                .coerce_to(&yss_data_contract::DataType::Float64)
                .map_err(|_| KernelExecutionError::Failed)?,
            // Broadcast is a kernel-owned shape operation. Keeping the scalar
            // value here makes the coercion explicit without fabricating a
            // DataSeries length in the scheduler.
            crate::plan::PlanInputCoercionKind::BroadcastScalarToSeries => value,
        };
    }
    Ok(value)
}

fn execute_node(
    kind: &str,
    inputs: &[RuntimeValue],
    parameter: Option<&crate::plan::PlanParameterValue>,
    resources: &PreparedRunResources,
    outputs: &[crate::plan::PlanOutputBinding],
    specialization: &crate::plan::PlanKernelSpecialization,
) -> Result<BTreeMap<crate::plan::PlanOutputRef, RuntimeValue>, KernelExecutionError> {
    let value = match kind {
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
        "yssbi.numeric.add" => numeric_fold(inputs, specialization, |left, right| left + right),
        "yssbi.numeric.subtract" => {
            binary_numeric(inputs, specialization, |left, right| left - right)
        }
        "yssbi.numeric.multiply" => {
            binary_numeric(inputs, specialization, |left, right| left * right)
        }
        "yssbi.numeric.divide" => {
            binary_numeric(inputs, specialization, |left, right| left / right)
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
        "yssbi.value.convert" => {
            let target = specialization
                .output_types()
                .first()
                .map(crate::plan::PlanTypeBinding::data_type)
                .ok_or(KernelExecutionError::Failed)?;
            inputs
                .first()
                .cloned()
                .ok_or(KernelExecutionError::Failed)?
                .coerce_to(target)
                .map_err(|_| KernelExecutionError::Failed)
        }
        "yssbi.debug.view"
        | "yssbi.project.function.entry"
        | "yssbi.project.function.return"
        | "yssbi.project.function.call"
        | "yssbi.core.reroute" => Ok(inputs.first().cloned().unwrap_or(RuntimeValue::Null)),
        _ => Err(KernelExecutionError::Failed),
    }?;
    let [output] = outputs else {
        return Err(KernelExecutionError::Failed);
    };
    Ok(BTreeMap::from([(output.output().clone(), value)]))
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
    specialization: &crate::plan::PlanKernelSpecialization,
    operation: impl FnOnce(f64, f64) -> f64,
) -> Result<RuntimeValue, KernelExecutionError> {
    let value = operation(
        numeric_input(inputs.first())?,
        numeric_input(inputs.get(1))?,
    );
    numeric_result(value, specialization)
}

fn numeric_fold(
    inputs: &[RuntimeValue],
    specialization: &crate::plan::PlanKernelSpecialization,
    operation: impl Fn(f64, f64) -> f64,
) -> Result<RuntimeValue, KernelExecutionError> {
    let mut values = inputs.iter();
    let mut result = numeric_input(values.next())?;
    for value in values {
        result = operation(result, numeric_input(Some(value))?);
    }
    numeric_result(result, specialization)
}

fn numeric_result(
    value: f64,
    specialization: &crate::plan::PlanKernelSpecialization,
) -> Result<RuntimeValue, KernelExecutionError> {
    if !value.is_finite() {
        return Err(KernelExecutionError::Failed);
    }
    match specialization
        .output_types()
        .first()
        .map(crate::plan::PlanTypeBinding::data_type)
    {
        Some(yss_data_contract::DataType::Int64)
            if value.fract() == 0.0 && value >= i64::MIN as f64 && value <= i64::MAX as f64 =>
        {
            Ok(RuntimeValue::Integer(value as i64))
        }
        Some(yss_data_contract::DataType::Float64) => Ok(RuntimeValue::Decimal(value)),
        Some(
            yss_data_contract::DataType::DataSeries(_)
            | yss_data_contract::DataType::Int64
            | yss_data_contract::DataType::Boolean
            | yss_data_contract::DataType::String
            | yss_data_contract::DataType::Date
            | yss_data_contract::DataType::Datetime
            | yss_data_contract::DataType::Time
            | yss_data_contract::DataType::Categorical
            | yss_data_contract::DataType::Array(_)
            | yss_data_contract::DataType::Object
            | yss_data_contract::DataType::DataFrame
            | yss_data_contract::DataType::Struct(_)
            | yss_data_contract::DataType::OneOf(_)
            | yss_data_contract::DataType::Any,
        )
        | None => Err(KernelExecutionError::Failed),
    }
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PreparedExecutionEvent {
    RunStarted(crate::run_registry::RunId),
    RunOutput(RunOutputMessage),
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
    next_result_id: AtomicU64,
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
            next_result_id: AtomicU64::new(1),
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

    pub fn query_result(&self, result_id: ResultId) -> Option<StoredResultSnapshot> {
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
            PreparedExecutionDispatch {
                demand: &crate::plan::PlanExecutionDemand::Default,
                executor: self.executor.as_ref(),
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
        mut on_event: impl FnMut(PreparedExecutionEvent),
    ) -> Result<ExecutedPreparedRun, ExecutePreparedError> {
        self.execute_prepared_inner(
            plan,
            bindings,
            resources,
            control,
            PreparedExecutionDispatch {
                demand,
                executor: self.executor.as_ref(),
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
            executor,
            mut on_event,
        } = dispatch;
        let actual_generation = self.generation();
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
        if let Some(on_event) = on_event.as_mut() {
            on_event(PreparedExecutionEvent::RunStarted(run_id));
        }
        if let Err(error) = control.check(RunPhase::Execution) {
            let result = terminate_run(&mut lifecycle, run_id, error);
            self.remove_active_control(run_id);
            return result;
        }

        let mut on_output = |message| {
            if let Some(on_event) = on_event.as_mut() {
                on_event(PreparedExecutionEvent::RunOutput(message));
            }
        };
        let output = match executor.execute(
            PreparedPlanExecution {
                package: plan.package(),
                bindings: bindings.bindings(),
                resources: &prepared_resources,
                control,
                run_id,
                demand,
            },
            &mut on_output,
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
                PinResultEntry::produced(
                    result_id,
                    run_id,
                    ActivationId::from_existing(result_id.get()),
                    created_at_ms,
                ),
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
                    ExecutePreparedError::Kernel(KernelExecutionError::Failed),
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

    pub fn publish_committed_results(&self, handoff: &ExecutionFinalizationHandoff) {
        for result in handoff.results() {
            let pin = result.pin();
            self.results.publish_for_output(
                pin.output().clone(),
                pin.entry().clone(),
                result.value().clone(),
            );
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
        CompiledParameterHandle, ExecutionPlan, PlanCompilationBasis, PlanCompileId,
        PlanExecutionDemand, PlanGraphId, PlanInputBinding, PlanInputSource, PlanOperation,
        PlanOperationKind, PlanOutputBinding, PlanOutputRef, PlanParameterPayload,
        PlanParameterScalar, PlanParameterSchemaId, PlanParameterValue, PlanPortAddress,
        PlanProjectSessionId, PlanProvenance, PlanRegistryFingerprint, PlanResourceId,
        PlanResourceObservedState, PlanResourceRequirement, PlanResourceVersion,
        PlanSourceIdentity, ResourceAccess, ResourceKind, ValueRef,
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
        parameter_entries: impl IntoIterator<Item = (CompiledParameterHandle, PlanParameterPayload)>,
    ) -> PreparedExecutionPlan {
        let basis = PlanCompilationBasis::new(
            PlanProjectSessionId::from_existing("session".into()),
            PlanRegistryFingerprint::from_bytes([4; 32]),
            BTreeMap::new(),
            BTreeMap::new(),
        );
        let mut parameters = CompiledParameterBundleBuilder::new(basis.clone());
        for (handle, payload) in parameter_entries {
            parameters
                .insert(handle, payload)
                .expect("test parameter handles are unique");
        }
        let package = CompiledExecutionPackage::new(
            Arc::new(ExecutionPlan::new(
                operations
                    .into_iter()
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            )),
            Arc::new(CompiledFunctionBundle::new(basis.clone(), Box::new([]), 0)),
            Arc::new(parameters.freeze()),
            PlanProvenance::new(
                PlanSourceIdentity::new(
                    PlanGraphId::from_existing("events/main".into()),
                    None,
                    None,
                ),
                basis,
                PlanCompileId::from_existing(12),
            ),
        );
        state
            .prepare_compiled_package(package, RuntimeGeneration::INITIAL)
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
        )
    }

    fn operation_specialization(kind: &str, node: &str) -> crate::plan::PlanKernelSpecialization {
        crate::plan::PlanKernelSpecialization::new(
            PlanOperationKind::from_existing(kind.into()),
            Box::new([]),
            Box::new([crate::plan::PlanTypeBinding::new(
                PlanPortAddress::from_existing(format!("{node}:result").into_boxed_str()),
                yss_data_contract::DataType::Int64,
            )]),
            Box::new([]),
        )
    }

    struct TestExecutor;

    impl PreparedPlanExecutor for TestExecutor {
        fn execute(
            &self,
            execution: PreparedPlanExecution<'_>,
            _on_output: &mut dyn FnMut(RunOutputMessage),
        ) -> Result<SchedulerOutput, KernelExecutionError> {
            let bindings = execution.bindings;
            let resources = execution.resources;
            assert_eq!(bindings.len(), 1);
            assert_eq!(
                resources.value(&PlanResourceId::from_existing("variables/answer".into())),
                Some(&crate::value::RuntimeValue::Integer(4))
            );
            Ok(SchedulerOutput::new(
                vec![SchedulerResult {
                    value: StoredResult::Runtime(crate::value::RuntimeValue::Integer(5)),
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
            state.runs().state(RunId::from_existing(1)),
            Some(RunState::Succeeded)
        );
        assert!(state.results().get(ResultId::from_existing(1)).is_none());
    }

    #[test]
    fn neutral_executor_waits_for_an_upstream_value_later_in_the_plan() {
        let state = state();
        let parameter_handle = CompiledParameterHandle::from_existing("constant/value".into());
        let consumer = PlanOperation::new(
            operation_source("consumer"),
            crate::plan::ResultCategory::Value,
            Box::new([]),
            Box::new([PlanInputBinding::new(
                PlanPortAddress::from_existing("consumer:value".into()),
                PlanInputSource::Value(ValueRef::new(1)),
            )]),
            Box::new([]),
            Box::new([operation_output("consumer", ValueRef::new(0))]),
            operation_specialization("yssbi.value.convert", "consumer"),
        );
        let producer = PlanOperation::new(
            operation_source("producer"),
            crate::plan::ResultCategory::Value,
            Box::new([parameter_handle.clone()]),
            Box::new([]),
            Box::new([]),
            Box::new([operation_output("producer", ValueRef::new(1))]),
            operation_specialization("yssbi.constant.int64", "producer"),
        );
        let plan = prepared_operation_plan(
            &state,
            [consumer, producer],
            [(
                parameter_handle,
                PlanParameterPayload::new(
                    PlanParameterSchemaId::from_existing("constant/int64".into()),
                    PlanParameterValue::Scalar(PlanParameterScalar::Integer(7)),
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
            result.value().value() == &StoredResult::Runtime(crate::value::RuntimeValue::Integer(7))
        }));
        let outputs = candidate
            .results()
            .iter()
            .map(|result| result.output().clone())
            .collect::<Vec<_>>();
        let handoff = candidate.into_finalization_handoff();
        state.publish_committed_results(&handoff);
        assert!(outputs.iter().all(|output| {
            state
                .query_pin_result_history(output)
                .is_ok_and(|history| history.len() == 1)
        }));
    }

    #[test]
    fn explicit_output_demand_skips_unrelated_graph_components() {
        let state = state();
        let parameter_handle = CompiledParameterHandle::from_existing("constant/value".into());
        let selected_output = operation_output("selected", ValueRef::new(0));
        let requested = selected_output.output().clone();
        let selected = PlanOperation::new(
            operation_source("selected"),
            crate::plan::ResultCategory::Value,
            Box::new([parameter_handle.clone()]),
            Box::new([]),
            Box::new([]),
            Box::new([selected_output]),
            operation_specialization("yssbi.constant.int64", "selected"),
        );
        let unrelated = PlanOperation::new(
            operation_source("unrelated"),
            crate::plan::ResultCategory::Value,
            Box::new([]),
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
                    PlanParameterSchemaId::from_existing("constant/int64".into()),
                    PlanParameterValue::Scalar(PlanParameterScalar::Integer(7)),
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
                |_| {},
            )
            .expect("an unrelated unsupported component must not be scheduled");

        assert_eq!(
            executed.handoff().results()[0].value().value(),
            &StoredResult::Runtime(crate::value::RuntimeValue::Integer(7))
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
            &StoredResult::Runtime(crate::value::RuntimeValue::Integer(5))
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
