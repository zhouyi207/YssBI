use super::*;

#[test]
fn cancellation_stops_run_and_releases_resources() {
    let token = CancellationToken::new();
    let kernel_token = token.clone();
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("cancel", KernelHandle::new),
            FnKernel(move |_: &[RuntimeValue]| {
                kernel_token.cancel();
                Ok(vec![Value::Null.into()])
            }),
        )
        .unwrap();
    let mut execution_plan = plan(
        vec![operation("cancel", &[], &[0])],
        1,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    execution_plan.resources = Box::new([requirement("temporary")]);
    let resources = no_resources();
    let released = resources.released.clone();

    let error = RunExecutor::new(
        &kernels,
        &resources,
        &NoFunctions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .run(&execution_plan, token)
    .unwrap_err();

    assert_eq!(error, RunError::Cancelled);
    assert_eq!(released.load(Ordering::SeqCst), 1);
}

#[test]
fn cancelled_kernel_error_maps_to_run_cancelled() {
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("cancelled_error", KernelHandle::new),
            ErrorKernel {
                cancel_token: false,
                cancelled_error: true,
            },
        )
        .unwrap();
    let execution_plan = plan(
        vec![operation("cancelled_error", &[], &[])],
        0,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );

    let error = RunExecutor::new(
        &kernels,
        &no_resources(),
        &NoFunctions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .run(&execution_plan, CancellationToken::new())
    .unwrap_err();

    assert_eq!(error, RunError::Cancelled);
}

#[test]
fn cancellation_before_ordinary_outcome_wins() {
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("ordinary_error", KernelHandle::new),
            ErrorKernel {
                cancel_token: true,
                cancelled_error: false,
            },
        )
        .unwrap();
    let execution_plan = plan(
        vec![operation("ordinary_error", &[], &[])],
        0,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );

    let error = RunExecutor::new(
        &kernels,
        &no_resources(),
        &NoFunctions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .run(&execution_plan, CancellationToken::new())
    .unwrap_err();

    assert_eq!(error, RunError::Cancelled);
}

#[test]
fn simultaneous_or_later_ordinary_outcome_cannot_replace_cancellation() {
    let cancellation = Instant::now();
    let before = cancellation.checked_sub(Duration::from_nanos(1)).unwrap();
    let after = cancellation.checked_add(Duration::from_nanos(1)).unwrap();

    assert!(
        super::super::scheduler::ordinary_error_precedes_cancellation_at(
            true,
            before,
            Some(cancellation),
        )
    );
    assert!(
        !super::super::scheduler::ordinary_error_precedes_cancellation_at(
            true,
            cancellation,
            Some(cancellation),
        )
    );
    assert!(
        !super::super::scheduler::ordinary_error_precedes_cancellation_at(
            true,
            after,
            Some(cancellation),
        )
    );
}

#[test]
fn ordinary_outcome_produced_before_cancellation_is_preserved() {
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("ordinary_before_cancel", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| Err(KernelError::new("ordinary first"))),
        )
        .unwrap();
    let execution_plan = plan(
        vec![operation("ordinary_before_cancel", &[], &[])],
        0,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );

    let error = RunExecutor::new(
        &kernels,
        &no_resources(),
        &NoFunctions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .with_test_checkpoint(Arc::new(|checkpoint, cancellation| {
        if checkpoint == SchedulerCheckpoint::WorkerOutcomeProduced {
            cancellation.cancel();
        }
    }))
    .run(&execution_plan, CancellationToken::new())
    .unwrap_err();

    assert!(matches!(
        error,
        RunError::KernelFailed { message, .. } if message.as_ref() == "ordinary first"
    ));
}

struct OrdinaryErrorAfterCancellationBackend;

impl RelationalBackend for OrdinaryErrorAfterCancellationBackend {
    fn execute(
        &self,
        context: &RelationalContext<'_>,
        _: &CompiledRelationalPlan,
        _: &[RuntimeValue],
    ) -> Result<RelationalExecution, RelationalError> {
        context.cancellation.cancel();
        Err(RelationalError::operator_invalid(
            "ordinary backend failure won the boundary",
        ))
    }
}

#[test]
fn relational_cancellation_installed_before_ordinary_error_wins() {
    let mut relational = RelationalBackendRegistry::new();
    relational
        .register(
            id("ordinary-after-cancel", RelationalBackendId::new),
            OrdinaryErrorAfterCancellationBackend,
        )
        .unwrap();
    let mut execution_plan = plan(
        vec![relational_operation(0, &[0])],
        1,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    execution_plan.relational_subplans = Box::new([relational_subplan(
        "ordinary-after-cancel",
        "ordinary-fragment",
        Box::new([]),
    )]);

    let error = RunExecutor::new(
        &KernelRegistry::new(),
        &no_resources(),
        &NoFunctions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .with_relational_backends(&relational)
    .run(&execution_plan, CancellationToken::new())
    .unwrap_err();

    assert_eq!(error, RunError::Cancelled);
}

fn assert_production_source_cancellation(
    target: super::super::production_relational::ProductionRelationalCheckpoint,
    expected_checkpoints: &[super::super::production_relational::ProductionRelationalCheckpoint],
    expected_scan_limits: &[Option<usize>],
) {
    use polars::prelude::{Column, DataFrame};

    let resource = id("databases/main", ResourceId::new);
    let dataframe = DataFrame::new(2, vec![Column::new("value".into(), &[1_i64, 2])]).unwrap();
    let resource_versions = BTreeMap::from([(
        ResourceKey::new(resource.as_str()),
        ResourceVersion::new("1"),
    )]);
    let lease_observer = ProjectResourceLeaseObserver::default();
    let mut provider = ProjectResourceProvider::new(
        ProjectResourceSnapshot::new(
            ProjectSessionId::new("test-session"),
            resource_versions.clone(),
        )
        .with_database(resource.clone(), Arc::new(dataframe)),
    );
    provider.set_lease_observer(lease_observer.clone());
    let scan_limits = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::new(Mutex::new(Vec::new()));
    let observed_for_hook = Arc::clone(&observed);
    let mut relational = RelationalBackendRegistry::new();
    relational
        .register(
            id("production", RelationalBackendId::new),
            ProductionRelationalBackend::recording_scan_limits(Arc::clone(&scan_limits))
                .with_test_checkpoint(Arc::new(move |checkpoint, cancellation| {
                    observed_for_hook.lock().unwrap().push(checkpoint);
                    if checkpoint == target {
                        cancellation.cancel();
                    }
                })),
        )
        .unwrap();
    let mut execution_plan = plan(
        vec![relational_operation(0, &[0])],
        1,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    execution_plan.provenance.basis.resource_versions = resource_versions;
    execution_plan.resources = Box::new([CompiledResourceRequirement {
        resource: resource.clone(),
        kind: ResourceKind::DatabaseConnection,
        access: ResourceAccess::Shared,
        optional: false,
    }]);
    let source_fragment = id("source", RelationalFragmentId::new);
    execution_plan.relational_subplans = Box::new([RelationalSubplan {
        backend: id("production", RelationalBackendId::new),
        compiled_plan: CompiledRelationalPlan {
            fragment_order: Box::new([source_fragment.clone()]),
            operators: Box::new([RelationalOperator::Source {
                resource,
                relation: "main".into(),
            }]),
            fragment_roots: Box::new([RelationalFragmentRoot {
                fragment: source_fragment,
                operator: RelationalOperatorIndex::new(0),
            }]),
            roots: Box::new([RelationalOperatorIndex::new(0)]),
            pushdown_hints: Box::new([]),
        },
    }]);
    execution_plan.results = Box::new([PlanResult {
        name: "result".into(),
        output: stable_output("result"),
        value: ValueRef::new(0),
    }]);
    publish_graph_results(&mut execution_plan);
    let events = RecordingRunEvents::default();
    let results = ResultStore::new();

    let error = RunExecutor::new(
        &KernelRegistry::new(),
        &provider,
        &NoFunctions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .with_relational_backends(&relational)
    .with_event_sink(&events)
    .with_result_store(&results)
    .run(&execution_plan, CancellationToken::new())
    .unwrap_err();

    assert_eq!(error, RunError::Cancelled);
    assert_eq!(observed.lock().unwrap().as_slice(), expected_checkpoints);
    assert_eq!(scan_limits.lock().unwrap().as_slice(), expected_scan_limits);
    let run_id = assert_cancelled_without_completion(&events);
    let stored_results = results.results_for_run(run_id);
    assert_eq!(stored_results.len(), 1);
    assert!(matches!(stored_results[0].state, ResultState::Cancelled));
    assert_eq!(lease_observer.acquired(), lease_observer.dropped());
    assert_eq!(lease_observer.active(), 0);
}

#[test]
fn cancellation_at_production_source_scan_stops_before_scan_and_publication() {
    use super::super::production_relational::ProductionRelationalCheckpoint;

    assert_production_source_cancellation(
        ProductionRelationalCheckpoint::SourceScan,
        &[
            ProductionRelationalCheckpoint::OperatorEvaluation,
            ProductionRelationalCheckpoint::SourceScan,
        ],
        &[],
    );
}

#[test]
fn cancellation_at_production_operator_evaluation_stops_before_dependencies_and_publication() {
    use super::super::production_relational::ProductionRelationalCheckpoint;

    assert_production_source_cancellation(
        ProductionRelationalCheckpoint::OperatorEvaluation,
        &[ProductionRelationalCheckpoint::OperatorEvaluation],
        &[],
    );
}

#[test]
fn cancellation_at_production_result_materialization_prevents_publication_and_completion() {
    use super::super::production_relational::ProductionRelationalCheckpoint;

    assert_production_source_cancellation(
        ProductionRelationalCheckpoint::ResultMaterialization,
        &[
            ProductionRelationalCheckpoint::OperatorEvaluation,
            ProductionRelationalCheckpoint::SourceScan,
            ProductionRelationalCheckpoint::ResultMaterialization,
        ],
        &[None],
    );
}

#[test]
fn cancellation_during_production_result_conversion_stops_without_publication_or_leaks() {
    use super::super::production_relational::ProductionRelationalCheckpoint;

    assert_production_source_cancellation(
        ProductionRelationalCheckpoint::ResultConversion,
        &[
            ProductionRelationalCheckpoint::OperatorEvaluation,
            ProductionRelationalCheckpoint::SourceScan,
            ProductionRelationalCheckpoint::ResultMaterialization,
            ProductionRelationalCheckpoint::ResultConversion,
        ],
        &[None],
    );
}

struct PromptCancellationKernel {
    started: mpsc::SyncSender<()>,
}

impl Kernel for PromptCancellationKernel {
    fn execute(
        &self,
        context: &KernelContext<'_>,
        _: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        self.started.send(()).unwrap();
        context.wait_for(Duration::from_secs(5))?;
        Ok(Vec::new())
    }
}

#[test]
fn kernel_context_wait_wakes_promptly_on_cancellation() {
    let (started_tx, started_rx) = mpsc::sync_channel(1);
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("prompt_cancellation", KernelHandle::new),
            PromptCancellationKernel {
                started: started_tx,
            },
        )
        .unwrap();
    let execution_plan = plan(
        vec![operation("prompt_cancellation", &[], &[])],
        0,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    let cancellation = CancellationToken::new();

    thread::scope(|scope| {
        let run_cancellation = cancellation.clone();
        let run = scope.spawn(|| {
            RunExecutor::new(
                &kernels,
                &no_resources(),
                &NoFunctions,
                crate::node_system::runtime::ResultStore::new(),
                std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
            )
            .run(&execution_plan, run_cancellation)
        });
        started_rx.recv().unwrap();
        let cancelled_at = std::time::Instant::now();
        cancellation.cancel();
        assert_eq!(run.join().unwrap(), Err(RunError::Cancelled));
        assert!(cancelled_at.elapsed() < Duration::from_millis(100));
    });
}

struct CancelAfterDeadlineKernel;

impl Kernel for CancelAfterDeadlineKernel {
    fn execute(
        &self,
        context: &KernelContext<'_>,
        _: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        thread::sleep(Duration::from_millis(25));
        context.cancellation.cancel();
        Err(KernelError::cancelled("cancel after scheduler deadline"))
    }
}

#[test]
fn cancellation_observed_while_draining_upgrades_deadline() {
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("cancel_after_deadline", KernelHandle::new),
            CancelAfterDeadlineKernel,
        )
        .unwrap();
    let execution_plan = plan(
        vec![operation("cancel_after_deadline", &[], &[])],
        0,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );

    assert_eq!(
        RunExecutor::new(
            &kernels,
            &no_resources(),
            &NoFunctions,
            crate::node_system::runtime::ResultStore::new(),
            std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new())
        )
        .with_deadline(RunDeadline::after(Duration::from_millis(5)))
        .run(&execution_plan, CancellationToken::new()),
        Err(RunError::Cancelled),
    );
}

#[test]
fn cancellation_wakes_blocked_stream_send_and_receive() {
    let token = CancellationToken::new();
    let (sender, _receiver) = bounded_stream_channel(1, token.clone()).unwrap();
    sender.send(1).unwrap();
    let blocked_sender = sender.clone();
    let send = thread::spawn(move || blocked_sender.send(2));
    thread::sleep(Duration::from_millis(20));
    token.cancel();
    assert_eq!(send.join().unwrap(), Err(StreamSendError::Cancelled(2)));

    let receive_token = CancellationToken::new();
    let (_sender, receiver) = bounded_stream_channel::<i32>(1, receive_token.clone()).unwrap();
    let receive = thread::spawn(move || receiver.recv());
    thread::sleep(Duration::from_millis(20));
    receive_token.cancel();
    assert_eq!(receive.join().unwrap(), Err(StreamReceiveError::Cancelled));
}
