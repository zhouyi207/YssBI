use super::*;

#[test]
fn deadline_phase_codes_are_stable_and_cancellation_has_priority() {
    let phases = [
        (RunPhase::QueueWait, "\"queueWait\""),
        (RunPhase::Kernel, "\"kernel\""),
        (RunPhase::StreamSend, "\"streamSend\""),
        (RunPhase::StreamReceive, "\"streamReceive\""),
        (RunPhase::AdapterIo, "\"adapterIo\""),
        (RunPhase::ResultPublication, "\"resultPublication\""),
        (RunPhase::Cleanup, "\"cleanup\""),
    ];
    for (phase, wire) in phases {
        assert_eq!(serde_json::to_string(&phase).unwrap(), wire);
        assert_eq!(
            RunDeadline::after(Duration::ZERO).check(&CancellationToken::new(), phase),
            Err(RunError::DeadlineExceeded { phase })
        );
    }

    let cancellation = CancellationToken::new();
    cancellation.cancel();
    assert_eq!(
        RunDeadline::after(Duration::ZERO).check(&cancellation, RunPhase::Kernel),
        Err(RunError::Cancelled)
    );
}

#[test]
fn deadline_wakes_blocked_stream_send_and_receive_with_typed_phases() {
    let cancellation = CancellationToken::new();
    let deadline = RunDeadline::after(Duration::from_millis(20));
    let (sender, _receiver) =
        bounded_stream_channel_with_deadline(1, cancellation.clone(), Some(deadline)).unwrap();
    sender.send(1).unwrap();
    assert_eq!(sender.send(2), Err(StreamSendError::DeadlineExceeded(2)));

    let deadline = RunDeadline::after(Duration::from_millis(20));
    let (_sender, receiver) =
        bounded_stream_channel_with_deadline::<i32>(1, cancellation, Some(deadline)).unwrap();
    assert_eq!(receiver.recv(), Err(StreamReceiveError::DeadlineExceeded));
}

#[test]
fn deadline_late_kernel_completion_is_joined_without_commit() {
    let events = RecordingRunEvents::default();
    let results = ResultStore::new();
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("deadline_late_kernel", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| {
                thread::sleep(Duration::from_millis(40));
                Ok(vec![Value::Integer(7).into()])
            }),
        )
        .unwrap();
    let mut execution_plan = plan(
        vec![operation("deadline_late_kernel", &[], &[0])],
        1,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    execution_plan.results = Box::new([PlanResult {
        name: "late".into(),
        output: stable_output("late"),
        value: ValueRef::new(0),
    }]);
    publish_graph_results(&mut execution_plan);

    let result = RunExecutor::new(
        &kernels,
        &no_resources(),
        &NoFunctions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .with_event_sink(&events)
    .with_result_store(&results)
    .with_deadline(RunDeadline::after(Duration::from_millis(10)))
    .run(&execution_plan, CancellationToken::new());

    assert_eq!(
        result,
        Err(RunError::DeadlineExceeded {
            phase: RunPhase::Kernel,
        })
    );
    let events = events.0.lock().unwrap();
    let run_id = events
        .iter()
        .find(|event| event.kind == RunEventKind::RunStarted)
        .map(|event| event.run.run_id)
        .expect("RunStarted carries the active run ID");
    let stored_results = results.results_for_run(run_id);
    assert_eq!(stored_results.len(), 1);
    assert!(matches!(&stored_results[0].state, ResultState::Failed(_)));
    assert!(
        events
            .iter()
            .all(|event| event.kind != RunEventKind::RunCompleted)
    );
    assert!(events.iter().any(|event| matches!(
        event.kind,
        RunEventKind::RunErrored {
            outcome: RunErrorOutcome::DeadlineExceeded {
                phase: RunPhase::Kernel,
            },
        }
    )));
}

#[test]
fn deadline_before_envelope_receive_does_not_commit_completed_worker_output() {
    struct CompletedOutputBackend(Arc<AtomicUsize>);

    impl RelationalBackend for CompletedOutputBackend {
        fn execute(
            &self,
            _: &RelationalContext<'_>,
            _: &CompiledRelationalPlan,
            _: &[RuntimeValue],
        ) -> Result<RelationalExecution, RelationalError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(RelationalExecution {
                outputs: vec![Value::Integer(7).into()],
            })
        }
    }

    let calls = Arc::new(AtomicUsize::new(0));
    let mut relational = RelationalBackendRegistry::new();
    relational
        .register(
            id("deadline-before-receive", RelationalBackendId::new),
            CompletedOutputBackend(Arc::clone(&calls)),
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
        "deadline-before-receive",
        "synchronized-fragment",
        Box::new([]),
    )]);
    execution_plan.results = Box::new([PlanResult {
        name: "result".into(),
        output: stable_output("deadline_before_receive"),
        value: ValueRef::new(0),
    }]);
    publish_graph_results(&mut execution_plan);
    let events = RecordingRunEvents::default();
    let results = ResultStore::new();
    let deadline = RunDeadline::after(Duration::from_secs(1));
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
                ResultStore::new(),
                Arc::new(SessionMemoization::new()),
            )
            .with_relational_backends(&relational)
            .with_event_sink(&events)
            .with_result_store(&results)
            .with_deadline(deadline)
            .with_test_checkpoint(Arc::new(move |checkpoint, _| {
                if checkpoint == SchedulerCheckpoint::WorkerOutcomeProduced {
                    produced_tx
                        .send(!deadline.exceeded_at(Instant::now()))
                        .unwrap();
                    checkpoint_release.lock().unwrap().recv().unwrap();
                }
            }))
            .run(&execution_plan, CancellationToken::new())
        });
        assert!(
            produced_rx.recv().unwrap(),
            "worker completion must be produced before the deadline"
        );
        while !deadline.exceeded_at(Instant::now()) {
            thread::sleep(Duration::from_millis(1));
        }
        release_tx.send(()).unwrap();
        run.join().unwrap()
    });

    assert_eq!(
        result,
        Err(RunError::DeadlineExceeded {
            phase: RunPhase::Kernel,
        })
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let recorded = events.0.lock().unwrap();
    let run_id = recorded
        .iter()
        .find(|event| event.kind == RunEventKind::RunStarted)
        .map(|event| event.run.run_id)
        .expect("RunStarted carries the active run ID");
    let stored_results = results.results_for_run(run_id);
    assert_eq!(stored_results.len(), 1);
    assert!(matches!(&stored_results[0].state, ResultState::Failed(_)));
    assert_eq!(
        recorded.last().map(|event| &event.kind),
        Some(&RunEventKind::RunErrored {
            outcome: RunErrorOutcome::DeadlineExceeded {
                phase: RunPhase::Kernel,
            },
        })
    );
}

#[test]
fn deadline_queue_wait_is_typed_and_late_workers_do_not_commit() {
    let events = RecordingRunEvents::default();
    let results = ResultStore::new();
    let mut kernels = KernelRegistry::new();
    for name in ["parallel0", "parallel1"] {
        kernels
            .register(
                id(name, KernelHandle::new),
                FnKernel(|_: &[RuntimeValue]| {
                    thread::sleep(Duration::from_millis(30));
                    Ok(vec![Value::Integer(1).into()])
                }),
            )
            .unwrap();
    }
    let deadline = RunDeadline::after(Duration::from_millis(10));
    let result = RunExecutor::new(
        &kernels,
        &no_resources(),
        &NoFunctions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .with_event_sink(&events)
    .with_result_store(&results)
    .with_scheduling_policy(parallel_policy(1, 1, 1))
    .with_deadline(deadline)
    .run(
        &independent_parallel_plan(&[WorkloadClass::Cpu, WorkloadClass::Cpu]),
        CancellationToken::new(),
    );

    assert_eq!(
        result,
        Err(RunError::DeadlineExceeded {
            phase: RunPhase::QueueWait,
        })
    );
    let events = events.0.lock().unwrap();
    let run_id = events
        .iter()
        .find(|event| event.kind == RunEventKind::RunStarted)
        .map(|event| event.run.run_id)
        .expect("RunStarted carries the active run ID");
    let stored_results = results.results_for_run(run_id);
    assert!(!stored_results.is_empty());
    assert!(
        stored_results
            .iter()
            .all(|result| matches!(&result.state, ResultState::Failed(_)))
    );
}

#[test]
fn deadline_adapter_io_uses_the_owner_deadline_without_a_local_timer() {
    let cancellation = CancellationToken::new();
    let owner = RunResourceOwner::new_with_deadline(
        RunId::new(99),
        RunResourceBudgets::default(),
        cancellation.clone(),
        Some(RunDeadline::after(Duration::ZERO)),
    )
    .unwrap();

    assert_eq!(
        execute_planned_adapter(
            &PlannedAdapter::Buffer { capacity: 1 },
            Value::Integer(1).into(),
            &owner,
            &cancellation,
        ),
        Err(RunError::DeadlineExceeded {
            phase: RunPhase::AdapterIo,
        })
    );
    let _ = owner.cleanup();
}

#[test]
fn deadline_publication_preserves_ready_result_but_emits_run_error() {
    let events = RecordingRunEvents::default();
    let results = ResultStore::new();
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("deadline_publication", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| Ok(vec![Value::Integer(1).into()])),
        )
        .unwrap();
    let mut execution_plan = plan(
        vec![operation("deadline_publication", &[], &[0])],
        1,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    execution_plan.results = Box::new([PlanResult {
        name: "publication".into(),
        output: stable_output("publication"),
        value: ValueRef::new(0),
    }]);
    publish_graph_results(&mut execution_plan);
    let result = RunExecutor::new(
        &kernels,
        &no_resources(),
        &NoFunctions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .with_event_sink(&events)
    .with_result_store(&results)
    .with_deadline(RunDeadline::after(Duration::from_millis(20)))
    .with_test_checkpoint(Arc::new(|checkpoint, _| {
        if checkpoint == SchedulerCheckpoint::FinalResultPublication {
            thread::sleep(Duration::from_millis(30));
        }
    }))
    .run(&execution_plan, CancellationToken::new());

    assert_eq!(
        result,
        Err(RunError::DeadlineExceeded {
            phase: RunPhase::ResultPublication,
        })
    );
    let events = events.0.lock().unwrap();
    let run_id = events
        .iter()
        .find(|event| event.kind == RunEventKind::RunStarted)
        .map(|event| event.run.run_id)
        .expect("RunStarted carries the active run ID");
    let stored_results = results.results_for_run(run_id);
    assert_eq!(stored_results.len(), 1);
    assert!(matches!(&stored_results[0].state, ResultState::Ready(_)));
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event.kind, RunEventKind::RunErrored { .. }))
            .count(),
        1
    );
    assert!(
        events
            .iter()
            .all(|event| event.kind != RunEventKind::RunCompleted)
    );
}

struct CleanupDeadlineKernel;

impl Kernel for CleanupDeadlineKernel {
    fn execute(
        &self,
        context: &KernelContext<'_>,
        _: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        context
            .resource_owner
            .register_cleanup_delay_for_test(Duration::from_millis(30));
        Ok(Vec::new())
    }
}

#[test]
fn deadline_cleanup_runs_to_completion_without_replacing_an_earlier_error() {
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("deadline_cleanup", KernelHandle::new),
            CleanupDeadlineKernel,
        )
        .unwrap();
    let execution_plan = plan(
        vec![operation("deadline_cleanup", &[], &[])],
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
        .with_deadline(RunDeadline::after(Duration::from_millis(10)))
        .run(&execution_plan, CancellationToken::new()),
        Err(RunError::DeadlineExceeded {
            phase: RunPhase::Cleanup,
        })
    );
}

struct CooperativeDeadlineKernel;

impl Kernel for CooperativeDeadlineKernel {
    fn execute(
        &self,
        context: &KernelContext<'_>,
        _: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        assert_eq!(context.deadline, context.resource_owner.deadline());
        context.wait_for(Duration::from_secs(1))?;
        Ok(Vec::new())
    }
}

#[test]
fn deadline_is_propagated_into_cooperative_kernel_context() {
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("cooperative_deadline", KernelHandle::new),
            CooperativeDeadlineKernel,
        )
        .unwrap();
    let execution_plan = plan(
        vec![operation("cooperative_deadline", &[], &[])],
        0,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    let started = std::time::Instant::now();

    let error = RunExecutor::new(
        &kernels,
        &no_resources(),
        &NoFunctions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .with_deadline(RunDeadline::after(Duration::from_millis(20)))
    .run(&execution_plan, CancellationToken::new())
    .unwrap_err();

    assert_eq!(
        error,
        RunError::DeadlineExceeded {
            phase: RunPhase::Kernel
        }
    );
    assert!(started.elapsed() < Duration::from_millis(200));
}

struct CooperativeDeadlineBackend;

impl RelationalBackend for CooperativeDeadlineBackend {
    fn execute(
        &self,
        context: &RelationalContext<'_>,
        _: &CompiledRelationalPlan,
        _: &[RuntimeValue],
    ) -> Result<RelationalExecution, RelationalError> {
        assert_eq!(context.deadline, context.resource_owner.deadline());
        context.wait_for(Duration::from_secs(1))?;
        Ok(RelationalExecution {
            outputs: Vec::new(),
        })
    }
}

#[test]
fn deadline_is_propagated_into_cooperative_relational_context() {
    let mut relational = RelationalBackendRegistry::new();
    relational
        .register(
            id("deadline-backend", RelationalBackendId::new),
            CooperativeDeadlineBackend,
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
        "deadline-backend",
        "deadline-fragment",
        Box::new([]),
    )]);
    let started = std::time::Instant::now();

    let error = RunExecutor::new(
        &KernelRegistry::new(),
        &no_resources(),
        &NoFunctions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .with_relational_backends(&relational)
    .with_deadline(RunDeadline::after(Duration::from_millis(20)))
    .run(&execution_plan, CancellationToken::new())
    .unwrap_err();

    assert_eq!(
        error,
        RunError::DeadlineExceeded {
            phase: RunPhase::Kernel
        }
    );
    assert!(started.elapsed() < Duration::from_millis(200));
}

#[test]
fn deadline_stream_fast_paths_never_mutate_after_expiry() {
    let cancellation = CancellationToken::new();
    let (sender, receiver) = bounded_stream_channel_with_deadline(
        2,
        cancellation.clone(),
        Some(RunDeadline::after(Duration::ZERO)),
    )
    .unwrap();
    assert_eq!(sender.send(1), Err(StreamSendError::DeadlineExceeded(1)));
    assert_eq!(
        sender.try_send(2),
        Err(StreamSendError::DeadlineExceeded(2))
    );
    assert_eq!(
        receiver.try_recv(),
        Err(StreamReceiveError::DeadlineExceeded)
    );

    let deadline = RunDeadline::after(Duration::from_millis(20));
    let (sender, receiver) =
        bounded_stream_channel_with_deadline(1, cancellation.clone(), Some(deadline)).unwrap();
    sender.send(3).unwrap();
    thread::sleep(Duration::from_millis(25));
    assert_eq!(receiver.recv(), Err(StreamReceiveError::DeadlineExceeded));
    cancellation.cancel();
    assert_eq!(receiver.try_recv(), Err(StreamReceiveError::Cancelled));
    assert_eq!(sender.try_send(4), Err(StreamSendError::Cancelled(4)));
}

#[test]
fn worker_outcome_timestamp_precedes_envelope_preparation_delay() {
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("timestamp_boundary", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| Err(KernelError::new("boundary ordinary error"))),
        )
        .unwrap();
    let execution_plan = plan(
        vec![operation("timestamp_boundary", &[], &[])],
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
    .with_deadline(RunDeadline::after(Duration::from_millis(20)))
    .with_test_checkpoint(Arc::new(|checkpoint, _| {
        if checkpoint == SchedulerCheckpoint::WorkerOutcomeProduced {
            thread::sleep(Duration::from_millis(40));
        }
    }))
    .run(&execution_plan, CancellationToken::new())
    .unwrap_err();

    assert!(matches!(
        error,
        RunError::KernelFailed { message, .. }
            if message.as_ref() == "boundary ordinary error"
    ));
}

#[test]
fn worker_panic_timestamp_is_captured_at_unwind_boundary() {
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("panic_timestamp_boundary", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| panic!("worker panic timestamp sentinel")),
        )
        .unwrap();
    let execution_plan = plan(
        vec![operation("panic_timestamp_boundary", &[], &[])],
        0,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );

    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = RunExecutor::new(
            &kernels,
            &no_resources(),
            &NoFunctions,
            crate::node_system::runtime::ResultStore::new(),
            std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
        )
        .with_deadline(RunDeadline::after(Duration::from_millis(20)))
        .with_test_checkpoint(Arc::new(|checkpoint, _| {
            if checkpoint == SchedulerCheckpoint::WorkerOutcomeProduced {
                thread::sleep(Duration::from_millis(40));
            }
        }))
        .run(&execution_plan, CancellationToken::new());
    }));

    assert!(panic.is_err());
}

#[test]
fn rust_error_outcomes_are_strict_by_construction() {
    assert_eq!(
        RunErrorOutcome::from(&RunError::DeadlineExceeded {
            phase: RunPhase::Kernel
        }),
        RunErrorOutcome::DeadlineExceeded {
            phase: RunPhase::Kernel
        },
    );
    assert_eq!(
        RunErrorOutcome::from(&RunError::KernelFailed {
            operation: OperationIndex::new(0),
            kind: KernelErrorKind::Permanent,
            message: "failed".into(),
        }),
        RunErrorOutcome::Ordinary {
            code: OrdinaryRunErrorCode::KernelFailed
        },
    );
}
