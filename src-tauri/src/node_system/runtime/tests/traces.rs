use super::*;

#[test]
fn trace_sink_completion_panic_does_not_replace_successful_run() {
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("trace_sink_success", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| Ok(vec![Value::Integer(1).into()])),
        )
        .unwrap();
    let execution_plan = plan(
        vec![operation("trace_sink_success", &[], &[0])],
        1,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );

    let result = RunExecutor::new(
        &kernels,
        &no_resources(),
        &NoFunctions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .with_trace_sink(&PanickingCompletionTrace)
    .run(&execution_plan, CancellationToken::new());

    assert!(result.is_ok());
}

#[test]
fn cleanup_spans_cover_success_failure_and_cancellation() {
    let trace = RecordingTrace::default();
    let resources = no_resources();
    let kernels = KernelRegistry::new();
    let functions = NoFunctions;
    let executor = RunExecutor::new(
        &kernels,
        &resources,
        &functions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .with_trace_sink(&trace);
    let valid = plan(vec![], 0, StructuredControlRegion::Sequence(Box::new([])));
    executor.run(&valid, CancellationToken::new()).unwrap();

    let invalid = plan(
        vec![operation("missing", &[], &[])],
        0,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    assert!(matches!(
        executor.run(&invalid, CancellationToken::new()),
        Err(RunError::KernelNotFound(_))
    ));

    let cancelled = CancellationToken::new();
    cancelled.cancel();
    assert_eq!(executor.run(&valid, cancelled), Err(RunError::Cancelled));

    let cleanup = trace
        .0
        .lock()
        .unwrap()
        .iter()
        .filter(|span| span.kind == SpanKind::Cleanup && span.correlation.parent_call.is_none())
        .map(|span| span.outcome.clone())
        .collect::<Vec<_>>();
    assert_eq!(cleanup.len(), 3);
    assert!(cleanup.iter().all(|outcome| matches!(
        outcome,
        SpanOutcome::Cleanup {
            error_count: 0,
            panicking: false,
        }
    )));
}

#[test]
fn nested_call_spans_record_the_parent_call_and_callee_compile() {
    let mut callee = plan(vec![], 0, StructuredControlRegion::Sequence(Box::new([])));
    callee.provenance.compile_id = CompileId::new(22);
    callee.provenance.graph_path = GraphResourcePath("functions/callee".into());
    let mut caller = plan(
        vec![],
        0,
        StructuredControlRegion::Call {
            target: id("functions/callee", FunctionPlanHandle::new),
            arguments: Box::new([]),
            results: Box::new([]),
            mandatory: true,
        },
    );
    caller.provenance.compile_id = CompileId::new(11);
    let trace = RecordingTrace::default();

    RunExecutor::new(
        &KernelRegistry::new(),
        &no_resources(),
        &OneFunction(published_function(callee, "functions/callee", &[], &[])),
        ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .with_trace_sink(&trace)
    .run(&caller, CancellationToken::new())
    .unwrap();

    let events = trace.0.lock().unwrap();
    let child = events
        .iter()
        .find(|span| {
            span.kind == SpanKind::Run
                && span.outcome == SpanOutcome::Success
                && span.correlation.compile_id == CompileId::new(22)
        })
        .expect("callee run span");
    assert!(child.correlation.parent_call.is_some());
    assert_eq!(child.correlation.graph_path.0.as_ref(), "functions/callee");
}

#[derive(Clone, Copy)]
enum TraceRelationalOutcome {
    Succeed,
    Fail,
    Cancel,
}

struct TraceRelationalBackend(TraceRelationalOutcome);

impl RelationalBackend for TraceRelationalBackend {
    fn execute(
        &self,
        context: &RelationalContext<'_>,
        _: &CompiledRelationalPlan,
        _: &[RuntimeValue],
    ) -> Result<RelationalExecution, RelationalError> {
        match self.0 {
            TraceRelationalOutcome::Succeed => Ok(RelationalExecution {
                outputs: vec![Value::Integer(41).into()],
            }),
            TraceRelationalOutcome::Fail => {
                Err(RelationalError::operator_invalid("backend failed"))
            }
            TraceRelationalOutcome::Cancel => {
                context.cancellation.cancel();
                Err(RelationalError::cancelled(
                    "relational execution was cancelled",
                ))
            }
        }
    }
}

fn run_relational_backend_trace(
    outcome: TraceRelationalOutcome,
) -> (Result<RunResult, RunError>, ExecutionPlan, Vec<TraceSpan>) {
    let mut relational = RelationalBackendRegistry::new();
    relational
        .register(
            id("trace-backend", RelationalBackendId::new),
            TraceRelationalBackend(outcome),
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
        "trace-backend",
        "private-fragment",
        Box::new([]),
    )]);
    execution_plan.results = Box::new([PlanResult {
        name: "result".into(),
        output: stable_output("result"),
        value: ValueRef::new(0),
    }]);
    publish_graph_results(&mut execution_plan);
    let trace = RecordingTrace::default();

    let result = RunExecutor::new(
        &KernelRegistry::new(),
        &no_resources(),
        &NoFunctions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .with_relational_backends(&relational)
    .with_trace_sink(&trace)
    .run(&execution_plan, CancellationToken::new());
    let events = trace.0.into_inner().unwrap();
    (result, execution_plan, events)
}

struct OwnerThreadTrace {
    owner: thread::ThreadId,
    off_owner_calls: AtomicUsize,
    events: Mutex<Vec<TraceSpan>>,
}

impl OwnerThreadTrace {
    fn current() -> Self {
        Self {
            owner: thread::current().id(),
            off_owner_calls: AtomicUsize::new(0),
            events: Mutex::new(Vec::new()),
        }
    }
}

impl TraceSink for OwnerThreadTrace {
    fn start_span(&self, spec: SpanSpec) -> SpanGuard<'_> {
        SpanGuard::new(self, spec, &SYSTEM_TRACE_CLOCK)
    }

    fn complete_span(&self, span: TraceSpan) {
        if thread::current().id() != self.owner {
            self.off_owner_calls.fetch_add(1, Ordering::SeqCst);
        }
        self.events.lock().unwrap().push(span);
    }
}

struct SynchronizedSuccessBackend {
    started: mpsc::SyncSender<()>,
    release: Mutex<mpsc::Receiver<()>>,
}

impl RelationalBackend for SynchronizedSuccessBackend {
    fn execute(
        &self,
        _: &RelationalContext<'_>,
        _: &CompiledRelationalPlan,
        _: &[RuntimeValue],
    ) -> Result<RelationalExecution, RelationalError> {
        self.started.send(()).unwrap();
        self.release.lock().unwrap().recv().unwrap();
        Ok(RelationalExecution {
            outputs: vec![Value::Integer(7).into()],
        })
    }
}

fn synchronized_relational_plan(backend: &str) -> ExecutionPlan {
    let mut execution_plan = plan(
        vec![relational_operation(0, &[0])],
        1,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    execution_plan.relational_subplans = Box::new([relational_subplan(
        backend,
        "synchronized-fragment",
        Box::new([]),
    )]);
    execution_plan
}

fn assert_deadline_worker_trace(spans: &[TraceSpan], attempt_outcome: SpanOutcome) {
    let run = spans
        .iter()
        .find(|span| span.kind == SpanKind::Run)
        .unwrap();
    let attempt = spans
        .iter()
        .filter(|span| span.kind == SpanKind::OperationAttempt)
        .collect::<Vec<_>>();
    let adapter = spans
        .iter()
        .filter(|span| span.kind == SpanKind::AdapterIo)
        .collect::<Vec<_>>();
    assert_eq!(attempt.len(), 1);
    assert_eq!(adapter.len(), 1);
    assert_eq!(attempt[0].parent_span_id, Some(run.span_id));
    assert_eq!(attempt[0].outcome, attempt_outcome);
    assert_eq!(adapter[0].parent_span_id, Some(attempt[0].span_id));
    assert_eq!(adapter[0].outcome, SpanOutcome::Success);
    assert_eq!(adapter[0].operation_id, attempt[0].operation_id);
    assert_eq!(adapter[0].activation_id, attempt[0].activation_id);
    assert_eq!(adapter[0].attempt_id, attempt[0].attempt_id);
    for kind in [
        SpanKind::ResourceAcquire,
        SpanKind::ResultPublication,
        SpanKind::Cleanup,
    ] {
        let phase = spans
            .iter()
            .filter(|span| span.kind == kind)
            .collect::<Vec<_>>();
        assert_eq!(phase.len(), 1, "expected exactly one {kind:?} span");
        assert_eq!(phase[0].parent_span_id, Some(run.span_id));
    }
    let ids = spans
        .iter()
        .map(|span| span.span_id)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        ids.len(),
        spans.len(),
        "completed spans must be forwarded once"
    );
}

#[test]
fn deadline_before_envelope_receive_forwards_worker_spans_once() {
    let trace = RecordingTrace::default();
    let mut relational = RelationalBackendRegistry::new();
    relational
        .register(
            id("deadline-before-receive", RelationalBackendId::new),
            TraceRelationalBackend(TraceRelationalOutcome::Succeed),
        )
        .unwrap();
    let execution_plan = synchronized_relational_plan("deadline-before-receive");
    let (produced_tx, produced_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let release_rx = Arc::new(Mutex::new(release_rx));
    let checkpoint_release = Arc::clone(&release_rx);

    let result = thread::scope(|scope| {
        let run = scope.spawn(|| {
            RunExecutor::new(
                &KernelRegistry::new(),
                &no_resources(),
                &NoFunctions,
                crate::node_system::runtime::ResultStore::new(),
                std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
            )
            .with_relational_backends(&relational)
            .with_trace_sink(&trace)
            .with_deadline(RunDeadline::after(Duration::from_millis(20)))
            .with_test_checkpoint(Arc::new(move |checkpoint, _| {
                if checkpoint == SchedulerCheckpoint::WorkerOutcomeProduced {
                    produced_tx.send(()).unwrap();
                    checkpoint_release.lock().unwrap().recv().unwrap();
                }
            }))
            .run(&execution_plan, CancellationToken::new())
        });
        produced_rx.recv().unwrap();
        thread::sleep(Duration::from_millis(40));
        release_tx.send(()).unwrap();
        run.join().unwrap()
    });

    assert_eq!(
        result,
        Err(RunError::DeadlineExceeded {
            phase: RunPhase::Kernel,
        })
    );
    assert_deadline_worker_trace(&trace.0.lock().unwrap(), SpanOutcome::Success);
}

#[test]
fn completion_after_deadline_forwards_worker_spans_once() {
    let trace = RecordingTrace::default();
    let (started_tx, started_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let mut relational = RelationalBackendRegistry::new();
    relational
        .register(
            id("completion-after-deadline", RelationalBackendId::new),
            SynchronizedSuccessBackend {
                started: started_tx,
                release: Mutex::new(release_rx),
            },
        )
        .unwrap();
    let execution_plan = synchronized_relational_plan("completion-after-deadline");

    let result = thread::scope(|scope| {
        let run = scope.spawn(|| {
            RunExecutor::new(
                &KernelRegistry::new(),
                &no_resources(),
                &NoFunctions,
                crate::node_system::runtime::ResultStore::new(),
                std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
            )
            .with_relational_backends(&relational)
            .with_trace_sink(&trace)
            .with_deadline(RunDeadline::after(Duration::from_millis(20)))
            .run(&execution_plan, CancellationToken::new())
        });
        started_rx.recv().unwrap();
        thread::sleep(Duration::from_millis(40));
        release_tx.send(()).unwrap();
        run.join().unwrap()
    });

    assert_eq!(
        result,
        Err(RunError::DeadlineExceeded {
            phase: RunPhase::Kernel,
        })
    );
    assert_deadline_worker_trace(&trace.0.lock().unwrap(), SpanOutcome::Timeout);
}

#[test]
fn retryable_failure_after_deadline_keeps_retry_attempt_truth() {
    let trace = RecordingTrace::default();
    let (started_tx, started_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let release_rx = Arc::new(Mutex::new(release_rx));
    let kernel_release = Arc::clone(&release_rx);
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("retry_truth", KernelHandle::new),
            FnKernel(move |_: &[RuntimeValue]| {
                started_tx.send(()).unwrap();
                kernel_release.lock().unwrap().recv().unwrap();
                Err(KernelError::transient("retry truth"))
            }),
        )
        .unwrap();
    let execution_plan = retry_plan("retry_truth", 2, Duration::ZERO);

    let result = thread::scope(|scope| {
        let run = scope.spawn(|| {
            RunExecutor::new(
                &kernels,
                &no_resources(),
                &NoFunctions,
                crate::node_system::runtime::ResultStore::new(),
                std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
            )
            .with_trace_sink(&trace)
            .with_deadline(RunDeadline::after(Duration::from_millis(20)))
            .run(&execution_plan, CancellationToken::new())
        });
        started_rx.recv().unwrap();
        thread::sleep(Duration::from_millis(40));
        release_tx.send(()).unwrap();
        run.join().unwrap()
    });

    assert!(matches!(result, Err(RunError::DeadlineExceeded { .. })));
    let attempts = trace
        .0
        .lock()
        .unwrap()
        .iter()
        .filter(|span| span.kind == SpanKind::OperationAttempt)
        .map(|span| span.outcome.clone())
        .collect::<Vec<_>>();
    assert_eq!(attempts, [SpanOutcome::Retry]);
}

#[test]
fn success_completed_after_cancellation_rewrites_attempt_but_preserves_adapter_truth() {
    let trace = RecordingTrace::default();
    let cancellation = CancellationToken::new();
    let run_cancellation = cancellation.clone();
    let (started_tx, started_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let mut relational = RelationalBackendRegistry::new();
    relational
        .register(
            id("success_after_cancel", RelationalBackendId::new),
            SynchronizedSuccessBackend {
                started: started_tx,
                release: Mutex::new(release_rx),
            },
        )
        .unwrap();
    let execution_plan = synchronized_relational_plan("success_after_cancel");

    let result = thread::scope(|scope| {
        let run = scope.spawn(|| {
            RunExecutor::new(
                &KernelRegistry::new(),
                &no_resources(),
                &NoFunctions,
                crate::node_system::runtime::ResultStore::new(),
                std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
            )
            .with_relational_backends(&relational)
            .with_trace_sink(&trace)
            .run(&execution_plan, run_cancellation)
        });
        started_rx.recv().unwrap();
        cancellation.cancel();
        release_tx.send(()).unwrap();
        run.join().unwrap()
    });

    assert_eq!(result, Err(RunError::Cancelled));
    let spans = trace.0.lock().unwrap();
    assert!(spans.iter().any(|span| {
        span.kind == SpanKind::OperationAttempt && span.outcome == SpanOutcome::Cancellation
    }));
    assert!(spans.iter().any(|span| {
        span.kind == SpanKind::AdapterIo && span.outcome == SpanOutcome::Cancellation
    }));
}

#[test]
fn success_completed_before_cancellation_keeps_attempt_truth_while_envelope_drains() {
    let trace = RecordingTrace::default();
    let cancellation = CancellationToken::new();
    let run_cancellation = cancellation.clone();
    let (produced_tx, produced_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let release_rx = Arc::new(Mutex::new(release_rx));
    let checkpoint_release = Arc::clone(&release_rx);
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("success_before_cancel", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| Ok(vec![])),
        )
        .unwrap();
    let execution_plan = plan(
        vec![operation("success_before_cancel", &[], &[])],
        0,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );

    let result = thread::scope(|scope| {
        let run = scope.spawn(|| {
            RunExecutor::new(
                &kernels,
                &no_resources(),
                &NoFunctions,
                crate::node_system::runtime::ResultStore::new(),
                std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
            )
            .with_trace_sink(&trace)
            .with_test_checkpoint(Arc::new(move |checkpoint, _| {
                if checkpoint == SchedulerCheckpoint::WorkerOutcomeProduced {
                    produced_tx.send(()).unwrap();
                    checkpoint_release.lock().unwrap().recv().unwrap();
                }
            }))
            .run(&execution_plan, run_cancellation)
        });
        produced_rx.recv().unwrap();
        cancellation.cancel();
        release_tx.send(()).unwrap();
        run.join().unwrap()
    });

    assert_eq!(result, Err(RunError::Cancelled));
    let attempts = trace
        .0
        .lock()
        .unwrap()
        .iter()
        .filter(|span| span.kind == SpanKind::OperationAttempt)
        .map(|span| span.outcome.clone())
        .collect::<Vec<_>>();
    assert_eq!(attempts, [SpanOutcome::Success]);
}

#[test]
fn panic_attempt_truth_survives_deadline_and_cancellation() {
    for terminal in ["deadline", "cancellation"] {
        let trace = RecordingTrace::default();
        let cancellation = CancellationToken::new();
        let run_cancellation = cancellation.clone();
        let (started_tx, started_rx) = mpsc::sync_channel(1);
        let (release_tx, release_rx) = mpsc::sync_channel(1);
        let release_rx = Arc::new(Mutex::new(release_rx));
        let kernel_release = Arc::clone(&release_rx);
        let mut kernels = KernelRegistry::new();
        kernels
            .register(
                id("panic_terminal_truth", KernelHandle::new),
                FnKernel(move |_: &[RuntimeValue]| {
                    started_tx.send(()).unwrap();
                    kernel_release.lock().unwrap().recv().unwrap();
                    panic!("panic terminal truth sentinel")
                }),
            )
            .unwrap();
        let execution_plan = plan(
            vec![operation("panic_terminal_truth", &[], &[])],
            0,
            StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(
                OperationIndex::new(0),
            )])),
        );

        let panic = thread::scope(|scope| {
            let run = scope.spawn(|| {
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let resources = no_resources();
                    let mut executor = RunExecutor::new(
                        &kernels,
                        &resources,
                        &NoFunctions,
                        crate::node_system::runtime::ResultStore::new(),
                        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
                    )
                    .with_trace_sink(&trace);
                    if terminal == "deadline" {
                        executor =
                            executor.with_deadline(RunDeadline::after(Duration::from_millis(20)));
                    }
                    let _ = executor.run(&execution_plan, run_cancellation);
                }))
            });
            started_rx.recv().unwrap();
            if terminal == "deadline" {
                thread::sleep(Duration::from_millis(40));
            } else {
                cancellation.cancel();
            }
            release_tx.send(()).unwrap();
            run.join().unwrap()
        });
        assert!(panic.is_err());
        assert!(trace.0.lock().unwrap().iter().any(|span| {
            span.kind == SpanKind::OperationAttempt && span.outcome == SpanOutcome::InternalAborted
        }));
    }
}

#[test]
fn peer_ordinary_error_does_not_rewrite_drained_success_attempt() {
    let trace = RecordingTrace::default();
    let entered = Arc::new(Barrier::new(3));
    let success_thread = Arc::new(Mutex::new(None));
    let (produced_tx, produced_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let release_rx = Arc::new(Mutex::new(release_rx));
    let mut kernels = KernelRegistry::new();
    let error_entered = Arc::clone(&entered);
    kernels
        .register(
            id("parallel0", KernelHandle::new),
            FnKernel(move |_: &[RuntimeValue]| {
                error_entered.wait();
                thread::sleep(Duration::from_millis(20));
                Err(KernelError::new("peer ordinary error"))
            }),
        )
        .unwrap();
    let success_entered = Arc::clone(&entered);
    let worker_thread = Arc::clone(&success_thread);
    kernels
        .register(
            id("parallel1", KernelHandle::new),
            FnKernel(move |_: &[RuntimeValue]| {
                success_entered.wait();
                *worker_thread.lock().unwrap() = Some(thread::current().id());
                Ok(vec![Value::Integer(1).into()])
            }),
        )
        .unwrap();
    let execution_plan = independent_parallel_plan(&[WorkloadClass::Cpu, WorkloadClass::Cpu]);

    let checkpoint_thread = Arc::clone(&success_thread);
    let checkpoint_release = Arc::clone(&release_rx);
    let error = thread::scope(|scope| {
        let run = scope.spawn(|| {
            RunExecutor::new(
                &kernels,
                &no_resources(),
                &NoFunctions,
                crate::node_system::runtime::ResultStore::new(),
                std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
            )
            .with_trace_sink(&trace)
            .with_scheduling_policy(parallel_policy(2, 1, 1))
            .with_test_checkpoint(Arc::new(move |checkpoint, _| {
                if checkpoint == SchedulerCheckpoint::WorkerOutcomeProduced
                    && checkpoint_thread.lock().unwrap().as_ref() == Some(&thread::current().id())
                {
                    produced_tx.send(()).unwrap();
                    checkpoint_release.lock().unwrap().recv().unwrap();
                }
            }))
            .run(&execution_plan, CancellationToken::new())
        });
        entered.wait();
        produced_rx.recv().unwrap();
        thread::sleep(Duration::from_millis(40));
        release_tx.send(()).unwrap();
        run.join().unwrap().unwrap_err()
    });
    assert!(matches!(error, RunError::KernelFailed { .. }));
    let spans = trace.0.lock().unwrap();
    let success_operation = OperationStableId::new("test.operation.parallel1").unwrap();
    let attempts = spans
        .iter()
        .filter(|span| span.kind == SpanKind::OperationAttempt)
        .map(|span| (span.operation_id.clone(), span.outcome.clone()))
        .collect::<Vec<_>>();
    assert!(
        attempts.iter().any(|(operation, outcome)| {
            operation.as_ref() == Some(&success_operation) && *outcome == SpanOutcome::Success
        }),
        "attempts: {attempts:?}"
    );
}

#[test]
fn parallel_scheduler_workers_return_relational_trace_to_owner_thread() {
    let mut relational = RelationalBackendRegistry::new();
    relational
        .register(
            id("trace-owner", RelationalBackendId::new),
            TraceRelationalBackend(TraceRelationalOutcome::Succeed),
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
        "trace-owner",
        "private-fragment",
        Box::new([]),
    )]);
    let trace = OwnerThreadTrace::current();

    RunExecutor::new(
        &KernelRegistry::new(),
        &no_resources(),
        &NoFunctions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .with_relational_backends(&relational)
    .with_trace_sink(&trace)
    .run(&execution_plan, CancellationToken::new())
    .unwrap();

    assert_eq!(trace.off_owner_calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        trace
            .events
            .lock()
            .unwrap()
            .iter()
            .filter(|span| span.kind == SpanKind::AdapterIo)
            .count(),
        1
    );
}

fn assert_relational_backend_trace(
    execution_plan: &ExecutionPlan,
    spans: &[TraceSpan],
    terminal_outcome: SpanOutcome,
) {
    let spans = spans
        .iter()
        .filter(|span| span.kind == SpanKind::AdapterIo)
        .collect::<Vec<_>>();
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].outcome, terminal_outcome);

    let correlation = &spans[0].correlation;
    assert_eq!(
        correlation.project_session_id,
        execution_plan.provenance.project_session_id
    );
    assert_eq!(correlation.graph_path, execution_plan.provenance.graph_path);
    assert_eq!(
        correlation.graph_revision,
        execution_plan.provenance.basis.graph_revision
    );
    assert_eq!(
        correlation.registry_fingerprint,
        execution_plan.provenance.basis.registry_fingerprint
    );
    assert_eq!(
        correlation.resource_versions,
        execution_plan.provenance.basis.resource_versions
    );
    assert_eq!(correlation.compile_id, execution_plan.provenance.compile_id);
    assert!(correlation.run_id.is_some());
    assert_eq!(
        correlation.node_id,
        Some(execution_plan.operations[0].source_node_id)
    );
    assert_eq!(
        correlation.node_type_id,
        Some(execution_plan.operations[0].source_node_type_id.clone())
    );
    assert_eq!(correlation.parent_call, None);
    assert_eq!(
        spans[0].operation_id.as_ref(),
        Some(&execution_plan.operations[0].stable_id)
    );
    assert!(spans[0].activation_id.is_some());
    assert_eq!(spans[0].attempt_id, Some(AttemptId::initial()));
}

#[test]
fn relational_backend_trace_records_success_with_full_operation_correlation() {
    let (result, execution_plan, events) =
        run_relational_backend_trace(TraceRelationalOutcome::Succeed);

    result.unwrap();
    assert_relational_backend_trace(&execution_plan, &events, SpanOutcome::Success);
}

#[test]
fn relational_backend_trace_records_failure_with_full_operation_correlation() {
    let (result, execution_plan, events) =
        run_relational_backend_trace(TraceRelationalOutcome::Fail);

    assert!(matches!(result, Err(RunError::RelationalFailed { .. })));
    assert_relational_backend_trace(&execution_plan, &events, SpanOutcome::Error);
}

#[test]
fn relational_backend_trace_records_cancellation_with_full_operation_correlation() {
    let (result, execution_plan, events) =
        run_relational_backend_trace(TraceRelationalOutcome::Cancel);

    assert_eq!(result, Err(RunError::Cancelled));
    assert_relational_backend_trace(&execution_plan, &events, SpanOutcome::Cancellation);
}
