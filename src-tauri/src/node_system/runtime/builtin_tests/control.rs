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

    let diagnostics = yss_diagnostics::DiagnosticsRuntime::initialize().unwrap();
    let subscriber = tracing_subscriber::registry()
        .with(yss_tracing::LogLayer::new(diagnostics.rust_log_sink()));
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
    let subscription = diagnostics.subscribe_batches(|_| true).unwrap();
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
fn print_output_preserves_exact_first_second_third_order() {
    #[derive(Default)]
    struct Events {
        run_output: Mutex<Vec<RunOutputMessage>>,
    }
    impl RunEventSink for Events {
        fn record(&self, _: RunEvent) {}

        fn record_run_output(&self, event: RunOutputMessage) {
            self.run_output.lock().unwrap().push(event);
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
    .run(&execution_plan, CancellationToken::new())
    .unwrap();

    let output = events.run_output.lock().unwrap();
    let text = output
        .iter()
        .filter_map(|message| match message {
            RunOutputMessage::Output(event) => Some((
                event.sequence,
                event.stream,
                event.text.as_ref(),
                event.source_node_id,
                event.source_port.clone(),
            )),
            RunOutputMessage::Status(_) => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        text.iter()
            .map(|(_, _, text, _, _)| *text)
            .collect::<Vec<_>>(),
        ["First", "Second", "Third"],
    );
    assert_eq!(
        text.iter()
            .map(|(sequence, _, _, _, _)| *sequence)
            .collect::<Vec<_>>(),
        [1, 2, 3],
    );
    assert!(
        text.iter()
            .all(|(_, stream, _, _, _)| *stream == RunOutputStream::Stdout)
    );
    assert_eq!(
        text.iter()
            .map(|(_, _, _, source_node_id, _)| *source_node_id)
            .collect::<Vec<_>>(),
        [
            NodeId::from_uuid(uuid::Uuid::from_u128(101)),
            NodeId::from_uuid(uuid::Uuid::from_u128(102)),
            NodeId::from_uuid(uuid::Uuid::from_u128(103)),
        ],
    );
    assert_eq!(
        text.iter()
            .map(|(_, _, _, _, source_port)| source_port.clone())
            .collect::<Vec<_>>(),
        [
            PortAddress::declared(
                NodeId::from_uuid(uuid::Uuid::from_u128(101)),
                PortKey::new("message").unwrap(),
            ),
            PortAddress::declared(
                NodeId::from_uuid(uuid::Uuid::from_u128(102)),
                PortKey::new("message").unwrap(),
            ),
            PortAddress::declared(
                NodeId::from_uuid(uuid::Uuid::from_u128(103)),
                PortKey::new("message").unwrap(),
            ),
        ],
    );
}

#[test]
fn real_graph_connection_overrides_print_protocol_default_at_runtime() {
    struct Resources;
    impl ResourceSnapshot for Resources {
        fn versions(&self) -> crate::graph::analysis::contracts::ResourceVersionSet {
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

    let system = crate::graph::catalog::build_builtin_node_system().unwrap();
    let registry = Arc::unwrap_or_clone(system.registry);
    let constant_id = NodeId::from_uuid(uuid::Uuid::from_u128(201));
    let print_id = NodeId::from_uuid(uuid::Uuid::from_u128(202));
    let mut constant_parameters = ParameterValues::new();
    constant_parameters.insert(
        crate::graph::protocol::ParameterKey::new("value").unwrap(),
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
    use crate::graph::catalog::build_builtin_node_system;
    use crate::graph::protocol::{EffectSemantics, PortKey, Purity};

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
