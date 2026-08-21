use super::*;

#[test]
fn successful_run_releases_all_resources() {
    let resources = no_resources();
    let released = resources.released.clone();
    let mut execution_plan = plan(vec![], 0, StructuredControlRegion::Sequence(Box::new([])));
    execution_plan.resources = Box::new([requirement("one"), requirement("two")]);

    RunExecutor::new(
        &KernelRegistry::new(),
        &resources,
        &NoFunctions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .run(&execution_plan, CancellationToken::new())
    .unwrap();

    assert_eq!(released.load(Ordering::SeqCst), 2);
}

#[test]
fn acquire_failure_releases_previously_acquired_resources() {
    let trace = RecordingTrace::default();
    let released = Arc::new(AtomicUsize::new(0));
    let resources = TrackingResources {
        acquired: Arc::new(AtomicUsize::new(0)),
        released: released.clone(),
        fail_at: Some(2),
    };
    let mut execution_plan = plan(vec![], 0, StructuredControlRegion::Sequence(Box::new([])));
    execution_plan.resources = Box::new([requirement("one"), requirement("two")]);

    let error = RunExecutor::new(
        &KernelRegistry::new(),
        &resources,
        &NoFunctions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .with_trace_sink(&trace)
    .run(&execution_plan, CancellationToken::new())
    .unwrap_err();

    assert!(matches!(error, RunError::ResourceAcquire { .. }));
    assert_eq!(released.load(Ordering::SeqCst), 1);
    assert_run_phase_coverage(
        &trace.0.lock().unwrap(),
        SpanOutcome::Error,
        SpanOutcome::NotReached,
    );
}

#[test]
fn kernel_failure_releases_resources_without_retry() {
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("fail", KernelHandle::new),
            FnKernel(move |_: &[RuntimeValue]| {
                observed.fetch_add(1, Ordering::SeqCst);
                Err(KernelError::new("kernel failed"))
            }),
        )
        .unwrap();
    let mut execution_plan = plan(
        vec![operation("fail", &[], &[0])],
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
    .run(&execution_plan, CancellationToken::new())
    .unwrap_err();

    assert!(matches!(error, RunError::KernelFailed { .. }));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(released.load(Ordering::SeqCst), 1);
}
