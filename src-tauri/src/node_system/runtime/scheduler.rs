use super::relational::RunRelationalBackends;
use super::{
    ActivationId, CancellationToken, CompiledParameterStore, FrameId, KernelContext,
    KernelRegistry, NOOP_RUN_EVENT_SINK, ProjectRunRegistry, RelationalBackendProvider,
    RelationalContext, RelationalInput, ResourceErrorKind, ResourceProvider, ResultStore, RunError,
    RunErrorCode, RunEvent, RunEventKind, RunEventSink, RunResourceSet, RunResult, RuntimeValue,
    materialize_bridge,
};
use crate::node_system::analysis::{
    CorrelationContext, NOOP_TRACE_SINK, ParentCallId, RunId, SpanEvent, SpanKind, SpanStatus,
    TraceSink,
};
use crate::node_system::plan::{
    ControlStep, ExecutionPlan, FunctionPlanHandle, OperationIndex, PlannedKernel,
    RelationalFragmentId, RelationalSubplanIndex, StructuredControlRegion, ValueRef,
};
use crate::node_system::protocol::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

const DEFAULT_RECURSION_LIMIT: usize = 64;
static NEXT_RUN_ID: AtomicU64 = AtomicU64::new(1);
static NEXT_PARENT_CALL_ID: AtomicU64 = AtomicU64::new(1);

pub trait FunctionPlanProvider: Send + Sync {
    fn get_plan(&self, handle: &FunctionPlanHandle)
    -> Result<Option<Arc<ExecutionPlan>>, Box<str>>;

    fn recursion_limit(&self) -> usize {
        DEFAULT_RECURSION_LIMIT
    }
}

pub struct RunExecutor<'a> {
    kernels: &'a KernelRegistry,
    resources: &'a dyn ResourceProvider,
    functions: &'a dyn FunctionPlanProvider,
    relational_backends: Option<&'a dyn RelationalBackendProvider>,
    compiled_parameters: Option<&'a CompiledParameterStore>,
    run_registry: Option<&'a ProjectRunRegistry>,
    recursion_limit: usize,
    trace: &'a dyn TraceSink,
    events: &'a dyn RunEventSink,
    results: Option<&'a ResultStore>,
}

impl<'a> RunExecutor<'a> {
    pub fn new(
        kernels: &'a KernelRegistry,
        resources: &'a dyn ResourceProvider,
        functions: &'a dyn FunctionPlanProvider,
    ) -> Self {
        Self {
            kernels,
            resources,
            functions,
            relational_backends: None,
            compiled_parameters: None,
            run_registry: None,
            recursion_limit: functions.recursion_limit().max(1),
            trace: &NOOP_TRACE_SINK,
            events: &NOOP_RUN_EVENT_SINK,
            results: None,
        }
    }

    pub fn with_recursion_limit(mut self, recursion_limit: usize) -> Self {
        self.recursion_limit = recursion_limit;
        self
    }

    pub fn with_relational_backends(mut self, provider: &'a dyn RelationalBackendProvider) -> Self {
        self.relational_backends = Some(provider);
        self
    }

    pub fn with_compiled_parameters(mut self, parameters: &'a CompiledParameterStore) -> Self {
        self.compiled_parameters = Some(parameters);
        self
    }

    pub fn with_run_registry(mut self, registry: &'a ProjectRunRegistry) -> Self {
        self.run_registry = Some(registry);
        self
    }

    pub fn with_trace_sink(mut self, trace: &'a dyn TraceSink) -> Self {
        self.trace = trace;
        self
    }

    pub fn with_event_sink(mut self, events: &'a dyn RunEventSink) -> Self {
        self.events = events;
        self
    }

    pub fn with_result_store(mut self, results: &'a ResultStore) -> Self {
        self.results = Some(results);
        self
    }

    pub fn run(
        &self,
        plan: &ExecutionPlan,
        cancellation: CancellationToken,
    ) -> Result<RunResult, RunError> {
        let run_id = RunId::new(NEXT_RUN_ID.fetch_add(1, Ordering::Relaxed));
        let _registration = self
            .run_registry
            .map(|registry| {
                registry.track(
                    plan.provenance.project_session_id.clone(),
                    run_id,
                    cancellation.clone(),
                )
            })
            .transpose()
            .map_err(|error| RunError::ProjectDraining(error.to_string().into()))?;
        let correlation = CorrelationContext::compile(&plan.provenance).for_run(run_id, None);
        self.record_event(plan, correlation.clone(), RunEventKind::RunStarted);
        self.trace.record(SpanEvent::new(
            SpanKind::Run,
            SpanStatus::Started,
            correlation.clone(),
        ));
        let result = self.run_root(plan, cancellation, run_id, correlation.clone());
        if let (Some(results), Ok(run_result)) = (self.results, &result) {
            for (name, value) in &run_result.values {
                if let Some(descriptor) = results.publish_runtime_value(
                    run_id,
                    correlation.clone(),
                    plan.provenance.basis.clone(),
                    name.clone(),
                    value,
                ) {
                    self.record_event(
                        plan,
                        correlation.clone(),
                        RunEventKind::ResultReady {
                            name: name.clone(),
                            source_id: descriptor.source_id,
                        },
                    );
                }
            }
        }
        let status = run_status(&result);
        self.trace.record(SpanEvent::new(
            SpanKind::Cleanup,
            status,
            correlation.clone(),
        ));
        self.trace
            .record(SpanEvent::new(SpanKind::Run, status, correlation.clone()));
        let event = match &result {
            Ok(_) => RunEventKind::RunCompleted,
            Err(RunError::Cancelled) => RunEventKind::RunCancelled,
            Err(error) => RunEventKind::RunErrored {
                code: RunErrorCode::from(error),
            },
        };
        self.record_event(plan, correlation, event);
        if let Some(results) = self.results {
            results.cleanup_run(run_id);
        }
        result
    }

    fn run_root(
        &self,
        plan: &ExecutionPlan,
        cancellation: CancellationToken,
        run_id: RunId,
        correlation: CorrelationContext,
    ) -> Result<RunResult, RunError> {
        plan.validate()
            .map_err(|error| RunError::InvalidPlan(error.to_string().into()))?;
        cancellation.check()?;
        let resource_set = self.acquire_resources(plan, &correlation)?;
        let relational_backends = RunRelationalBackends::acquire(
            &plan.relational_subplans,
            self.relational_backends,
            &resource_set,
            &cancellation,
        )?;
        let mut frame = Frame::new(plan.value_count);
        let result = (|| {
            self.execute_region(
                run_id,
                plan,
                &plan.root_region,
                &mut frame,
                &resource_set,
                &relational_backends,
                &cancellation,
                1,
                None,
            )?;
            cancellation.check()?;

            let mut values = BTreeMap::new();
            for result in &plan.results {
                values.insert(result.name.clone(), frame.value(result.value)?.clone());
            }
            Ok(RunResult {
                run_id,
                provenance: plan.provenance.clone(),
                correlation,
                values,
                committed_variable_ids: Box::new([]),
                resource_mutation: None,
            })
        })();
        frame.close_streams();
        result
    }

    fn acquire_resources(
        &self,
        plan: &ExecutionPlan,
        correlation: &CorrelationContext,
    ) -> Result<RunResourceSet, RunError> {
        self.trace.record(SpanEvent::new(
            SpanKind::ResourceAcquire,
            SpanStatus::Started,
            correlation.clone(),
        ));
        let result = self
            .resources
            .validate_plan(&plan.provenance, &plan.resources)
            .map_err(|error| match error.kind() {
                ResourceErrorKind::SnapshotMismatch => {
                    RunError::ResourceSnapshotMismatch(error.into_message())
                }
                ResourceErrorKind::UnsupportedAccess => RunError::InvalidPlan(error.into_message()),
                ResourceErrorKind::Acquire => RunError::InvalidPlan(error.into_message()),
            })
            .and_then(|()| RunResourceSet::acquire(&plan.resources, self.resources));
        self.trace.record(SpanEvent::new(
            SpanKind::ResourceAcquire,
            run_status(&result),
            correlation.clone(),
        ));
        result
    }

    #[allow(clippy::too_many_arguments)]
    fn execute_region(
        &self,
        run_id: RunId,
        plan: &ExecutionPlan,
        region: &StructuredControlRegion,
        frame: &mut Frame,
        resources: &RunResourceSet,
        relational_backends: &RunRelationalBackends,
        cancellation: &CancellationToken,
        frame_depth: usize,
        parent_call: Option<ParentCallId>,
    ) -> Result<(), RunError> {
        cancellation.check()?;
        match region {
            StructuredControlRegion::Sequence(steps) => self.execute_sequence(
                run_id,
                plan,
                steps,
                frame,
                resources,
                relational_backends,
                cancellation,
                frame_depth,
                parent_call,
            )?,
            StructuredControlRegion::If {
                condition,
                then_region,
                else_region,
                results,
            } => {
                let selected_then = boolean(frame, *condition)?;
                let selected = if selected_then {
                    then_region
                } else {
                    else_region
                };
                self.execute_region(
                    run_id,
                    plan,
                    selected,
                    frame,
                    resources,
                    relational_backends,
                    cancellation,
                    frame_depth,
                    parent_call,
                )?;
                for binding in results {
                    let source = if selected_then {
                        binding.then_source
                    } else {
                        binding.else_source
                    };
                    frame.copy(source, binding.destination)?;
                }
            }
            StructuredControlRegion::Loop {
                body,
                carried,
                continue_condition,
                max_iterations,
            } => {
                for binding in carried {
                    frame.copy(binding.initial_source, binding.body_input)?;
                }
                let mut should_continue = true;
                for _ in 0..*max_iterations {
                    cancellation.check()?;
                    self.execute_region(
                        run_id,
                        plan,
                        body,
                        frame,
                        resources,
                        relational_backends,
                        cancellation,
                        frame_depth,
                        parent_call,
                    )?;
                    should_continue = boolean(frame, *continue_condition)?;
                    for binding in carried {
                        frame.copy(binding.next_source, binding.result)?;
                        if should_continue {
                            frame.copy(binding.next_source, binding.body_input)?;
                        }
                    }
                    if !should_continue {
                        break;
                    }
                }
                if should_continue {
                    return Err(RunError::LoopLimitExceeded {
                        max_iterations: *max_iterations,
                    });
                }
            }
            StructuredControlRegion::Call {
                target,
                arguments,
                results,
            } => {
                if frame_depth >= self.recursion_limit {
                    return Err(RunError::RecursionLimitExceeded {
                        recursion_limit: self.recursion_limit,
                    });
                }
                let callee = self
                    .functions
                    .get_plan(target)
                    .map_err(RunError::FunctionPlanFailed)?
                    .ok_or_else(|| RunError::FunctionPlanNotFound(target.as_str().into()))?;
                let call_id =
                    ParentCallId::new(NEXT_PARENT_CALL_ID.fetch_add(1, Ordering::Relaxed));
                let correlation =
                    CorrelationContext::compile(&callee.provenance).for_run(run_id, Some(call_id));
                self.trace.record(SpanEvent::new(
                    SpanKind::Run,
                    SpanStatus::Started,
                    correlation.clone(),
                ));
                let call_result = (|| {
                    callee
                        .validate()
                        .map_err(|error| RunError::InvalidPlan(error.to_string().into()))?;
                    let callee_resources = self.acquire_resources(&callee, &correlation)?;
                    let callee_backends = RunRelationalBackends::acquire(
                        &callee.relational_subplans,
                        self.relational_backends,
                        &callee_resources,
                        cancellation,
                    )?;
                    let mut callee_frame = Frame::new(callee.value_count);
                    let result = (|| {
                        for binding in arguments {
                            callee_frame
                                .set(binding.destination, frame.value(binding.source)?.clone())?;
                        }
                        self.execute_region(
                            run_id,
                            &callee,
                            &callee.root_region,
                            &mut callee_frame,
                            &callee_resources,
                            &callee_backends,
                            cancellation,
                            frame_depth + 1,
                            Some(call_id),
                        )?;
                        for binding in results {
                            frame.set(
                                binding.destination,
                                callee_frame.value(binding.source)?.clone(),
                            )?;
                        }
                        Ok(())
                    })();
                    if result.is_err() {
                        callee_frame.close_streams();
                    }
                    result
                })();
                let status = run_status(&call_result);
                self.trace.record(SpanEvent::new(
                    SpanKind::Cleanup,
                    status,
                    correlation.clone(),
                ));
                self.trace
                    .record(SpanEvent::new(SpanKind::Run, status, correlation));
                call_result?;
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn execute_sequence(
        &self,
        run_id: RunId,
        plan: &ExecutionPlan,
        steps: &[ControlStep],
        frame: &mut Frame,
        resources: &RunResourceSet,
        relational_backends: &RunRelationalBackends,
        cancellation: &CancellationToken,
        frame_depth: usize,
        parent_call: Option<ParentCallId>,
    ) -> Result<(), RunError> {
        let activation_id = ActivationId::next();
        let mut activated = BTreeSet::new();
        let mut operations = Vec::new();

        for step in steps {
            cancellation.check()?;
            match step {
                ControlStep::Operation(operation) => operations.push(*operation),
                ControlStep::Region(child) => {
                    self.execute_ready_operations(
                        run_id,
                        plan,
                        &operations,
                        activation_id,
                        &mut activated,
                        frame,
                        resources,
                        relational_backends,
                        cancellation,
                        parent_call,
                    )?;
                    operations.clear();
                    self.execute_region(
                        run_id,
                        plan,
                        child,
                        frame,
                        resources,
                        relational_backends,
                        cancellation,
                        frame_depth,
                        parent_call,
                    )?;
                }
            }
        }
        self.execute_ready_operations(
            run_id,
            plan,
            &operations,
            activation_id,
            &mut activated,
            frame,
            resources,
            relational_backends,
            cancellation,
            parent_call,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn execute_ready_operations(
        &self,
        run_id: RunId,
        plan: &ExecutionPlan,
        operations: &[OperationIndex],
        activation_id: ActivationId,
        activated: &mut BTreeSet<OperationIndex>,
        frame: &mut Frame,
        resources: &RunResourceSet,
        relational_backends: &RunRelationalBackends,
        cancellation: &CancellationToken,
        parent_call: Option<ParentCallId>,
    ) -> Result<(), RunError> {
        let mut pending = BTreeSet::new();
        for operation in operations {
            if !activated.insert(*operation) || !pending.insert(*operation) {
                return Err(RunError::OperationAlreadyExecuted {
                    operation: *operation,
                    activation: activation_id,
                });
            }
        }

        while !pending.is_empty() {
            cancellation.check()?;
            let ready = pending.iter().copied().find(|operation| {
                self.operation_is_ready(plan, *operation, activation_id, activated, frame)
            });
            let Some(operation) = ready else {
                return Err(self.blocked_operation_error(
                    plan,
                    *pending.first().expect("pending is not empty"),
                    activation_id,
                    activated,
                    frame,
                ));
            };
            self.execute_operation(
                run_id,
                plan,
                operation,
                activation_id,
                frame,
                resources,
                relational_backends,
                cancellation,
                parent_call,
            )?;
            pending.remove(&operation);
        }
        Ok(())
    }

    fn operation_is_ready(
        &self,
        plan: &ExecutionPlan,
        operation_index: OperationIndex,
        activation_id: ActivationId,
        activated: &BTreeSet<OperationIndex>,
        frame: &Frame,
    ) -> bool {
        let operation = &plan.operations[operation_index.index()];
        let inputs_ready = operation.inputs.iter().all(|input| frame.has(input.value));
        let values_ready = operation.outputs.iter().all(|output| {
            plan.value_dependencies
                .iter()
                .filter(|edge| edge.destination == output.value)
                .all(|edge| frame.has(edge.source))
        });
        let relational_ready = match &operation.kernel {
            PlannedKernel::Relational(index) => plan.relational_subplans[index.index()]
                .materialization_bridges
                .iter()
                .all(|bridge| {
                    frame.has_relational_fragment(
                        activation_id,
                        bridge.producer_subplan,
                        &bridge.producer_fragment,
                    )
                }),
            PlannedKernel::Native(_) => true,
        };
        let effects_ready = plan
            .effect_dependencies
            .iter()
            .filter(|edge| edge.after == operation_index)
            .all(|edge| {
                if activated.contains(&edge.before) {
                    frame.completed(activation_id, edge.before)
                } else {
                    frame.completion_count(edge.before) > frame.completion_count(operation_index)
                }
            });
        inputs_ready && values_ready && relational_ready && effects_ready
    }

    fn blocked_operation_error(
        &self,
        plan: &ExecutionPlan,
        operation_index: OperationIndex,
        activation_id: ActivationId,
        activated: &BTreeSet<OperationIndex>,
        frame: &Frame,
    ) -> RunError {
        let operation = &plan.operations[operation_index.index()];
        if let Some(input) = operation
            .inputs
            .iter()
            .find(|input| !frame.has(input.value))
        {
            return RunError::MissingValue(input.value);
        }
        if let Some(source) = operation.outputs.iter().find_map(|output| {
            plan.value_dependencies
                .iter()
                .find(|edge| edge.destination == output.value && !frame.has(edge.source))
                .map(|edge| edge.source)
        }) {
            return RunError::MissingValue(source);
        }
        if let PlannedKernel::Relational(index) = &operation.kernel {
            if let Some(bridge) = plan.relational_subplans[index.index()]
                .materialization_bridges
                .iter()
                .find(|bridge| {
                    !frame.has_relational_fragment(
                        activation_id,
                        bridge.producer_subplan,
                        &bridge.producer_fragment,
                    )
                })
            {
                return RunError::MissingRelationalFragment(bridge.producer_fragment.clone());
            }
        }
        if let Some(edge) = plan
            .effect_dependencies
            .iter()
            .filter(|edge| edge.after == operation_index)
            .find(|edge| {
                if activated.contains(&edge.before) {
                    !frame.completed(activation_id, edge.before)
                } else {
                    frame.completion_count(edge.before) <= frame.completion_count(operation_index)
                }
            })
        {
            return RunError::UnsatisfiedEffectDependency {
                operation: operation_index,
                required: edge.before,
            };
        }
        RunError::InvalidPlan("dependency gating produced no ready operation".into())
    }

    #[allow(clippy::too_many_arguments)]
    fn execute_operation(
        &self,
        run_id: RunId,
        plan: &ExecutionPlan,
        operation_index: OperationIndex,
        activation_id: ActivationId,
        frame: &mut Frame,
        resources: &RunResourceSet,
        relational_backends: &RunRelationalBackends,
        cancellation: &CancellationToken,
        parent_call: Option<ParentCallId>,
    ) -> Result<(), RunError> {
        let operation = &plan.operations[operation_index.index()];
        let correlation = CorrelationContext::compile(&plan.provenance)
            .for_run(run_id, parent_call)
            .for_node(
                operation.source_node_id,
                operation.source_node_type_id.clone(),
            );
        self.record_event(
            plan,
            correlation.clone(),
            RunEventKind::OperationStarted {
                operation_index: operation_index.index() as u32,
                activation_id: activation_id.get(),
            },
        );
        self.trace.record(SpanEvent::new(
            SpanKind::Operation,
            SpanStatus::Started,
            correlation.clone(),
        ));
        let result = self.execute_operation_inner(
            run_id,
            plan,
            operation_index,
            activation_id,
            frame,
            resources,
            relational_backends,
            cancellation,
        );
        self.trace.record(SpanEvent::new(
            SpanKind::Operation,
            run_status(&result),
            correlation.clone(),
        ));
        match &result {
            Ok(()) => {
                if let Some(results) = self.results {
                    for output in &operation.outputs {
                        let value = frame.value(output.value)?;
                        if let Some(descriptor) = results.publish_runtime_value(
                            run_id,
                            correlation.clone(),
                            plan.provenance.basis.clone(),
                            format!("value:{}", output.value.index()),
                            value,
                        ) {
                            self.record_event(
                                plan,
                                correlation.clone(),
                                RunEventKind::ValueReady {
                                    value_index: output.value.index() as u32,
                                    source_id: descriptor.source_id,
                                },
                            );
                        }
                    }
                }
                self.record_event(
                    plan,
                    correlation,
                    RunEventKind::OperationCompleted {
                        operation_index: operation_index.index() as u32,
                        activation_id: activation_id.get(),
                    },
                );
            }
            Err(error) => self.record_event(
                plan,
                correlation,
                RunEventKind::OperationErrored {
                    operation_index: operation_index.index() as u32,
                    activation_id: activation_id.get(),
                    code: RunErrorCode::from(error),
                },
            ),
        }
        result
    }

    fn record_event(
        &self,
        plan: &ExecutionPlan,
        correlation: CorrelationContext,
        kind: RunEventKind,
    ) {
        self.events.record(RunEvent {
            correlation,
            basis: plan.provenance.basis.clone(),
            kind,
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn execute_operation_inner(
        &self,
        run_id: RunId,
        plan: &ExecutionPlan,
        operation_index: OperationIndex,
        activation_id: ActivationId,
        frame: &mut Frame,
        resources: &RunResourceSet,
        relational_backends: &RunRelationalBackends,
        cancellation: &CancellationToken,
    ) -> Result<(), RunError> {
        cancellation.check()?;
        let memo_key = MemoKey {
            frame: frame.id,
            activation: activation_id,
            operation: operation_index,
        };
        if !frame.attempted.insert(memo_key) {
            return Err(RunError::OperationAlreadyExecuted {
                operation: operation_index,
                activation: activation_id,
            });
        }

        let operation = &plan.operations[operation_index.index()];
        let inputs = operation
            .inputs
            .iter()
            .map(|input| frame.value(input.value).cloned())
            .collect::<Result<Vec<_>, _>>()?;
        let outputs = match &operation.kernel {
            PlannedKernel::Native(handle) => {
                let kernel = self
                    .kernels
                    .get(handle)
                    .ok_or_else(|| RunError::KernelNotFound(handle.as_str().into()))?;
                let context = KernelContext {
                    run_id,
                    frame_id: frame.id,
                    activation_id,
                    params: &operation.params,
                    compiled_parameters: self.compiled_parameters,
                    resources,
                    cancellation,
                };
                kernel
                    .execute(&context, &inputs)
                    .map_err(|error| RunError::KernelFailed {
                        operation: operation_index,
                        message: error.0,
                    })?
            }
            PlannedKernel::Relational(index) => {
                let subplan = &plan.relational_subplans[index.index()];
                let mut bridge_inputs = Vec::with_capacity(subplan.materialization_bridges.len());
                for bridge in &subplan.materialization_bridges {
                    let value = frame
                        .relational_fragment(
                            activation_id,
                            bridge.producer_subplan,
                            &bridge.producer_fragment,
                        )?
                        .clone();
                    bridge_inputs.push(RelationalInput {
                        bridge: bridge.clone(),
                        value: materialize_bridge(bridge.bridge, value, cancellation)
                            .map_err(|error| RunError::BridgeFailed(error.0))?,
                    });
                }
                let backend = relational_backends
                    .get(&subplan.backend)
                    .ok_or_else(|| RunError::RelationalBackendNotFound(subplan.backend.clone()))?;
                let context = RelationalContext {
                    run_id,
                    resources,
                    cancellation,
                };
                let execution = match backend.execute(
                    &context,
                    &subplan.compiled_plan,
                    &inputs,
                    &bridge_inputs,
                ) {
                    Ok(execution) => execution,
                    Err(error) => {
                        cancellation.check()?;
                        return Err(RunError::RelationalFailed {
                            operation: operation_index,
                            message: error.0,
                        });
                    }
                };
                for (fragment, value) in execution.fragment_outputs {
                    frame.set_relational_fragment(activation_id, *index, fragment, value);
                }
                execution.outputs
            }
        };
        cancellation.check()?;
        if outputs.len() != operation.outputs.len() {
            return Err(RunError::OutputCount {
                operation: operation_index,
                expected: operation.outputs.len(),
                actual: outputs.len(),
            });
        }
        for (output, value) in operation.outputs.iter().zip(outputs) {
            frame.set(output.value, value)?;
        }
        frame.completed.insert(memo_key);
        *frame.completion_counts.entry(operation_index).or_default() += 1;
        Ok(())
    }
}

fn run_status<T>(result: &Result<T, RunError>) -> SpanStatus {
    match result {
        Ok(_) => SpanStatus::Succeeded,
        Err(RunError::Cancelled) => SpanStatus::Cancelled,
        Err(_) => SpanStatus::Failed,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct MemoKey {
    frame: FrameId,
    activation: ActivationId,
    operation: OperationIndex,
}

struct Frame {
    id: FrameId,
    values: Vec<Option<RuntimeValue>>,
    relational_fragments:
        BTreeMap<(ActivationId, RelationalSubplanIndex, RelationalFragmentId), RuntimeValue>,
    attempted: BTreeSet<MemoKey>,
    completed: BTreeSet<MemoKey>,
    completion_counts: BTreeMap<OperationIndex, usize>,
}

impl Frame {
    fn new(value_count: u32) -> Self {
        Self {
            id: FrameId::next(),
            values: vec![None; value_count as usize],
            relational_fragments: BTreeMap::new(),
            attempted: BTreeSet::new(),
            completed: BTreeSet::new(),
            completion_counts: BTreeMap::new(),
        }
    }

    fn has(&self, reference: ValueRef) -> bool {
        self.values
            .get(reference.index())
            .is_some_and(Option::is_some)
    }

    fn value(&self, reference: ValueRef) -> Result<&RuntimeValue, RunError> {
        self.values
            .get(reference.index())
            .and_then(Option::as_ref)
            .ok_or(RunError::MissingValue(reference))
    }

    fn set(&mut self, reference: ValueRef, value: RuntimeValue) -> Result<(), RunError> {
        let slot = self
            .values
            .get_mut(reference.index())
            .ok_or(RunError::MissingValue(reference))?;
        *slot = Some(value);
        Ok(())
    }

    fn copy(&mut self, source: ValueRef, destination: ValueRef) -> Result<(), RunError> {
        let value = self.value(source)?.clone();
        self.set(destination, value)
    }

    fn has_relational_fragment(
        &self,
        activation: ActivationId,
        subplan: RelationalSubplanIndex,
        fragment: &RelationalFragmentId,
    ) -> bool {
        self.relational_fragments
            .contains_key(&(activation, subplan, fragment.clone()))
    }

    fn relational_fragment(
        &self,
        activation: ActivationId,
        subplan: RelationalSubplanIndex,
        fragment: &RelationalFragmentId,
    ) -> Result<&RuntimeValue, RunError> {
        self.relational_fragments
            .get(&(activation, subplan, fragment.clone()))
            .ok_or_else(|| RunError::MissingRelationalFragment(fragment.clone()))
    }

    fn set_relational_fragment(
        &mut self,
        activation: ActivationId,
        subplan: RelationalSubplanIndex,
        fragment: RelationalFragmentId,
        value: RuntimeValue,
    ) {
        self.relational_fragments
            .insert((activation, subplan, fragment), value);
    }

    fn close_streams(&self) {
        for value in self.values.iter().flatten() {
            value.close_stream();
        }
        for value in self.relational_fragments.values() {
            value.close_stream();
        }
    }

    fn completed(&self, activation: ActivationId, operation: OperationIndex) -> bool {
        self.completed.contains(&MemoKey {
            frame: self.id,
            activation,
            operation,
        })
    }

    fn completion_count(&self, operation: OperationIndex) -> usize {
        self.completion_counts.get(&operation).copied().unwrap_or(0)
    }
}

fn boolean(frame: &Frame, reference: ValueRef) -> Result<bool, RunError> {
    match frame.value(reference)? {
        RuntimeValue::Scalar(Value::Bool(value)) => Ok(*value),
        _ => Err(RunError::InvalidCondition { value: reference }),
    }
}
