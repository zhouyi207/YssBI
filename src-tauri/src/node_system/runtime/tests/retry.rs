use super::*;

struct RetryProgressGate {
    release: Mutex<mpsc::Receiver<()>>,
    completed: mpsc::Sender<()>,
}

impl Kernel for RetryProgressGate {
    fn execute(
        &self,
        _: &KernelContext<'_>,
        _: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        self.release.lock().unwrap().recv().unwrap();
        self.completed.send(()).unwrap();
        Ok(vec![Value::Integer(1).into()])
    }
}

fn retry_progress_plan(gates: usize, exclusive_tail: bool) -> ExecutionPlan {
    let mut retry = operation("retry_progress", &[], &[0]);
    retry.cache_policy = CachePolicy::PerRun;
    retry.retry = PlannedRetry {
        idempotent: true,
        policy: Some(retry_policy(2, Duration::from_secs(5))),
    };
    let mut operations = vec![retry];
    for index in 0..gates {
        let mut gate = operation("retry_progress_gate", &[], &[(index + 1) as u32]);
        gate.stable_id =
            OperationStableId::new(format!("test.retry.progress.gate.{index}")).unwrap();
        if exclusive_tail && index + 1 == gates {
            gate.workload = WorkloadClass::Exclusive;
        }
        operations.push(gate);
    }
    let mut execution_plan = plan(
        operations,
        (gates + 1) as u32,
        StructuredControlRegion::Sequence(
            (0..=gates)
                .map(|index| ControlStep::Operation(OperationIndex::new(index as u32)))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        ),
    );
    if exclusive_tail && gates >= 2 {
        execution_plan.effect_dependencies = Box::new([EffectDependency {
            before: OperationIndex::new((gates - 1) as u32),
            after: OperationIndex::new(gates as u32),
        }]);
    }
    execution_plan
}

#[test]
fn retry_delayed_queue_drains_bounded_completions_during_long_backoff() {
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&calls);
    let (release_tx, release_rx) = mpsc::channel();
    let (completed_tx, completed_rx) = mpsc::channel();
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("retry_progress", KernelHandle::new),
            FnKernel(move |_: &[RuntimeValue]| {
                if observed.fetch_add(1, Ordering::SeqCst) == 0 {
                    Err(KernelError::transient("delay"))
                } else {
                    Ok(vec![Value::Integer(0).into()])
                }
            }),
        )
        .unwrap();
    kernels
        .register(
            id("retry_progress_gate", KernelHandle::new),
            RetryProgressGate {
                release: Mutex::new(release_rx),
                completed: completed_tx,
            },
        )
        .unwrap();
    let cancellation = CancellationToken::new();
    let cancel_run = cancellation.clone();
    let release = Arc::new(Mutex::new(Some(release_tx)));
    let release_at_backoff = Arc::clone(&release);
    let execution_plan = retry_progress_plan(6, false);

    thread::scope(|scope| {
        let run = scope.spawn(|| {
            RunExecutor::new(
                &kernels,
                &no_resources(),
                &NoFunctions,
                crate::node_system::runtime::ResultStore::new(),
                std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
            )
            .with_scheduling_policy(parallel_policy(2, 1, 1))
            .with_test_checkpoint(Arc::new(move |checkpoint, _| {
                if matches!(checkpoint, SchedulerCheckpoint::RetryBackoff { .. })
                    && let Some(release) = release_at_backoff.lock().unwrap().take()
                {
                    for _ in 0..6 {
                        release.send(()).unwrap();
                    }
                }
            }))
            .run(&execution_plan, cancellation)
        });
        for _ in 0..6 {
            completed_rx
                .recv_timeout(Duration::from_millis(500))
                .unwrap();
        }
        cancel_run.cancel();
        assert_eq!(run.join().unwrap(), Err(RunError::Cancelled));
    });
}

#[test]
fn retry_delayed_queue_allows_exclusive_effect_progress() {
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&calls);
    let (release_tx, release_rx) = mpsc::channel();
    let (completed_tx, completed_rx) = mpsc::channel();
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("retry_progress", KernelHandle::new),
            FnKernel(move |_: &[RuntimeValue]| {
                if observed.fetch_add(1, Ordering::SeqCst) == 0 {
                    Err(KernelError::transient("delay"))
                } else {
                    Ok(vec![Value::Integer(0).into()])
                }
            }),
        )
        .unwrap();
    kernels
        .register(
            id("retry_progress_gate", KernelHandle::new),
            RetryProgressGate {
                release: Mutex::new(release_rx),
                completed: completed_tx,
            },
        )
        .unwrap();
    let cancellation = CancellationToken::new();
    let cancel_run = cancellation.clone();
    let release = Arc::new(Mutex::new(Some(release_tx)));
    let release_at_backoff = Arc::clone(&release);
    let execution_plan = retry_progress_plan(2, true);

    thread::scope(|scope| {
        let run = scope.spawn(|| {
            RunExecutor::new(
                &kernels,
                &no_resources(),
                &NoFunctions,
                crate::node_system::runtime::ResultStore::new(),
                std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
            )
            .with_scheduling_policy(parallel_policy(2, 1, 1))
            .with_test_checkpoint(Arc::new(move |checkpoint, _| {
                if matches!(checkpoint, SchedulerCheckpoint::RetryBackoff { .. })
                    && let Some(release) = release_at_backoff.lock().unwrap().take()
                {
                    release.send(()).unwrap();
                    release.send(()).unwrap();
                }
            }))
            .run(&execution_plan, cancellation)
        });
        completed_rx
            .recv_timeout(Duration::from_millis(500))
            .unwrap();
        completed_rx
            .recv_timeout(Duration::from_millis(500))
            .unwrap();
        cancel_run.cancel();
        assert_eq!(run.join().unwrap(), Err(RunError::Cancelled));
    });
}

#[derive(Debug, Clone, Copy)]
enum AdmissionRejection {
    Cancellation,
    Deadline,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AdmissionRollbackObservation {
    operation: OperationIndex,
    attempt: AttemptId,
    running_count: usize,
    tracked_running: usize,
    memo_owned: bool,
    frame_attempt: Option<AttemptId>,
}

struct CooperativeAdmissionPeer {
    calls: Arc<AtomicUsize>,
}

impl Kernel for CooperativeAdmissionPeer {
    fn execute(
        &self,
        context: &KernelContext<'_>,
        _: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        context.wait_for(Duration::from_millis(100))?;
        Ok(vec![Value::Integer(1).into()])
    }
}

fn admission_rejection_deadline(rejection: AdmissionRejection) -> RunDeadline {
    match rejection {
        AdmissionRejection::Cancellation => RunDeadline::after(Duration::from_secs(5)),
        AdmissionRejection::Deadline => RunDeadline::after(Duration::from_millis(10)),
    }
}

fn reject_admission(rejection: AdmissionRejection, cancellation: &CancellationToken) {
    match rejection {
        AdmissionRejection::Cancellation => cancellation.cancel(),
        AdmissionRejection::Deadline => thread::sleep(Duration::from_millis(30)),
    }
}

fn run_initial_admission_rejection(rejection: AdmissionRejection) {
    let peer_calls = Arc::new(AtomicUsize::new(0));
    let rejected_calls = Arc::new(AtomicUsize::new(0));
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("admission_peer", KernelHandle::new),
            CooperativeAdmissionPeer {
                calls: Arc::clone(&peer_calls),
            },
        )
        .unwrap();
    let observed_rejected = Arc::clone(&rejected_calls);
    kernels
        .register(
            id("admission_rejected", KernelHandle::new),
            FnKernel(move |_: &[RuntimeValue]| {
                observed_rejected.fetch_add(1, Ordering::SeqCst);
                Ok(vec![Value::Integer(2).into()])
            }),
        )
        .unwrap();
    let mut rejected = operation("admission_rejected", &[], &[1]);
    rejected.cache_policy = CachePolicy::PerRun;
    let execution_plan = plan(
        vec![operation("admission_peer", &[], &[0]), rejected],
        2,
        StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(0)),
            ControlStep::Operation(OperationIndex::new(1)),
        ])),
    );
    let rollback = Arc::new(Mutex::new(None));
    let observed_rollback = Arc::clone(&rollback);

    let error = RunExecutor::new(
        &kernels,
        &no_resources(),
        &NoFunctions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .with_scheduling_policy(parallel_policy(2, 1, 1))
    .with_deadline(admission_rejection_deadline(rejection))
    .with_test_checkpoint(Arc::new(move |checkpoint, cancellation| match checkpoint {
        SchedulerCheckpoint::AdmissionBookkept {
            operation, attempt, ..
        } if operation == OperationIndex::new(1) && attempt == AttemptId::initial() => {
            reject_admission(rejection, cancellation);
        }
        SchedulerCheckpoint::AdmissionRolledBack {
            operation,
            attempt,
            running_count,
            tracked_running,
            memo_owned,
            frame_attempt,
        } if operation == OperationIndex::new(1) => {
            *observed_rollback.lock().unwrap() = Some(AdmissionRollbackObservation {
                operation,
                attempt,
                running_count,
                tracked_running,
                memo_owned,
                frame_attempt,
            });
        }
        _ => {}
    }))
    .run(&execution_plan, CancellationToken::new())
    .unwrap_err();

    assert!(matches!(
        (rejection, error),
        (AdmissionRejection::Cancellation, RunError::Cancelled)
            | (
                AdmissionRejection::Deadline,
                RunError::DeadlineExceeded {
                    phase: RunPhase::QueueWait
                }
            )
    ));
    assert!(
        peer_calls.load(Ordering::SeqCst) <= 1,
        "cancellation may stop the admitted peer before kernel invocation"
    );
    assert_eq!(rejected_calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        rollback.lock().unwrap().clone(),
        Some(AdmissionRollbackObservation {
            operation: OperationIndex::new(1),
            attempt: AttemptId::initial(),
            running_count: 1,
            tracked_running: 1,
            memo_owned: false,
            frame_attempt: None,
        })
    );
}

fn run_promoted_retry_admission_rejection(rejection: AdmissionRejection) {
    let retry_calls = Arc::new(AtomicUsize::new(0));
    let peer_calls = Arc::new(AtomicUsize::new(0));
    let mut kernels = KernelRegistry::new();
    let observed_retry = Arc::clone(&retry_calls);
    kernels
        .register(
            id("retry_admission_rejected", KernelHandle::new),
            FnKernel(move |_: &[RuntimeValue]| {
                if observed_retry.fetch_add(1, Ordering::SeqCst) == 0 {
                    Err(KernelError::transient("promote retry"))
                } else {
                    Ok(vec![Value::Integer(2).into()])
                }
            }),
        )
        .unwrap();
    kernels
        .register(
            id("retry_admission_peer", KernelHandle::new),
            CooperativeAdmissionPeer {
                calls: Arc::clone(&peer_calls),
            },
        )
        .unwrap();
    let mut retry = operation("retry_admission_rejected", &[], &[0]);
    retry.cache_policy = CachePolicy::PerRun;
    retry.retry = PlannedRetry {
        idempotent: true,
        policy: Some(retry_policy(2, Duration::ZERO)),
    };
    let execution_plan = plan(
        vec![retry, operation("retry_admission_peer", &[], &[1])],
        2,
        StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(0)),
            ControlStep::Operation(OperationIndex::new(1)),
        ])),
    );
    let rollback = Arc::new(Mutex::new(None));
    let observed_rollback = Arc::clone(&rollback);

    let error = RunExecutor::new(
        &kernels,
        &no_resources(),
        &NoFunctions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .with_scheduling_policy(parallel_policy(2, 1, 1))
    .with_deadline(admission_rejection_deadline(rejection))
    .with_test_checkpoint(Arc::new(move |checkpoint, cancellation| match checkpoint {
        SchedulerCheckpoint::AdmissionBookkept {
            operation, attempt, ..
        } if operation == OperationIndex::new(0) && attempt == AttemptId::new(2) => {
            reject_admission(rejection, cancellation);
        }
        SchedulerCheckpoint::AdmissionRolledBack {
            operation,
            attempt,
            running_count,
            tracked_running,
            memo_owned,
            frame_attempt,
        } if operation == OperationIndex::new(0) => {
            *observed_rollback.lock().unwrap() = Some(AdmissionRollbackObservation {
                operation,
                attempt,
                running_count,
                tracked_running,
                memo_owned,
                frame_attempt,
            });
        }
        _ => {}
    }))
    .run(&execution_plan, CancellationToken::new())
    .unwrap_err();

    assert!(matches!(
        (rejection, error),
        (AdmissionRejection::Cancellation, RunError::Cancelled)
            | (
                AdmissionRejection::Deadline,
                RunError::DeadlineExceeded {
                    phase: RunPhase::QueueWait
                }
            )
    ));
    assert_eq!(retry_calls.load(Ordering::SeqCst), 1);
    assert_eq!(peer_calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        rollback.lock().unwrap().clone(),
        Some(AdmissionRollbackObservation {
            operation: OperationIndex::new(0),
            attempt: AttemptId::new(2),
            running_count: 1,
            tracked_running: 1,
            memo_owned: false,
            frame_attempt: Some(AttemptId::initial()),
        })
    );
}

#[test]
fn admission_rejections_roll_back_before_queue_submission() {
    for run_rejection in [
        run_initial_admission_rejection as fn(AdmissionRejection),
        run_promoted_retry_admission_rejection,
    ] {
        for rejection in [
            AdmissionRejection::Cancellation,
            AdmissionRejection::Deadline,
        ] {
            run_rejection(rejection);
        }
    }
}

#[test]
fn retry_backoff_is_exponential_capped_and_overflow_safe() {
    let policy = RetryPolicy::new(
        NonZeroU32::new(10).unwrap(),
        Duration::from_millis(3),
        Duration::from_millis(10),
    )
    .unwrap();

    assert_eq!(
        super::super::scheduler::retry_backoff(policy, AttemptId::new(1)),
        Duration::from_millis(3)
    );
    assert_eq!(
        super::super::scheduler::retry_backoff(policy, AttemptId::new(2)),
        Duration::from_millis(6)
    );
    assert_eq!(
        super::super::scheduler::retry_backoff(policy, AttemptId::new(3)),
        Duration::from_millis(10)
    );
    assert_eq!(
        super::super::scheduler::retry_backoff(policy, AttemptId::new(u64::MAX)),
        Duration::from_millis(10)
    );
}

#[test]
fn retry_transient_failure_then_success_publishes_only_final_output() {
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&calls);
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("retry_transient_success", KernelHandle::new),
            FnKernel(move |_: &[RuntimeValue]| {
                if observed.fetch_add(1, Ordering::SeqCst) == 0 {
                    Err(KernelError::transient("try again"))
                } else {
                    Ok(vec![Value::Integer(42).into()])
                }
            }),
        )
        .unwrap();
    let trace = RecordingTrace::default();
    let results = ResultStore::new();

    let execution_plan = retry_plan("retry_transient_success", 3, Duration::ZERO);
    execution_plan.validate().unwrap();
    let run_result = RunExecutor::new(
        &kernels,
        &no_resources(),
        &NoFunctions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .with_trace_sink(&trace)
    .with_result_store(&results)
    .run(&execution_plan, CancellationToken::new())
    .unwrap();

    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(
        run_result.value_for_test("result").unwrap(),
        Value::Integer(42).into()
    );
    let result_id = run_result.result_ids["result"];
    let history = results.pin_history(&stable_output("retry_result"));
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].result_id, result_id);
    let spans = trace.0.lock().unwrap();
    let run = spans
        .iter()
        .find(|span| span.kind == SpanKind::Run)
        .unwrap();
    let attempts = spans
        .iter()
        .filter(|span| span.kind == SpanKind::OperationAttempt)
        .collect::<Vec<_>>();
    assert_eq!(attempts.len(), 2);
    assert_eq!(attempts[0].outcome, SpanOutcome::Retry);
    assert_eq!(attempts[1].outcome, SpanOutcome::Success);
    assert_eq!(attempts[0].attempt_id, Some(AttemptId::new(1)));
    assert_eq!(attempts[1].attempt_id, Some(AttemptId::new(2)));
    assert_eq!(attempts[0].operation_id, attempts[1].operation_id);
    assert_eq!(attempts[0].run_id, attempts[1].run_id);
    assert!(
        attempts
            .iter()
            .all(|span| span.parent_span_id == Some(run.span_id))
    );
    for kind in [
        SpanKind::ResourceAcquire,
        SpanKind::ResultPublication,
        SpanKind::Cleanup,
    ] {
        assert!(
            spans
                .iter()
                .any(|span| span.kind == kind && span.parent_span_id == Some(run.span_id))
        );
    }
}

#[test]
fn retry_permanent_error_never_retries() {
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&calls);
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("retry_permanent", KernelHandle::new),
            FnKernel(move |_: &[RuntimeValue]| {
                observed.fetch_add(1, Ordering::SeqCst);
                Err(KernelError::new("permanent"))
            }),
        )
        .unwrap();

    let error = RunExecutor::new(
        &kernels,
        &no_resources(),
        &NoFunctions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .run(
        &retry_plan("retry_permanent", 3, Duration::ZERO),
        CancellationToken::new(),
    )
    .unwrap_err();

    assert!(matches!(error, RunError::KernelFailed { .. }));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn retry_max_attempts_includes_initial_attempt() {
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&calls);
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("retry_exact_max", KernelHandle::new),
            FnKernel(move |_: &[RuntimeValue]| {
                observed.fetch_add(1, Ordering::SeqCst);
                Err(KernelError::transient("still transient"))
            }),
        )
        .unwrap();

    let error = RunExecutor::new(
        &kernels,
        &no_resources(),
        &NoFunctions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .run(
        &retry_plan("retry_exact_max", 3, Duration::ZERO),
        CancellationToken::new(),
    )
    .unwrap_err();

    assert!(matches!(error, RunError::KernelFailed { .. }));
    assert_eq!(calls.load(Ordering::SeqCst), 3);
}

#[test]
fn retry_insufficient_deadline_returns_typed_deadline_without_next_attempt() {
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&calls);
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("retry_deadline", KernelHandle::new),
            FnKernel(move |_: &[RuntimeValue]| {
                observed.fetch_add(1, Ordering::SeqCst);
                Err(KernelError::transient("retry later"))
            }),
        )
        .unwrap();

    let trace = RecordingTrace::default();
    let results = ResultStore::new();
    let error = RunExecutor::new(
        &kernels,
        &no_resources(),
        &NoFunctions,
        results.clone(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .with_trace_sink(&trace)
    .with_deadline(RunDeadline::after(Duration::from_millis(10)))
    .run(
        &retry_plan("retry_deadline", 3, Duration::from_millis(100)),
        CancellationToken::new(),
    )
    .unwrap_err();

    assert_eq!(
        error,
        RunError::DeadlineExceeded {
            phase: RunPhase::QueueWait,
        }
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let result_id = results.pin_history(&stable_output("retry_result"))[0].result_id;
    assert!(matches!(
        results.result(result_id).unwrap().state,
        ResultState::Failed(_)
    ));
    let spans = trace.0.lock().unwrap();
    assert!(spans.iter().any(|span| {
        span.kind == SpanKind::OperationAttempt && span.outcome == SpanOutcome::Retry
    }));
    assert!(
        spans
            .iter()
            .any(|span| span.kind == SpanKind::Run && span.outcome == SpanOutcome::Timeout)
    );
    assert!(spans.iter().any(|span| matches!(
        (&span.kind, &span.outcome),
        (SpanKind::Cleanup, SpanOutcome::Cleanup { .. })
    )));
}

#[test]
fn retry_cancellation_during_backoff_wakes_promptly() {
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("retry_cancel_backoff", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| Err(KernelError::transient("retry later"))),
        )
        .unwrap();
    let cancellation = CancellationToken::new();
    let started = Instant::now();
    let results = ResultStore::new();

    let error = RunExecutor::new(
        &kernels,
        &no_resources(),
        &NoFunctions,
        results.clone(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .with_test_checkpoint(Arc::new(|checkpoint, cancellation| {
        if matches!(checkpoint, SchedulerCheckpoint::RetryBackoff { .. }) {
            cancellation.cancel();
        }
    }))
    .run(
        &retry_plan("retry_cancel_backoff", 3, Duration::from_secs(5)),
        cancellation,
    )
    .unwrap_err();

    assert_eq!(error, RunError::Cancelled);
    assert!(started.elapsed() < Duration::from_millis(200));
    let result_id = results.pin_history(&stable_output("retry_result"))[0].result_id;
    assert!(matches!(
        results.result(result_id).unwrap().state,
        ResultState::Cancelled
    ));
}

struct RetryIdentityKernel {
    calls: AtomicUsize,
    activations: Arc<Mutex<Vec<ActivationId>>>,
}

impl Kernel for RetryIdentityKernel {
    fn execute(
        &self,
        context: &KernelContext<'_>,
        _: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        self.activations.lock().unwrap().push(context.activation_id);
        if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
            Err(KernelError::transient("retry with fresh identity"))
        } else {
            Ok(vec![Value::Integer(7).into()])
        }
    }
}

#[test]
fn retry_attempts_preserve_activation_and_result_provenance() {
    let activations = Arc::new(Mutex::new(Vec::new()));
    let attempts = Arc::new(Mutex::new(Vec::new()));
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("retry_identity", KernelHandle::new),
            RetryIdentityKernel {
                calls: AtomicUsize::new(0),
                activations: Arc::clone(&activations),
            },
        )
        .unwrap();
    let observed_attempts = Arc::clone(&attempts);
    let results = ResultStore::new();

    let run_result = RunExecutor::new(
        &kernels,
        &no_resources(),
        &NoFunctions,
        results.clone(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .with_test_checkpoint(Arc::new(move |checkpoint, _| {
        if let SchedulerCheckpoint::AttemptPrepared {
            operation,
            activation,
            attempt,
        } = checkpoint
        {
            observed_attempts
                .lock()
                .unwrap()
                .push((operation, activation, attempt));
        }
    }))
    .run(
        &retry_plan("retry_identity", 2, Duration::ZERO),
        CancellationToken::new(),
    )
    .unwrap();

    let activations = activations.lock().unwrap();
    assert_eq!(activations.len(), 2);
    assert_eq!(activations[0], activations[1]);
    let attempts = attempts.lock().unwrap();
    assert_eq!(attempts.len(), 2);
    assert_eq!(attempts[0].0, OperationIndex::new(0));
    assert_eq!(attempts[1].0, OperationIndex::new(0));
    assert_eq!(attempts[0].2, AttemptId::new(1));
    assert_eq!(attempts[1].2, AttemptId::new(2));
    assert_eq!(attempts[0].1, activations[0]);
    assert_eq!(attempts[1].1, activations[0]);
    let stored = results.result(run_result.result_ids["result"]).unwrap();
    assert_eq!(stored.provenance.run_id, run_result.run_id);
    assert_eq!(stored.provenance.activation_id, activations[0]);
}

#[test]
fn retry_runtime_defense_rejects_malformed_side_effect_plan() {
    let mut unsafe_operation = adapter_operation(
        "test.unsafe.retry",
        0,
        1,
        OutputProduction::Streaming,
        InputConsumption::FullyMaterialized,
    );
    unsafe_operation.retry = PlannedRetry {
        idempotent: true,
        policy: Some(retry_policy(2, Duration::ZERO)),
    };
    let mut execution_plan = plan(
        vec![unsafe_operation],
        2,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    execution_plan.value_sources = Box::new([PlanValueSource::ExternalInput(
        ValueRef::new(0),
        OutputProduction::Streaming,
    )]);

    assert!(
        execution_plan
            .validate()
            .unwrap_err()
            .0
            .iter()
            .any(|error| {
                matches!(error, PlanValidationError::InvalidRetryPolicy { operation }
            if *operation == OperationIndex::new(0))
            })
    );
    let error = RunExecutor::new(
        &KernelRegistry::new(),
        &no_resources(),
        &NoFunctions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .run(&execution_plan, CancellationToken::new())
    .unwrap_err();

    assert!(matches!(error, RunError::InvalidPlan(_)));
}
