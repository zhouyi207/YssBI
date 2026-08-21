use super::*;

#[test]
fn effect_dependencies_determine_ready_queue_order() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut kernels = KernelRegistry::new();
    for name in ["after", "before"] {
        let events = events.clone();
        kernels
            .register(
                id(name, KernelHandle::new),
                FnKernel(move |_: &[RuntimeValue]| {
                    events.lock().unwrap().push(name);
                    Ok(vec![Value::Null.into()])
                }),
            )
            .unwrap();
    }
    let mut execution_plan = plan(
        vec![
            operation("after", &[], &[0]),
            operation("before", &[], &[1]),
        ],
        2,
        StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(0)),
            ControlStep::Operation(OperationIndex::new(1)),
        ])),
    );
    execution_plan.effect_dependencies = Box::new([EffectDependency {
        before: OperationIndex::new(1),
        after: OperationIndex::new(0),
    }]);

    RunExecutor::new(
        &kernels,
        &no_resources(),
        &NoFunctions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .run(&execution_plan, CancellationToken::new())
    .unwrap();

    assert_eq!(*events.lock().unwrap(), vec!["before", "after"]);
}

struct ParallelGate {
    open: Mutex<bool>,
    ready: Condvar,
}

impl ParallelGate {
    fn closed() -> Arc<Self> {
        Arc::new(Self {
            open: Mutex::new(false),
            ready: Condvar::new(),
        })
    }

    fn wait(&self) {
        let open = self.open.lock().unwrap();
        drop(self.ready.wait_while(open, |open| !*open).unwrap());
    }

    fn release(&self) {
        *self.open.lock().unwrap() = true;
        self.ready.notify_all();
    }
}

struct GatedKernel {
    name: &'static str,
    started: mpsc::Sender<&'static str>,
    finished: Option<mpsc::Sender<&'static str>>,
    gate: Arc<ParallelGate>,
    active: Arc<AtomicUsize>,
    maximum: Arc<AtomicUsize>,
    output: Value,
}

impl Kernel for GatedKernel {
    fn execute(
        &self,
        _: &KernelContext<'_>,
        _: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.maximum.fetch_max(active, Ordering::SeqCst);
        self.started.send(self.name).unwrap();
        self.gate.wait();
        self.active.fetch_sub(1, Ordering::SeqCst);
        if let Some(finished) = &self.finished {
            finished.send(self.name).unwrap();
        }
        Ok(vec![self.output.clone().into()])
    }
}

fn register_gated_kernels(
    kernels: &mut KernelRegistry,
    gates: &[Arc<ParallelGate>],
    started: &mpsc::Sender<&'static str>,
    finished: Option<&mpsc::Sender<&'static str>>,
    active: &Arc<AtomicUsize>,
    maximum: &Arc<AtomicUsize>,
) {
    const NAMES: [&str; 8] = [
        "parallel0",
        "parallel1",
        "parallel2",
        "parallel3",
        "parallel4",
        "parallel5",
        "parallel6",
        "parallel7",
    ];
    for (index, gate) in gates.iter().enumerate() {
        kernels
            .register(
                id(NAMES[index], KernelHandle::new),
                GatedKernel {
                    name: NAMES[index],
                    started: started.clone(),
                    finished: finished.cloned(),
                    gate: Arc::clone(gate),
                    active: Arc::clone(active),
                    maximum: Arc::clone(maximum),
                    output: Value::Integer(index as i64),
                },
            )
            .unwrap();
    }
}

fn release_all(gates: &[Arc<ParallelGate>]) {
    for gate in gates {
        gate.release();
    }
}

#[test]
fn parallel_scheduler_independent_cpu_operations_overlap() {
    let gates = [ParallelGate::closed(), ParallelGate::closed()];
    let (started_tx, started_rx) = mpsc::channel();
    let active = Arc::new(AtomicUsize::new(0));
    let maximum = Arc::new(AtomicUsize::new(0));
    let mut kernels = KernelRegistry::new();
    register_gated_kernels(&mut kernels, &gates, &started_tx, None, &active, &maximum);
    let execution_plan = independent_parallel_plan(&[WorkloadClass::Cpu, WorkloadClass::Cpu]);

    let run = thread::spawn(move || {
        RunExecutor::new(
            &kernels,
            &no_resources(),
            &NoFunctions,
            crate::node_system::runtime::ResultStore::new(),
            std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
        )
        .with_scheduling_policy(parallel_policy(2, 1, 1))
        .run(&execution_plan, CancellationToken::new())
    });
    let first = started_rx.recv_timeout(Duration::from_secs(2));
    let second = started_rx.recv_timeout(Duration::from_secs(2));
    release_all(&gates);
    let result = run.join().unwrap();

    assert!(first.is_ok(), "first CPU operation did not start");
    assert!(second.is_ok(), "independent CPU operations did not overlap");
    result.unwrap();
    assert_eq!(maximum.load(Ordering::SeqCst), 2);
}

fn assert_parallel_class_limit(class: WorkloadClass, policy: SchedulingPolicy) {
    let gates = [
        ParallelGate::closed(),
        ParallelGate::closed(),
        ParallelGate::closed(),
    ];
    let (started_tx, started_rx) = mpsc::channel();
    let (blocked_tx, blocked_rx) = mpsc::channel();
    let active = Arc::new(AtomicUsize::new(0));
    let maximum = Arc::new(AtomicUsize::new(0));
    let mut kernels = KernelRegistry::new();
    register_gated_kernels(&mut kernels, &gates, &started_tx, None, &active, &maximum);
    let execution_plan = independent_parallel_plan(&[class, class, class]);

    let run = thread::spawn(move || {
        RunExecutor::new(
            &kernels,
            &no_resources(),
            &NoFunctions,
            crate::node_system::runtime::ResultStore::new(),
            std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
        )
        .with_scheduling_policy(policy)
        .with_test_checkpoint(Arc::new(move |checkpoint, _| {
            if checkpoint == SchedulerCheckpoint::AdmissionBlocked(class) {
                let _ = blocked_tx.send(());
            }
        }))
        .run(&execution_plan, CancellationToken::new())
    });
    started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    blocked_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(matches!(
        started_rx.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    release_all(&gates);
    let result = run.join().unwrap();

    result.unwrap();
    assert_eq!(maximum.load(Ordering::SeqCst), 2);
}

#[test]
fn parallel_scheduler_enforces_hard_class_limits_after_blocked_admission() {
    for (class, policy) in [
        (WorkloadClass::Cpu, parallel_policy(2, 1, 1)),
        (WorkloadClass::Io, parallel_policy(1, 2, 1)),
        (WorkloadClass::AdapterIo, parallel_policy(1, 1, 2)),
    ] {
        assert_parallel_class_limit(class, policy);
    }
}

#[test]
fn parallel_scheduler_io_has_a_separate_budget() {
    let gates = [ParallelGate::closed(), ParallelGate::closed()];
    let (started_tx, started_rx) = mpsc::channel();
    let active = Arc::new(AtomicUsize::new(0));
    let maximum = Arc::new(AtomicUsize::new(0));
    let mut kernels = KernelRegistry::new();
    register_gated_kernels(&mut kernels, &gates, &started_tx, None, &active, &maximum);
    let execution_plan = independent_parallel_plan(&[WorkloadClass::Cpu, WorkloadClass::Io]);

    let run = thread::spawn(move || {
        RunExecutor::new(
            &kernels,
            &no_resources(),
            &NoFunctions,
            crate::node_system::runtime::ResultStore::new(),
            std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
        )
        .with_scheduling_policy(parallel_policy(1, 1, 1))
        .run(&execution_plan, CancellationToken::new())
    });
    let first = started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let second = started_rx.recv_timeout(Duration::from_secs(2));
    release_all(&gates);
    run.join().unwrap().unwrap();

    assert_eq!(
        BTreeSet::from([first, second.unwrap()]),
        BTreeSet::from(["parallel0", "parallel1"])
    );
    assert_eq!(maximum.load(Ordering::SeqCst), 2);
}

#[test]
fn parallel_scheduler_exclusive_work_never_overlaps_other_work() {
    let gates = [ParallelGate::closed(), ParallelGate::closed()];
    let (started_tx, started_rx) = mpsc::channel();
    let (blocked_tx, blocked_rx) = mpsc::channel();
    let active = Arc::new(AtomicUsize::new(0));
    let maximum = Arc::new(AtomicUsize::new(0));
    let mut kernels = KernelRegistry::new();
    register_gated_kernels(&mut kernels, &gates, &started_tx, None, &active, &maximum);
    let execution_plan = independent_parallel_plan(&[WorkloadClass::Cpu, WorkloadClass::Exclusive]);

    let run = thread::spawn(move || {
        RunExecutor::new(
            &kernels,
            &no_resources(),
            &NoFunctions,
            crate::node_system::runtime::ResultStore::new(),
            std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
        )
        .with_scheduling_policy(parallel_policy(2, 1, 1))
        .with_test_checkpoint(Arc::new(move |checkpoint, _| {
            if checkpoint == SchedulerCheckpoint::AdmissionBlocked(WorkloadClass::Exclusive) {
                let _ = blocked_tx.send(());
            }
        }))
        .run(&execution_plan, CancellationToken::new())
    });
    assert_eq!(
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap(),
        "parallel0"
    );
    blocked_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(matches!(
        started_rx.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    gates[0].release();
    let exclusive = started_rx.recv_timeout(Duration::from_secs(2));
    gates[1].release();
    run.join().unwrap().unwrap();

    assert_eq!(exclusive.unwrap(), "parallel1");
    assert_eq!(maximum.load(Ordering::SeqCst), 1);
}

#[test]
fn parallel_scheduler_io_is_not_starved_by_sustained_cpu_load() {
    let gates = [
        ParallelGate::closed(),
        ParallelGate::closed(),
        ParallelGate::closed(),
        ParallelGate::closed(),
    ];
    let (started_tx, started_rx) = mpsc::channel();
    let active = Arc::new(AtomicUsize::new(0));
    let maximum = Arc::new(AtomicUsize::new(0));
    let mut kernels = KernelRegistry::new();
    register_gated_kernels(&mut kernels, &gates, &started_tx, None, &active, &maximum);
    let execution_plan = independent_parallel_plan(&[
        WorkloadClass::Cpu,
        WorkloadClass::Cpu,
        WorkloadClass::Cpu,
        WorkloadClass::Io,
    ]);

    let run = thread::spawn(move || {
        RunExecutor::new(
            &kernels,
            &no_resources(),
            &NoFunctions,
            crate::node_system::runtime::ResultStore::new(),
            std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
        )
        .with_scheduling_policy(parallel_policy(1, 1, 1))
        .run(&execution_plan, CancellationToken::new())
    });
    let first = started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let second = started_rx.recv_timeout(Duration::from_secs(2));
    release_all(&gates);
    run.join().unwrap().unwrap();

    assert_eq!(
        BTreeSet::from([first, second.unwrap()]),
        BTreeSet::from(["parallel0", "parallel3"])
    );
}

struct ThreadIdentityKernel(Arc<Mutex<HashSet<thread::ThreadId>>>);

impl Kernel for ThreadIdentityKernel {
    fn execute(
        &self,
        _: &KernelContext<'_>,
        _: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        self.0.lock().unwrap().insert(thread::current().id());
        Ok(vec![Value::Null.into()])
    }
}

#[test]
fn parallel_scheduler_reuses_a_policy_bounded_worker_pool() {
    let worker_threads = Arc::new(Mutex::new(HashSet::new()));
    let mut kernels = KernelRegistry::new();
    for index in 0..8 {
        kernels
            .register(
                id(&format!("parallel{index}"), KernelHandle::new),
                ThreadIdentityKernel(Arc::clone(&worker_threads)),
            )
            .unwrap();
    }
    let execution_plan = independent_parallel_plan(&[WorkloadClass::Cpu; 8]);

    RunExecutor::new(
        &kernels,
        &no_resources(),
        &NoFunctions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .with_scheduling_policy(parallel_policy(2, 1, 1))
    .run(&execution_plan, CancellationToken::new())
    .unwrap();

    assert!(worker_threads.lock().unwrap().len() <= 4);
}

#[test]
fn parallel_scheduler_completion_order_does_not_change_value_mapping() {
    let gates = [ParallelGate::closed(), ParallelGate::closed()];
    let (started_tx, started_rx) = mpsc::channel();
    let (finished_tx, finished_rx) = mpsc::channel();
    let active = Arc::new(AtomicUsize::new(0));
    let maximum = Arc::new(AtomicUsize::new(0));
    let mut kernels = KernelRegistry::new();
    register_gated_kernels(
        &mut kernels,
        &gates,
        &started_tx,
        Some(&finished_tx),
        &active,
        &maximum,
    );
    let mut execution_plan = independent_parallel_plan(&[WorkloadClass::Cpu, WorkloadClass::Cpu]);
    execution_plan.results = Box::new([
        PlanResult {
            name: "first".into(),
            value: ValueRef::new(0),
            output: stable_output("first"),
        },
        PlanResult {
            name: "second".into(),
            value: ValueRef::new(1),
            output: stable_output("second"),
        },
    ]);
    publish_graph_results(&mut execution_plan);

    let run = thread::spawn(move || {
        RunExecutor::new(
            &kernels,
            &no_resources(),
            &NoFunctions,
            crate::node_system::runtime::ResultStore::new(),
            std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
        )
        .with_scheduling_policy(parallel_policy(2, 1, 1))
        .run(&execution_plan, CancellationToken::new())
    });
    started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    gates[1].release();
    assert_eq!(
        finished_rx.recv_timeout(Duration::from_secs(2)).unwrap(),
        "parallel1"
    );
    gates[0].release();
    let result = run.join().unwrap().unwrap();

    assert_eq!(
        result.value_for_test("first").unwrap(),
        Value::Integer(0).into()
    );
    assert_eq!(
        result.value_for_test("second").unwrap(),
        Value::Integer(1).into()
    );
}

struct CancellationDrainKernel {
    started: mpsc::Sender<()>,
    exited: mpsc::Sender<()>,
}

impl Kernel for CancellationDrainKernel {
    fn execute(
        &self,
        context: &KernelContext<'_>,
        _: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        let waiter = Arc::new(Condvar::new());
        context.cancellation.register_waiter(&waiter);
        self.started.send(()).unwrap();
        let lock = Mutex::new(());
        let guard = lock.lock().unwrap();
        drop(
            waiter
                .wait_while(guard, |_| !context.cancellation.is_cancelled())
                .unwrap(),
        );
        self.exited.send(()).unwrap();
        Err(KernelError::cancelled("cancelled for drain"))
    }
}

enum MultiWorkerTerminalKind {
    Error,
    Panic,
    WaitForCancellation,
}

struct MultiWorkerTerminalKernel {
    kind: MultiWorkerTerminalKind,
    entered: Arc<Barrier>,
    exited: mpsc::Sender<()>,
}

impl Kernel for MultiWorkerTerminalKernel {
    fn execute(
        &self,
        context: &KernelContext<'_>,
        _: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        let waiter = Arc::new(Condvar::new());
        context.cancellation.register_waiter(&waiter);
        self.entered.wait();
        match self.kind {
            MultiWorkerTerminalKind::Error => Err(KernelError::new("multi-worker failure")),
            MultiWorkerTerminalKind::Panic => panic!("multi-worker panic"),
            MultiWorkerTerminalKind::WaitForCancellation => {
                let lock = Mutex::new(());
                let guard = lock.lock().unwrap();
                drop(
                    waiter
                        .wait_while(guard, |_| !context.cancellation.is_cancelled())
                        .unwrap(),
                );
                self.exited.send(()).unwrap();
                Err(KernelError::cancelled("peer drained"))
            }
        }
    }
}

fn multi_worker_terminal_fixture(
    terminal: MultiWorkerTerminalKind,
) -> (
    KernelRegistry,
    ExecutionPlan,
    Arc<Barrier>,
    mpsc::Receiver<()>,
) {
    let entered = Arc::new(Barrier::new(3));
    let (exited_tx, exited_rx) = mpsc::channel();
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("parallel0", KernelHandle::new),
            MultiWorkerTerminalKernel {
                kind: terminal,
                entered: Arc::clone(&entered),
                exited: exited_tx.clone(),
            },
        )
        .unwrap();
    kernels
        .register(
            id("parallel1", KernelHandle::new),
            MultiWorkerTerminalKernel {
                kind: MultiWorkerTerminalKind::WaitForCancellation,
                entered: Arc::clone(&entered),
                exited: exited_tx,
            },
        )
        .unwrap();
    (
        kernels,
        independent_parallel_plan(&[WorkloadClass::Cpu, WorkloadClass::Cpu]),
        entered,
        exited_rx,
    )
}

#[test]
fn parallel_scheduler_ordinary_error_drains_and_joins_peer_worker() {
    let (kernels, execution_plan, entered, exited) =
        multi_worker_terminal_fixture(MultiWorkerTerminalKind::Error);
    let run = thread::spawn(move || {
        RunExecutor::new(
            &kernels,
            &no_resources(),
            &NoFunctions,
            crate::node_system::runtime::ResultStore::new(),
            std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
        )
        .with_scheduling_policy(parallel_policy(2, 1, 1))
        .run(&execution_plan, CancellationToken::new())
    });
    entered.wait();

    exited.recv_timeout(Duration::from_secs(2)).unwrap();
    let error = run.join().unwrap().unwrap_err();

    assert!(
        matches!(error, RunError::KernelFailed { message, .. } if message.as_ref() == "multi-worker failure")
    );
}

#[test]
fn parallel_scheduler_panic_drains_and_joins_peer_worker_before_unwind() {
    let (kernels, execution_plan, entered, exited) =
        multi_worker_terminal_fixture(MultiWorkerTerminalKind::Panic);
    let run = thread::spawn(move || {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = RunExecutor::new(
                &kernels,
                &no_resources(),
                &NoFunctions,
                crate::node_system::runtime::ResultStore::new(),
                std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
            )
            .with_scheduling_policy(parallel_policy(2, 1, 1))
            .run(&execution_plan, CancellationToken::new());
        }))
    });
    entered.wait();

    exited.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(run.join().unwrap().is_err());
}

#[test]
fn parallel_scheduler_cancellation_drains_all_workers() {
    let (started_tx, started_rx) = mpsc::channel();
    let (exited_tx, exited_rx) = mpsc::channel();
    let mut kernels = KernelRegistry::new();
    for index in 0..2 {
        kernels
            .register(
                id(&format!("parallel{index}"), KernelHandle::new),
                CancellationDrainKernel {
                    started: started_tx.clone(),
                    exited: exited_tx.clone(),
                },
            )
            .unwrap();
    }
    let execution_plan = independent_parallel_plan(&[WorkloadClass::Cpu, WorkloadClass::Cpu]);
    let cancellation = CancellationToken::new();
    let run_cancellation = cancellation.clone();
    let run = thread::spawn(move || {
        RunExecutor::new(
            &kernels,
            &no_resources(),
            &NoFunctions,
            crate::node_system::runtime::ResultStore::new(),
            std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
        )
        .with_scheduling_policy(parallel_policy(2, 1, 1))
        .run(&execution_plan, run_cancellation)
    });
    started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    started_rx.recv_timeout(Duration::from_secs(2)).unwrap();

    cancellation.cancel();
    exited_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    exited_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let result = run.join().unwrap();

    assert_eq!(result.unwrap_err(), RunError::Cancelled);
}

#[test]
fn worker_panic_outranks_deadline_and_cancellation() {
    #[derive(Clone, Copy)]
    enum CompetingTerminal {
        Deadline,
        Cancellation,
    }

    const PANIC_PAYLOAD: &str = "panic terminal truth sentinel";

    for terminal in [CompetingTerminal::Deadline, CompetingTerminal::Cancellation] {
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
                        ResultStore::new(),
                        Arc::new(SessionMemoization::new()),
                    );
                    if matches!(terminal, CompetingTerminal::Deadline) {
                        executor =
                            executor.with_deadline(RunDeadline::after(Duration::from_millis(20)));
                    }
                    let _ = executor.run(&execution_plan, run_cancellation);
                }))
            });
            started_rx.recv().unwrap();
            match terminal {
                CompetingTerminal::Deadline => thread::sleep(Duration::from_millis(40)),
                CompetingTerminal::Cancellation => cancellation.cancel(),
            }
            release_tx.send(()).unwrap();
            run.join().unwrap()
        });
        let payload = panic.expect_err("the original worker panic must propagate");
        assert_eq!(payload.downcast_ref::<&str>().copied(), Some(PANIC_PAYLOAD));
    }
}

#[test]
fn duplicate_operation_in_one_activation_is_rejected() {
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("once", KernelHandle::new),
            FnKernel(move |_: &[RuntimeValue]| {
                observed.fetch_add(1, Ordering::SeqCst);
                Ok(vec![Value::Null.into()])
            }),
        )
        .unwrap();
    let execution_plan = plan(
        vec![operation("once", &[], &[0])],
        1,
        StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(0)),
            ControlStep::Operation(OperationIndex::new(0)),
        ])),
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

    assert!(matches!(error, RunError::OperationAlreadyExecuted { .. }));
    assert!(calls.load(Ordering::SeqCst) <= 1);
}

#[test]
fn activation_allocator_exhaustion_is_typed_without_global_contamination() {
    let allocator = ActivationIdAllocator::for_test(NonZeroU64::new(u64::MAX).unwrap());
    let execution_plan = plan(vec![], 0, StructuredControlRegion::Sequence(Box::new([])));
    let kernels = KernelRegistry::new();
    let resources = no_resources();
    let executor = RunExecutor::new(
        &kernels,
        &resources,
        &NoFunctions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .with_activation_allocator_for_test(&allocator);

    executor
        .run(&execution_plan, CancellationToken::new())
        .unwrap();
    assert_eq!(
        executor.run(&execution_plan, CancellationToken::new()),
        Err(RunError::ActivationIdExhausted)
    );
}

#[test]
fn frame_allocator_exhaustion_is_a_typed_runtime_failure() {
    let allocator = AtomicU64::new(u64::MAX);
    let execution_plan = plan(vec![], 0, StructuredControlRegion::Sequence(Box::new([])));
    let kernels = KernelRegistry::new();
    let resources = no_resources();
    let executor = RunExecutor::new(
        &kernels,
        &resources,
        &NoFunctions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .with_frame_allocator_for_test(&allocator);

    assert_eq!(
        executor.run(&execution_plan, CancellationToken::new()),
        Err(RunError::RuntimeIdExhausted)
    );
}
