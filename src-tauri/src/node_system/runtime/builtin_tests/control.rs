use super::*;

#[test]
fn unary_math_kernels_execute_supported_operations() {
    let params = handle("unary", CompiledParameterHandle::new);
    for (kernel, input, expected) in [
        ("yssbi.numeric.ln", "1", "0"),
        ("yssbi.numeric.log2", "8", "3"),
        ("yssbi.numeric.log10", "100", "2"),
        ("yssbi.numeric.exp", "0", "1"),
        ("yssbi.numeric.sqrt", "9", "3"),
        ("yssbi.numeric.square", "4", "16"),
    ] {
        let output =
            execute_kernel_direct(kernel, &params, None, &[decimal(input).into()]).unwrap();
        assert_eq!(output, vec![decimal(expected).into()], "{kernel}");
    }
}

#[test]
fn do_sleep_print_and_view_scheduler_contracts() {
    let params = handle("effects", CompiledParameterHandle::new);
    assert!(
        execute_kernel_direct("yssbi.control.do", &params, None, &[])
            .unwrap()
            .is_empty()
    );
    assert!(
        execute_kernel_direct("yssbi.control.sleep", &params, None, &[decimal("0").into()])
            .unwrap()
            .is_empty()
    );
    let sleep_error = execute_kernel_direct(
        "yssbi.control.sleep",
        &params,
        None,
        &[decimal("-0.01").into()],
    )
    .unwrap_err();
    assert_eq!(
        sleep_error.message(),
        "Sleep duration must be between zero and sixty seconds"
    );
    let started = std::time::Instant::now();
    let deadline_error = execute_kernel_direct_with_deadline(
        "yssbi.control.sleep",
        &params,
        None,
        &[decimal("1").into()],
        Some(RunDeadline::after(std::time::Duration::from_millis(10))),
    )
    .unwrap_err();
    assert_eq!(deadline_error.kind(), KernelErrorKind::DeadlineExceeded);
    assert!(started.elapsed() < std::time::Duration::from_millis(200));
    use tracing_subscriber::layer::SubscriberExt;

    let (diagnostics, _diagnostics_guard) = crate::diagnostics::dispatcher::DiagnosticsHub::start();
    let subscriber = tracing_subscriber::registry()
        .with(crate::diagnostics::recent_layer::RecentDiagnosticsLayer::new(diagnostics.clone()));
    assert!(
        tracing::subscriber::with_default(subscriber, || {
            execute_kernel_direct(
                "yssbi.debug.print",
                &params,
                None,
                &[Value::String("fine".into()).into()],
            )
        })
        .unwrap()
        .is_empty()
    );
    let subscription = diagnostics.subscribe(|_| true).unwrap();
    assert!(
        subscription
            .entries
            .iter()
            .all(|record| record.message != "fine"),
        "Print output must not enter diagnostic recent storage"
    );
    diagnostics
        .unsubscribe(subscription.subscription_id)
        .unwrap();
    let print_error = execute_kernel_direct(
        "yssbi.debug.print",
        &params,
        None,
        &[Value::Integer(1).into()],
    )
    .unwrap_err();
    assert_eq!(
        print_error.message(),
        "Print message must be a String scalar"
    );
    assert!(
        build_builtin_kernel_registry()
            .get(&KernelHandle::new("yssbi.debug.view").unwrap())
            .is_none(),
        "View Data is a scheduler side effect, not an ordinary kernel"
    );

    let mut parameters = CompiledParameterStore::new();
    for (name, message) in [("first", "First"), ("second", "Second"), ("third", "Third")] {
        insert_constant(&mut parameters, name, Value::String(message.into()));
    }
    let chain = plan(
        vec![
            operation("yssbi.constant.string", "first", &[], 0),
            operation("yssbi.constant.string", "second", &[], 1),
            operation("yssbi.constant.string", "third", &[], 2),
            effect_operation("yssbi.debug.print", "unused.print.1", &[0]),
            effect_operation("yssbi.control.do", "unused.do", &[]),
            effect_operation("yssbi.debug.print", "unused.print.2", &[1]),
            effect_operation("yssbi.debug.print", "unused.print.3", &[2]),
        ],
        3,
        &[],
    );
    execute(&chain, &parameters).unwrap();
}

#[test]
fn print_output_and_trace_preserve_exact_first_second_third_order() {
    #[derive(Default)]
    struct Events {
        run_events: Mutex<Vec<RunEvent>>,
        run_output: Mutex<Vec<RunOutputMessage>>,
    }
    impl RunEventSink for Events {
        fn record(&self, event: RunEvent) {
            self.run_events.lock().unwrap().push(event);
        }

        fn record_run_output(&self, event: RunOutputMessage) {
            self.run_output.lock().unwrap().push(event);
        }
    }
    #[derive(Default)]
    struct Trace(Mutex<Vec<TraceSpan>>);
    impl TraceSink for Trace {
        fn start_span(&self, spec: SpanSpec) -> SpanGuard<'_> {
            SpanGuard::new(self, spec, &SYSTEM_TRACE_CLOCK)
        }

        fn complete_span(&self, span: TraceSpan) {
            self.0.lock().unwrap().push(span);
        }
    }

    let mut parameters = CompiledParameterStore::new();
    for (name, message) in [("first", "First"), ("second", "Second"), ("third", "Third")] {
        insert_constant(&mut parameters, name, Value::String(message.into()));
    }
    let mut operations = vec![
        operation("yssbi.constant.string", "first", &[], 0),
        operation("yssbi.constant.string", "second", &[], 1),
        operation("yssbi.constant.string", "third", &[], 2),
        effect_operation("yssbi.debug.print", "unused.print.1", &[0]),
        effect_operation("yssbi.control.do", "unused.do", &[]),
        effect_operation("yssbi.debug.print", "unused.print.2", &[1]),
        effect_operation("yssbi.debug.print", "unused.print.3", &[2]),
    ];
    for (index, node) in [(3, 101_u128), (5, 102), (6, 103)] {
        operations[index].source_node_id = NodeId::from_uuid(uuid::Uuid::from_u128(node));
        operations[index].source_node_type_id = NodeTypeId::new("yssbi.debug.print").unwrap();
    }
    let mut execution_plan = plan(operations, 3, &[]);
    execution_plan.effect_dependencies = Box::new([
        EffectDependency {
            before: OperationIndex::new(3),
            after: OperationIndex::new(5),
        },
        EffectDependency {
            before: OperationIndex::new(5),
            after: OperationIndex::new(6),
        },
    ]);
    let events = Events::default();
    let trace = Trace::default();
    let kernels = build_builtin_kernel_registry();

    RunExecutor::new(
        &kernels,
        &NoResources,
        &NoFunctions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .with_compiled_parameters(&parameters)
    .with_event_sink(&events)
    .with_trace_sink(&trace)
    .run(&execution_plan, CancellationToken::new())
    .unwrap();

    let label = |node_id: NodeId| match node_id.as_uuid().as_u128() {
        101 => Some("First"),
        102 => Some("Second"),
        103 => Some("Third"),
        _ => None,
    };
    let event_order = events
        .run_events
        .lock()
        .unwrap()
        .iter()
        .filter(|event| matches!(event.kind, RunEventKind::OperationCompleted { .. }))
        .filter_map(|event| event.correlation.node_id.and_then(label))
        .collect::<Vec<_>>();
    let output = events
        .run_output
        .lock()
        .unwrap()
        .iter()
        .filter_map(|event| match event {
            RunOutputMessage::Output(event) => Some(event.clone()),
            RunOutputMessage::Status(_) => None,
        })
        .collect::<Vec<_>>();
    let trace_order = trace
        .0
        .lock()
        .unwrap()
        .iter()
        .filter(|span| {
            span.kind == SpanKind::OperationAttempt && span.outcome == SpanOutcome::Success
        })
        .filter_map(|event| event.correlation.node_id.and_then(label))
        .collect::<Vec<_>>();
    assert_eq!(event_order, ["First", "Second", "Third"]);
    assert_eq!(
        output
            .iter()
            .map(|event| event.text.as_ref())
            .collect::<Vec<_>>(),
        ["First", "Second", "Third"]
    );
    assert_eq!(
        output
            .iter()
            .map(|event| event.sequence)
            .collect::<Vec<_>>(),
        [1, 2, 3]
    );
    assert_eq!(
        output
            .iter()
            .map(|event| label(event.source_node_id))
            .collect::<Vec<_>>(),
        [Some("First"), Some("Second"), Some("Third")]
    );
    assert!(
        output
            .iter()
            .all(|event| event.stream == RunOutputStream::Stdout)
    );
    assert_eq!(trace_order, ["First", "Second", "Third"]);
}

#[test]
fn real_graph_connection_overrides_print_protocol_default_at_runtime() {
    struct Resources;
    impl ResourceSnapshot for Resources {
        fn versions(&self) -> crate::node_system::analysis::ResourceVersionSet {
            BTreeMap::new()
        }
    }
    struct CapturePrint(Arc<Mutex<Vec<Value>>>);
    impl Kernel for CapturePrint {
        fn execute(
            &self,
            _: &KernelContext<'_>,
            inputs: &[RuntimeValue],
        ) -> Result<Vec<RuntimeValue>, KernelError> {
            let [RuntimeValue::Scalar(value)] = inputs else {
                return Err(KernelError::new("expected one scalar print input"));
            };
            self.0.lock().unwrap().push(value.clone());
            Ok(Vec::new())
        }
    }

    let system = crate::node_system::catalog::build_builtin_node_system().unwrap();
    let registry = Arc::unwrap_or_clone(system.registry);
    let constant_id = NodeId::from_uuid(uuid::Uuid::from_u128(201));
    let print_id = NodeId::from_uuid(uuid::Uuid::from_u128(202));
    let mut constant_parameters = ParameterValues::new();
    constant_parameters.insert(
        crate::node_system::protocol::ParameterKey::new("value").unwrap(),
        serde_json::json!("Connected message"),
    );
    let mut graph = GraphDocument::default();
    graph.nodes.insert(
        constant_id,
        DocumentNode {
            id: constant_id,
            node_type: NodeTypeId::new("yssbi.constant.string").unwrap(),
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters: constant_parameters,
            user_label: None,
        },
    );
    graph.nodes.insert(
        print_id,
        DocumentNode {
            id: print_id,
            node_type: NodeTypeId::new("yssbi.debug.print").unwrap(),
            position: NodePosition { x: 1.0, y: 0.0 },
            parameters: ParameterValues::new(),
            user_label: None,
        },
    );
    graph.connections.insert(
        ConnectionId::from_uuid(uuid::Uuid::from_u128(203)),
        DocumentConnection {
            id: ConnectionId::from_uuid(uuid::Uuid::from_u128(203)),
            output: PortAddress::declared(constant_id, PortKey::new("value").unwrap()),
            input: PortAddress::declared(print_id, PortKey::new("message").unwrap()),
            order: None,
        },
    );

    let compiled = GraphCompiler::new(&registry, &Resources).compile(&graph);
    let mut execution_plan = compiled
        .plan
        .unwrap_or_else(|| panic!("print diagnostics: {:?}", compiled.analysis.diagnostics));
    let constant_index = execution_plan
        .operations
        .iter()
        .position(|operation| operation.source_node_id == constant_id)
        .unwrap();
    let print_index = execution_plan
        .operations
        .iter()
        .position(|operation| operation.source_node_id == print_id)
        .unwrap();
    assert_eq!(execution_plan.operations[print_index].inputs.len(), 1);
    assert_eq!(
        execution_plan.operations[print_index].inputs[0].bound_value,
        None
    );
    let print_input = execution_plan.operations[print_index].inputs[0].value;
    let mut reachable =
        BTreeSet::from([execution_plan.operations[constant_index].outputs[0].value]);
    loop {
        let previous_len = reachable.len();
        for dependency in &execution_plan.value_dependencies {
            if reachable.contains(&dependency.source) {
                reachable.insert(dependency.destination);
            }
        }
        for operation in &execution_plan.operations {
            if matches!(operation.kernel, PlannedKernel::Adapter(_))
                && operation
                    .inputs
                    .iter()
                    .any(|input| reachable.contains(&input.value))
            {
                reachable.extend(operation.outputs.iter().map(|output| output.value));
            }
        }
        if reachable.len() == previous_len {
            break;
        }
    }
    assert!(reachable.contains(&print_input));

    let capture_handle = handle("test.capture.print", KernelHandle::new);
    execution_plan.operations[print_index].kernel = PlannedKernel::Native(capture_handle.clone());
    let captured = Arc::new(Mutex::new(Vec::new()));
    let mut kernels = build_builtin_kernel_registry();
    kernels
        .register(capture_handle, CapturePrint(Arc::clone(&captured)))
        .unwrap();
    let mut parameters = CompiledParameterStore::new();
    parameters
        .insert(
            execution_plan.operations[constant_index].params.clone(),
            BuiltinConstantParameters::new(Value::String("Connected message".into())),
        )
        .unwrap();

    RunExecutor::new(
        &kernels,
        &NoResources,
        &NoFunctions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .with_compiled_parameters(&parameters)
    .run(&execution_plan, CancellationToken::new())
    .unwrap();

    assert_eq!(
        captured.lock().unwrap().as_slice(),
        [Value::String("Connected message".into())]
    );
}

#[test]
fn print_protocol_has_default_and_ordered_chain_contract() {
    use crate::node_system::catalog::build_builtin_node_system;
    use crate::node_system::protocol::{EffectSemantics, PortKey, Purity};

    let system = build_builtin_node_system().unwrap();
    let print = system
        .registry
        .get(&NodeTypeId::new("yssbi.debug.print").unwrap())
        .unwrap();
    assert_eq!(print.protocol().execution.effects, EffectSemantics::Ordered);
    assert_eq!(print.protocol().execution.purity, Purity::Effectful);
    let message = print
        .protocol()
        .interface
        .ports
        .iter()
        .find(|port| port.key == PortKey::new("message").unwrap())
        .unwrap();
    assert_eq!(
        message
            .input_binding
            .as_ref()
            .and_then(|binding| binding.default_value.as_ref())
            .map(|value| &value.value),
        Some(&Value::String("Hello, World!".into()))
    );

    let mut default_print = effect_operation("yssbi.debug.print", "unused.default", &[0]);
    default_print.inputs[0].bound_value = Some(Value::String("Hello, World!".into()));
    execute(
        &plan(vec![default_print], 1, &[]),
        &CompiledParameterStore::new(),
    )
    .unwrap();
}

#[test]
fn builtin_kernels_report_division_by_zero_and_type_errors() {
    let mut parameters = CompiledParameterStore::new();
    insert_constant(&mut parameters, "int.one", Value::Integer(1));
    insert_constant(&mut parameters, "int.zero", Value::Integer(0));
    insert_constant(&mut parameters, "bool.true", Value::Bool(true));
    let divide_by_zero = plan(
        vec![
            operation("yssbi.constant.int64", "int.one", &[], 0),
            operation("yssbi.constant.int64", "int.zero", &[], 1),
            operation("yssbi.numeric.divide.int64", "unused.0", &[0, 1], 2),
        ],
        3,
        &[2],
    );
    let wrong_type = plan(
        vec![
            operation("yssbi.constant.bool", "bool.true", &[], 0),
            operation("yssbi.constant.int64", "int.one", &[], 1),
            operation("yssbi.numeric.add.int64", "unused.1", &[0, 1], 2),
        ],
        3,
        &[2],
    );

    let zero_error = execute(&divide_by_zero, &parameters).unwrap_err();
    let type_error = execute(&wrong_type, &parameters).unwrap_err();

    assert!(
        matches!(zero_error, RunError::KernelFailed { ref message, .. } if message.contains("division by zero"))
    );
    assert!(
        matches!(type_error, RunError::KernelFailed { ref message, .. } if message.contains("expected int64"))
    );
}
