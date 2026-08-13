use super::relational::RunRelationalBackends;
use super::scheduling::ClassScheduler;
use super::{
    ACTIVATION_IDS, ActivationId, ActivationIdAllocator, CancellationToken, CompiledParameterStore,
    DemandFingerprint, EffectiveComputationSettings, FrameId, KernelContext, KernelErrorKind,
    KernelRegistry, NOOP_RUN_EVENT_SINK, OperationCompletion, OperationMemoKey,
    PendingResultSource, ProjectRunRegistry, PublishedFunctionPlan, RelationalBackendProvider,
    RelationalContext, ResourceErrorKind, ResourceProvider, ResultReportKind,
    ResultSourceDescriptor, ResultSourcePresentation, ResultStore, RunDeadline, RunError,
    RunErrorOutcome, RunEvent, RunEventKind, RunEventSink, RunMemoization, RunOptions, RunPhase,
    RunResourceBudgets, RunResourceOwner, RunResourceSet, RunResult, RuntimeValue,
    SchedulingPolicy, check_terminal, execute_planned_adapter, validate_data_series_type_expr,
};
use crate::node_system::analysis::{
    CorrelationContext, NOOP_TRACE_SINK, ParentCallId, ResourceVersionSet, RunId,
    SYSTEM_TRACE_CLOCK, SpanGuard, SpanId, SpanKind, SpanOutcome, SpanSpec, TraceSink, TraceSpan,
    complete_span_safely, start_span_safely,
};
use crate::node_system::document::PortRef;
use crate::node_system::plan::{
    AttemptId, CallArgumentBinding, CallResultBinding, ControlStep, ExecutionPlan,
    FunctionPlanHandle, GraphOutputRef, OperationIndex, PlannedKernel, PlannedPublication,
    PlannedValueKind, StructuredControlRegion, ValueRef, WorkloadClass,
};
use crate::node_system::protocol::{CachePolicy, RetryPolicy, Value};
use std::cell::Cell;
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::time::{Duration, Instant};

const DEFAULT_RECURSION_LIMIT: usize = 64;
static NEXT_RUN_ID: AtomicU64 = AtomicU64::new(1);
static NEXT_PARENT_CALL_ID: AtomicU64 = AtomicU64::new(1);

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SchedulerCheckpoint {
    ResultSourceStaged,
    FinalResultPublication,
    WorkerOutcomeProduced,
    AdmissionBlocked(WorkloadClass),
    RetryBackoff {
        operation: OperationIndex,
        activation: ActivationId,
        attempt: AttemptId,
    },
    AttemptPrepared {
        operation: OperationIndex,
        activation: ActivationId,
        attempt: AttemptId,
    },
    AdmissionBookkept {
        operation: OperationIndex,
        activation: ActivationId,
        attempt: AttemptId,
    },
    AdmissionRolledBack {
        operation: OperationIndex,
        attempt: AttemptId,
        running_count: usize,
        tracked_running: usize,
        memo_owned: bool,
        frame_attempt: Option<AttemptId>,
    },
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

impl PendingSourceEvent {
    fn output(&self) -> &GraphOutputRef {
        match self {
            Self::GraphResult { output } | Self::PinPreview { output, .. } => output,
        }
    }
}

fn result_presentation(plan: &ExecutionPlan, output: &GraphOutputRef) -> ResultSourcePresentation {
    let PortRef::Declared { key } = &output.port.port else {
        return ResultSourcePresentation::Inspector;
    };
    if key.as_str() != "report" {
        return ResultSourcePresentation::Inspector;
    }
    let Some(value) = plan
        .results
        .iter()
        .find(|result| result.output == *output)
        .map(|result| result.value)
    else {
        return ResultSourcePresentation::Inspector;
    };
    let Some(node_type) = plan
        .operations
        .iter()
        .find(|operation| {
            operation
                .outputs
                .iter()
                .any(|candidate| candidate.value == value)
        })
        .map(|operation| operation.source_node_type_id.as_str())
    else {
        return ResultSourcePresentation::Inspector;
    };
    let report = match node_type {
        "yssbi.statistics.ols.summary"
        | "yssbi.statistics.gls.summary"
        | "yssbi.statistics.wls.summary" => ResultReportKind::OlsSummary,
        "yssbi.statistics.logit.summary" | "yssbi.statistics.probit.summary" => {
            ResultReportKind::BinarySummary
        }
        "yssbi.statistics.iv.2sls.summary" => ResultReportKind::Iv2slsSummary,
        "yssbi.statistics.iv.liml.summary" => ResultReportKind::IvLimlSummary,
        "yssbi.statistics.prais.summary" => ResultReportKind::PraisSummary,
        "yssbi.statistics.var.summary" => ResultReportKind::VarSummary,
        "yssbi.statistics.var.lag_order" => ResultReportKind::VarSoc,
        "yssbi.statistics.panel.summary" => ResultReportKind::PanelSummary,
        "yssbi.statistics.panel.did.twfe" => ResultReportKind::PanelDid,
        "yssbi.statistics.adf.summary" => ResultReportKind::DfAdfSummary,
        "yssbi.statistics.vec.fit" => ResultReportKind::VecSummary,
        "yssbi.statistics.vec.rank_test" => ResultReportKind::VecRankSummary,
        _ => return ResultSourcePresentation::Inspector,
    };
    ResultSourcePresentation::Report { report }
}

struct PendingSourcePublication {
    source: PendingResultSource,
    event: PendingSourceEvent,
}

struct PreparedOperation {
    operation: OperationIndex,
    owner_activation: ActivationId,
    activation: ActivationId,
    attempt: AttemptId,
    inputs: Box<[RuntimeValue]>,
    memo_key: Option<OperationMemoKey>,
    owns_memo_flight: bool,
    class: WorkloadClass,
}

struct DelayedRetry {
    eligible_at: Instant,
    tie_break: u64,
    operation: OperationIndex,
    owner_activation: ActivationId,
    attempt: AttemptId,
    inputs: Box<[RuntimeValue]>,
    memo_key: Option<OperationMemoKey>,
    class: WorkloadClass,
}

impl PartialEq for DelayedRetry {
    fn eq(&self, other: &Self) -> bool {
        (self.eligible_at, self.tie_break) == (other.eligible_at, other.tie_break)
    }
}

impl Eq for DelayedRetry {}

impl PartialOrd for DelayedRetry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DelayedRetry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.eligible_at, self.tie_break).cmp(&(other.eligible_at, other.tie_break))
    }
}

struct RunningOperation {
    class: WorkloadClass,
    owner_activation: ActivationId,
    activation: ActivationId,
    attempt: AttemptId,
    inputs: Box<[RuntimeValue]>,
    memo_key: Option<OperationMemoKey>,
}

struct AdmissionBookkeeping {
    operation: OperationIndex,
    class: WorkloadClass,
    activation_key: MemoKey,
    previous_attempt: Option<AttemptId>,
    memo_key: Option<OperationMemoKey>,
}

struct OperationWorkerContext<'a> {
    run_id: RunId,
    frame_id: FrameId,
    computation_settings: EffectiveComputationSettings,
    plan: &'a ExecutionPlan,
    kernels: &'a KernelRegistry,
    compiled_parameters: Option<&'a CompiledParameterStore>,
    resources: &'a RunResourceSet,
    resource_owner: &'a RunResourceOwner,
    relational_backends: &'a RunRelationalBackends,
    cancellation: &'a CancellationToken,
    deadline: Option<RunDeadline>,
    run_parent_span_id: SpanId,
    parent_call: Option<ParentCallId>,
    #[cfg(test)]
    checkpoint:
        Option<&'a Arc<dyn Fn(SchedulerCheckpoint, &CancellationToken) + Send + Sync + 'static>>,
}

struct WorkerCompletion {
    completed_at: Instant,
    completion: OperationCompletion,
    trace_spans: Box<[TraceSpan]>,
    panic: Option<Box<dyn std::any::Any + Send>>,
}

struct SchedulerSignal {
    generation: Mutex<u64>,
    ready: Arc<Condvar>,
}

impl SchedulerSignal {
    fn new(cancellation: &CancellationToken) -> Self {
        let ready = Arc::new(Condvar::new());
        cancellation.register_waiter(&ready);
        Self {
            generation: Mutex::new(0),
            ready,
        }
    }

    fn notify(&self) {
        let mut generation = self
            .generation
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        *generation = generation.saturating_add(1);
        drop(generation);
        self.ready.notify_all();
    }
}

#[derive(Default)]
struct WorkerTrace(Mutex<Vec<TraceSpan>>);

impl WorkerTrace {
    fn into_spans(self) -> Box<[TraceSpan]> {
        self.0
            .into_inner()
            .unwrap_or_else(|error| error.into_inner())
            .into_boxed_slice()
    }
}

impl TraceSink for WorkerTrace {
    fn start_span(&self, spec: SpanSpec) -> SpanGuard<'_> {
        SpanGuard::new(self, spec, &SYSTEM_TRACE_CLOCK)
    }

    fn complete_span(&self, span: TraceSpan) {
        self.0
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .push(span);
    }
}

struct WorkerQueue<T> {
    capacity: usize,
    state: Mutex<WorkerQueueState<T>>,
    ready: Condvar,
    space: Condvar,
    cancellation: CancellationToken,
    deadline: Option<RunDeadline>,
}

struct WorkerQueueState<T> {
    closed: bool,
    jobs: VecDeque<T>,
}

struct WorkerQueueCloseGuard<'a, T>(&'a WorkerQueue<T>);

impl<T> Drop for WorkerQueueCloseGuard<'_, T> {
    fn drop(&mut self) {
        self.0.close();
    }
}

impl<T> WorkerQueue<T> {
    fn new(
        capacity: usize,
        cancellation: CancellationToken,
        deadline: Option<RunDeadline>,
    ) -> Self {
        Self {
            capacity,
            state: Mutex::new(WorkerQueueState {
                closed: false,
                jobs: VecDeque::new(),
            }),
            ready: Condvar::new(),
            space: Condvar::new(),
            cancellation,
            deadline,
        }
    }

    fn push(&self, job: T) -> Result<(), T> {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        if self.deadline.is_none() {
            state = self
                .space
                .wait_while(state, |state| {
                    !state.closed && state.jobs.len() == self.capacity
                })
                .unwrap_or_else(|error| error.into_inner());
        } else {
            while !state.closed && state.jobs.len() == self.capacity {
                let Some(deadline) = self.deadline else {
                    unreachable!("untimed queue wait handled above");
                };
                let Ok(remaining) = deadline.remaining(&self.cancellation, RunPhase::QueueWait)
                else {
                    return Err(job);
                };
                let (next, _) = self
                    .space
                    .wait_timeout(state, remaining)
                    .unwrap_or_else(|error| error.into_inner());
                state = next;
            }
            if check_terminal(&self.cancellation, self.deadline, RunPhase::QueueWait).is_err() {
                return Err(job);
            }
        }
        if state.closed {
            return Err(job);
        }
        state.jobs.push_back(job);
        drop(state);
        self.ready.notify_one();
        Ok(())
    }

    fn pop(&self) -> Option<T> {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        if self.deadline.is_none() {
            state = self
                .ready
                .wait_while(state, |state| !state.closed && state.jobs.is_empty())
                .unwrap_or_else(|error| error.into_inner());
        } else {
            while !state.closed && state.jobs.is_empty() {
                let Some(deadline) = self.deadline else {
                    unreachable!("untimed queue wait handled above");
                };
                let Ok(remaining) = deadline.remaining(&self.cancellation, RunPhase::QueueWait)
                else {
                    return None;
                };
                let (next, _) = self
                    .ready
                    .wait_timeout(state, remaining)
                    .unwrap_or_else(|error| error.into_inner());
                state = next;
            }
        }
        let job = state.jobs.pop_front();
        if job.is_some() {
            self.space.notify_one();
        }
        job
    }

    fn close(&self) {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        state.closed = true;
        drop(state);
        self.ready.notify_all();
        self.space.notify_all();
    }
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
    if abi.results.keys().collect::<BTreeSet<_>>()
        != abi.result_productions.keys().collect::<BTreeSet<_>>()
    {
        return Err(invalid(
            "function result ABI production keys are stale".into(),
        ));
    }
    for (result, value) in &abi.results {
        let declared = abi.result_productions[result];
        if source_facts.production(*value) != Some(declared) {
            return Err(invalid(
                "function result ABI production does not match its plan".into(),
            ));
        }
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
    for binding in results {
        let expected = abi
            .results
            .iter()
            .find_map(|(result, value)| {
                (*value == binding.callee_source).then(|| abi.result_productions[result])
            })
            .expect("exact result value sets were checked above");
        if binding.production != Some(expected) {
            return Err(invalid(
                "Call result production does not match the current function ABI".into(),
            ));
        }
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

struct RunTraceCoverage<'a> {
    trace: &'a dyn TraceSink,
    correlation: CorrelationContext,
    resource_acquire: Cell<bool>,
    result_publication: Cell<bool>,
    cleanup: Cell<bool>,
}

impl<'a> RunTraceCoverage<'a> {
    fn new(trace: &'a dyn TraceSink, correlation: CorrelationContext) -> Self {
        Self {
            trace,
            correlation,
            resource_acquire: Cell::new(false),
            result_publication: Cell::new(false),
            cleanup: Cell::new(false),
        }
    }

    fn start_span(&self, kind: SpanKind) -> SpanGuard<'a> {
        self.phase(kind).set(true);
        start_span_safely(
            self.trace,
            SpanSpec {
                parent_span_id: self.correlation.trace_parent_span_id,
                run_id: self.correlation.run_id,
                operation_id: None,
                activation_id: None,
                attempt_id: None,
                kind,
                correlation: self.correlation.clone(),
            },
        )
    }

    fn ensure_not_reached(&self, kind: SpanKind) {
        let phase = self.phase(kind);
        if phase.replace(true) {
            return;
        }
        let mut span = start_span_safely(
            self.trace,
            SpanSpec {
                parent_span_id: self.correlation.trace_parent_span_id,
                run_id: self.correlation.run_id,
                operation_id: None,
                activation_id: None,
                attempt_id: None,
                kind,
                correlation: self.correlation.clone(),
            },
        );
        span.finish(SpanOutcome::NotReached);
    }

    fn ensure_all_not_reached(&self) {
        self.ensure_not_reached(SpanKind::ResourceAcquire);
        self.ensure_not_reached(SpanKind::ResultPublication);
        self.ensure_not_reached(SpanKind::Cleanup);
    }

    fn phase(&self, kind: SpanKind) -> &Cell<bool> {
        match kind {
            SpanKind::ResourceAcquire => &self.resource_acquire,
            SpanKind::ResultPublication => &self.result_publication,
            SpanKind::Cleanup => &self.cleanup,
            _ => unreachable!("coverage tracks only run phase spans"),
        }
    }
}

pub struct RunExecutor<'a> {
    kernels: &'a KernelRegistry,
    resources: &'a dyn ResourceProvider,
    functions: &'a dyn FunctionPlanProvider,
    relational_backends: Option<&'a dyn RelationalBackendProvider>,
    compiled_parameters: Option<&'a CompiledParameterStore>,
    run_registry: Option<&'a ProjectRunRegistry>,
    selection_digest: Option<[u8; 32]>,
    computation_settings: EffectiveComputationSettings,
    recursion_limit: usize,
    trace: &'a dyn TraceSink,
    events: &'a dyn RunEventSink,
    results: Option<&'a ResultStore>,
    success_finalizer: Option<
        &'a dyn Fn(&mut RunResult, &CancellationToken, Option<RunDeadline>) -> Result<(), RunError>,
    >,
    atomic_success_preparer: Option<
        &'a dyn Fn(&mut RunResult, &CancellationToken, Option<RunDeadline>) -> Result<(), RunError>,
    >,
    atomic_success_finalizer: Option<
        &'a dyn Fn(&mut RunResult, &CancellationToken, Option<RunDeadline>) -> Result<(), RunError>,
    >,
    options: RunOptions,
    activation_ids: &'a ActivationIdAllocator,
    #[cfg(test)]
    spill_root: Option<std::path::PathBuf>,
    #[cfg(test)]
    cleanup_delay: Option<std::time::Duration>,
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
            computation_settings: EffectiveComputationSettings::default(),
            recursion_limit: functions.recursion_limit().max(1),
            trace: &NOOP_TRACE_SINK,
            events: &NOOP_RUN_EVENT_SINK,
            results: None,
            success_finalizer: None,
            atomic_success_preparer: None,
            atomic_success_finalizer: None,
            options: RunOptions::default(),
            activation_ids: &ACTIVATION_IDS,
            #[cfg(test)]
            spill_root: None,
            #[cfg(test)]
            cleanup_delay: None,
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

    pub fn with_computation_settings_snapshot(
        mut self,
        settings: &crate::project::ProjectComputationSettings,
    ) -> Self {
        self.computation_settings = EffectiveComputationSettings::from(settings);
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

    pub fn with_resource_budgets(mut self, budgets: RunResourceBudgets) -> Self {
        self.options.budgets = budgets;
        self
    }

    pub fn with_scheduling_policy(mut self, policy: SchedulingPolicy) -> Self {
        self.options.scheduling = policy;
        self
    }

    pub fn with_deadline(mut self, deadline: RunDeadline) -> Self {
        self.options.deadline = Some(deadline);
        self
    }

    pub fn with_options(mut self, options: RunOptions) -> Self {
        self.options = options;
        self
    }

    #[cfg(test)]
    pub(crate) fn with_activation_allocator_for_test(
        mut self,
        allocator: &'a ActivationIdAllocator,
    ) -> Self {
        self.activation_ids = allocator;
        self
    }

    #[cfg(test)]
    pub(crate) fn with_cleanup_delay_for_test(mut self, delay: std::time::Duration) -> Self {
        self.cleanup_delay = Some(delay);
        self
    }

    #[cfg(test)]
    pub(crate) fn with_test_spill_root(mut self, spill_root: std::path::PathBuf) -> Self {
        self.spill_root = Some(spill_root);
        self
    }

    pub fn with_success_finalizer(
        mut self,
        finalizer: &'a dyn Fn(
            &mut RunResult,
            &CancellationToken,
            Option<RunDeadline>,
        ) -> Result<(), RunError>,
    ) -> Self {
        self.success_finalizer = Some(finalizer);
        self
    }

    pub fn with_atomic_success_finalizer(
        mut self,
        finalizer: &'a dyn Fn(
            &mut RunResult,
            &CancellationToken,
            Option<RunDeadline>,
        ) -> Result<(), RunError>,
    ) -> Self {
        self.atomic_success_finalizer = Some(finalizer);
        self
    }

    pub fn with_atomic_success_transaction(
        mut self,
        preparer: &'a dyn Fn(
            &mut RunResult,
            &CancellationToken,
            Option<RunDeadline>,
        ) -> Result<(), RunError>,
        finalizer: &'a dyn Fn(
            &mut RunResult,
            &CancellationToken,
            Option<RunDeadline>,
        ) -> Result<(), RunError>,
    ) -> Self {
        self.atomic_success_preparer = Some(preparer);
        self.atomic_success_finalizer = Some(finalizer);
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
        let run_id = RunId::try_new(NEXT_RUN_ID.fetch_add(1, Ordering::Relaxed))
            .map_err(|_| RunError::InvalidPlan("run identity space exhausted".into()))?;
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
        let mut run_span = start_span_safely(
            self.trace,
            SpanSpec {
                parent_span_id: None,
                run_id: Some(run_id),
                operation_id: None,
                activation_id: None,
                attempt_id: None,
                kind: SpanKind::Run,
                correlation: correlation.clone(),
            },
        );
        correlation = correlation.with_trace_parent(run_span.span_id());
        let trace_coverage = RunTraceCoverage::new(self.trace, correlation.clone());
        #[cfg(test)]
        let resource_owner = match &self.spill_root {
            Some(root) => RunResourceOwner::with_spill_root_and_deadline(
                run_id,
                self.options.budgets,
                cancellation.clone(),
                self.options.deadline,
                root.clone(),
            ),
            None => RunResourceOwner::new_with_deadline(
                run_id,
                self.options.budgets,
                cancellation.clone(),
                self.options.deadline,
            ),
        };
        #[cfg(not(test))]
        let resource_owner = RunResourceOwner::new_with_deadline(
            run_id,
            self.options.budgets,
            cancellation.clone(),
            self.options.deadline,
        );
        let result = resource_owner.and_then(|resource_owner| {
            #[cfg(test)]
            if let Some(delay) = self.cleanup_delay {
                resource_owner.register_cleanup_delay_for_test(delay);
            }
            let execution = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                self.finish_run(
                    plan,
                    cancellation.clone(),
                    run_id,
                    correlation.clone(),
                    &resource_owner,
                    &trace_coverage,
                )
            }));
            match execution {
                Ok(result) => {
                    trace_coverage.ensure_not_reached(SpanKind::ResourceAcquire);
                    trace_coverage.ensure_not_reached(SpanKind::ResultPublication);
                    self.record_resource_cleanup(&resource_owner, &trace_coverage, false);
                    if result.is_ok() {
                        check_terminal(&cancellation, self.options.deadline, RunPhase::Cleanup)?;
                    }
                    result
                }
                Err(payload) => {
                    trace_coverage.ensure_not_reached(SpanKind::ResourceAcquire);
                    trace_coverage.ensure_not_reached(SpanKind::ResultPublication);
                    self.record_resource_cleanup(&resource_owner, &trace_coverage, true);
                    std::panic::resume_unwind(payload)
                }
            }
        });
        trace_coverage.ensure_all_not_reached();
        run_span.finish(span_outcome(&result));
        let event = match &result {
            Ok(_) => RunEventKind::RunCompleted,
            Err(RunError::Cancelled) => RunEventKind::RunCancelled,
            Err(error) => RunEventKind::RunErrored {
                outcome: RunErrorOutcome::from(error),
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

    fn finish_run(
        &self,
        plan: &ExecutionPlan,
        cancellation: CancellationToken,
        run_id: RunId,
        correlation: CorrelationContext,
        resource_owner: &RunResourceOwner,
        trace_coverage: &RunTraceCoverage<'_>,
    ) -> Result<RunResult, RunError> {
        check_terminal(&cancellation, self.options.deadline, RunPhase::Kernel)?;
        let execution_result = self.run_root(
            plan,
            cancellation.clone(),
            run_id,
            correlation.clone(),
            resource_owner,
            trace_coverage,
        );
        let Ok(run_result) = execution_result else {
            return execution_result;
        };
        let mut publication_span = trace_coverage.start_span(SpanKind::ResultPublication);
        let result = (|| {
            let mut result = Ok(run_result);
            if result.is_ok()
                && let Err(error) = check_terminal(
                    &cancellation,
                    self.options.deadline,
                    RunPhase::ResultPublication,
                )
            {
                result = Err(error);
            }
            if let Ok(run_result) = result.as_ref()
                && let Err(error) =
                    promote_run_result(run_result, &cancellation, self.options.deadline)
            {
                result = Err(error);
            }
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
                self.run_test_checkpoint(
                    SchedulerCheckpoint::FinalResultPublication,
                    &cancellation,
                );
            }
            if result.is_ok()
                && let Err(error) = check_terminal(
                    &cancellation,
                    self.options.deadline,
                    RunPhase::ResultPublication,
                )
            {
                result = Err(error);
                pending_sources.clear();
            }
            if let (Some(finalizer), Ok(run_result)) = (self.success_finalizer, result.as_mut())
                && let Err(error) = finalizer(run_result, &cancellation, self.options.deadline)
            {
                pending_sources.clear();
                result = Err(error);
            }
            if let (Some(preparer), Ok(run_result)) =
                (self.atomic_success_preparer, result.as_mut())
                && let Err(error) = preparer(run_result, &cancellation, self.options.deadline)
            {
                pending_sources.clear();
                result = Err(error);
            }
            if let (Some(finalizer), Some(results), Ok(run_result)) =
                (self.atomic_success_finalizer, self.results, result.as_mut())
            {
                let (sources, source_events): (Vec<_>, Vec<_>) = pending_sources
                    .drain(..)
                    .map(|pending| (pending.source, pending.event))
                    .unzip();
                let mut publication = results.begin_publication(run_id, sources);
                publication.prepare(&cancellation, self.options.deadline)?;
                match publication.publish_with_authority(
                    &cancellation,
                    self.options.deadline,
                    |_| finalizer(run_result, &cancellation, self.options.deadline),
                ) {
                    Ok(descriptors) => {
                        for (event, descriptor) in source_events.into_iter().zip(descriptors) {
                            if let Some(descriptor) = descriptor {
                                self.record_source_event(descriptor, event);
                            }
                        }
                    }
                    Err(error) => result = Err(error),
                }
            }
            if result.is_ok()
                && self.atomic_success_finalizer.is_none()
                && let Some(results) = self.results
            {
                let committed_publications = self.commit_result_sources(
                    results,
                    run_id,
                    pending_sources,
                    &cancellation,
                    self.success_finalizer.is_some(),
                )?;
                for (descriptor, event) in committed_publications {
                    self.record_source_event(descriptor, event);
                }
            }
            result
        })();
        publication_span.finish(span_outcome(&result));
        result
    }

    fn record_resource_cleanup(
        &self,
        resource_owner: &RunResourceOwner,
        trace_coverage: &RunTraceCoverage<'_>,
        panicking: bool,
    ) {
        let mut span = trace_coverage.start_span(SpanKind::Cleanup);
        let cleanup_errors = resource_owner.cleanup();
        for error in &cleanup_errors {
            tauri_plugin_log::log::warn!(
                target: "yssbi::node_system::runtime::cleanup",
                "{error}"
            );
        }
        span.finish(SpanOutcome::Cleanup {
            error_count: cleanup_errors.len() as u64,
            panicking,
        });
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
            let presentation = result_presentation(plan, event.output());
            if let Some(source) = results.prepare_runtime_value_with_presentation(
                correlation.clone(),
                plan.provenance.basis.clone(),
                name,
                presentation,
                value,
            ) {
                pending_sources.push(PendingSourcePublication { source, event });
                #[cfg(test)]
                self.run_test_checkpoint(SchedulerCheckpoint::ResultSourceStaged, cancellation);
            }
            check_terminal(
                cancellation,
                self.options.deadline,
                RunPhase::ResultPublication,
            )?;
        }
        Ok(())
    }

    fn commit_result_sources(
        &self,
        results: &ResultStore,
        run_id: RunId,
        pending_sources: Vec<PendingSourcePublication>,
        cancellation: &CancellationToken,
        finalizer_linearized: bool,
    ) -> Result<Vec<(ResultSourceDescriptor, PendingSourceEvent)>, RunError> {
        let (sources, events): (Vec<_>, Vec<_>) = pending_sources
            .into_iter()
            .map(|pending| (pending.source, pending.event))
            .unzip();
        let descriptors = if finalizer_linearized {
            results.commit_batch(run_id, sources)
        } else {
            results.commit_batch_with_deadline(
                run_id,
                sources,
                cancellation,
                self.options.deadline,
            )?
        };
        Ok(events
            .into_iter()
            .zip(descriptors)
            .filter_map(|(event, descriptor)| descriptor.map(|descriptor| (descriptor, event)))
            .collect())
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
        resource_owner: &RunResourceOwner,
        trace_coverage: &RunTraceCoverage<'_>,
    ) -> Result<RunResult, RunError> {
        plan.validate()
            .map_err(|error| RunError::InvalidPlan(error.to_string().into()))?;
        cancellation.check()?;
        let resource_set = self.acquire_resources(plan, trace_coverage)?;
        let relational_backends = RunRelationalBackends::acquire(
            &plan.relational_subplans,
            self.relational_backends,
            &resource_set,
            &cancellation,
        )?;
        let mut frame = Frame::new(plan.value_count);
        frame.bind(&plan.bound_values)?;
        let memoization = RunMemoization::new();
        let root_demand = DemandFingerprint::for_root(plan, self.selection_digest);
        let result = (|| {
            self.execute_region(
                run_id,
                plan,
                &plan.root_region,
                &mut frame,
                &resource_set,
                resource_owner,
                &relational_backends,
                &memoization,
                &root_demand,
                &cancellation,
                1,
                correlation
                    .trace_parent_span_id
                    .expect("run span parent is set"),
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
        trace_coverage: &RunTraceCoverage<'_>,
    ) -> Result<RunResourceSet, RunError> {
        let mut span = trace_coverage.start_span(SpanKind::ResourceAcquire);
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
        span.finish(span_outcome(&result));
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
        resource_owner: &RunResourceOwner,
        relational_backends: &RunRelationalBackends,
        memoization: &RunMemoization,
        demand: &DemandFingerprint,
        cancellation: &CancellationToken,
        frame_depth: usize,
        run_parent_span_id: SpanId,
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
                resource_owner,
                relational_backends,
                memoization,
                demand,
                cancellation,
                frame_depth,
                run_parent_span_id,
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
                    resource_owner,
                    relational_backends,
                    memoization,
                    demand,
                    cancellation,
                    frame_depth,
                    run_parent_span_id,
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
                        resource_owner,
                        relational_backends,
                        memoization,
                        demand,
                        cancellation,
                        frame_depth,
                        run_parent_span_id,
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
                let mut correlation =
                    CorrelationContext::compile(&callee.provenance).for_run(run_id, Some(call_id));
                let call_parent_span_id = (callee.provenance.graph_path
                    == plan.provenance.graph_path)
                    .then_some(run_parent_span_id);
                let mut call_span = start_span_safely(
                    self.trace,
                    SpanSpec {
                        parent_span_id: call_parent_span_id,
                        run_id: Some(run_id),
                        operation_id: None,
                        activation_id: None,
                        attempt_id: None,
                        kind: SpanKind::Run,
                        correlation: correlation.clone(),
                    },
                );
                correlation = correlation.with_trace_parent(call_span.span_id());
                let call_trace_coverage = RunTraceCoverage::new(self.trace, correlation.clone());
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
                    let callee_resources = self.acquire_resources(&callee, &call_trace_coverage)?;
                    let callee_backends = RunRelationalBackends::acquire(
                        &callee.relational_subplans,
                        self.relational_backends,
                        &callee_resources,
                        cancellation,
                    )?;
                    let mut callee_frame = Frame::new(callee.value_count);
                    callee_frame.bind(&callee.bound_values)?;
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
                            resource_owner,
                            &callee_backends,
                            memoization,
                            &callee_demand,
                            cancellation,
                            frame_depth + 1,
                            correlation
                                .trace_parent_span_id
                                .expect("callee run span parent is set"),
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
                call_trace_coverage.ensure_not_reached(SpanKind::ResourceAcquire);
                call_trace_coverage.ensure_not_reached(SpanKind::ResultPublication);
                let mut cleanup_span = call_trace_coverage.start_span(SpanKind::Cleanup);
                cleanup_span.finish(SpanOutcome::Cleanup {
                    error_count: 0,
                    panicking: false,
                });
                call_span.finish(span_outcome(&call_result));
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
        resource_owner: &RunResourceOwner,
        relational_backends: &RunRelationalBackends,
        memoization: &RunMemoization,
        demand: &DemandFingerprint,
        cancellation: &CancellationToken,
        frame_depth: usize,
        run_parent_span_id: SpanId,
        parent_call: Option<ParentCallId>,
    ) -> Result<(), RunError> {
        let activation_id = self.activation_ids.allocate()?;
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
                        resource_owner,
                        relational_backends,
                        memoization,
                        demand,
                        cancellation,
                        run_parent_span_id,
                        parent_call,
                    )?;
                    operations.clear();
                    self.execute_region(
                        run_id,
                        plan,
                        child,
                        frame,
                        resources,
                        resource_owner,
                        relational_backends,
                        memoization,
                        demand,
                        cancellation,
                        frame_depth,
                        run_parent_span_id,
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
            resource_owner,
            relational_backends,
            memoization,
            demand,
            cancellation,
            run_parent_span_id,
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
        resource_owner: &RunResourceOwner,
        relational_backends: &RunRelationalBackends,
        memoization: &RunMemoization,
        demand: &DemandFingerprint,
        cancellation: &CancellationToken,
        run_parent_span_id: SpanId,
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

        let mut prepared = BTreeMap::new();
        let mut queued = BTreeSet::new();
        let mut running = BTreeMap::new();
        let mut delayed_retries: BinaryHeap<Reverse<DelayedRetry>> = BinaryHeap::new();
        let mut delayed_operations = BTreeSet::new();
        let mut next_retry_tie = 0_u64;
        let mut memo_inflight = BTreeSet::new();
        let mut admission = ClassScheduler::new(self.options.scheduling);
        let worker_count = self.options.scheduling.worker_count();
        let job_queue: WorkerQueue<PreparedOperation> =
            WorkerQueue::new(worker_count, cancellation.clone(), self.options.deadline);
        let (completion_sender, completion_receiver) = mpsc::sync_channel(worker_count);
        let scheduler_signal = SchedulerSignal::new(cancellation);
        let mut worker_panic = None;
        let worker_context = OperationWorkerContext {
            run_id,
            frame_id: frame.id,
            computation_settings: self.computation_settings,
            plan,
            kernels: self.kernels,
            compiled_parameters: self.compiled_parameters,
            resources,
            resource_owner,
            relational_backends,
            cancellation,
            deadline: self.options.deadline,
            run_parent_span_id,
            parent_call,
            #[cfg(test)]
            checkpoint: self.checkpoint.as_ref(),
        };

        let result = std::thread::scope(|scope| {
            let _queue_close = WorkerQueueCloseGuard(&job_queue);
            for _ in 0..worker_count {
                let sender = completion_sender.clone();
                let queue = &job_queue;
                let context = &worker_context;
                let signal = &scheduler_signal;
                scope.spawn(move || {
                    while let Some(job) = queue.pop() {
                        let operation = job.operation;
                        let activation = job.activation;
                        let attempt = job.attempt;
                        let trace = WorkerTrace::default();
                        let (outputs, panic, completed_at) =
                            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                let outputs = execute_operation_worker(context, job, &trace);
                                (outputs, Instant::now())
                            })) {
                                Ok((outputs, completed_at)) => (outputs, None, completed_at),
                                Err(payload) => (
                                    Err(RunError::InvalidPlan("operation worker panicked".into())),
                                    Some(payload),
                                    Instant::now(),
                                ),
                            };
                        #[cfg(test)]
                        if let Some(checkpoint) = context.checkpoint {
                            checkpoint(
                                SchedulerCheckpoint::WorkerOutcomeProduced,
                                context.cancellation,
                            );
                        }
                        if sender
                            .send(WorkerCompletion {
                                completed_at,
                                completion: OperationCompletion {
                                    operation,
                                    activation,
                                    attempt,
                                    outputs,
                                },
                                trace_spans: trace.into_spans(),
                                panic,
                            })
                            .is_err()
                        {
                            break;
                        }
                        signal.notify();
                    }
                });
            }

            let scheduler_result = (|| {
                let mut terminal_error = None;
                loop {
                    if terminal_error.is_none() {
                        let phase =
                            if admission.has_queued() || !prepared.is_empty() || !queued.is_empty()
                            {
                                RunPhase::QueueWait
                            } else {
                                RunPhase::Kernel
                            };
                        if let Err(error) =
                            check_terminal(cancellation, self.options.deadline, phase)
                        {
                            terminal_error = Some(error);
                        }
                    }
                    if terminal_error.is_none() && cancellation.is_cancelled() && running.is_empty()
                    {
                        terminal_error = Some(RunError::Cancelled);
                    }

                    if terminal_error.is_none() && !cancellation.is_cancelled() {
                        while delayed_retries
                            .peek()
                            .is_some_and(|Reverse(retry)| retry.eligible_at <= Instant::now())
                        {
                            let Reverse(retry) =
                                delayed_retries.pop().expect("peeked delayed retry exists");
                            delayed_operations.remove(&retry.operation);
                            if let Err(error) = check_terminal(
                                cancellation,
                                self.options.deadline,
                                RunPhase::QueueWait,
                            ) {
                                if let Some(key) = &retry.memo_key {
                                    memo_inflight.remove(key);
                                }
                                terminal_error = Some(error);
                                break;
                            }
                            let activation = match self.activation_ids.allocate() {
                                Ok(activation) => activation,
                                Err(error) => {
                                    if let Some(key) = &retry.memo_key {
                                        memo_inflight.remove(key);
                                    }
                                    terminal_error = Some(error);
                                    break;
                                }
                            };
                            let prepared_retry = PreparedOperation {
                                operation: retry.operation,
                                owner_activation: retry.owner_activation,
                                activation,
                                attempt: retry.attempt,
                                inputs: retry.inputs,
                                memo_key: retry.memo_key,
                                owns_memo_flight: true,
                                class: retry.class,
                            };
                            #[cfg(test)]
                            self.run_test_checkpoint(
                                SchedulerCheckpoint::AttemptPrepared {
                                    operation: prepared_retry.operation,
                                    activation: prepared_retry.activation,
                                    attempt: prepared_retry.attempt,
                                },
                                cancellation,
                            );
                            prepared.insert(prepared_retry.operation, prepared_retry);
                        }

                        for operation in pending.iter().copied().collect::<Vec<_>>() {
                            if prepared.contains_key(&operation)
                                || queued.contains(&operation)
                                || running.contains_key(&operation)
                                || delayed_operations.contains(&operation)
                                || !self.operation_is_ready(
                                    plan,
                                    operation,
                                    activation_id,
                                    activated,
                                    frame,
                                )
                            {
                                continue;
                            }
                            match self.prepare_operation(
                                plan,
                                operation,
                                activation_id,
                                frame,
                                demand,
                            ) {
                                Ok(operation) => {
                                    #[cfg(test)]
                                    self.run_test_checkpoint(
                                        SchedulerCheckpoint::AttemptPrepared {
                                            operation: operation.operation,
                                            activation: operation.activation,
                                            attempt: operation.attempt,
                                        },
                                        cancellation,
                                    );
                                    prepared.insert(operation.operation, operation);
                                }
                                Err(error) => terminal_error = Some(error),
                            }
                        }

                        for (operation, job) in &prepared {
                            if queued.contains(operation)
                                || job.memo_key.as_ref().is_some_and(|key| {
                                    memo_inflight.contains(key) && !job.owns_memo_flight
                                })
                            {
                                continue;
                            }
                            admission.enqueue(*operation, job.class);
                            queued.insert(*operation);
                        }

                        while let Some((operation, class)) = admission.admit() {
                            queued.remove(&operation);
                            let job = prepared
                                .remove(&operation)
                                .expect("admitted operations are prepared");

                            if let Some(outputs) = job
                                .memo_key
                                .as_ref()
                                .and_then(|key| memoization.completed(key))
                            {
                                self.bookkeep_admission(
                                    frame,
                                    &mut admission,
                                    &mut running,
                                    &mut memo_inflight,
                                    &job,
                                    class,
                                    false,
                                );
                                let correlation =
                                    operation_correlation(plan, run_id, parent_call, operation);
                                self.record_operation_started(
                                    plan,
                                    correlation,
                                    operation,
                                    job.activation,
                                    job.attempt,
                                );
                                self.finish_operation_completion(
                                    plan,
                                    frame,
                                    memoization,
                                    &mut admission,
                                    &mut running,
                                    &mut prepared,
                                    &mut delayed_retries,
                                    &mut delayed_operations,
                                    &mut next_retry_tie,
                                    &mut memo_inflight,
                                    &mut pending,
                                    &mut terminal_error,
                                    cancellation,
                                    parent_call,
                                    run_id,
                                    &mut worker_panic,
                                    WorkerCompletion {
                                        completed_at: Instant::now(),
                                        completion: OperationCompletion {
                                            operation,
                                            activation: job.activation,
                                            attempt: job.attempt,
                                            outputs: Ok(outputs),
                                        },
                                        trace_spans: Box::new([]),
                                        panic: None,
                                    },
                                );
                                continue;
                            }
                            if let Err(error) = self.submit_admitted_operation(
                                plan,
                                frame,
                                &mut admission,
                                &mut running,
                                &mut memo_inflight,
                                &job_queue,
                                job,
                                class,
                                cancellation,
                                parent_call,
                                run_id,
                            ) {
                                terminal_error.get_or_insert(error);
                                break;
                            }
                        }
                        #[cfg(test)]
                        if let Some(class) = admission.blocked_class() {
                            self.run_test_checkpoint(
                                SchedulerCheckpoint::AdmissionBlocked(class),
                                cancellation,
                            );
                        }
                    }

                    let mut drained_completion = false;
                    while let Ok(completion) = completion_receiver.try_recv() {
                        drained_completion = true;
                        self.finish_operation_completion(
                            plan,
                            frame,
                            memoization,
                            &mut admission,
                            &mut running,
                            &mut prepared,
                            &mut delayed_retries,
                            &mut delayed_operations,
                            &mut next_retry_tie,
                            &mut memo_inflight,
                            &mut pending,
                            &mut terminal_error,
                            cancellation,
                            parent_call,
                            run_id,
                            &mut worker_panic,
                            completion,
                        );
                    }
                    if drained_completion && terminal_error.is_none() {
                        continue;
                    }

                    if running.is_empty() {
                        if let Some(error) = terminal_error {
                            return Err(error);
                        }
                        if pending.is_empty() {
                            return Ok(());
                        }
                        if !admission.has_queued()
                            && prepared.is_empty()
                            && delayed_retries.is_empty()
                        {
                            return Err(self.blocked_operation_error(
                                plan,
                                *pending.first().expect("pending is not empty"),
                                activation_id,
                                activated,
                                frame,
                            ));
                        }
                    }

                    if terminal_error.is_some() && !running.is_empty() {
                        match completion_receiver.recv() {
                            Ok(completion) => self.finish_operation_completion(
                                plan,
                                frame,
                                memoization,
                                &mut admission,
                                &mut running,
                                &mut prepared,
                                &mut delayed_retries,
                                &mut delayed_operations,
                                &mut next_retry_tie,
                                &mut memo_inflight,
                                &mut pending,
                                &mut terminal_error,
                                cancellation,
                                parent_call,
                                run_id,
                                &mut worker_panic,
                                completion,
                            ),
                            Err(_) => {
                                terminal_error.get_or_insert_with(|| {
                                    RunError::InvalidPlan(
                                        "operation completion channel closed".into(),
                                    )
                                });
                            }
                        }
                        continue;
                    }

                    if terminal_error.is_none()
                        && (!running.is_empty() || !delayed_retries.is_empty())
                    {
                        let phase = if admission.has_queued() || !delayed_retries.is_empty() {
                            RunPhase::QueueWait
                        } else {
                            RunPhase::Kernel
                        };
                        let retry_wait = delayed_retries.peek().map(|Reverse(retry)| {
                            retry.eligible_at.saturating_duration_since(Instant::now())
                        });
                        let deadline_wait = match self.options.deadline {
                            Some(deadline) => match deadline.remaining(cancellation, phase) {
                                Ok(remaining) => Some(remaining),
                                Err(error) => {
                                    terminal_error = Some(error);
                                    continue;
                                }
                            },
                            None => None,
                        };
                        let timeout = match (retry_wait, deadline_wait) {
                            (Some(retry), Some(deadline)) => Some(retry.min(deadline)),
                            (Some(retry), None) => Some(retry),
                            (None, Some(deadline)) => Some(deadline),
                            (None, None) => None,
                        };
                        if timeout.is_some_and(|timeout| timeout.is_zero()) {
                            continue;
                        }

                        let generation = scheduler_signal
                            .generation
                            .lock()
                            .unwrap_or_else(|error| error.into_inner());
                        let observed = *generation;
                        if let Ok(completion) = completion_receiver.try_recv() {
                            drop(generation);
                            self.finish_operation_completion(
                                plan,
                                frame,
                                memoization,
                                &mut admission,
                                &mut running,
                                &mut prepared,
                                &mut delayed_retries,
                                &mut delayed_operations,
                                &mut next_retry_tie,
                                &mut memo_inflight,
                                &mut pending,
                                &mut terminal_error,
                                cancellation,
                                parent_call,
                                run_id,
                                &mut worker_panic,
                                completion,
                            );
                            continue;
                        }
                        if cancellation.is_cancelled() {
                            drop(generation);
                            continue;
                        }
                        if let Some(timeout) = timeout {
                            let _ = scheduler_signal
                                .ready
                                .wait_timeout_while(generation, timeout, |generation| {
                                    *generation == observed && !cancellation.is_cancelled()
                                })
                                .unwrap_or_else(|error| error.into_inner());
                        } else {
                            let _guard = scheduler_signal
                                .ready
                                .wait_while(generation, |generation| {
                                    *generation == observed && !cancellation.is_cancelled()
                                })
                                .unwrap_or_else(|error| error.into_inner());
                        }
                        continue;
                    }
                }
            })();
            scheduler_result
        });
        if let Some(payload) = worker_panic {
            std::panic::resume_unwind(payload);
        }
        result
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

    fn prepare_operation(
        &self,
        plan: &ExecutionPlan,
        operation_index: OperationIndex,
        activation_id: ActivationId,
        frame: &mut Frame,
        demand: &DemandFingerprint,
    ) -> Result<PreparedOperation, RunError> {
        let attempt = AttemptId::initial();
        let operation = &plan.operations[operation_index.index()];
        let inputs = operation
            .inputs
            .iter()
            .map(|input| {
                let value = if frame.has(input.value) {
                    frame.value(input.value).cloned()
                } else if let Some(value) = &input.bound_value {
                    Ok(RuntimeValue::Scalar(value.clone()))
                } else {
                    frame.value(input.value).cloned()
                }?;
                if input.contract.kind == PlannedValueKind::DataSeries {
                    let RuntimeValue::Artifact(artifact) = &value else {
                        return Err(RunError::InvalidPlan(
                            format!(
                                "DataSeries input value {} did not receive a DataSeries Artifact",
                                input.value.index()
                            )
                            .into(),
                        ));
                    };
                    let metadata = artifact.data_series_metadata().ok_or_else(|| {
                        RunError::InvalidPlan(
                            format!(
                                "DataSeries input value {} did not receive a DataSeries Artifact",
                                input.value.index()
                            )
                            .into(),
                        )
                    })?;
                    validate_data_series_type_expr(metadata, &input.contract.type_expr)
                        .map_err(|error| RunError::InvalidPlan(error.to_string().into()))?;
                }
                Ok(value)
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_boxed_slice();
        let memo_key = if operation.cache_policy == CachePolicy::PerRun
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
        Ok(PreparedOperation {
            operation: operation_index,
            owner_activation: activation_id,
            activation: activation_id,
            attempt,
            inputs,
            memo_key,
            owns_memo_flight: false,
            class: operation.workload,
        })
    }

    fn bookkeep_admission(
        &self,
        frame: &mut Frame,
        admission: &mut ClassScheduler,
        running: &mut BTreeMap<OperationIndex, RunningOperation>,
        memo_inflight: &mut BTreeSet<OperationMemoKey>,
        job: &PreparedOperation,
        class: WorkloadClass,
        track_memo: bool,
    ) -> AdmissionBookkeeping {
        let activation_key = MemoKey {
            frame: frame.id,
            activation: job.owner_activation,
            operation: job.operation,
        };
        let previous_attempt = frame.attempted.insert(activation_key, job.attempt);
        if job.attempt == AttemptId::initial() {
            debug_assert!(previous_attempt.is_none());
        } else {
            debug_assert_eq!(
                previous_attempt.and_then(AttemptId::next_checked),
                Some(job.attempt)
            );
        }
        let previous_running = running.insert(
            job.operation,
            RunningOperation {
                class,
                owner_activation: job.owner_activation,
                activation: job.activation,
                attempt: job.attempt,
                inputs: job.inputs.clone(),
                memo_key: job.memo_key.clone(),
            },
        );
        debug_assert!(previous_running.is_none());
        if track_memo && let Some(key) = &job.memo_key {
            if job.owns_memo_flight {
                debug_assert!(memo_inflight.contains(key));
            } else {
                debug_assert!(memo_inflight.insert(key.clone()));
            }
        }
        debug_assert_eq!(admission.running_count(), running.len());
        AdmissionBookkeeping {
            operation: job.operation,
            class,
            activation_key,
            previous_attempt,
            memo_key: if track_memo {
                job.memo_key.clone()
            } else {
                None
            },
        }
    }

    fn rollback_admission(
        &self,
        frame: &mut Frame,
        admission: &mut ClassScheduler,
        running: &mut BTreeMap<OperationIndex, RunningOperation>,
        memo_inflight: &mut BTreeSet<OperationMemoKey>,
        bookkeeping: AdmissionBookkeeping,
        _attempt: AttemptId,
        _cancellation: &CancellationToken,
    ) {
        let removed = running.remove(&bookkeeping.operation);
        debug_assert!(removed.is_some());
        admission.release(bookkeeping.class);
        match bookkeeping.previous_attempt {
            Some(previous) => {
                frame.attempted.insert(bookkeeping.activation_key, previous);
            }
            None => {
                frame.attempted.remove(&bookkeeping.activation_key);
            }
        }
        if let Some(key) = &bookkeeping.memo_key {
            memo_inflight.remove(key);
        }
        debug_assert_eq!(admission.running_count(), running.len());
        #[cfg(test)]
        self.run_test_checkpoint(
            SchedulerCheckpoint::AdmissionRolledBack {
                operation: bookkeeping.operation,
                attempt: _attempt,
                running_count: admission.running_count(),
                tracked_running: running.len(),
                memo_owned: bookkeeping
                    .memo_key
                    .as_ref()
                    .is_some_and(|key| memo_inflight.contains(key)),
                frame_attempt: frame.attempted.get(&bookkeeping.activation_key).copied(),
            },
            _cancellation,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn submit_admitted_operation(
        &self,
        plan: &ExecutionPlan,
        frame: &mut Frame,
        admission: &mut ClassScheduler,
        running: &mut BTreeMap<OperationIndex, RunningOperation>,
        memo_inflight: &mut BTreeSet<OperationMemoKey>,
        job_queue: &WorkerQueue<PreparedOperation>,
        job: PreparedOperation,
        class: WorkloadClass,
        cancellation: &CancellationToken,
        parent_call: Option<ParentCallId>,
        run_id: RunId,
    ) -> Result<(), RunError> {
        let operation = job.operation;
        let activation = job.activation;
        let attempt = job.attempt;
        let bookkeeping =
            self.bookkeep_admission(frame, admission, running, memo_inflight, &job, class, true);
        #[cfg(test)]
        self.run_test_checkpoint(
            SchedulerCheckpoint::AdmissionBookkept {
                operation,
                activation,
                attempt,
            },
            cancellation,
        );
        if job_queue.push(job).is_err() {
            self.rollback_admission(
                frame,
                admission,
                running,
                memo_inflight,
                bookkeeping,
                attempt,
                cancellation,
            );
            return Err(
                check_terminal(cancellation, self.options.deadline, RunPhase::QueueWait)
                    .err()
                    .unwrap_or_else(|| {
                        RunError::InvalidPlan("operation worker queue closed".into())
                    }),
            );
        }
        let correlation = operation_correlation(plan, run_id, parent_call, operation);
        self.record_operation_started(plan, correlation, operation, activation, attempt);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn finish_operation_completion(
        &self,
        plan: &ExecutionPlan,
        frame: &mut Frame,
        memoization: &RunMemoization,
        admission: &mut ClassScheduler,
        running: &mut BTreeMap<OperationIndex, RunningOperation>,
        _prepared: &mut BTreeMap<OperationIndex, PreparedOperation>,
        delayed_retries: &mut BinaryHeap<Reverse<DelayedRetry>>,
        delayed_operations: &mut BTreeSet<OperationIndex>,
        next_retry_tie: &mut u64,
        memo_inflight: &mut BTreeSet<OperationMemoKey>,
        pending: &mut BTreeSet<OperationIndex>,
        terminal_error: &mut Option<RunError>,
        cancellation: &CancellationToken,
        parent_call: Option<ParentCallId>,
        run_id: RunId,
        worker_panic: &mut Option<Box<dyn std::any::Any + Send>>,
        mut envelope: WorkerCompletion,
    ) {
        let completed_at = envelope.completed_at;
        let completion = envelope.completion;
        let Some(active) = running.get(&completion.operation) else {
            return;
        };
        if active.activation != completion.activation || active.attempt != completion.attempt {
            return;
        }
        let completed_before_deadline = self
            .options
            .deadline
            .is_none_or(|deadline| !deadline.exceeded_at(completed_at));
        let produced_ordinary_error = completion.outputs.as_ref().is_err_and(|error| {
            !matches!(
                error,
                RunError::Cancelled | RunError::DeadlineExceeded { .. }
            )
        });
        let active = running
            .remove(&completion.operation)
            .expect("validated running operation exists");
        admission.release(active.class);
        let mut suppress_completion = false;
        if matches!(terminal_error, Some(RunError::DeadlineExceeded { .. })) {
            if cancellation.is_cancelled() {
                *terminal_error = Some(RunError::Cancelled);
            }
            if completed_before_deadline
                && ordinary_precedes_cancellation(
                    produced_ordinary_error,
                    completed_at,
                    cancellation,
                )
            {
                *terminal_error = completion.outputs.clone().err();
            } else {
                if let Some(key) = &active.memo_key {
                    memo_inflight.remove(key);
                }
                suppress_completion = true;
            }
        } else if !completed_before_deadline {
            *terminal_error = Some(if cancellation.is_cancelled() {
                RunError::Cancelled
            } else {
                RunError::DeadlineExceeded {
                    phase: RunPhase::Kernel,
                }
            });
            if let Some(key) = &active.memo_key {
                memo_inflight.remove(key);
            }
            suppress_completion = true;
        } else if terminal_error.is_none()
            && !produced_ordinary_error
            && let Err(error) =
                check_terminal(cancellation, self.options.deadline, RunPhase::Kernel)
        {
            *terminal_error = Some(error);
        }
        if terminal_error.is_none()
            && cancellation.is_cancelled()
            && !ordinary_precedes_cancellation(produced_ordinary_error, completed_at, cancellation)
        {
            *terminal_error = Some(RunError::Cancelled);
        }
        apply_authoritative_attempt_outcome(
            &mut envelope.trace_spans,
            &plan.operations[completion.operation.index()].stable_id,
            &completion,
            completed_at,
            cancellation.cancelled_at(),
            self.options.deadline,
        );
        for span in envelope.trace_spans {
            complete_span_safely(self.trace, span);
        }
        if worker_panic.is_none() {
            *worker_panic = envelope.panic;
        }
        if suppress_completion {
            return;
        }

        let correlation = operation_correlation(plan, run_id, parent_call, completion.operation);
        if terminal_error.is_some() {
            if matches!(terminal_error, Some(RunError::Cancelled))
                && completion
                    .outputs
                    .as_ref()
                    .is_err_and(|error| !matches!(error, RunError::Cancelled))
                && ordinary_precedes_cancellation(
                    produced_ordinary_error,
                    completed_at,
                    cancellation,
                )
            {
                *terminal_error = completion.outputs.clone().err();
            }
            self.record_operation_terminal_event(plan, correlation, &completion);
            if let Some(key) = &active.memo_key {
                memo_inflight.remove(key);
            }
            return;
        }

        let retry_policy = plan.operations[completion.operation.index()]
            .retry
            .policy
            .filter(|_| {
                completion.outputs.as_ref().is_err_and(|error| {
                    matches!(
                        error,
                        RunError::KernelFailed {
                            kind: KernelErrorKind::Transient,
                            ..
                        }
                    )
                })
            })
            .filter(|policy| completion.attempt.get() < u64::from(policy.max_attempts.get()));
        if let Some(policy) = retry_policy {
            self.record_operation_terminal_event(plan, correlation, &completion);
            let backoff = retry_backoff(policy, completion.attempt);
            let Some(eligible_at) = Instant::now().checked_add(backoff) else {
                if let Some(key) = &active.memo_key {
                    memo_inflight.remove(key);
                }
                *terminal_error = Some(RunError::DeadlineExceeded {
                    phase: RunPhase::QueueWait,
                });
                return;
            };
            if cancellation.is_cancelled()
                || self
                    .options
                    .deadline
                    .is_some_and(|deadline| deadline.exceeded_at(eligible_at))
            {
                if let Some(key) = &active.memo_key {
                    memo_inflight.remove(key);
                }
                *terminal_error = Some(if cancellation.is_cancelled() {
                    RunError::Cancelled
                } else {
                    RunError::DeadlineExceeded {
                        phase: RunPhase::QueueWait,
                    }
                });
                return;
            }
            let Some(attempt) = completion.attempt.next_checked() else {
                if let Some(key) = &active.memo_key {
                    memo_inflight.remove(key);
                }
                *terminal_error = Some(RunError::InvalidPlan(
                    "retry attempt identity overflowed".into(),
                ));
                return;
            };
            let tie_break = *next_retry_tie;
            let Some(next_tie) = tie_break.checked_add(1) else {
                if let Some(key) = &active.memo_key {
                    memo_inflight.remove(key);
                }
                *terminal_error = Some(RunError::InvalidPlan(
                    "delayed retry tie-break identity overflowed".into(),
                ));
                return;
            };
            *next_retry_tie = next_tie;
            #[cfg(test)]
            self.run_test_checkpoint(
                SchedulerCheckpoint::RetryBackoff {
                    operation: completion.operation,
                    activation: completion.activation,
                    attempt: completion.attempt,
                },
                cancellation,
            );
            delayed_operations.insert(completion.operation);
            delayed_retries.push(Reverse(DelayedRetry {
                eligible_at,
                tie_break,
                operation: completion.operation,
                owner_activation: active.owner_activation,
                attempt,
                inputs: active.inputs,
                memo_key: active.memo_key,
                class: active.class,
            }));
            return;
        }

        if let Some(key) = &active.memo_key {
            memo_inflight.remove(key);
        }
        match completion.outputs {
            Ok(outputs) => {
                if cancellation.is_cancelled() {
                    *terminal_error = Some(RunError::Cancelled);
                    return;
                }
                let activation_key = MemoKey {
                    frame: frame.id,
                    activation: active.owner_activation,
                    operation: completion.operation,
                };
                if frame.attempted.get(&activation_key) != Some(&completion.attempt)
                    || frame.completed.contains(&activation_key)
                {
                    return;
                }
                if let Some(key) = active.memo_key {
                    memoization.commit_completed(key, &outputs);
                }
                let operation = &plan.operations[completion.operation.index()];
                for (output, value) in operation.outputs.iter().zip(outputs) {
                    if let Err(error) = frame.set(output.value, value) {
                        *terminal_error = Some(error);
                        cancellation.cancel();
                        return;
                    }
                }
                frame.completed.insert(activation_key);
                *frame
                    .completion_counts
                    .entry(completion.operation)
                    .or_default() += 1;
                pending.remove(&completion.operation);
                if let Err(error) = self.propagate_value_dependencies(plan, frame) {
                    *terminal_error = Some(error);
                    cancellation.cancel();
                    return;
                }
                self.record_event(
                    plan,
                    correlation,
                    RunEventKind::OperationCompleted {
                        operation_index: completion.operation.index() as u32,
                        activation_id: completion.activation.get(),
                        attempt_id: completion.attempt.get(),
                    },
                );
            }
            Err(error) => {
                self.record_event(
                    plan,
                    correlation,
                    RunEventKind::OperationErrored {
                        operation_index: completion.operation.index() as u32,
                        activation_id: completion.activation.get(),
                        attempt_id: completion.attempt.get(),
                        outcome: RunErrorOutcome::from(&error),
                    },
                );
                *terminal_error = Some(error);
                cancellation.cancel();
            }
        }
    }

    fn record_operation_started(
        &self,
        plan: &ExecutionPlan,
        correlation: CorrelationContext,
        operation: OperationIndex,
        activation: ActivationId,
        attempt: AttemptId,
    ) {
        self.record_event(
            plan,
            correlation.clone(),
            RunEventKind::OperationStarted {
                operation_index: operation.index() as u32,
                activation_id: activation.get(),
                attempt_id: attempt.get(),
            },
        );
    }

    fn record_operation_terminal_event(
        &self,
        plan: &ExecutionPlan,
        correlation: CorrelationContext,
        completion: &OperationCompletion,
    ) {
        let kind = match &completion.outputs {
            Ok(_) => RunEventKind::OperationCompleted {
                operation_index: completion.operation.index() as u32,
                activation_id: completion.activation.get(),
                attempt_id: completion.attempt.get(),
            },
            Err(error) => RunEventKind::OperationErrored {
                operation_index: completion.operation.index() as u32,
                activation_id: completion.activation.get(),
                attempt_id: completion.attempt.get(),
                outcome: RunErrorOutcome::from(error),
            },
        };
        self.record_event(plan, correlation, kind);
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
}

fn ordinary_precedes_cancellation(
    produced_ordinary_error: bool,
    completed_at: Instant,
    cancellation: &CancellationToken,
) -> bool {
    ordinary_error_precedes_cancellation_at(
        produced_ordinary_error,
        completed_at,
        cancellation.cancelled_at(),
    )
}

pub(crate) fn ordinary_error_precedes_cancellation_at(
    produced_ordinary_error: bool,
    completed_at: Instant,
    cancelled_at: Option<Instant>,
) -> bool {
    produced_ordinary_error && cancelled_at.is_none_or(|cancelled_at| completed_at < cancelled_at)
}

pub(crate) fn retry_backoff(policy: RetryPolicy, failed_attempt: AttemptId) -> Duration {
    if policy.initial_backoff >= policy.max_backoff {
        return policy.max_backoff;
    }
    let exponent = failed_attempt.get().saturating_sub(1);
    let multiplier = u32::try_from(exponent)
        .ok()
        .and_then(|exponent| 1_u32.checked_shl(exponent))
        .unwrap_or(u32::MAX);
    policy
        .initial_backoff
        .checked_mul(multiplier)
        .unwrap_or(policy.max_backoff)
        .min(policy.max_backoff)
}

fn operation_correlation(
    plan: &ExecutionPlan,
    run_id: RunId,
    parent_call: Option<ParentCallId>,
    operation: OperationIndex,
) -> CorrelationContext {
    let planned = &plan.operations[operation.index()];
    CorrelationContext::compile(&plan.provenance)
        .for_run(run_id, parent_call)
        .for_node(planned.source_node_id, planned.source_node_type_id.clone())
}

fn execute_operation_worker(
    context: &OperationWorkerContext<'_>,
    job: PreparedOperation,
    trace: &dyn TraceSink,
) -> Result<Box<[RuntimeValue]>, RunError> {
    let operation = job.operation;
    let activation = job.activation;
    let attempt = job.attempt;
    let correlation =
        operation_correlation(context.plan, context.run_id, context.parent_call, operation);
    let planned = &context.plan.operations[operation.index()];
    let mut span = start_span_safely(
        trace,
        SpanSpec {
            parent_span_id: Some(context.run_parent_span_id),
            run_id: Some(context.run_id),
            operation_id: Some(planned.stable_id.clone()),
            activation_id: Some(activation),
            attempt_id: Some(attempt),
            kind: SpanKind::OperationAttempt,
            correlation,
        },
    );
    let operation_span_id = span.span_id();
    let result = execute_operation_worker_inner(context, job, trace, operation_span_id);
    span.finish(operation_span_outcome(
        context.plan,
        operation,
        attempt,
        &result,
    ));
    result
}

fn execute_operation_worker_inner(
    context: &OperationWorkerContext<'_>,
    job: PreparedOperation,
    trace: &dyn TraceSink,
    operation_span_id: SpanId,
) -> Result<Box<[RuntimeValue]>, RunError> {
    check_terminal(context.cancellation, context.deadline, RunPhase::Kernel)?;
    let operation = &context.plan.operations[job.operation.index()];
    let inputs = job.inputs;
    let outputs = match &operation.kernel {
        PlannedKernel::Native(handle) => {
            let kernel = context
                .kernels
                .get(handle)
                .ok_or_else(|| RunError::KernelNotFound(handle.as_str().into()))?;
            let kernel_context = KernelContext {
                run_id: context.run_id,
                frame_id: context.frame_id,
                activation_id: job.activation,
                computation_settings: context.computation_settings,
                params: &operation.params,
                compiled_parameters: context.compiled_parameters,
                resources: context.resources,
                resource_owner: context.resource_owner,
                cancellation: context.cancellation,
                deadline: context.deadline,
            };
            match kernel.execute(&kernel_context, &inputs) {
                Ok(outputs) => outputs,
                Err(error) if error.kind() == KernelErrorKind::Cancelled => {
                    return Err(RunError::Cancelled);
                }
                Err(error) if error.kind() == KernelErrorKind::DeadlineExceeded => {
                    return Err(RunError::DeadlineExceeded {
                        phase: RunPhase::Kernel,
                    });
                }
                Err(error) => {
                    return Err(RunError::KernelFailed {
                        operation: job.operation,
                        kind: error.kind(),
                        message: error.message().into(),
                    });
                }
            }
        }
        PlannedKernel::Adapter(adapter) => {
            let input =
                inputs.into_vec().into_iter().next().ok_or_else(|| {
                    RunError::InvalidPlan("adapter operation has no input".into())
                })?;
            let correlation = operation_correlation(
                context.plan,
                context.run_id,
                context.parent_call,
                job.operation,
            );
            let mut adapter_span = start_span_safely(
                trace,
                SpanSpec {
                    parent_span_id: Some(operation_span_id),
                    run_id: Some(context.run_id),
                    operation_id: Some(operation.stable_id.clone()),
                    activation_id: Some(job.activation),
                    attempt_id: Some(job.attempt),
                    kind: SpanKind::AdapterIo,
                    correlation,
                },
            );
            let result = execute_planned_adapter(
                adapter,
                input,
                context.resource_owner,
                context.cancellation,
            );
            adapter_span.finish(span_outcome(&result));
            vec![result?]
        }
        PlannedKernel::Relational(index) => {
            let subplan = &context.plan.relational_subplans[index.index()];
            let backend = context
                .relational_backends
                .get(&subplan.backend)
                .ok_or_else(|| RunError::RelationalBackendNotFound(subplan.backend.clone()))?;
            let relational_context = RelationalContext {
                run_id: context.run_id,
                resources: context.resources,
                resource_owner: context.resource_owner,
                cancellation: context.cancellation,
                deadline: context.deadline,
            };
            let correlation = operation_correlation(
                context.plan,
                context.run_id,
                context.parent_call,
                job.operation,
            );
            let mut adapter_span = start_span_safely(
                trace,
                SpanSpec {
                    parent_span_id: Some(operation_span_id),
                    run_id: Some(context.run_id),
                    operation_id: Some(operation.stable_id.clone()),
                    activation_id: Some(job.activation),
                    attempt_id: Some(job.attempt),
                    kind: SpanKind::AdapterIo,
                    correlation,
                },
            );
            let backend_result =
                backend.execute(&relational_context, &subplan.compiled_plan, &inputs);
            let backend_outcome = match &backend_result {
                Err(error) if error.code() == super::RelationalErrorCode::Cancelled => {
                    SpanOutcome::Cancellation
                }
                Err(_) => SpanOutcome::Error,
                Ok(_) if context.cancellation.is_cancelled() => SpanOutcome::Cancellation,
                Ok(_) => SpanOutcome::Success,
            };
            adapter_span.finish(backend_outcome);
            match backend_result {
                Ok(execution) => execution.outputs,
                Err(error) => return Err(RunError::from_relational(job.operation, error)),
            }
        }
    };
    check_terminal(context.cancellation, context.deadline, RunPhase::Kernel)?;
    if outputs.len() != operation.outputs.len() {
        return Err(RunError::OutputCount {
            operation: job.operation,
            expected: operation.outputs.len(),
            actual: outputs.len(),
        });
    }
    Ok(outputs.into_boxed_slice())
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

fn promote_run_result(
    run_result: &RunResult,
    cancellation: &CancellationToken,
    deadline: Option<RunDeadline>,
) -> Result<(), RunError> {
    for value in run_result.values.values() {
        if let RuntimeValue::Artifact(artifact) = value {
            artifact.promote(cancellation, deadline)?;
        }
    }
    Ok(())
}

fn apply_authoritative_attempt_outcome(
    spans: &mut [TraceSpan],
    operation_id: &crate::node_system::plan::OperationStableId,
    completion: &OperationCompletion,
    completed_at: Instant,
    cancelled_at: Option<Instant>,
    deadline: Option<RunDeadline>,
) {
    for span in spans.iter_mut().filter(|span| {
        span.kind == SpanKind::OperationAttempt
            && span.operation_id.as_ref() == Some(operation_id)
            && span.activation_id == Some(completion.activation)
            && span.attempt_id == Some(completion.attempt)
    }) {
        if span.outcome != SpanOutcome::Success {
            continue;
        }
        if cancelled_at.is_some_and(|cancelled_at| completed_at >= cancelled_at) {
            span.outcome = SpanOutcome::Cancellation;
        } else if deadline.is_some_and(|deadline| deadline.exceeded_at(completed_at)) {
            span.outcome = SpanOutcome::Timeout;
        }
    }
}

fn span_outcome<T>(result: &Result<T, RunError>) -> SpanOutcome {
    match result {
        Ok(_) => SpanOutcome::Success,
        Err(RunError::Cancelled) => SpanOutcome::Cancellation,
        Err(RunError::DeadlineExceeded { .. }) => SpanOutcome::Timeout,
        Err(_) => SpanOutcome::Error,
    }
}

fn operation_span_outcome<T>(
    plan: &ExecutionPlan,
    operation: OperationIndex,
    attempt: AttemptId,
    result: &Result<T, RunError>,
) -> SpanOutcome {
    let retryable = result.as_ref().is_err_and(|error| {
        matches!(
            error,
            RunError::KernelFailed {
                kind: KernelErrorKind::Transient,
                ..
            }
        )
    });
    let has_retry = plan.operations[operation.index()]
        .retry
        .policy
        .is_some_and(|policy| attempt.get() < u64::from(policy.max_attempts.get()));
    if retryable && has_retry {
        SpanOutcome::Retry
    } else {
        span_outcome(result)
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

    attempted: BTreeMap<MemoKey, AttemptId>,
    completed: BTreeSet<MemoKey>,
    completion_counts: BTreeMap<OperationIndex, usize>,
}

impl Frame {
    fn new(value_count: u32) -> Self {
        Self {
            id: FrameId::next(),
            values: vec![None; value_count as usize],

            attempted: BTreeMap::new(),
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

    fn bind(&mut self, values: &BTreeMap<ValueRef, Value>) -> Result<(), RunError> {
        for (reference, value) in values {
            self.set(*reference, RuntimeValue::Scalar(value.clone()))?;
        }
        Ok(())
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
