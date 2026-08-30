use super::relational::RunRelationalBackends;
use super::scheduling::ClassScheduler;
use super::{
    ACTIVATION_IDS, ActivationId, ActivationIdAllocator, ActivationProvenance,
    ActivationResultGroup, CancellationToken, CompiledParameterStore, DemandFingerprint,
    EffectiveComputationSettings, FRAME_IDS, FrameId, GraphRunIdentity, KernelContext,
    KernelErrorKind, KernelRegistry, NOOP_RUN_EVENT_SINK, OperationCompletion, OperationMemoKey,
    PendingOutputDescriptor, ProjectRunRegistry, PublishedFunctionPlan, RelationalBackendProvider,
    RelationalContext, ResourceErrorKind, ResourceProvider, ResultFailure, ResultId, ResultState,
    ResultStore, ResultUsage, RunDeadline, RunError, RunErrorOutcome, RunEvent, RunEventKind,
    RunEventSink, RunOptions, RunOutputEmitter, RunOutputSink, RunPhase, RunResourceBudgets,
    RunResourceOwner, RunResourceSet, RunResult, RuntimeValue, SchedulingPolicy,
    SessionMemoization, StoredValue, check_terminal, execute_planned_adapter,
    validate_data_series_type_expr,
};
use crate::execution::plan::legacy::{
    AttemptId, CallArgumentBinding, CallResultBinding, ControlStep, ExecutionPlan,
    FunctionPlanHandle, OperationIndex, PlannedKernel, PlannedPublication, PlannedValueContract,
    PlannedValueKind, ResultPresentation, StructuredControlRegion, ValueRef, WorkloadClass,
};
use crate::node_system::runtime::RunId;
use yss_graph_analysis_contract::ResourceVersionSet;
use yss_graph_protocol::{CachePolicy, RetryPolicy, Value};

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap, VecDeque};
use std::num::NonZeroU64;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::time::{Duration, Instant};

#[path = "scheduler/admission.rs"]
mod admission;
#[path = "scheduler/frame.rs"]
mod frame;
#[path = "scheduler/operation_completion.rs"]
mod operation_completion;
#[path = "scheduler/operation_scheduler.rs"]
mod operation_scheduler;
#[path = "scheduler/operation_state.rs"]
mod operation_state;
#[path = "scheduler/worker_execution.rs"]
mod worker_execution;
use frame::{Frame, MemoKey};
use operation_scheduler::ReadyOperationContext;
use operation_state::{
    AdmissionBookkeeping, DelayedRetry, MemoTables, PreparedOperation, RunningOperation,
    WorkerCompletion,
};
use worker_execution::execute_operation_worker;

const DEFAULT_RECURSION_LIMIT: usize = 64;
static NEXT_RUN_ID: AtomicU64 = AtomicU64::new(1);

fn allocate_runtime_id(allocator: &AtomicU64) -> Result<NonZeroU64, RunError> {
    allocator
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            NonZeroU64::new(current)?.get().checked_add(1)
        })
        .map(|id| NonZeroU64::new(id).expect("runtime IDs are non-zero"))
        .map_err(|_| RunError::RuntimeIdExhausted)
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SchedulerCheckpoint {
    FinalResultPublication,
    WorkerOutcomeProduced,
    BeforeGroupCommit,
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

struct OperationWorkerContext<'a> {
    run_id: RunId,
    run_output: &'a dyn RunOutputSink,
    frame_id: FrameId,
    computation_settings: EffectiveComputationSettings,
    plan: &'a ExecutionPlan,
    kernels: &'a KernelRegistry,
    compiled_parameters: Option<&'a CompiledParameterStore>,
    resources: &'a RunResourceSet,
    resource_owner: &'a RunResourceOwner,
    relational_backends: &'a RunRelationalBackends,
    results: &'a ResultStore,
    cancellation: &'a CancellationToken,
    deadline: Option<RunDeadline>,
    #[cfg(test)]
    checkpoint:
        Option<&'a Arc<dyn Fn(SchedulerCheckpoint, &CancellationToken) + Send + Sync + 'static>>,
}

struct SchedulerSignal {
    notified: Mutex<bool>,
    ready: Arc<Condvar>,
}

impl SchedulerSignal {
    fn new(cancellation: &CancellationToken) -> Self {
        let ready = Arc::new(Condvar::new());
        cancellation.register_waiter(&ready);
        Self {
            notified: Mutex::new(false),
            ready,
        }
    }

    fn notify(&self) {
        let mut notified = self
            .notified
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        *notified = true;
        drop(notified);
        self.ready.notify_all();
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
    if callee.provenance.graph_path.as_str() != target.as_str() {
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
    events: &'a dyn RunEventSink,
    results: ResultStore,
    memoization: Arc<SessionMemoization>,
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
    frame_ids: &'a AtomicU64,
    #[cfg(test)]
    spill_root: Option<std::path::PathBuf>,
    #[cfg(test)]
    checkpoint:
        Option<Arc<dyn Fn(SchedulerCheckpoint, &CancellationToken) + Send + Sync + 'static>>,
}

impl<'a> RunExecutor<'a> {
    pub fn new(
        kernels: &'a KernelRegistry,
        resources: &'a dyn ResourceProvider,
        functions: &'a dyn FunctionPlanProvider,
        results: ResultStore,
        memoization: Arc<SessionMemoization>,
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
            events: &NOOP_RUN_EVENT_SINK,
            results,
            memoization,
            success_finalizer: None,
            atomic_success_preparer: None,
            atomic_success_finalizer: None,
            options: RunOptions::default(),
            activation_ids: &ACTIVATION_IDS,
            frame_ids: &FRAME_IDS,
            #[cfg(test)]
            spill_root: None,
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

    pub fn with_event_sink(mut self, events: &'a dyn RunEventSink) -> Self {
        self.events = events;
        self
    }

    pub fn with_result_store(mut self, results: &ResultStore) -> Self {
        self.results = results.clone();
        self
    }

    fn result_store(&self) -> &ResultStore {
        &self.results
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

    #[cfg(test)]
    pub(crate) fn with_activation_allocator_for_test(
        mut self,
        allocator: &'a ActivationIdAllocator,
    ) -> Self {
        self.activation_ids = allocator;
        self
    }

    #[cfg(test)]
    pub(crate) fn with_frame_allocator_for_test(mut self, allocator: &'a AtomicU64) -> Self {
        self.frame_ids = allocator;
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
        let run_id = allocate_runtime_id(&NEXT_RUN_ID).map(|id| RunId::new(id.get()))?;
        let run = GraphRunIdentity {
            project_session_id: crate::project::ProjectSessionId::new(
                plan.provenance.project_session_id.as_str(),
            ),
            graph_path: plan.provenance.graph_path.clone(),
            run_id,
        };
        let _registration = self
            .run_registry
            .map(|registry| {
                registry.track(
                    crate::project::ProjectSessionId::new(run.project_session_id.as_str()),
                    run.run_id,
                    cancellation.clone(),
                )
            })
            .transpose()
            .map_err(|error| RunError::ProjectDraining(error.to_string().into()))?;

        let run_output = RunOutputEmitter::new(run.run_id, self.events);
        self.record_event(&run, RunEventKind::RunStarted);
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
            let execution = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                self.finish_run(
                    plan,
                    cancellation.clone(),
                    &run,
                    &run_output,
                    &resource_owner,
                )
            }));
            match execution {
                Ok(result) => {
                    self.cleanup_resources(&resource_owner);
                    if result.is_ok() {
                        check_terminal(&cancellation, self.options.deadline, RunPhase::Cleanup)?;
                    }
                    result
                }
                Err(payload) => {
                    self.cleanup_resources(&resource_owner);
                    std::panic::resume_unwind(payload)
                }
            }
        });

        let event = match &result {
            Ok(_) => RunEventKind::RunCompleted,
            Err(RunError::Cancelled) => RunEventKind::RunCancelled,
            Err(error) => RunEventKind::RunErrored {
                outcome: RunErrorOutcome::from(error),
            },
        };
        self.record_event(&run, event);
        result
    }

    fn finish_run(
        &self,
        plan: &ExecutionPlan,
        cancellation: CancellationToken,
        run: &GraphRunIdentity,
        run_output: &dyn RunOutputSink,
        resource_owner: &RunResourceOwner,
    ) -> Result<RunResult, RunError> {
        check_terminal(&cancellation, self.options.deadline, RunPhase::Kernel)?;
        let execution_result =
            self.run_root(plan, cancellation.clone(), run, run_output, resource_owner);
        let Ok(run_result) = execution_result else {
            return execution_result;
        };
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

        #[cfg(test)]
        if result.is_ok() {
            self.run_test_checkpoint(SchedulerCheckpoint::FinalResultPublication, &cancellation);
        }
        if result.is_ok()
            && let Err(error) = check_terminal(
                &cancellation,
                self.options.deadline,
                RunPhase::ResultPublication,
            )
        {
            result = Err(error);
        }
        if let (Some(finalizer), Ok(run_result)) = (self.success_finalizer, result.as_mut())
            && let Err(error) = finalizer(run_result, &cancellation, self.options.deadline)
        {
            result = Err(error);
        }
        if let (Some(preparer), Ok(run_result)) = (self.atomic_success_preparer, result.as_mut())
            && let Err(error) = preparer(run_result, &cancellation, self.options.deadline)
        {
            result = Err(error);
        }
        if let (Some(finalizer), Ok(run_result)) = (self.atomic_success_finalizer, result.as_mut())
            && let Err(error) = finalizer(run_result, &cancellation, self.options.deadline)
        {
            result = Err(error);
        }
        if let Ok(run_result) = result.as_ref() {
            self.record_pin_preview_result_events(plan, run, run_result);
        }
        result
    }

    fn cleanup_resources(&self, resource_owner: &RunResourceOwner) {
        for error in resource_owner.cleanup() {
            tracing::warn!(
                target: "yssbi::node_system::runtime::cleanup",
                diagnostic_domain = "execution",
                diagnostic_event = "resourceCleanupFailed",
                error = %error,
                "Runtime resource cleanup failed"
            );
        }
    }

    fn record_pin_preview_result_events(
        &self,
        plan: &ExecutionPlan,
        run: &GraphRunIdentity,
        run_result: &RunResult,
    ) {
        for publication in &plan.publications {
            let PlannedPublication::PinPreview {
                output,
                generation,
                value,
            } = publication
            else {
                continue;
            };

            let result_id = plan
                .results
                .iter()
                .find(|result| result.output == *output && result.value == *value)
                .and_then(|result| run_result.result_ids.get(&result.name))
                .copied();

            if let Some(result_id) = result_id {
                self.record_event(
                    run,
                    RunEventKind::PinPreviewResultReady {
                        output: output.clone(),
                        generation: *generation,
                        result_id,
                    },
                );
            }
        }
    }

    fn run_root(
        &self,
        plan: &ExecutionPlan,
        cancellation: CancellationToken,
        run: &GraphRunIdentity,
        run_output: &dyn RunOutputSink,
        resource_owner: &RunResourceOwner,
    ) -> Result<RunResult, RunError> {
        plan.validate()
            .map_err(|error| RunError::InvalidPlan(error.to_string().into()))?;
        cancellation.check()?;
        let resource_set = self.acquire_resources(plan)?;
        let relational_backends = RunRelationalBackends::acquire(
            &plan.relational_subplans,
            self.relational_backends,
            &resource_set,
            &cancellation,
        )?;
        let mut frame = Frame::new(plan.value_count, self.frame_ids)?;
        self.bind_internal_values(run.run_id, plan, &mut frame, &plan.bound_values)?;
        let run_memoization = SessionMemoization::new();
        let memoization = MemoTables {
            per_run: &run_memoization,
            per_session: self.memoization.as_ref(),
        };
        let root_demand = DemandFingerprint::for_root(plan, self.selection_digest);
        let result = (|| {
            self.execute_region(
                run,
                run_output,
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
            )?;
            cancellation.check()?;

            let mut result_ids = BTreeMap::new();
            for result in &plan.results {
                let result_id = frame.result_id(result.value)?;
                result_ids.insert(result.name.clone(), result_id);
            }
            Ok(RunResult {
                run_id: run.run_id,
                provenance: plan.provenance.clone(),
                result_ids,
                results: self.results.clone(),
                committed_variable_ids: Box::new([]),
                resource_mutation: None,
            })
        })();
        run_memoization.finalize();
        result
    }

    fn activation_provenance(
        &self,
        run_id: RunId,
        activation_id: ActivationId,
        plan: &ExecutionPlan,
        node_id: yss_graph_document::NodeId,
    ) -> ActivationProvenance {
        ActivationProvenance {
            run_id,
            activation_id,
            graph_path: plan.provenance.graph_path.clone(),
            graph_revision: plan.provenance.basis.graph_revision,
            node_id,
            created_at_ms: current_time_ms(),
            usage: ResultUsage::Produced,
        }
    }

    fn create_internal_ready_result(
        &self,
        run_id: RunId,
        plan: &ExecutionPlan,
        node_id: yss_graph_document::NodeId,
        value_ref: ValueRef,
        contract: PlannedValueContract,
        value: Value,
    ) -> Result<ResultId, RunError> {
        let activation = self.activation_ids.allocate()?;
        let group = self
            .result_store()
            .create_pending_group(
                self.activation_provenance(run_id, activation, plan, node_id),
                &[PendingOutputDescriptor {
                    value: value_ref,
                    output: None,
                    presentation: ResultPresentation::Inspector,
                    contract,
                }],
            )
            .map_err(result_store_error)?;
        self.complete_result_group(&group, vec![StoredValue::scalar(value)].into_boxed_slice())?;
        Ok(group.output_result_ids[0])
    }

    fn bind_internal_values(
        &self,
        run_id: RunId,
        plan: &ExecutionPlan,
        frame: &mut Frame,
        values: &BTreeMap<ValueRef, Value>,
    ) -> Result<(), RunError> {
        for (value_ref, value) in values {
            let result_id = self.create_internal_ready_result(
                run_id,
                plan,
                yss_graph_document::NodeId::from_uuid(uuid::Uuid::nil()),
                *value_ref,
                plan.value_contracts
                    .get(value_ref)
                    .cloned()
                    .ok_or(RunError::MissingValue(*value_ref))?,
                value.clone(),
            )?;
            frame.bind_result(*value_ref, result_id)?;
        }
        Ok(())
    }

    fn ready_runtime_value(
        &self,
        result_id: ResultId,
        cancellation: &CancellationToken,
    ) -> Result<RuntimeValue, RunError> {
        let result = self
            .result_store()
            .wait_terminal(result_id, cancellation, self.options.deadline)
            .map_err(result_store_error)?;
        match &result.state {
            ResultState::Ready(value) => Ok(value.to_runtime_value()),
            ResultState::Failed(failure) => Err(RunError::InvalidPlan(failure.message.clone())),
            ResultState::Cancelled => Err(RunError::Cancelled),
            ResultState::Pending(_) => unreachable!("wait_terminal returned pending result"),
        }
    }

    fn boolean(
        &self,
        frame: &Frame,
        reference: ValueRef,
        cancellation: &CancellationToken,
    ) -> Result<bool, RunError> {
        match self.ready_runtime_value(frame.result_id(reference)?, cancellation)? {
            RuntimeValue::Scalar(Value::Bool(value)) => Ok(value),
            _ => Err(RunError::InvalidCondition { value: reference }),
        }
    }

    fn transition_group_terminal(&self, group: Option<&ActivationResultGroup>, error: &RunError) {
        let Some(group) = group else {
            return;
        };
        let transition = if matches!(
            error,
            RunError::Cancelled | RunError::UpstreamResultCancelled { .. }
        ) {
            self.result_store().cancel_group(group)
        } else {
            let failure = match error {
                RunError::UpstreamResultFailed {
                    source_result_id,
                    message,
                } => ResultFailure::upstream(*source_result_id, message.clone()),
                _ => ResultFailure::new(error.to_string()),
            };
            self.result_store().fail_group(group, Arc::new(failure))
        };
        match transition {
            Ok(()) | Err(super::ResultStoreError::TerminalResult(_)) => {}
            Err(error) => tracing::warn!(
                target: "yssbi::node_system::runtime::results",
                diagnostic_domain = "execution",
                diagnostic_event = "resultGroupTransitionFailed",
                error = %error,
                "Failed to transition activation result group"
            ),
        }
    }

    fn complete_result_group(
        &self,
        group: &ActivationResultGroup,
        values: Box<[StoredValue]>,
    ) -> Result<(), RunError> {
        self.result_store()
            .complete_group(group, values)
            .map_err(result_store_error)?;
        Ok(())
    }

    fn acquire_resources(&self, plan: &ExecutionPlan) -> Result<RunResourceSet, RunError> {
        self.resources
            .validate_plan(&plan.provenance, &plan.resources)
            .map_err(|error| match error.kind() {
                ResourceErrorKind::SnapshotMismatch => {
                    RunError::ResourceSnapshotMismatch(error.into_message())
                }
                ResourceErrorKind::UnsupportedAccess => RunError::InvalidPlan(error.into_message()),
                ResourceErrorKind::Acquire => RunError::InvalidPlan(error.into_message()),
            })
            .and_then(|()| RunResourceSet::acquire(&plan.resources, self.resources))
    }

    #[allow(clippy::too_many_arguments)]
    fn execute_region(
        &self,
        run: &GraphRunIdentity,
        run_output: &dyn RunOutputSink,
        plan: &ExecutionPlan,
        region: &StructuredControlRegion,
        frame: &mut Frame,
        resources: &RunResourceSet,
        resource_owner: &RunResourceOwner,
        relational_backends: &RunRelationalBackends,
        memoization: &MemoTables,
        demand: &DemandFingerprint,
        cancellation: &CancellationToken,
        frame_depth: usize,
    ) -> Result<(), RunError> {
        cancellation.check()?;
        frame.clear_region_values(plan, region);
        self.propagate_value_dependencies(plan, frame)?;
        match region {
            StructuredControlRegion::Sequence(steps) => self.execute_sequence(
                run,
                run_output,
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
            )?,
            StructuredControlRegion::If {
                condition,
                then_region,
                else_region,
                results,
            } => {
                let selected_then = self.boolean(frame, *condition, cancellation)?;
                let selected = if selected_then {
                    then_region
                } else {
                    else_region
                };
                self.execute_region(
                    run,
                    run_output,
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
                )?;
                for binding in results {
                    let source = if selected_then {
                        binding.then_source
                    } else {
                        binding.else_source
                    };
                    frame.copy_result(source, binding.destination)?;
                }
            }
            StructuredControlRegion::Loop {
                body,
                carried,
                continue_condition,
                max_iterations,
            } => {
                for binding in carried {
                    frame.copy_result(binding.initial_source, binding.body_input)?;
                }
                let mut should_continue = true;
                for _ in 0..*max_iterations {
                    cancellation.check()?;
                    self.execute_region(
                        run,
                        run_output,
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
                    )?;
                    should_continue = self.boolean(frame, *continue_condition, cancellation)?;
                    for binding in carried {
                        frame.copy_result(binding.next_source, binding.result)?;
                        if should_continue {
                            frame.copy_result(binding.next_source, binding.body_input)?;
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
                let call_result = (|| {
                    callee
                        .validate()
                        .map_err(|error| RunError::InvalidPlan(error.to_string().into()))?;
                    let argument_values = arguments
                        .iter()
                        .map(|binding| {
                            Ok((
                                binding.callee_destination,
                                frame.result_id(binding.caller_source)?,
                            ))
                        })
                        .collect::<Result<Vec<_>, RunError>>()?;
                    let callee_resources = self.acquire_resources(&callee)?;
                    let callee_backends = RunRelationalBackends::acquire(
                        &callee.relational_subplans,
                        self.relational_backends,
                        &callee_resources,
                        cancellation,
                    )?;
                    let mut callee_frame = Frame::new(callee.value_count, self.frame_ids)?;
                    self.bind_internal_values(
                        run.run_id,
                        &callee,
                        &mut callee_frame,
                        &callee.bound_values,
                    )?;
                    let callee_demand =
                        DemandFingerprint::for_callee(&callee, target, arguments, results);
                    let result: Result<(), RunError> = (|| {
                        for (destination, result_id) in argument_values {
                            callee_frame.bind_result(destination, result_id)?;
                        }
                        self.execute_region(
                            run,
                            run_output,
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
                        )?;
                        for binding in results {
                            frame.bind_result(
                                binding.caller_destination,
                                callee_frame.result_id(binding.callee_source)?,
                            )?;
                        }
                        Ok(())
                    })();
                    result
                })();
                call_result?;
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn execute_sequence(
        &self,
        run: &GraphRunIdentity,
        run_output: &dyn RunOutputSink,
        plan: &ExecutionPlan,
        steps: &[ControlStep],
        frame: &mut Frame,
        resources: &RunResourceSet,
        resource_owner: &RunResourceOwner,
        relational_backends: &RunRelationalBackends,
        memoization: &MemoTables,
        demand: &DemandFingerprint,
        cancellation: &CancellationToken,
        frame_depth: usize,
    ) -> Result<(), RunError> {
        let activation_id = self.activation_ids.allocate()?;
        let mut activated = BTreeSet::new();
        let mut operations = Vec::new();

        for step in steps {
            cancellation.check()?;
            match step {
                ControlStep::Operation(operation) => operations.push(*operation),
                ControlStep::Region(child) => {
                    self.execute_ready_operations(ReadyOperationContext {
                        run,
                        run_output,
                        plan,
                        operations: &operations,
                        activation_id,
                        activated: &mut activated,
                        frame,
                        resources,
                        resource_owner,
                        relational_backends,
                        memoization,
                        demand,
                        cancellation,
                    })?;
                    operations.clear();
                    self.execute_region(
                        run,
                        run_output,
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
                    )?;
                }
            }
        }
        self.execute_ready_operations(ReadyOperationContext {
            run,
            run_output,
            plan,
            operations: &operations,
            activation_id,
            activated: &mut activated,
            frame,
            resources,
            resource_owner,
            relational_backends,
            memoization,
            demand,
            cancellation,
        })
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
                    frame.copy_result(dependency.source, dependency.destination)?;
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
        let inputs_ready = operation.inputs.iter().all(|input| {
            frame
                .result_id(input.value)
                .ok()
                .and_then(|result_id| self.result_store().result(result_id))
                .is_some_and(|result| result.state.is_terminal())
                || (!frame.has(input.value) && input.bound_value.is_some())
        });
        let values_ready = operation.outputs.iter().all(|output| {
            plan.value_dependencies
                .iter()
                .filter(|edge| edge.destination == output.value)
                .all(|edge| {
                    frame
                        .result_id(edge.source)
                        .ok()
                        .and_then(|result_id| self.result_store().result(result_id))
                        .is_some_and(|result| result.state.is_terminal())
                })
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
    fn record_event(&self, run: &GraphRunIdentity, kind: RunEventKind) {
        self.events.record(RunEvent {
            run: run.clone(),
            kind,
        });
    }
}

fn result_store_error(error: super::ResultStoreError) -> RunError {
    match error {
        super::ResultStoreError::WaitCancelled => RunError::Cancelled,
        super::ResultStoreError::WaitDeadlineExceeded => RunError::DeadlineExceeded {
            phase: RunPhase::ResultPublication,
        },
        error => RunError::InvalidPlan(error.to_string().into()),
    }
}

fn current_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
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
