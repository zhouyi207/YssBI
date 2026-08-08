use super::relational::RunRelationalBackends;
use super::{
    ActivationId, CancellationToken, CompiledParameterStore, DemandFingerprint, FrameId,
    KernelContext, KernelErrorKind, KernelRegistry, NOOP_RUN_EVENT_SINK, OperationMemoKey,
    PendingResultSource, ProjectRunRegistry, PublishedFunctionPlan, RelationalBackendProvider,
    RelationalContext, ResourceErrorKind, ResourceProvider, ResultSourceDescriptor, ResultStore,
    RunError, RunErrorCode, RunEvent, RunEventKind, RunEventSink, RunMemoization, RunResourceSet,
    RunResult, RuntimeValue, execute_planned_adapter,
};
use crate::node_system::analysis::{
    CorrelationContext, NOOP_TRACE_SINK, ParentCallId, RedactionPolicy, ResourceVersionSet, RunId,
    SpanEvent, SpanKind, SpanStatus, TraceFieldSensitivity, TraceSink, TraceValue,
};
use crate::node_system::plan::{
    CallArgumentBinding, CallResultBinding, ControlStep, ExecutionPlan, FunctionPlanHandle,
    GraphOutputRef, OperationIndex, PlannedKernel, PlannedPublication, RelationalSubplanIndex,
    StructuredControlRegion, ValueRef,
};
use crate::node_system::protocol::{CachePolicy, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

const DEFAULT_RECURSION_LIMIT: usize = 64;
static NEXT_RUN_ID: AtomicU64 = AtomicU64::new(1);
static NEXT_PARENT_CALL_ID: AtomicU64 = AtomicU64::new(1);

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SchedulerCheckpoint {
    ResultSourceStaged,
    FinalResultPublication,
}

enum PendingSourceEvent {
    GraphResult {
        output: GraphOutputRef,
    },
    PinPreview {
        output: GraphOutputRef,
        generation: u64,
    },
}

struct PendingSourcePublication {
    source: PendingResultSource,
    event: PendingSourceEvent,
}

pub trait FunctionPlanProvider: Send + Sync {
    fn get_function(
        &self,
        handle: &FunctionPlanHandle,
    ) -> Result<Option<Arc<PublishedFunctionPlan>>, Box<str>>;

    fn recursion_limit(&self) -> usize {
        DEFAULT_RECURSION_LIMIT
    }
}

fn validate_published_call(
    caller: &ExecutionPlan,
    target: &FunctionPlanHandle,
    arguments: &[CallArgumentBinding],
    results: &[CallResultBinding],
    published: &PublishedFunctionPlan,
) -> Result<(), RunError> {
    let callee = published.plan.as_ref();
    let abi = published.abi.as_ref();
    let invalid = |message: String| RunError::FunctionPlanFailed(message.into());
    let source_facts = callee
        .validate_with_source_facts()
        .map_err(|error| invalid(format!("function execution plan is invalid: {error}")))?;
    if abi.provenance != callee.provenance {
        return Err(invalid(
            "function ABI provenance does not match its plan".into(),
        ));
    }
    if callee.provenance.graph_path.0.as_ref() != target.as_str() {
        return Err(invalid(
            "function plan target does not match the Call target".into(),
        ));
    }
    if callee.provenance.project_session_id != caller.provenance.project_session_id {
        return Err(invalid("function plan project session is stale".into()));
    }
    if callee.provenance.basis.registry_fingerprint != caller.provenance.basis.registry_fingerprint
    {
        return Err(invalid(
            "function plan Registry fingerprint is stale".into(),
        ));
    }
    if !callee
        .provenance
        .basis
        .resource_versions
        .iter()
        .all(|(key, version)| caller.provenance.basis.resource_versions.get(key) == Some(version))
    {
        return Err(invalid("function plan resource versions are stale".into()));
    }
    let parameter_values = abi.parameters.values().copied().collect::<BTreeSet<_>>();
    if parameter_values.len() != abi.parameters.len() {
        return Err(invalid(
            "function parameter ABI aliases frame values".into(),
        ));
    }
    if parameter_values
        .iter()
        .any(|value| !source_facts.is_external_input(*value))
    {
        return Err(invalid(
            "function parameter ABI is out of bounds or not ExternalInput".into(),
        ));
    }
    let result_values = abi.results.values().copied().collect::<BTreeSet<_>>();
    if result_values.len() != abi.results.len() {
        return Err(invalid("function result ABI aliases frame values".into()));
    }
    if result_values
        .iter()
        .any(|value| !source_facts.is_statically_sourced(*value))
    {
        return Err(invalid(
            "function result ABI is out of bounds or not statically producible".into(),
        ));
    }

    let argument_destinations = arguments
        .iter()
        .map(|binding| binding.callee_destination)
        .collect::<BTreeSet<_>>();
    if argument_destinations.len() != arguments.len() {
        return Err(invalid(
            "Call has duplicate callee argument destinations".into(),
        ));
    }
    if argument_destinations != parameter_values {
        return Err(invalid(
            "Call arguments do not exactly match the current function ABI".into(),
        ));
    }

    let callee_result_sources = results
        .iter()
        .map(|binding| binding.callee_source)
        .collect::<BTreeSet<_>>();
    if callee_result_sources.len() != results.len() {
        return Err(invalid("Call has duplicate callee result sources".into()));
    }
    if callee_result_sources != result_values {
        return Err(invalid(
            "Call results do not exactly match the current function ABI".into(),
        ));
    }
    let caller_result_destinations = results
        .iter()
        .map(|binding| binding.caller_destination)
        .collect::<BTreeSet<_>>();
    if caller_result_destinations.len() != results.len() {
        return Err(invalid(
            "Call has duplicate caller result destinations".into(),
        ));
    }
    Ok(())
}

pub struct RunExecutor<'a> {
    kernels: &'a KernelRegistry,
    resources: &'a dyn ResourceProvider,
    functions: &'a dyn FunctionPlanProvider,
    relational_backends: Option<&'a dyn RelationalBackendProvider>,
    compiled_parameters: Option<&'a CompiledParameterStore>,
    run_registry: Option<&'a ProjectRunRegistry>,
    selection_digest: Option<[u8; 32]>,
    recursion_limit: usize,
    trace: &'a dyn TraceSink,
    events: &'a dyn RunEventSink,
    results: Option<&'a ResultStore>,
    success_finalizer:
        Option<&'a dyn Fn(&mut RunResult, &CancellationToken) -> Result<(), RunError>>,
    #[cfg(test)]
    checkpoint:
        Option<Arc<dyn Fn(SchedulerCheckpoint, &CancellationToken) + Send + Sync + 'static>>,
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
            selection_digest: None,
            recursion_limit: functions.recursion_limit().max(1),
            trace: &NOOP_TRACE_SINK,
            events: &NOOP_RUN_EVENT_SINK,
            results: None,
            success_finalizer: None,
            #[cfg(test)]
            checkpoint: None,
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

    pub fn with_selection_digest(mut self, digest: [u8; 32]) -> Self {
        self.selection_digest = Some(digest);
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

    pub fn with_success_finalizer(
        mut self,
        finalizer: &'a dyn Fn(&mut RunResult, &CancellationToken) -> Result<(), RunError>,
    ) -> Self {
        self.success_finalizer = Some(finalizer);
        self
    }

    #[cfg(test)]
    pub(crate) fn with_test_checkpoint(
        mut self,
        checkpoint: Arc<dyn Fn(SchedulerCheckpoint, &CancellationToken) + Send + Sync + 'static>,
    ) -> Self {
        self.checkpoint = Some(checkpoint);
        self
    }

    #[cfg(test)]
    fn run_test_checkpoint(
        &self,
        checkpoint: SchedulerCheckpoint,
        cancellation: &CancellationToken,
    ) {
        if let Some(hook) = &self.checkpoint {
            hook(checkpoint, cancellation);
        }
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
        let mut correlation = CorrelationContext::compile(&plan.provenance).for_run(run_id, None);
        if let Some(digest) = self.selection_digest {
            correlation = correlation.with_selection_digest(digest);
        }
        self.record_event(plan, correlation.clone(), RunEventKind::RunStarted);
        self.trace.record(SpanEvent::new(
            SpanKind::Run,
            SpanStatus::Started,
            correlation.clone(),
        ));
        let mut result = self.run_root(plan, cancellation.clone(), run_id, correlation.clone());
        let mut pending_sources = Vec::new();
        if let (Some(results), Ok(run_result)) = (self.results, result.as_ref())
            && let Err(error) = self.stage_result_sources(
                results,
                plan,
                &correlation,
                run_result,
                &cancellation,
                &mut pending_sources,
            )
        {
            result = Err(error);
            pending_sources.clear();
        }
        #[cfg(test)]
        if result.is_ok() {
            self.run_test_checkpoint(SchedulerCheckpoint::FinalResultPublication, &cancellation);
        }
        if result.is_ok()
            && let Err(error) = cancellation.check()
        {
            result = Err(error);
            pending_sources.clear();
        }
        if let (Some(finalizer), Ok(run_result)) = (self.success_finalizer, result.as_mut())
            && let Err(error) = finalizer(run_result, &cancellation)
        {
            result = Err(error);
            pending_sources.clear();
        }
        if result.is_ok()
            && let Some(results) = self.results
        {
            self.commit_result_sources(results, run_id, pending_sources);
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
            if result.is_err() {
                results.release_run_sources(run_id);
            }
            results.cleanup_run(run_id);
        }
        result
    }

    fn stage_result_sources(
        &self,
        results: &ResultStore,
        plan: &ExecutionPlan,
        correlation: &CorrelationContext,
        run_result: &RunResult,
        cancellation: &CancellationToken,
        pending_sources: &mut Vec<PendingSourcePublication>,
    ) -> Result<(), RunError> {
        for publication in &plan.publications {
            let (name, value, event) = match publication {
                PlannedPublication::GraphResult { name, output, .. } => {
                    let value = run_result.values.get(name).ok_or_else(|| {
                        RunError::InvalidPlan(
                            "graph publication does not match a retained result value".into(),
                        )
                    })?;
                    (
                        name.clone(),
                        value,
                        PendingSourceEvent::GraphResult {
                            output: output.clone(),
                        },
                    )
                }
                PlannedPublication::PinPreview {
                    output,
                    generation,
                    value,
                } => {
                    let result = plan
                        .results
                        .iter()
                        .find(|result| result.output == *output && result.value == *value)
                        .ok_or_else(|| {
                            RunError::InvalidPlan(
                                "preview publication does not match its selected result".into(),
                            )
                        })?;
                    let value = run_result.values.get(&result.name).ok_or_else(|| {
                        RunError::InvalidPlan(
                            "preview publication result value is unavailable".into(),
                        )
                    })?;
                    (
                        format!("preview:{}", output.port).into(),
                        value,
                        PendingSourceEvent::PinPreview {
                            output: output.clone(),
                            generation: *generation,
                        },
                    )
                }
            };
            if let Some(source) = results.prepare_runtime_value(
                correlation.clone(),
                plan.provenance.basis.clone(),
                name,
                value,
            ) {
                pending_sources.push(PendingSourcePublication { source, event });
                #[cfg(test)]
                self.run_test_checkpoint(SchedulerCheckpoint::ResultSourceStaged, cancellation);
            }
            cancellation.check()?;
        }
        Ok(())
    }

    fn commit_result_sources(
        &self,
        results: &ResultStore,
        run_id: RunId,
        pending_sources: Vec<PendingSourcePublication>,
    ) {
        let (sources, events): (Vec<_>, Vec<_>) = pending_sources
            .into_iter()
            .map(|pending| (pending.source, pending.event))
            .unzip();
        let descriptors = results.commit_batch(run_id, sources);
        for (event, descriptor) in events.into_iter().zip(descriptors) {
            let Some(descriptor) = descriptor else {
                continue;
            };
            self.record_source_event(descriptor, event);
        }
    }

    fn record_source_event(&self, descriptor: ResultSourceDescriptor, event: PendingSourceEvent) {
        let ResultSourceDescriptor {
            source_id,
            name,
            correlation,
            basis,
            ..
        } = descriptor;
        let kind = match event {
            PendingSourceEvent::GraphResult { output } => {
                self.events.record(RunEvent {
                    correlation: correlation.clone(),
                    basis: basis.clone(),
                    kind: RunEventKind::ResultReady { name, source_id },
                });
                RunEventKind::OutputReady {
                    output,
                    generation: None,
                    source_id,
                }
            }
            PendingSourceEvent::PinPreview { output, generation } => RunEventKind::OutputReady {
                output,
                generation: Some(generation),
                source_id,
            },
        };
        self.events.record(RunEvent {
            correlation,
            basis,
            kind,
        });
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
        let memoization = RunMemoization::new();
        let root_demand = DemandFingerprint::for_root(plan, self.selection_digest);
        let result = (|| {
            self.execute_region(
                run_id,
                plan,
                &plan.root_region,
                &mut frame,
                &resource_set,
                &relational_backends,
                &memoization,
                &root_demand,
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
        memoization.finalize();
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
        memoization: &RunMemoization,
        demand: &DemandFingerprint,
        cancellation: &CancellationToken,
        frame_depth: usize,
        parent_call: Option<ParentCallId>,
    ) -> Result<(), RunError> {
        cancellation.check()?;
        frame.clear_region_values(plan, region);
        self.propagate_value_dependencies(plan, frame)?;
        match region {
            StructuredControlRegion::Sequence(steps) => self.execute_sequence(
                run_id,
                plan,
                steps,
                frame,
                resources,
                relational_backends,
                memoization,
                demand,
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
                    memoization,
                    demand,
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
                        memoization,
                        demand,
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
                ..
            } => {
                if frame_depth >= self.recursion_limit {
                    return Err(RunError::RecursionLimitExceeded {
                        recursion_limit: self.recursion_limit,
                    });
                }
                let published = self
                    .functions
                    .get_function(target)
                    .map_err(RunError::FunctionPlanFailed)?
                    .ok_or_else(|| RunError::FunctionPlanNotFound(target.as_str().into()))?;
                validate_published_call(plan, target, arguments, results, &published)?;
                let callee = Arc::clone(&published.plan);
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
                    let argument_values = arguments
                        .iter()
                        .map(|binding| {
                            Ok((
                                binding.callee_destination,
                                frame.value(binding.caller_source)?.clone(),
                            ))
                        })
                        .collect::<Result<Vec<_>, RunError>>()?;
                    let callee_resources = self.acquire_resources(&callee, &correlation)?;
                    let callee_backends = RunRelationalBackends::acquire(
                        &callee.relational_subplans,
                        self.relational_backends,
                        &callee_resources,
                        cancellation,
                    )?;
                    let mut callee_frame = Frame::new(callee.value_count);
                    let callee_demand =
                        DemandFingerprint::for_callee(&callee, target, arguments, results);
                    let result = (|| {
                        for (destination, value) in argument_values {
                            callee_frame.set(destination, value)?;
                        }
                        self.execute_region(
                            run_id,
                            &callee,
                            &callee.root_region,
                            &mut callee_frame,
                            &callee_resources,
                            &callee_backends,
                            memoization,
                            &callee_demand,
                            cancellation,
                            frame_depth + 1,
                            Some(call_id),
                        )?;
                        for binding in results {
                            frame.set(
                                binding.caller_destination,
                                callee_frame.value(binding.callee_source)?.clone(),
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
        memoization: &RunMemoization,
        demand: &DemandFingerprint,
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
                        memoization,
                        demand,
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
                        memoization,
                        demand,
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
            memoization,
            demand,
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
        memoization: &RunMemoization,
        demand: &DemandFingerprint,
        cancellation: &CancellationToken,
        parent_call: Option<ParentCallId>,
    ) -> Result<(), RunError> {
        self.propagate_value_dependencies(plan, frame)?;
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
                memoization,
                demand,
                cancellation,
                parent_call,
            )?;
            self.propagate_value_dependencies(plan, frame)?;
            pending.remove(&operation);
        }
        Ok(())
    }

    fn propagate_value_dependencies(
        &self,
        plan: &ExecutionPlan,
        frame: &mut Frame,
    ) -> Result<(), RunError> {
        let operation_outputs = plan
            .operations
            .iter()
            .flat_map(|operation| operation.outputs.iter().map(|output| output.value))
            .collect::<BTreeSet<_>>();
        for _ in 0..=plan.value_dependencies.len() {
            for dependency in &plan.value_dependencies {
                if !operation_outputs.contains(&dependency.destination)
                    && frame.has(dependency.source)
                {
                    frame.copy(dependency.source, dependency.destination)?;
                }
            }
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
        let inputs_ready = operation
            .inputs
            .iter()
            .all(|input| frame.has(input.value) || input.bound_value.is_some());
        let values_ready = operation.outputs.iter().all(|output| {
            plan.value_dependencies
                .iter()
                .filter(|edge| edge.destination == output.value)
                .all(|edge| frame.has(edge.source))
        });
        let relational_ready = true;
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
            .find(|input| !frame.has(input.value) && input.bound_value.is_none())
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
        memoization: &RunMemoization,
        demand: &DemandFingerprint,
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
            &correlation,
            plan,
            operation_index,
            activation_id,
            frame,
            resources,
            relational_backends,
            memoization,
            demand,
            cancellation,
        );
        self.trace.record(SpanEvent::new(
            SpanKind::Operation,
            run_status(&result),
            correlation.clone(),
        ));
        match &result {
            Ok(()) => {
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
        correlation: &CorrelationContext,
        plan: &ExecutionPlan,
        operation_index: OperationIndex,
        activation_id: ActivationId,
        frame: &mut Frame,
        resources: &RunResourceSet,
        relational_backends: &RunRelationalBackends,
        memoization: &RunMemoization,
        demand: &DemandFingerprint,
        cancellation: &CancellationToken,
    ) -> Result<(), RunError> {
        cancellation.check()?;
        let activation_key = MemoKey {
            frame: frame.id,
            activation: activation_id,
            operation: operation_index,
        };
        if !frame.attempted.insert(activation_key) {
            return Err(RunError::OperationAlreadyExecuted {
                operation: operation_index,
                activation: activation_id,
            });
        }

        let operation = &plan.operations[operation_index.index()];
        let inputs = operation
            .inputs
            .iter()
            .map(|input| {
                if frame.has(input.value) {
                    frame.value(input.value).cloned()
                } else if let Some(value) = &input.bound_value {
                    Ok(RuntimeValue::Scalar(value.clone()))
                } else {
                    frame.value(input.value).cloned()
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        let operation_memo_key = if operation.cache_policy == CachePolicy::PerRun
            && operation_memoization_safe(plan, operation_index)
        {
            operation_resource_versions(plan, operation_index).and_then(|resource_versions| {
                OperationMemoKey::from_inputs(
                    operation.stable_id.clone(),
                    &inputs,
                    resource_versions,
                    operation.semantics_version,
                    demand.clone(),
                )
            })
        } else {
            None
        };
        let produce = || -> Result<Box<[RuntimeValue]>, RunError> {
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
                    match kernel.execute(&context, &inputs) {
                        Ok(outputs) => outputs,
                        Err(error) if error.kind() == KernelErrorKind::Cancelled => {
                            return Err(RunError::Cancelled);
                        }
                        Err(error) => {
                            return Err(RunError::KernelFailed {
                                operation: operation_index,
                                message: error.message().into(),
                            });
                        }
                    }
                }
                PlannedKernel::Adapter(adapter) => {
                    let input = inputs.into_iter().next().ok_or_else(|| {
                        RunError::InvalidPlan("adapter operation has no input".into())
                    })?;
                    let output = execute_planned_adapter(adapter, input, cancellation)?;
                    vec![output]
                }
                PlannedKernel::Relational(index) => {
                    let subplan = &plan.relational_subplans[index.index()];
                    let backend = relational_backends.get(&subplan.backend).ok_or_else(|| {
                        RunError::RelationalBackendNotFound(subplan.backend.clone())
                    })?;
                    let context = RelationalContext {
                        run_id,
                        resources,
                        cancellation,
                    };
                    self.trace.record(relational_backend_event(
                        SpanStatus::Started,
                        correlation.clone(),
                        subplan.backend.as_str(),
                        *index,
                    ));
                    let backend_result = backend.execute(&context, &subplan.compiled_plan, &inputs);
                    let backend_status = if cancellation.check().is_err() {
                        SpanStatus::Cancelled
                    } else if backend_result.is_ok() {
                        SpanStatus::Succeeded
                    } else {
                        SpanStatus::Failed
                    };
                    self.trace.record(relational_backend_event(
                        backend_status,
                        correlation.clone(),
                        subplan.backend.as_str(),
                        *index,
                    ));
                    let execution = match backend_result {
                        Ok(execution) => execution,
                        Err(error) => {
                            cancellation.check()?;
                            return Err(RunError::from_relational(operation_index, error));
                        }
                    };
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
            Ok(outputs.into_boxed_slice())
        };
        let outputs = if let Some(key) = operation_memo_key {
            memoization.get_or_produce(key, cancellation, produce)?
        } else {
            produce()?
        };
        for (output, value) in operation.outputs.iter().zip(outputs) {
            frame.set(output.value, value)?;
        }
        frame.completed.insert(activation_key);
        *frame.completion_counts.entry(operation_index).or_default() += 1;
        Ok(())
    }
}

pub(super) fn operation_resource_versions(
    plan: &ExecutionPlan,
    operation: OperationIndex,
) -> Option<ResourceVersionSet> {
    plan.operations[operation.index()]
        .resource_dependencies
        .iter()
        .map(|key| {
            plan.provenance
                .basis
                .resource_versions
                .get(key)
                .cloned()
                .map(|version| (key.clone(), version))
        })
        .collect()
}

pub(super) fn operation_memoization_safe(plan: &ExecutionPlan, operation: OperationIndex) -> bool {
    match plan.operations[operation.index()].kernel {
        PlannedKernel::Native(_) => true,
        PlannedKernel::Adapter(_) => false,
        PlannedKernel::Relational(_) => true,
    }
}

fn relational_backend_event(
    status: SpanStatus,
    correlation: CorrelationContext,
    backend_id: &str,
    subplan_index: RelationalSubplanIndex,
) -> SpanEvent {
    SpanEvent::new(SpanKind::RelationalBackend, status, correlation)
        .with_field(
            "backendId",
            TraceValue::Text(backend_id.into()),
            TraceFieldSensitivity::Public,
            RedactionPolicy::strict(),
        )
        .with_field(
            "subplanIndex",
            TraceValue::Integer(subplan_index.index() as i64),
            TraceFieldSensitivity::Public,
            RedactionPolicy::strict(),
        )
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

    attempted: BTreeSet<MemoKey>,
    completed: BTreeSet<MemoKey>,
    completion_counts: BTreeMap<OperationIndex, usize>,
}

impl Frame {
    fn new(value_count: u32) -> Self {
        Self {
            id: FrameId::next(),
            values: vec![None; value_count as usize],

            attempted: BTreeSet::new(),
            completed: BTreeSet::new(),
            completion_counts: BTreeMap::new(),
        }
    }

    fn clear_region_values(&mut self, plan: &ExecutionPlan, region: &StructuredControlRegion) {
        let mut operations = BTreeSet::new();
        collect_region_operations(region, &mut operations);
        let mut cleared = operations
            .into_iter()
            .flat_map(|operation| {
                plan.operations[operation.index()]
                    .outputs
                    .iter()
                    .map(|output| output.value)
            })
            .collect::<BTreeSet<_>>();
        loop {
            let derived = plan
                .value_dependencies
                .iter()
                .filter(|dependency| cleared.contains(&dependency.source))
                .map(|dependency| dependency.destination)
                .filter(|destination| !cleared.contains(destination))
                .collect::<Vec<_>>();
            if derived.is_empty() {
                break;
            }
            cleared.extend(derived);
        }
        for reference in cleared {
            if let Some(slot) = self.values.get_mut(reference.index())
                && let Some(value) = slot.take()
            {
                value.close_stream();
            }
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

    fn close_streams(&self) {
        for value in self.values.iter().flatten() {
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

fn collect_region_operations(
    region: &StructuredControlRegion,
    operations: &mut BTreeSet<OperationIndex>,
) {
    match region {
        StructuredControlRegion::Sequence(steps) => {
            for step in steps {
                match step {
                    ControlStep::Operation(operation) => {
                        operations.insert(*operation);
                    }
                    ControlStep::Region(child) => collect_region_operations(child, operations),
                }
            }
        }
        StructuredControlRegion::If {
            then_region,
            else_region,
            ..
        } => {
            collect_region_operations(then_region, operations);
            collect_region_operations(else_region, operations);
        }
        StructuredControlRegion::Loop { body, .. } => {
            collect_region_operations(body, operations);
        }
        StructuredControlRegion::Call { .. } => {}
    }
}

fn boolean(frame: &Frame, reference: ValueRef) -> Result<bool, RunError> {
    match frame.value(reference)? {
        RuntimeValue::Scalar(Value::Bool(value)) => Ok(*value),
        _ => Err(RunError::InvalidCondition { value: reference }),
    }
}
