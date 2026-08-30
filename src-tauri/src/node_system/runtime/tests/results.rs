use super::*;

#[test]
fn view_data_opens_exact_input_result_without_materialization() {
    struct ViewDataEvents {
        recorded: Mutex<Vec<RunEvent>>,
        open_result_window_seen: Arc<std::sync::atomic::AtomicBool>,
    }

    impl RunEventSink for ViewDataEvents {
        fn record(&self, event: RunEvent) {
            if matches!(&event.kind, RunEventKind::OpenResultWindow { .. }) {
                self.open_result_window_seen.store(true, Ordering::SeqCst);
            }
            self.recorded.lock().unwrap().push(event);
        }
    }

    let mut kernels = build_builtin_kernel_registry();
    kernels
        .register(
            id("view_data_source", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| Ok(vec![Value::Integer(42).into()])),
        )
        .unwrap();
    let then_executions = Arc::new(AtomicUsize::new(0));
    let observed_then_executions = Arc::clone(&then_executions);
    let open_result_window_seen = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let observed_open_result_window = Arc::clone(&open_result_window_seen);
    kernels
        .register(
            id("view_data_then", KernelHandle::new),
            FnKernel(move |_: &[RuntimeValue]| {
                assert!(
                    observed_open_result_window.load(Ordering::SeqCst),
                    "OpenResultWindow must be emitted before the downstream then kernel",
                );
                observed_then_executions.fetch_add(1, Ordering::SeqCst);
                Ok(Vec::new())
            }),
        )
        .unwrap();

    let source_output = stable_output("source");
    let mut source = operation("view_data_source", &[], &[0]);
    source.outputs[0].public_output = Some(source_output.clone());
    let mut view = operation("yssbi.debug.view", &[0], &[]);
    view.source_node_id = view_node_id();
    view.source_node_type_id = NodeTypeId::new("yssbi.debug.view").unwrap();
    let then = operation("view_data_then", &[], &[]);
    let mut plan = plan(
        vec![source, view, then],
        1,
        StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(0)),
            ControlStep::Operation(OperationIndex::new(1)),
            ControlStep::Operation(OperationIndex::new(2)),
        ])),
    );
    plan.effect_dependencies = Box::new([EffectDependency {
        before: OperationIndex::new(1),
        after: OperationIndex::new(2),
    }]);
    let results = ResultStore::new();
    let events = ViewDataEvents {
        recorded: Mutex::new(Vec::new()),
        open_result_window_seen,
    };
    use tracing_subscriber::layer::SubscriberExt;

    let diagnostics = yss_diagnostics::DiagnosticsRuntime::initialize().unwrap();
    let subscriber = tracing_subscriber::registry()
        .with(yss_tracing::LogLayer::new(diagnostics.rust_log_sink()));
    let run = tracing::subscriber::with_default(subscriber, || {
        RunExecutor::new(
            &kernels,
            &no_resources(),
            &NoFunctions,
            ResultStore::new(),
            Arc::new(SessionMemoization::new()),
        )
        .with_result_store(&results)
        .with_event_sink(&events)
        .run(&plan, CancellationToken::new())
    })
    .unwrap();

    let source_history = results.pin_history(&source_output);
    assert_eq!(source_history.len(), 1);
    let input_result_id = source_history[0].result_id;
    let recorded = events.recorded.lock().unwrap();
    let open_events = recorded
        .iter()
        .enumerate()
        .filter_map(|(index, event)| match event.kind {
            RunEventKind::OpenResultWindow { result_id } => Some((index, result_id)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(open_events.len(), 1);
    let (open_index, open_result_id) = open_events[0];
    assert_eq!(open_result_id, input_result_id);
    let completion_index = recorded
        .iter()
        .rposition(|event| event.kind == RunEventKind::RunCompleted)
        .expect("run completion must be published");
    assert!(open_index < completion_index);
    drop(recorded);

    let subscription = diagnostics.subscribe_batches(|_| true).unwrap();
    let notify_logs = subscription
        .entries
        .iter()
        .filter(|record| record.event.as_deref() == Some("openResultWindow"))
        .collect::<Vec<_>>();
    assert_eq!(notify_logs.len(), 1);
    let notify = notify_logs[0];
    assert_eq!(notify.level, yss_diagnostics::DiagnosticLevel::Info);
    assert_eq!(notify.domain, yss_diagnostics::DiagnosticDomain::Ui);
    assert_eq!(notify.source.as_deref(), Some("yssbi.debug.view"));
    assert_eq!(notify.fields["result_id"], input_result_id.get());
    assert_eq!(notify.fields["run_id"], run.run_id.get());

    assert_eq!(notify.fields["node_id"], view_node_id().to_string());
    diagnostics
        .unsubscribe(subscription.subscription_id)
        .unwrap();

    assert_eq!(then_executions.load(Ordering::SeqCst), 1);
    assert_eq!(results.authoritative_result_count_for_test(), 1);
    assert_eq!(results.group_count_for_test(), 1);
    assert_eq!(
        results.pin_history(&source_output).as_ref(),
        source_history.as_ref()
    );
}

fn view_node_id() -> NodeId {
    NodeId::from_uuid(uuid::Uuid::from_u128(9))
}

#[test]
fn nested_view_data_event_keeps_root_graph_run_identity() {
    let mut kernels = build_builtin_kernel_registry();
    kernels
        .register(
            id("nested_view_data_source", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| Ok(vec![Value::Integer(42).into()])),
        )
        .unwrap();

    let source = operation("nested_view_data_source", &[], &[0]);
    let mut view = operation("yssbi.debug.view", &[0], &[]);
    view.source_node_id = view_node_id();
    view.source_node_type_id = NodeTypeId::new("yssbi.debug.view").unwrap();
    let callee = plan(
        vec![source, view],
        1,
        StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(0)),
            ControlStep::Operation(OperationIndex::new(1)),
        ])),
    );
    let mut caller = plan(
        vec![],
        0,
        StructuredControlRegion::Call {
            target: id("functions/callee.yssbi-function", FunctionPlanHandle::new),
            arguments: Box::new([]),
            results: Box::new([]),
            mandatory: true,
        },
    );
    caller.provenance.graph_path = GraphResourcePath::new("events/caller.yssbi-event").unwrap();
    let callee_graph_path = GraphResourcePath::new("functions/callee.yssbi-function").unwrap();
    let events = RecordingRunEvents::default();
    let results = ResultStore::new();

    RunExecutor::new(
        &kernels,
        &no_resources(),
        &OneFunction(published_function(
            callee,
            "functions/callee.yssbi-function",
            &[],
            &[],
        )),
        ResultStore::new(),
        Arc::new(SessionMemoization::new()),
    )
    .with_event_sink(&events)
    .with_result_store(&results)
    .run(&caller, CancellationToken::new())
    .unwrap();

    let recorded = events.0.lock().unwrap();
    let open_events = recorded
        .iter()
        .filter(|event| matches!(event.kind, RunEventKind::OpenResultWindow { .. }))
        .collect::<Vec<_>>();
    assert_eq!(open_events.len(), 1);
    let open_event = open_events[0];
    assert_eq!(
        &open_event.run.graph_path, &caller.provenance.graph_path,
        "the public event belongs to the caller/root run",
    );
    let RunEventKind::OpenResultWindow { result_id } = &open_event.kind else {
        unreachable!("filtered to OpenResultWindow")
    };
    let stored = results.result(*result_id).expect("View Data input result");
    assert_eq!(
        &stored.provenance.graph_path, &callee_graph_path,
        "stored-result provenance belongs to the callee",
    );
}

#[test]
fn kernel_receives_all_inputs_once_and_outputs_publish_atomically() {
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::new(Mutex::new(Vec::new()));
    let observed_calls = Arc::clone(&calls);
    let observed_inputs = Arc::clone(&observed);
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("atomic_multi", KernelHandle::new),
            FnKernel(move |inputs: &[RuntimeValue]| {
                observed_calls.fetch_add(1, Ordering::SeqCst);
                *observed_inputs.lock().unwrap() = inputs.to_vec();
                Ok(vec![
                    RuntimeValue::Scalar(Value::Integer(5)),
                    RuntimeValue::Scalar(Value::Integer(6)),
                ])
            }),
        )
        .unwrap();

    let mut atomic = operation("atomic_multi", &[0, 1], &[2, 3]);
    atomic.inputs[0].bound_value = Some(Value::Integer(2));
    atomic.inputs[1].bound_value = Some(Value::Integer(3));
    let first_output = stable_output("first");
    let second_output = stable_output("second");
    atomic.outputs[0].public_output = Some(first_output.clone());
    atomic.outputs[1].public_output = Some(second_output.clone());
    let execution_plan = plan(
        vec![atomic],
        4,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    let store = ResultStore::new();
    let checkpoint_store = store.clone();
    let checkpoint_first = first_output.clone();
    let checkpoint_second = second_output.clone();
    let pending_states = Arc::new(Mutex::new(Vec::new()));
    let observed_pending = Arc::clone(&pending_states);

    RunExecutor::new(
        &kernels,
        &no_resources(),
        &NoFunctions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .with_result_store(&store)
    .with_test_checkpoint(Arc::new(move |checkpoint, _| {
        if checkpoint != SchedulerCheckpoint::BeforeGroupCommit {
            return;
        }
        let first = checkpoint_store.pin_history(&checkpoint_first)[0].result_id;
        let second = checkpoint_store.pin_history(&checkpoint_second)[0].result_id;
        *observed_pending.lock().unwrap() = vec![
            checkpoint_store.result(first).unwrap().state.is_pending(),
            checkpoint_store.result(second).unwrap().state.is_pending(),
        ];
    }))
    .run(&execution_plan, CancellationToken::new())
    .unwrap();

    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        observed.lock().unwrap().as_slice(),
        [
            RuntimeValue::Scalar(Value::Integer(2)),
            RuntimeValue::Scalar(Value::Integer(3)),
        ]
    );
    assert_eq!(pending_states.lock().unwrap().as_slice(), [true, true]);
    let first_id = store.pin_history(&first_output)[0].result_id;
    let second_id = store.pin_history(&second_output)[0].result_id;
    assert!(matches!(
        &store.result(first_id).unwrap().state,
        ResultState::Ready(value) if value.page(0, 1).unwrap().as_ref() == [Value::Integer(5)]
    ));
    assert!(matches!(
        &store.result(second_id).unwrap().state,
        ResultState::Ready(value) if value.page(0, 1).unwrap().as_ref() == [Value::Integer(6)]
    ));
}

#[test]
fn stream_materialization_finishes_before_group_commit() {
    struct StreamingKernel {
        produced: Arc<AtomicUsize>,
    }

    impl Kernel for StreamingKernel {
        fn execute(
            &self,
            context: &KernelContext<'_>,
            _: &[RuntimeValue],
        ) -> Result<Vec<RuntimeValue>, KernelError> {
            let produced = Arc::clone(&self.produced);
            let stream = context
                .resource_owner
                .stream_from_values((0..3).map(move |value| {
                    produced.fetch_add(1, Ordering::SeqCst);
                    Value::Integer(value)
                }))
                .map_err(|error| KernelError::new(error.to_string()))?;
            Ok(vec![RuntimeValue::Stream(stream)])
        }
    }

    let produced = Arc::new(AtomicUsize::new(0));
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("stream_before_commit", KernelHandle::new),
            StreamingKernel {
                produced: Arc::clone(&produced),
            },
        )
        .unwrap();
    let output = stable_output("streamed");
    let mut source = operation("stream_before_commit", &[], &[0]);
    source.outputs[0].production = OutputProduction::Streaming;
    source.outputs[0].public_output = Some(output.clone());
    let execution_plan = plan(
        vec![source],
        1,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    let store = ResultStore::new();
    let checkpoint_store = store.clone();
    let checkpoint_output = output.clone();
    let checkpoint_produced = Arc::clone(&produced);

    RunExecutor::new(
        &kernels,
        &no_resources(),
        &NoFunctions,
        store.clone(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .with_result_store(&store)
    .with_resource_budgets(materialization_test_budgets(1, 1024))
    .with_test_checkpoint(Arc::new(move |checkpoint, _| {
        if checkpoint != SchedulerCheckpoint::BeforeGroupCommit {
            return;
        }
        assert_eq!(checkpoint_produced.load(Ordering::SeqCst), 3);
        let result_id = checkpoint_store.pin_history(&checkpoint_output)[0].result_id;
        assert!(
            checkpoint_store
                .result(result_id)
                .unwrap()
                .state
                .is_pending()
        );
    }))
    .run(&execution_plan, CancellationToken::new())
    .unwrap();

    let result_id = store.pin_history(&output)[0].result_id;
    let result = store.result(result_id).unwrap();
    let ResultState::Ready(value) = &result.state else {
        panic!("stream result must be ready after group commit");
    };
    assert_eq!(
        value.page(0, 10).unwrap().as_ref(),
        &[Value::Integer(0), Value::Integer(1), Value::Integer(2)]
    );
}

#[test]
fn internal_bound_value_uses_plan_contract_and_coherent_provenance() {
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("bound_contract", KernelHandle::new),
            FnKernel(|inputs: &[RuntimeValue]| Ok(vec![inputs[0].clone()])),
        )
        .unwrap();
    let contract = PlannedValueContract {
        kind: PlannedValueKind::Scalar,
        type_expr: TypeExpr::Concrete(TypeId::new("core.integer").unwrap()),
    };
    let mut operation = operation("bound_contract", &[0], &[1]);
    operation.inputs[0].contract = contract.clone();
    operation.inputs[0].bound_value = Some(Value::Integer(4));
    operation.outputs[0].contract = contract.clone();
    let node_id = operation.source_node_id;
    let mut execution_plan = plan(
        vec![operation],
        2,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    execution_plan
        .value_contracts
        .insert(ValueRef::new(0), contract.clone());
    execution_plan
        .value_contracts
        .insert(ValueRef::new(1), contract.clone());
    let results = ResultStore::new();
    let run_result = RunExecutor::new(
        &kernels,
        &no_resources(),
        &NoFunctions,
        results.clone(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .run(&execution_plan, CancellationToken::new())
    .unwrap();
    let internal = results
        .results_for_run(run_result.run_id)
        .iter()
        .find(|result| result.value == ValueRef::new(0))
        .cloned()
        .expect("bound input result");

    assert_eq!(internal.contract, contract);
    assert_eq!(internal.provenance.node_id, node_id);
    assert_eq!(
        internal.provenance.graph_path,
        execution_plan.provenance.graph_path
    );
    assert_eq!(
        internal.provenance.graph_revision,
        execution_plan.provenance.basis.graph_revision
    );
    assert!(internal.provenance.activation_id.get() > 0);
}

#[test]
fn failed_exact_input_skips_kernel_and_fails_downstream_group_with_source_id() {
    let downstream_calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&downstream_calls);
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("upstream_failure", KernelHandle::new),
            ErrorKernel {
                cancel_token: false,
                cancelled_error: false,
            },
        )
        .unwrap();
    kernels
        .register(
            id("downstream_skipped", KernelHandle::new),
            FnKernel(move |_: &[RuntimeValue]| {
                observed.fetch_add(1, Ordering::SeqCst);
                Ok(vec![Value::Integer(1).into()])
            }),
        )
        .unwrap();
    let upstream_output = stable_output("failed_source");
    let downstream_output = stable_output("failed_downstream");
    let mut upstream = operation("upstream_failure", &[], &[0]);
    upstream.outputs[0].public_output = Some(upstream_output.clone());
    let mut downstream = operation("downstream_skipped", &[0], &[2]);
    downstream.outputs[0].public_output = Some(downstream_output.clone());
    let execution_plan = plan(
        vec![upstream, downstream],
        3,
        StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(0)),
            ControlStep::Operation(OperationIndex::new(1)),
        ])),
    );
    let results = ResultStore::new();

    let run = RunExecutor::new(
        &kernels,
        &no_resources(),
        &NoFunctions,
        results.clone(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .run(&execution_plan, CancellationToken::new());
    assert!(matches!(run, Err(RunError::KernelFailed { .. })), "{run:?}");
    assert_eq!(downstream_calls.load(Ordering::SeqCst), 0);
    let source_id = results.pin_history(&upstream_output)[0].result_id;
    let downstream_id = results.pin_history(&downstream_output)[0].result_id;
    let downstream = results.result(downstream_id).unwrap();
    assert!(matches!(
        &downstream.state,
        ResultState::Failed(failure)
            if failure.cause == ResultFailureCause::Upstream {
                upstream_result_id: source_id,
            }
    ));
}

#[test]
fn cancelled_exact_input_skips_kernel_and_cancels_downstream_group() {
    let downstream_calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&downstream_calls);
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("upstream_cancelled", KernelHandle::new),
            ErrorKernel {
                cancel_token: false,
                cancelled_error: true,
            },
        )
        .unwrap();
    kernels
        .register(
            id("cancelled_downstream_skipped", KernelHandle::new),
            FnKernel(move |_: &[RuntimeValue]| {
                observed.fetch_add(1, Ordering::SeqCst);
                Ok(vec![Value::Integer(1).into()])
            }),
        )
        .unwrap();
    let downstream_output = stable_output("cancelled_downstream");
    let upstream = operation("upstream_cancelled", &[], &[0]);
    let mut downstream = operation("cancelled_downstream_skipped", &[0], &[2]);
    downstream.outputs[0].public_output = Some(downstream_output.clone());
    let execution_plan = plan(
        vec![upstream, downstream],
        3,
        StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(0)),
            ControlStep::Operation(OperationIndex::new(1)),
        ])),
    );
    let results = ResultStore::new();

    assert_eq!(
        RunExecutor::new(
            &kernels,
            &no_resources(),
            &NoFunctions,
            results.clone(),
            std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new())
        )
        .run(&execution_plan, CancellationToken::new()),
        Err(RunError::Cancelled)
    );
    assert_eq!(downstream_calls.load(Ordering::SeqCst), 0);
    let downstream_id = results.pin_history(&downstream_output)[0].result_id;
    assert!(matches!(
        results.result(downstream_id).unwrap().state,
        ResultState::Cancelled
    ));
}

#[test]
fn output_count_mismatch_fails_the_entire_result_group() {
    let calls = Arc::new(AtomicUsize::new(0));
    let observed_calls = Arc::clone(&calls);
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("short_output", KernelHandle::new),
            FnKernel(move |_: &[RuntimeValue]| {
                observed_calls.fetch_add(1, Ordering::SeqCst);
                Ok(vec![RuntimeValue::Scalar(Value::Integer(1))])
            }),
        )
        .unwrap();
    let mut operation = operation("short_output", &[], &[0, 1]);
    let first_output = stable_output("short_first");
    let second_output = stable_output("short_second");
    operation.outputs[0].public_output = Some(first_output.clone());
    operation.outputs[1].public_output = Some(second_output.clone());
    let execution_plan = plan(
        vec![operation],
        2,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    let store = ResultStore::new();

    assert!(matches!(
        RunExecutor::new(
            &kernels,
            &no_resources(),
            &NoFunctions,
            crate::node_system::runtime::ResultStore::new(),
            std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new())
        )
        .with_result_store(&store)
        .run(&execution_plan, CancellationToken::new()),
        Err(RunError::OutputCount {
            expected: 2,
            actual: 1,
            ..
        })
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let first_id = store.pin_history(&first_output)[0].result_id;
    let second_id = store.pin_history(&second_output)[0].result_id;
    assert!(matches!(
        store.result(first_id).unwrap().state,
        ResultState::Failed(_)
    ));
    assert!(matches!(
        store.result(second_id).unwrap().state,
        ResultState::Failed(_)
    ));
}

fn disk_backed_result_plan(terminal_kernel: &str) -> ExecutionPlan {
    let mut source = operation("disk_result_source", &[], &[0]);
    source.outputs[0].production = OutputProduction::Streaming;
    let collect = adapter_operation(
        "disk.result.collect",
        1,
        2,
        OutputProduction::Streaming,
        InputConsumption::FullyMaterialized,
    );
    let terminal = operation(terminal_kernel, &[3], &[4]);
    let mut execution_plan = plan(
        vec![source, collect, terminal],
        5,
        StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(0)),
            ControlStep::Operation(OperationIndex::new(1)),
            ControlStep::Operation(OperationIndex::new(2)),
        ])),
    );
    execution_plan.value_dependencies = Box::new([
        ValueDependency {
            source: ValueRef::new(0),
            destination: ValueRef::new(1),
        },
        ValueDependency {
            source: ValueRef::new(2),
            destination: ValueRef::new(3),
        },
    ]);
    execution_plan.results = Box::new([PlanResult {
        name: "result".into(),
        output: stable_output("result"),
        value: ValueRef::new(4),
    }]);
    publish_graph_results(&mut execution_plan);
    execution_plan
}

#[test]
fn stream_output_is_stored_and_supports_two_independent_reads() {
    let root = materialization_test_root("stored-stream-output");
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("stored_stream", KernelHandle::new),
            OwnedStreamKernel {
                values: vec![Value::Integer(1), Value::Integer(2)].into_boxed_slice(),
                executions: None,
            },
        )
        .unwrap();
    let mut execution_plan = plan(
        vec![operation("stored_stream", &[], &[0])],
        1,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    execution_plan.results = Box::new([PlanResult {
        name: "result".into(),
        output: stable_output("stored_stream"),
        value: ValueRef::new(0),
    }]);
    publish_graph_results(&mut execution_plan);
    let results = ResultStore::new();
    let run_result = RunExecutor::new(
        &kernels,
        &no_resources(),
        &NoFunctions,
        results.clone(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .with_test_spill_root(root.clone())
    .run(&execution_plan, CancellationToken::new())
    .unwrap();
    let stored = results.result(run_result.result_ids["result"]).unwrap();
    let ResultState::Ready(value) = &stored.state else {
        panic!("stream output must be ready");
    };

    let first = value
        .open_reader()
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let second = value
        .open_reader()
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(first, [Value::Integer(1), Value::Integer(2)]);
    assert_eq!(second, first);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn run_result_ids_resolve_after_executor_drop() {
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("persistent_result", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| Ok(vec![Value::Integer(9).into()])),
        )
        .unwrap();
    let mut execution_plan = plan(
        vec![operation("persistent_result", &[], &[0])],
        1,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    execution_plan.results = Box::new([PlanResult {
        name: "result".into(),
        output: stable_output("persistent_result"),
        value: ValueRef::new(0),
    }]);
    publish_graph_results(&mut execution_plan);
    let results = ResultStore::new();
    let resources = no_resources();
    let run_result = {
        let executor = RunExecutor::new(
            &kernels,
            &resources,
            &NoFunctions,
            results.clone(),
            std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
        );
        executor
            .run(&execution_plan, CancellationToken::new())
            .unwrap()
    };

    assert!(matches!(
        results
            .result(run_result.result_ids["result"])
            .unwrap()
            .state,
        ResultState::Ready(_)
    ));
}

#[test]
fn bounded_materialization_run_result_and_result_store_keep_spill_durable() {
    let root = materialization_test_root("durable-result");
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("disk_result_source", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| Ok(vec![Value::String("durable".into()).into()])),
        )
        .unwrap();
    kernels
        .register(
            id("disk_result_passthrough", KernelHandle::new),
            FnKernel(|inputs: &[RuntimeValue]| Ok(vec![inputs[0].clone()])),
        )
        .unwrap();
    let results = ResultStore::new();
    let resources = no_resources();
    let functions = NoFunctions;

    let run_result = RunExecutor::new(
        &kernels,
        &resources,
        &functions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .with_result_store(&results)
    .with_resource_budgets(materialization_test_budgets(1, 1))
    .with_test_spill_root(root.clone())
    .run(
        &disk_backed_result_plan("disk_result_passthrough"),
        CancellationToken::new(),
    )
    .unwrap();

    let RuntimeValue::Artifact(artifact) = &run_result.value_for_test("result").unwrap() else {
        panic!("collected result must be an artifact");
    };
    assert_eq!(
        artifact
            .cursor()
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap(),
        [Value::String("durable".into())]
    );
    let result_id = run_result.result_ids["result"];
    let stored = results.result(result_id).unwrap();
    let ResultState::Ready(value) = &stored.state else {
        panic!("published result must be ready");
    };
    assert_eq!(
        value.page(0, 10).unwrap().as_ref(),
        &[Value::String("durable".into())]
    );
    assert!(std::fs::read_dir(&root).unwrap().next().is_none());
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn demand_driven_publication_exposes_only_the_requested_final_output() {
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("source", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| Ok(vec![Value::Integer(3).into()])),
        )
        .unwrap();
    kernels
        .register(
            id("target", KernelHandle::new),
            FnKernel(|inputs: &[RuntimeValue]| {
                let RuntimeValue::Scalar(Value::Integer(value)) = &inputs[0] else {
                    panic!("expected integer input")
                };
                Ok(vec![Value::Integer(value + 4).into()])
            }),
        )
        .unwrap();
    let output = stable_output("final");
    let mut target = operation("target", &[0], &[2]);
    target.outputs[0].public_output = Some(output.clone());
    let mut execution_plan = plan(
        vec![operation("source", &[], &[0]), target],
        3,
        StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(0)),
            ControlStep::Operation(OperationIndex::new(1)),
        ])),
    );
    execution_plan.results = Box::new([PlanResult {
        name: "final".into(),
        output: output.clone(),
        value: ValueRef::new(2),
    }]);
    execution_plan.publications = Box::new([PlannedPublication::GraphResult {
        name: "final".into(),
        output: output.clone(),
        value: ValueRef::new(2),
    }]);
    let results = ResultStore::new();

    let run = RunExecutor::new(
        &kernels,
        &no_resources(),
        &NoFunctions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .with_result_store(&results)
    .run(&execution_plan, CancellationToken::new())
    .unwrap();

    assert_eq!(
        run.value_for_test("final").unwrap(),
        RuntimeValue::from(Value::Integer(7))
    );

    assert_eq!(run.result_ids.len(), 1);
    let result_id = run.result_ids["final"];
    let history = results.pin_history(&output);
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].result_id, result_id);
}

#[test]
fn demand_driven_publication_pin_preview_emits_only_dedicated_ready_event() {
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("preview", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| Ok(vec![Value::Integer(7).into()])),
        )
        .unwrap();
    let output = stable_output("preview");
    let mut execution_plan = plan(
        vec![operation("preview", &[], &[0])],
        1,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    execution_plan.results = Box::new([PlanResult {
        name: "requested.preview".into(),
        output: output.clone(),
        value: ValueRef::new(0),
    }]);
    execution_plan.publications = Box::new([PlannedPublication::PinPreview {
        output: output.clone(),
        generation: 17,
        value: ValueRef::new(0),
    }]);
    let events = RecordingRunEvents::default();
    let results = ResultStore::new();

    RunExecutor::new(
        &kernels,
        &no_resources(),
        &NoFunctions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .with_event_sink(&events)
    .with_result_store(&results)
    .run(&execution_plan, CancellationToken::new())
    .unwrap();

    let recorded = events.0.lock().unwrap();
    let preview_events = recorded
        .iter()
        .filter(|event| matches!(event.kind, RunEventKind::PinPreviewResultReady { .. }))
        .collect::<Vec<_>>();
    assert_eq!(preview_events.len(), 1);
    assert!(matches!(
        &preview_events[0].kind,
        RunEventKind::PinPreviewResultReady {
            output: emitted,
            generation: 17,
            ..
        } if emitted == &output
    ));
}

#[test]
fn invalid_publication_returns_typed_invalid_plan_without_panicking() {
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("invalid_publication", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| Ok(vec![Value::Integer(7).into()])),
        )
        .unwrap();
    let output = stable_output("result");
    let mut execution_plan = plan(
        vec![operation("invalid_publication", &[], &[0])],
        1,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    execution_plan.results = Box::new([PlanResult {
        name: "result".into(),
        output: output.clone(),
        value: ValueRef::new(0),
    }]);
    execution_plan.publications = Box::new([PlannedPublication::GraphResult {
        name: "missing-result".into(),
        output,
        value: ValueRef::new(0),
    }]);
    let results = ResultStore::new();

    let error = RunExecutor::new(
        &kernels,
        &no_resources(),
        &NoFunctions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .with_result_store(&results)
    .run(&execution_plan, CancellationToken::new())
    .expect_err("invalid publication must be rejected before execution");

    assert!(matches!(error, RunError::InvalidPlan(_)));
}

#[test]
fn missing_publications_return_typed_invalid_plan_before_execution() {
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("missing_publication", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| Ok(vec![Value::Integer(7).into()])),
        )
        .unwrap();
    let mut execution_plan = plan(
        vec![operation("missing_publication", &[], &[0])],
        1,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    execution_plan.results = Box::new([PlanResult {
        name: "result".into(),
        output: stable_output("result"),
        value: ValueRef::new(0),
    }]);
    let results = ResultStore::new();

    let error = RunExecutor::new(
        &kernels,
        &no_resources(),
        &NoFunctions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .with_result_store(&results)
    .run(&execution_plan, CancellationToken::new())
    .expect_err("results without publications must be rejected before execution");

    assert!(matches!(error, RunError::InvalidPlan(_)));
    assert_eq!(results.authoritative_result_count_for_test(), 0);
    assert_eq!(results.group_count_for_test(), 0);
}

#[test]
fn run_result_keeps_run_id_and_plan_provenance() {
    let execution_plan = plan(vec![], 0, StructuredControlRegion::Sequence(Box::new([])));
    let events = RecordingRunEvents::default();

    let result = RunExecutor::new(
        &KernelRegistry::new(),
        &no_resources(),
        &NoFunctions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .with_event_sink(&events)
    .run(&execution_plan, CancellationToken::new())
    .unwrap();

    assert_eq!(result.provenance, execution_plan.provenance);
    let started_run_ids = events
        .0
        .lock()
        .unwrap()
        .iter()
        .filter(|event| event.kind == RunEventKind::RunStarted)
        .map(|event| event.run.run_id)
        .collect::<Vec<_>>();
    assert_eq!(started_run_ids, [result.run_id]);
}

#[test]
fn failed_success_finalizer_publishes_no_result_or_completion() {
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("value", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| Ok(vec![Value::Integer(1).into()])),
        )
        .unwrap();
    let mut execution_plan = plan(
        vec![operation("value", &[], &[0])],
        1,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    execution_plan.results = Box::new([PlanResult {
        name: "value".into(),
        output: stable_output("value"),
        value: ValueRef::new(0),
    }]);
    publish_graph_results(&mut execution_plan);
    let events = RecordingRunEvents::default();
    let results = ResultStore::new();
    let finalizer = |_: &mut RunResult, _: &CancellationToken, _: Option<RunDeadline>| {
        Err(RunError::ResourceSnapshotMismatch(
            "authoritative commit failed".into(),
        ))
    };

    let error = RunExecutor::new(
        &kernels,
        &no_resources(),
        &NoFunctions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .with_event_sink(&events)
    .with_result_store(&results)
    .with_success_finalizer(&finalizer)
    .run(&execution_plan, CancellationToken::new())
    .expect_err("failed authoritative finalization must fail the run");

    assert!(matches!(error, RunError::ResourceSnapshotMismatch(_)));
    let recorded = events.0.lock().unwrap();
    assert!(recorded.iter().any(|event| matches!(
        event.kind,
        RunEventKind::RunErrored {
            outcome: RunErrorOutcome::Ordinary {
                code: OrdinaryRunErrorCode::ResourceSnapshotMismatch,
            },
        }
    )));
    assert!(
        recorded
            .iter()
            .all(|event| event.kind != RunEventKind::RunCompleted)
    );
    let run_id = recorded
        .iter()
        .find(|event| event.kind == RunEventKind::RunStarted)
        .map(|event| event.run.run_id)
        .expect("RunStarted carries the active run ID");
    let stored_results = results.results_for_run(run_id);
    assert_eq!(stored_results.len(), 1);
    assert!(matches!(&stored_results[0].state, ResultState::Ready(_)));
}

#[test]
fn cancellation_before_final_result_publication_cleans_results_without_completion() {
    use super::super::scheduler::SchedulerCheckpoint;

    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("final_publication", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| Ok(vec![Value::Integer(1).into()])),
        )
        .unwrap();
    let mut execution_plan = plan(
        vec![operation("final_publication", &[], &[0])],
        1,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    execution_plan.results = Box::new([PlanResult {
        name: "result".into(),
        output: stable_output("result"),
        value: ValueRef::new(0),
    }]);
    publish_graph_results(&mut execution_plan);
    let events = RecordingRunEvents::default();
    let results = ResultStore::new();
    let final_checkpoints = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&final_checkpoints);

    let error = RunExecutor::new(
        &kernels,
        &no_resources(),
        &NoFunctions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .with_event_sink(&events)
    .with_result_store(&results)
    .with_test_checkpoint(Arc::new(move |checkpoint, cancellation| {
        if checkpoint == SchedulerCheckpoint::FinalResultPublication {
            observed.fetch_add(1, Ordering::SeqCst);
            cancellation.cancel();
        }
    }))
    .run(&execution_plan, CancellationToken::new())
    .unwrap_err();

    assert_eq!(error, RunError::Cancelled);
    assert_eq!(final_checkpoints.load(Ordering::SeqCst), 1);
    let run_id = assert_cancelled_without_completion(&events);
    let stored_results = results.results_for_run(run_id);
    assert_eq!(stored_results.len(), 1);
    assert!(matches!(&stored_results[0].state, ResultState::Ready(_)));
}
