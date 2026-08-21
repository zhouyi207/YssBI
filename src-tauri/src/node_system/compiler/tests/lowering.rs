use super::*;

#[test]
fn lowerability_invalid_dataframe_parameters_block_in_analysis() {
    let builtins = crate::node_system::catalog::build_builtin_node_system().unwrap();
    for (node_type, parameter, value) in [
        ("yssbi.dataframe.limit", "rows", serde_json::json!(0)),
        ("yssbi.dataframe.limit", "rows", serde_json::json!("ten")),
        ("yssbi.dataframe.rename", "from", serde_json::json!(42)),
        (
            "yssbi.dataframe.source.get",
            "dataframe",
            serde_json::json!(" databases/main"),
        ),
    ] {
        let protocol = builtins
            .registry
            .protocol(&NodeTypeId::new(node_type).unwrap())
            .unwrap()
            .clone();
        let node_type_id = protocol.type_id.clone();
        let calls = Arc::new(AtomicUsize::new(0));
        let registry = TestRegistry::new(vec![protocol])
            .with_lowerer(&node_type_id, CountingLowerer(calls.clone()));
        let mut graph = graph_with_node_types([(1, node_type.to_owned())]);
        graph
            .nodes
            .get_mut(&node_id(1))
            .unwrap()
            .parameters
            .insert(ParameterKey::new(parameter).unwrap(), value);
        if node_type == "yssbi.dataframe.rename" {
            graph.nodes.get_mut(&node_id(1)).unwrap().parameters.insert(
                ParameterKey::new("to").unwrap(),
                serde_json::json!("renamed"),
            );
        }
        let trace = RecordingTrace::default();

        let result = GraphCompiler::new(&registry, &Resources)
            .with_observability(ProjectSessionId::new("lowerability"), &trace)
            .compile(&graph);

        assert_analysis_blocks_before_lowering(&result, &trace, &calls);
        assert!(
            result.analysis.diagnostics.iter().any(|diagnostic| {
                diagnostic.code.as_str() == "compiler.parameter.invalid"
                    && matches!(
                        &diagnostic.primary,
                        DiagnosticLocation::Parameter { node_id: actual, key }
                            if *actual == node_id(1) && key.as_str() == parameter
                    )
            }),
            "missing precise parameter diagnostic for {node_type}:{parameter}"
        );
    }
}

#[test]
fn lowerability_malformed_persisted_literal_blocks_at_port_in_analysis() {
    let protocol = test_protocol(
        "malformed_literal",
        vec![data_port(
            "value",
            PortDirection::Input,
            TypeExpr::Unknown,
            None,
        )],
        vec![],
        vec![],
    );
    let node_type = protocol.type_id.clone();
    let calls = Arc::new(AtomicUsize::new(0));
    let registry =
        TestRegistry::new(vec![protocol]).with_lowerer(&node_type, CountingLowerer(calls.clone()));
    let address = PortAddress::declared(node_id(1), key("value"));
    let mut graph = graph_with_nodes(&[(1, "malformed_literal")]);
    graph.input_states.insert(
        address.clone(),
        InputState {
            literal_override: Some(serde_json::json!({"value_type": "not-a-type"})),
        },
    );
    let trace = RecordingTrace::default();

    let result = GraphCompiler::new(&registry, &Resources)
        .with_observability(ProjectSessionId::new("lowerability"), &trace)
        .compile(&graph);

    assert_analysis_blocks_before_lowering(&result, &trace, &calls);
    assert!(result.analysis.diagnostics.iter().any(|diagnostic| {
        diagnostic.code.as_str() == "compiler.input.literal_invalid"
            && diagnostic.primary == DiagnosticLocation::Port(address.clone())
    }));
}

#[test]
fn lowerability_legal_literal_wire_with_wrong_port_type_blocks_at_exact_port() {
    let protocol = test_protocol(
        "literal_type_mismatch",
        vec![data_port(
            "value",
            PortDirection::Input,
            TypeExpr::Concrete(type_id("core.int64")),
            None,
        )],
        vec![],
        vec![],
    );
    let node_type = protocol.type_id.clone();
    let calls = Arc::new(AtomicUsize::new(0));
    let registry =
        TestRegistry::new(vec![protocol]).with_lowerer(&node_type, CountingLowerer(calls.clone()));
    let address = PortAddress::declared(node_id(1), key("value"));
    let mut graph = graph_with_nodes(&[(1, "literal_type_mismatch")]);
    graph.input_states.insert(
        address.clone(),
        InputState {
            literal_override: Some(
                serde_json::to_value(crate::node_system::protocol::TypedValue {
                    value_type: TypeExpr::Concrete(type_id("core.string")),
                    value: Value::String("legal-string-wire".into()),
                })
                .unwrap(),
            ),
        },
    );
    let trace = RecordingTrace::default();

    let result = GraphCompiler::new(&registry, &Resources)
        .with_observability(ProjectSessionId::new("literal-mismatch"), &trace)
        .compile(&graph);

    assert_analysis_blocks_before_lowering(&result, &trace, &calls);
    assert!(result.analysis.diagnostics.iter().any(|diagnostic| {
        diagnostic.code.as_str() == "compiler.input.literal_invalid"
            && diagnostic.primary == DiagnosticLocation::Port(address.clone())
    }));
}

#[test]
fn lowerability_nested_literal_mismatch_blocks_at_exact_port_before_lowering() {
    let series = TypeExpr::Applied {
        constructor: TypeConstructorId::new("core.data_series").unwrap(),
        arguments: vec![TypeExpr::Concrete(type_id("core.int64"))],
    };
    let protocol = test_protocol(
        "nested_literal_mismatch",
        vec![data_port(
            "value",
            PortDirection::Input,
            series.clone(),
            None,
        )],
        vec![],
        vec![],
    );
    let node_type = protocol.type_id.clone();
    let calls = Arc::new(AtomicUsize::new(0));
    let registry =
        TestRegistry::new(vec![protocol]).with_lowerer(&node_type, CountingLowerer(calls.clone()));
    let address = PortAddress::declared(node_id(1), key("value"));
    let mut graph = graph_with_nodes(&[(1, "nested_literal_mismatch")]);
    graph.input_states.insert(
        address.clone(),
        InputState {
            literal_override: Some(
                serde_json::to_value(crate::node_system::protocol::TypedValue {
                    value_type: series,
                    value: Value::List(vec![Value::Integer(1), Value::String("wrong".into())]),
                })
                .unwrap(),
            ),
        },
    );
    let trace = RecordingTrace::default();

    let result = GraphCompiler::new(&registry, &Resources)
        .with_observability(ProjectSessionId::new("nested-literal-mismatch"), &trace)
        .compile(&graph);

    assert_analysis_blocks_before_lowering(&result, &trace, &calls);
    assert!(result.analysis.diagnostics.iter().any(|diagnostic| {
        diagnostic.code.as_str() == "compiler.input.literal_invalid"
            && diagnostic.primary == DiagnosticLocation::Port(address.clone())
    }));
}

#[test]
fn non_concrete_parameter_shapes_block_when_they_cannot_be_prepared() {
    let shapes = [
        (
            "union",
            TypeExpr::Union(vec![TypeExpr::Concrete(type_id("core.int64"))]),
        ),
        (
            "applied",
            TypeExpr::Applied {
                constructor: TypeConstructorId::new("core.list").unwrap(),
                arguments: vec![TypeExpr::Concrete(type_id("core.int64"))],
            },
        ),
        (
            "generic",
            TypeExpr::Generic(TypeParameterId::new("t").unwrap()),
        ),
        ("unknown", TypeExpr::Unknown),
    ];

    for (name, value_type) in shapes {
        let mut protocol = test_protocol(name, vec![], vec![], vec![]);
        protocol.parameters = ParameterSchema::new(vec![ParameterSpec {
            key: ParameterKey::new("value").unwrap(),
            title_key: I18nKey::new("parameters.value.title").unwrap(),
            description_key: None,
            value_type,
            default_value: None,
            constraints: vec![ParameterConstraint::Required],
            editor: ParameterEditorSpec::Auto,
            presentation: ParameterPresentation::DetailPanel,
        }])
        .unwrap();
        let node_type = protocol.type_id.clone();
        let calls = Arc::new(AtomicUsize::new(0));
        let registry = TestRegistry::new(vec![protocol])
            .with_lowerer(&node_type, CountingLowerer(calls.clone()));
        let mut graph = graph_with_nodes(&[(1, name)]);
        set_parameters(&mut graph, 1, &[("value", serde_json::json!(7))]);
        let trace = RecordingTrace::default();

        let result = GraphCompiler::new(&registry, &Resources)
            .with_observability(ProjectSessionId::new("unpreparable"), &trace)
            .compile(&graph);

        assert_analysis_blocks_before_lowering(&result, &trace, &calls);
        assert!(result.analysis.diagnostics.iter().any(|diagnostic| {
            diagnostic.code.as_str() == "compiler.parameter.invalid"
                && matches!(
                    &diagnostic.primary,
                    DiagnosticLocation::Parameter { node_id: actual, key }
                        if *actual == node_id(1) && key.as_str() == "value"
                )
        }));
    }
}

#[test]
fn typed_lowering_cancellation_cancels_compilation() {
    let protocol = test_protocol("lowering_cancelled", vec![], vec![], vec![]);
    let node_type = protocol.type_id.clone();
    let registry = TestRegistry::new(vec![protocol]).with_lowerer(&node_type, CancelledLowerer);
    let compiler = GraphCompiler::new(&registry, &Resources);
    let graph = graph_with_nodes(&[(1, "lowering_cancelled")]);
    let snapshot = compiler.snapshot(GraphResourcePath("events/cancelled".into()), &graph);

    let result = compiler.compile_snapshot(&snapshot, &CompileCancellationToken::new());

    assert!(matches!(result, Err(CompileCancelled)));
}

#[test]
fn internal_lowering_failure_preserves_semantic_without_plan() {
    let protocol = test_protocol("lowering_failure", vec![], vec![], vec![]);
    let node_type = protocol.type_id.clone();
    let registry = TestRegistry::new(vec![protocol]).with_lowerer(&node_type, FailingLowerer);

    let result = GraphCompiler::new(&registry, &Resources)
        .compile(&graph_with_nodes(&[(1, "lowering_failure")]));

    assert!(result.semantic.is_some());
    assert!(result.plan.is_none());
    assert!(
        result.analysis.diagnostics.iter().all(|diagnostic| {
            diagnostic.code.as_str() != "compiler.lowering.internal_invariant"
        })
    );
    assert!(matches!(
        result.outcome,
        CompilationOutcome::InternalFailure(ref failure)
            if failure.stage == CompilationStage::Lowering
                && failure.code.as_ref() == "compiler.lowering.internal_invariant"
                && failure.node_id == Some(node_id(1))
    ));
}

#[test]
fn unbound_input_diagnostic_carries_the_exact_port() {
    let protocol = test_protocol(
        "unbound_input",
        vec![data_port(
            "value",
            PortDirection::Input,
            TypeExpr::Unknown,
            None,
        )],
        vec![],
        vec![],
    );
    let registry = TestRegistry::new(vec![protocol]);
    let address = PortAddress::declared(node_id(1), key("value"));

    let result = GraphCompiler::new(&registry, &Resources)
        .compile(&graph_with_nodes(&[(1, "unbound_input")]));

    let diagnostic = result
        .analysis
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code.as_str() == "compiler.input.unbound")
        .expect("unbound input diagnostic");
    assert_eq!(
        diagnostic.arguments,
        BTreeMap::from([(Box::from("port"), address.to_string().into())])
    );
    assert_eq!(diagnostic.severity, DiagnosticSeverity::Warning);
    let basis = result.execution_basis.as_ref().expect("execution basis");
    assert!(result.plan.is_none());
    assert!(matches!(result.outcome, CompilationOutcome::Succeeded));
    let default_plan = basis
        .derive_plan(&ExecutionDemand::Default)
        .expect("default demand ignores an unbound orphan");
    assert!(default_plan.operations.is_empty());
}

#[test]
fn compiler_maps_data_edges_into_plan_dependencies() {
    let source = test_protocol(
        "plan_data_source",
        vec![data_port(
            "out",
            PortDirection::Output,
            TypeExpr::Unknown,
            None,
        )],
        vec![],
        vec![],
    );
    let sink = test_protocol(
        "plan_data_sink",
        vec![data_port(
            "in",
            PortDirection::Input,
            TypeExpr::Unknown,
            None,
        )],
        vec![],
        vec![],
    );
    let registry = TestRegistry::new(vec![source, sink]);
    let mut graph = graph_with_nodes(&[(1, "plan_data_source"), (2, "plan_data_sink")]);
    connect(&mut graph, 10, 1, "out", 2, "in");

    let plan = GraphCompiler::new(&registry, &Resources)
        .compile(&graph)
        .plan
        .expect("data graph should lower");

    assert_eq!(plan.operations.len(), 2);
    assert_eq!(plan.value_dependencies.len(), 1);
    assert!(
        plan.operations
            .iter()
            .all(|operation| !matches!(operation.kernel, PlannedKernel::Adapter(_)))
    );
    assert_eq!(
        plan.value_dependencies[0],
        crate::node_system::plan::ValueDependency {
            source: plan.operations[0].outputs[0].value,
            destination: plan.operations[1].inputs[0].value,
        }
    );
}

#[test]
fn data_series_contract_survives_materialization_adapter_insertion() {
    use crate::node_system::plan::PlannedValueKind;
    use crate::node_system::protocol::data_series_type;

    let series = data_series_type(TypeExpr::Concrete(TypeId::new("core.int64").unwrap()));
    let mut source_output = data_port("out", PortDirection::Output, series.clone(), None);
    source_output.production = Some(OutputProduction::Streaming);
    let source = test_protocol(
        "series_contract_source",
        vec![source_output],
        vec![],
        vec![],
    );
    let mut sink_input = data_port("in", PortDirection::Input, series, None);
    sink_input.consumption = Some(InputConsumption::FullyMaterialized);
    let sink = test_protocol("series_contract_sink", vec![sink_input], vec![], vec![]);
    let registry = TestRegistry::new(vec![source, sink]).with_constructor(
        TypeConstructorId::new("core.data_series").unwrap(),
        1,
        [],
    );
    let mut graph = graph_with_nodes(&[(1, "series_contract_source"), (2, "series_contract_sink")]);
    connect(&mut graph, 10, 1, "out", 2, "in");

    let plan = GraphCompiler::new(&registry, &Resources)
        .compile(&graph)
        .plan
        .expect("connected canonical DataSeries graph should lower");
    let adapter = plan
        .operations
        .iter()
        .find(|operation| matches!(operation.kernel, PlannedKernel::Adapter(_)))
        .expect("streaming DataSeries edge should insert a materialization adapter");

    assert!(
        adapter
            .inputs
            .iter()
            .all(|input| input.contract.kind == PlannedValueKind::DataSeries)
    );
    assert!(
        adapter
            .outputs
            .iter()
            .all(|output| output.contract.kind == PlannedValueKind::DataSeries)
    );
}

#[test]
fn function_plan_store_rejects_data_series_kind_mismatch() {
    use crate::node_system::plan::{PlannedValueContract, PlannedValueKind};
    use crate::node_system::protocol::data_series_type;
    use crate::node_system::runtime::{FunctionPlanStore, FunctionPlanStoreError};

    let series = data_series_type(TypeExpr::Concrete(TypeId::new("core.int64").unwrap()));
    let mut source_output = data_port("out", PortDirection::Output, series.clone(), None);
    source_output.production = Some(OutputProduction::FullyMaterialized);
    let source = test_protocol(
        "series_function_source",
        vec![source_output],
        vec![],
        vec![],
    );
    let registry = TestRegistry::new(vec![source]).with_constructor(
        TypeConstructorId::new("core.data_series").unwrap(),
        1,
        [],
    );
    let path = GraphResourcePath("functions/series-contract".into());
    let compiler = GraphCompiler::new(&registry, &Resources);
    let graph = graph_with_nodes(&[(1, "series_function_source")]);
    let mut plan = compiler
        .compile(&graph)
        .plan
        .expect("canonical DataSeries function body should lower");
    let version = ResourceVersion::new("1");
    let versions = BTreeMap::from([(ResourceKey::new(path.0.as_ref()), version.clone())]);
    plan.provenance.graph_path = path.clone();
    plan.provenance.basis.resource_versions = versions.clone();
    let output = &mut plan.operations[0].outputs[0];
    output
        .public_output
        .as_mut()
        .expect("compiled function output keeps its public identity")
        .graph_path = path.clone();
    let result_value = output.value;
    plan.operations[0].outputs[0].contract.kind = PlannedValueKind::Scalar;
    plan.value_contracts
        .get_mut(&result_value)
        .expect("compiled output has a plan-global value contract")
        .kind = PlannedValueKind::Scalar;
    let result = FunctionParameterId("return".into());
    let abi = FunctionPlanAbi {
        provenance: plan.provenance.clone(),
        parameters: BTreeMap::new(),
        parameter_contracts: BTreeMap::new(),
        results: BTreeMap::from([(result.clone(), result_value)]),
        result_productions: BTreeMap::from([(result.clone(), OutputProduction::FullyMaterialized)]),
        result_contracts: BTreeMap::from([(
            result,
            PlannedValueContract {
                kind: PlannedValueKind::DataSeries,
                type_expr: series,
            },
        )]),
    };
    let error = match FunctionPlanStore::new(plan.provenance.project_session_id.clone(), 64)
        .generation(
            registry.fingerprint.clone(),
            versions,
            vec![(path, version, Arc::new(plan), Arc::new(abi))],
        ) {
        Ok(_) => panic!("corrupt function ABI value contract must be rejected"),
        Err(error) => error,
    };

    assert!(
        matches!(
            error,
            FunctionPlanStoreError::AbiValueContractMismatch { .. }
        ),
        "unexpected function plan store error: {error:?}"
    );
}

#[test]
fn compiler_keeps_fully_materialized_ols_report_directly_connected_to_view_data() {
    let builtins = crate::node_system::catalog::build_builtin_node_system().unwrap();
    let mut ols = builtins
        .registry
        .protocol(&NodeTypeId::new("yssbi.statistics.ols.summary").unwrap())
        .unwrap()
        .clone();
    ols.interface.ports = ols
        .interface
        .ports
        .iter()
        .filter(|port| port.key.as_str() == "report")
        .cloned()
        .collect();
    let mut view = builtins
        .registry
        .protocol(&NodeTypeId::new("yssbi.debug.view").unwrap())
        .unwrap()
        .clone();
    view.interface.ports = view
        .interface
        .ports
        .iter()
        .filter(|port| port.key.as_str() == "data")
        .cloned()
        .collect();
    let registry = TestRegistry::new(vec![ols, view]);
    let mut graph =
        builtin_graph_with_nodes(&[(1, "yssbi.statistics.ols.summary"), (2, "yssbi.debug.view")]);
    connect(&mut graph, 10, 1, "report", 2, "data");

    let result = GraphCompiler::new(&registry, &Resources).compile(&graph);
    let plan = result.plan.unwrap_or_else(|| {
        panic!(
            "OLS report -> View Data diagnostics: {:?}",
            result.analysis.diagnostics
        )
    });
    let ols = &plan.operations[operation_index_for_node(&plan, 1).index()];
    let view = &plan.operations[operation_index_for_node(&plan, 2).index()];

    assert!(
        plan.operations
            .iter()
            .all(|operation| !matches!(operation.kernel, PlannedKernel::Adapter(_))),
        "an already fully-materialized boundary must not insert an adapter operation"
    );
    assert!(plan.value_dependencies.contains(&ValueDependency {
        source: ols.outputs[0].value,
        destination: view.inputs[0].value,
    }));
}

#[test]
fn compiler_keeps_rewindable_batches_fanout_directly_connected() {
    let mut source_output = data_port("out", PortDirection::Output, TypeExpr::Unknown, None);
    source_output.production = Some(OutputProduction::Batches);
    let source = test_protocol("batch_fanout_source", vec![source_output], vec![], vec![]);
    let mut sink_input = data_port("in", PortDirection::Input, TypeExpr::Unknown, None);
    sink_input.consumption = Some(InputConsumption::RewindableBatches);
    let sink = test_protocol("batch_fanout_sink", vec![sink_input], vec![], vec![]);
    let registry = TestRegistry::new(vec![source, sink]);
    let mut graph = graph_with_nodes(&[
        (1, "batch_fanout_source"),
        (2, "batch_fanout_sink"),
        (3, "batch_fanout_sink"),
    ]);
    connect(&mut graph, 10, 1, "out", 2, "in");
    connect(&mut graph, 11, 1, "out", 3, "in");

    let plan = GraphCompiler::new(&registry, &Resources)
        .compile(&graph)
        .plan
        .expect("batch fanout graph should lower");
    let source = &plan.operations[operation_index_for_node(&plan, 1).index()];
    let first_sink = &plan.operations[operation_index_for_node(&plan, 2).index()];
    let second_sink = &plan.operations[operation_index_for_node(&plan, 3).index()];

    assert!(
        plan.operations
            .iter()
            .all(|operation| !matches!(operation.kernel, PlannedKernel::Adapter(_))),
        "an already rewindable batch source must not create a no-op fanout operation"
    );
    assert!(plan.value_dependencies.contains(&ValueDependency {
        source: source.outputs[0].value,
        destination: first_sink.inputs[0].value,
    }));
    assert!(plan.value_dependencies.contains(&ValueDependency {
        source: source.outputs[0].value,
        destination: second_sink.inputs[0].value,
    }));
}

#[test]
fn compiler_materializes_stream_once_before_same_contract_fanout() {
    let mut source_output = data_port("out", PortDirection::Output, TypeExpr::Unknown, None);
    source_output.production = Some(OutputProduction::Streaming);
    let source = test_protocol("fanout_source", vec![source_output], vec![], vec![]);
    let mut sink_input = data_port("in", PortDirection::Input, TypeExpr::Unknown, None);
    sink_input.consumption = Some(InputConsumption::FullyMaterialized);
    let sink = test_protocol("fanout_sink", vec![sink_input], vec![], vec![]);
    let registry = TestRegistry::new(vec![source, sink]);
    let mut graph =
        graph_with_nodes(&[(1, "fanout_source"), (2, "fanout_sink"), (3, "fanout_sink")]);
    connect(&mut graph, 10, 1, "out", 2, "in");
    connect(&mut graph, 11, 1, "out", 3, "in");

    let plan = GraphCompiler::new(&registry, &Resources)
        .compile(&graph)
        .plan
        .expect("fanout graph should lower");
    let adapters = plan
        .operations
        .iter()
        .filter_map(|operation| match &operation.kernel {
            PlannedKernel::Adapter(adapter) => Some((operation, adapter)),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        adapters
            .iter()
            .filter(|(_, adapter)| matches!(adapter, PlannedAdapter::Collect { .. }))
            .count(),
        1
    );

    let shared = adapters
        .iter()
        .find(|(_, adapter)| matches!(adapter, PlannedAdapter::Collect { .. }))
        .unwrap()
        .0;
    assert_eq!(
        plan.value_dependencies
            .iter()
            .filter(|dependency| dependency.source == shared.outputs[0].value)
            .count(),
        2,
        "the stable collected artifact is the single fanout owner"
    );
}

#[test]
fn compiler_streaming_fanout_with_different_contracts_is_permutation_stable() {
    let mut source_output = data_port("out", PortDirection::Output, TypeExpr::Unknown, None);
    source_output.production = Some(OutputProduction::Streaming);
    let source = test_protocol("fanout_source_mixed", vec![source_output], vec![], vec![]);

    let sink = |name: &str, consumption| {
        let mut input = data_port("in", PortDirection::Input, TypeExpr::Unknown, None);
        input.consumption = Some(consumption);
        let output = data_port("out", PortDirection::Output, TypeExpr::Unknown, None);
        test_protocol(name, vec![input, output], vec![], vec![])
    };
    let registry = TestRegistry::new(vec![
        source,
        sink("fanout_sink_stream", InputConsumption::Streaming),
        sink(
            "fanout_sink_materialized",
            InputConsumption::FullyMaterialized,
        ),
    ]);

    let compile = |nodes: &[(u128, &str)], source: u128, streaming: u128, materialized: u128| {
        let mut graph = graph_with_nodes(nodes);
        connect(&mut graph, 10, source, "out", streaming, "in");
        connect(&mut graph, 11, source, "out", materialized, "in");
        GraphCompiler::new(&registry, &Resources)
            .compile(&graph)
            .plan
            .expect("mixed fanout graph should lower")
    };
    let forward = compile(
        &[
            (1, "fanout_source_mixed"),
            (2, "fanout_sink_stream"),
            (3, "fanout_sink_materialized"),
        ],
        1,
        2,
        3,
    );
    let permuted = compile(
        &[
            (103, "fanout_sink_materialized"),
            (101, "fanout_source_mixed"),
            (102, "fanout_sink_stream"),
        ],
        101,
        102,
        103,
    );

    let normalize = |plan: &ExecutionPlan| {
        let kind = |operation: &PlannedOperation| match &operation.kernel {
            PlannedKernel::Native(_) => format!("native:{}", operation.source_node_type_id),
            PlannedKernel::Relational(_) => "relational".to_owned(),
            PlannedKernel::Adapter(adapter) => format!("adapter:{adapter:?}"),
        };
        let owners = plan
            .operations
            .iter()
            .enumerate()
            .flat_map(|(index, operation)| {
                operation
                    .outputs
                    .iter()
                    .map(move |output| (output.value, index))
            })
            .collect::<BTreeMap<_, _>>();
        let consumers = plan
            .operations
            .iter()
            .enumerate()
            .flat_map(|(index, operation)| {
                operation
                    .inputs
                    .iter()
                    .map(move |input| (input.value, index))
            })
            .collect::<BTreeMap<_, _>>();
        let mut operations = plan
            .operations
            .iter()
            .map(|operation| {
                (
                    kind(operation),
                    operation
                        .inputs
                        .iter()
                        .map(|input| input.consumption)
                        .collect::<Vec<_>>(),
                    operation
                        .outputs
                        .iter()
                        .map(|output| output.production)
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<Vec<_>>();
        operations.sort_by(|left, right| format!("{left:?}").cmp(&format!("{right:?}")));
        let mut topology = plan
            .value_dependencies
            .iter()
            .filter_map(|dependency| {
                Some((
                    kind(&plan.operations[*owners.get(&dependency.source)?]),
                    kind(&plan.operations[*consumers.get(&dependency.destination)?]),
                ))
            })
            .collect::<Vec<_>>();
        topology.sort();
        (operations, topology)
    };

    assert_eq!(normalize(&forward), normalize(&permuted));
    assert_ne!(
        forward
            .operations
            .iter()
            .map(|operation| &operation.stable_id)
            .collect::<Vec<_>>(),
        permuted
            .operations
            .iter()
            .map(|operation| &operation.stable_id)
            .collect::<Vec<_>>(),
        "Task 10 permits stable IDs to follow real node UUIDs"
    );
    for plan in [&forward, &permuted] {
        assert_eq!(
            plan.operations
                .iter()
                .filter(|operation| matches!(
                    operation.kernel,
                    PlannedKernel::Adapter(PlannedAdapter::Collect { .. })
                ))
                .count(),
            1
        );
        assert_eq!(
            plan.operations
                .iter()
                .filter(|operation| matches!(
                    operation.kernel,
                    PlannedKernel::Adapter(PlannedAdapter::StreamBridge { .. })
                ))
                .count(),
            1
        );
    }

    let mut demand_graph = graph_with_nodes(&[
        (1, "fanout_source_mixed"),
        (2, "fanout_sink_stream"),
        (3, "fanout_sink_materialized"),
    ]);
    connect(&mut demand_graph, 10, 1, "out", 2, "in");
    connect(&mut demand_graph, 11, 1, "out", 3, "in");
    let compiled = GraphCompiler::new(&registry, &Resources).compile(&demand_graph);
    let basis = compiled
        .execution_basis
        .expect("mixed fanout graph keeps a demand basis");
    let graph_path = basis.provenance.graph_path.0.clone();
    let specialized = basis
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([demand_output(&graph_path, 3, "out")]),
            include_default_results: false,
        })
        .expect("materialized fanout consumer specializes");
    assert_eq!(
        specialized
            .operations
            .iter()
            .filter(|operation| matches!(operation.kernel, PlannedKernel::Adapter(_)))
            .count(),
        1,
        "demand specialization replans one retained boundary without duplicating shared fanout"
    );
    assert!(specialized.operations.iter().any(|operation| matches!(
        operation.kernel,
        PlannedKernel::Adapter(PlannedAdapter::Collect { .. })
    )));
}

#[test]
fn compiler_maps_effect_edges_into_plan_dependencies() {
    let mut before = test_protocol(
        "plan_effect_before",
        vec![effect_port("effect", PortDirection::Output)],
        vec![],
        vec![],
    );
    before.execution.purity = Purity::Effectful;
    before.execution.effects = EffectSemantics::Ordered;
    let mut after = test_protocol(
        "plan_effect_after",
        vec![effect_port("effect", PortDirection::Input)],
        vec![],
        vec![],
    );
    after.execution.purity = Purity::Effectful;
    after.execution.effects = EffectSemantics::Ordered;
    let before_type = before.type_id.clone();
    let after_type = after.type_id.clone();
    let registry = TestRegistry::new(vec![before, after])
        .with_lowerer(
            &before_type,
            FragmentLowerer {
                fragment: kernel_fragment(EffectSemantics::Ordered, FragmentMetadata::default()),
            },
        )
        .with_lowerer(
            &after_type,
            FragmentLowerer {
                fragment: kernel_fragment(EffectSemantics::Ordered, FragmentMetadata::default()),
            },
        );
    let mut graph = graph_with_nodes(&[(1, "plan_effect_before"), (2, "plan_effect_after")]);
    connect(&mut graph, 10, 1, "effect", 2, "effect");

    let plan = GraphCompiler::new(&registry, &Resources)
        .compile(&graph)
        .plan
        .expect("effect graph should lower");

    assert_eq!(plan.effect_dependencies.len(), 1);
    assert_eq!(plan.effect_dependencies[0].before.index(), 0);
    assert_eq!(plan.effect_dependencies[0].after.index(), 1);
}

#[test]
fn compiler_plans_relational_islands_with_valid_local_indices() {
    let mut source_output = data_port("out", PortDirection::Output, TypeExpr::Unknown, None);
    source_output.production = Some(OutputProduction::Streaming);
    let source = test_protocol("plan_relation_source", vec![source_output], vec![], vec![]);
    let mut sink_input = data_port("in", PortDirection::Input, TypeExpr::Unknown, None);
    sink_input.consumption = Some(InputConsumption::Streaming);
    let mut sink_output = data_port("out", PortDirection::Output, TypeExpr::Unknown, None);
    sink_output.production = Some(OutputProduction::Streaming);
    let sink = test_protocol(
        "plan_relation_sink",
        vec![sink_input, sink_output],
        vec![],
        vec![],
    );
    let source_type = source.type_id.clone();
    let sink_type = sink.type_id.clone();
    let backend = RelationalBackendId::new("test.relational").unwrap();
    let registry = TestRegistry::new(vec![source, sink])
        .with_lowerer(
            &source_type,
            FragmentLowerer {
                fragment: LoweredKernel::Relational(RelationalNodeFragment {
                    backend: backend.clone(),
                    fragment: relational::RelationalFragment {
                        id: RelationalFragmentId::new("source").unwrap(),
                        operators: Box::new([RelationalOperator::Source {
                            resource: ResourceId::new("database.source").unwrap(),
                            relation: "items".into(),
                        }]),
                        root: RelationalOperatorIndex::new(0),
                    },
                    inputs: Box::new([]),
                    metadata: FragmentMetadata::default(),
                }),
            },
        )
        .with_lowerer(
            &sink_type,
            FragmentLowerer {
                fragment: LoweredKernel::Relational(RelationalNodeFragment {
                    backend,
                    fragment: relational::RelationalFragment {
                        id: RelationalFragmentId::new("sink").unwrap(),
                        operators: Box::new([
                            RelationalOperator::Input {
                                name: "input".into(),
                            },
                            RelationalOperator::Limit {
                                input: RelationalOperatorIndex::new(0),
                                rows: 10,
                            },
                        ]),
                        root: RelationalOperatorIndex::new(1),
                    },
                    inputs: Box::new([RelationalInputBinding {
                        port: PortAddress::declared(node_id(2), key("in")),
                        operator: RelationalOperatorIndex::new(0),
                    }]),
                    metadata: FragmentMetadata::default(),
                }),
            },
        );
    let mut graph = graph_with_nodes(&[(1, "plan_relation_source"), (2, "plan_relation_sink")]);
    connect(&mut graph, 10, 1, "out", 2, "in");

    let compiler = GraphCompiler::new(&registry, &Resources);
    let snapshot = compiler.snapshot(GraphResourcePath("events/relational".into()), &graph);
    let result = compiler
        .compile_snapshot(&snapshot, &CompileCancellationToken::new())
        .unwrap();
    let basis = result
        .execution_basis
        .expect("relational graph should retain pre-group facts");
    let plan = result.plan.expect("relational graph should lower");

    assert_eq!(plan.relational_subplans.len(), 1);
    let subplan = &plan.relational_subplans[0];
    assert_eq!(subplan.compiled_plan.fragment_order.len(), 2);
    assert_eq!(subplan.compiled_plan.roots.len(), 1);

    assert_eq!(
        plan.operations.len(),
        1,
        "one operation must own the island"
    );
    let operation = &plan.operations[0];
    assert!(matches!(
        operation.kernel,
        crate::node_system::plan::PlannedKernel::Relational(index) if index.index() == 0
    ));
    assert!(
        operation.inputs.is_empty(),
        "the source makes the island self-contained"
    );
    assert_eq!(operation.outputs.len(), 1, "only the sink root is exposed");
    assert_eq!(operation.outputs[0].production, OutputProduction::Streaming);
    assert!(
        plan.value_dependencies.is_empty(),
        "an internal fragment edge must not make the island depend on itself"
    );
    assert!(matches!(
        plan.root_region,
        crate::node_system::plan::StructuredControlRegion::Sequence(ref steps)
            if matches!(steps.as_ref(), [crate::node_system::plan::ControlStep::Operation(index)] if index.index() == 0)
    ));

    assert_eq!(basis.operations.len(), 2, "basis remains pre-group");
    assert_eq!(basis.relational_connections.len(), 1);
    assert!(matches!(
        &basis.operations[0].kernel,
        super::super::specialization::IntermediateKernel::Relational { fragment, .. }
            if fragment.id.as_str() == "source"
    ));
    assert!(matches!(
        &basis.operations[1].kernel,
        super::super::specialization::IntermediateKernel::Relational { fragment, input_bindings, .. }
            if fragment.id.as_str() == "sink" && input_bindings.len() == 1
    ));

    let requested_source = basis
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([demand_output("events/relational", 1, "out")]),
            include_default_results: false,
        })
        .expect("same-island intermediate output must be derivable");
    assert_eq!(requested_source.operations.len(), 1);
    assert_eq!(requested_source.operations[0].outputs.len(), 1);
    assert_ne!(
        requested_source.operations[0].stable_id, plan.operations[0].stable_id,
        "different demand/member combinations need different composite IDs"
    );
    assert_ne!(
        requested_source.operations[0].semantics_version, plan.operations[0].semantics_version,
        "different fused relational semantics need different versions"
    );
    assert!(basis.operations.iter().all(|member| {
        member.stable_id != plan.operations[0].stable_id
            && member.semantics_version != plan.operations[0].semantics_version
    }));
    let requested_source_again = basis
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([demand_output("events/relational", 1, "out")]),
            include_default_results: false,
        })
        .unwrap();
    assert_eq!(
        requested_source.operations[0].stable_id,
        requested_source_again.operations[0].stable_id
    );
    assert_eq!(
        requested_source.operations[0].semantics_version,
        requested_source_again.operations[0].semantics_version
    );
    assert_eq!(requested_source.relational_subplans.len(), 1);
    assert_eq!(
        requested_source.relational_subplans[0]
            .compiled_plan
            .fragment_order
            .as_ref(),
        &[RelationalFragmentId::new("source").unwrap()]
    );
    assert_eq!(
        requested_source.relational_subplans[0]
            .compiled_plan
            .roots
            .len(),
        requested_source.operations[0].outputs.len()
    );
    requested_source
        .validate()
        .expect("requested relational output plan validates");

    let mut retry_basis = basis.clone();
    let retry = PlannedRetry {
        idempotent: true,
        policy: Some(
            RetryPolicy::new(
                std::num::NonZeroU32::new(2).unwrap(),
                std::time::Duration::from_millis(1),
                std::time::Duration::from_millis(2),
            )
            .unwrap(),
        ),
    };
    retry_basis.operations[0].retry = retry.clone();
    retry_basis.operations[1].retry = retry.clone();
    retry_basis.operations[0].semantics_version = ExecutionSemanticsVersion::from_bytes([1; 32]);
    retry_basis.operations[1].semantics_version = ExecutionSemanticsVersion::from_bytes([2; 32]);
    assert!(matches!(
        retry_basis.derive_full_plan().unwrap_err(),
        DemandPlanError::InvalidDerivedPlan(message)
            if message.contains("InvalidRetryPolicy")
    ));

    retry_basis.operations[0].retry = PlannedRetry::default();
    retry_basis.operations[1].retry = PlannedRetry::default();
    let conservative_retry = retry_basis.derive_full_plan().unwrap();
    assert_eq!(
        conservative_retry.operations[0].retry,
        PlannedRetry::default()
    );
    assert_ne!(
        conservative_retry.operations[0].semantics_version,
        retry_basis.operations[0].semantics_version
    );
    assert_ne!(
        conservative_retry.operations[0].semantics_version,
        retry_basis.operations[1].semantics_version
    );

    let mut reversed = graph_with_nodes(&[(2, "plan_relation_sink"), (1, "plan_relation_source")]);
    connect(&mut reversed, 10, 1, "out", 2, "in");
    let reversed_compiler = GraphCompiler::new(&registry, &Resources);
    let reversed_plan = reversed_compiler
        .compile_snapshot(
            &reversed_compiler.snapshot(GraphResourcePath("events/relational".into()), &reversed),
            &CompileCancellationToken::new(),
        )
        .unwrap()
        .plan
        .expect("reordered relational graph should lower");
    assert_eq!(reversed_plan.operations, plan.operations);
    assert_eq!(
        reversed_plan.operations[0].stable_id, plan.operations[0].stable_id,
        "composite identity is insertion-order independent"
    );
    assert_eq!(reversed_plan.value_dependencies, plan.value_dependencies);
    assert_eq!(reversed_plan.root_region, plan.root_region);
    assert_eq!(reversed_plan.relational_subplans, plan.relational_subplans);
}

#[test]
fn duplicate_lowering_result_emits_the_result_name() {
    let first = test_protocol(
        "duplicate_result_first",
        vec![data_port(
            "out",
            PortDirection::Output,
            TypeExpr::Unknown,
            None,
        )],
        vec![],
        vec![],
    );
    let second = test_protocol(
        "duplicate_result_second",
        vec![data_port(
            "out",
            PortDirection::Output,
            TypeExpr::Unknown,
            None,
        )],
        vec![],
        vec![],
    );
    let first_type = first.type_id.clone();
    let second_type = second.type_id.clone();
    let registry = TestRegistry::new(vec![first, second])
        .with_lowerer(
            &first_type,
            FragmentLowerer {
                fragment: kernel_fragment(
                    EffectSemantics::None,
                    FragmentMetadata {
                        effect: EffectSemantics::None,
                        resources: Box::new([]),
                        results: Box::new([FragmentResult {
                            name: "answer".into(),
                            output: PortAddress::declared(node_id(1), key("out")),
                        }]),
                    },
                ),
            },
        )
        .with_lowerer(
            &second_type,
            FragmentLowerer {
                fragment: kernel_fragment(
                    EffectSemantics::None,
                    FragmentMetadata {
                        effect: EffectSemantics::None,
                        resources: Box::new([]),
                        results: Box::new([FragmentResult {
                            name: "answer".into(),
                            output: PortAddress::declared(node_id(2), key("out")),
                        }]),
                    },
                ),
            },
        );

    let result = GraphCompiler::new(&registry, &Resources).compile(&graph_with_nodes(&[
        (1, "duplicate_result_first"),
        (2, "duplicate_result_second"),
    ]));

    assert!(result.semantic.is_some());
    assert!(result.plan.is_none());
    assert!(result.analysis.diagnostics.is_empty());
    assert!(matches!(
        result.outcome,
        CompilationOutcome::InternalFailure(ref failure)
            if failure.stage == CompilationStage::Lowering
                && failure.code.as_ref() == "compiler.lowering.result_duplicate"
                && failure.node_id == Some(node_id(2))
    ));
}

#[test]
fn finalization_rejects_public_output_that_conflicts_with_exact_port_facts() {
    let (registry, graph) = demand_fixture();
    let result = GraphCompiler::new(&registry, &Resources).compile(&graph);
    let mut basis = result.execution_basis.expect("demand fixture must lower");
    let operation = basis
        .operations
        .iter_mut()
        .find(|operation| operation.source_node_id == node_id(2))
        .unwrap();
    operation.outputs[0].public_output.as_mut().unwrap().port =
        PortAddress::declared(node_id(2), key("in"));

    let error = basis
        .derive_full_plan()
        .expect_err("input/non-output public identity must be rejected");
    assert!(matches!(error, DemandPlanError::InvalidDerivedPlan(_)));
}

#[test]
fn compiler_inserts_explicit_materialization_adapter_for_relational_boundary() {
    let mut source_output = data_port("out", PortDirection::Output, TypeExpr::Unknown, None);
    source_output.production = Some(OutputProduction::Streaming);
    let source = test_protocol("plan_bridge_source", vec![source_output], vec![], vec![]);
    let mut sink_input = data_port("in", PortDirection::Input, TypeExpr::Unknown, None);
    sink_input.consumption = Some(InputConsumption::FullyMaterialized);
    let sink_output = data_port("out", PortDirection::Output, TypeExpr::Unknown, None);
    let sink = test_protocol(
        "plan_bridge_sink",
        vec![sink_input, sink_output],
        vec![],
        vec![],
    );
    let condition = test_protocol(
        "pruned_bridge_condition",
        vec![data_port(
            "out",
            PortDirection::Output,
            TypeExpr::Unknown,
            None,
        )],
        vec![],
        vec![],
    );
    let mut then_source = data_port("then_source", PortDirection::Input, TypeExpr::Unknown, None);
    then_source.instances = PortInstances::UserCreated { min: 0, max: None };
    let mut else_source = data_port("else_source", PortDirection::Input, TypeExpr::Unknown, None);
    else_source.instances = PortInstances::UserCreated { min: 0, max: None };
    let mut branch_result = data_port("result", PortDirection::Output, TypeExpr::Unknown, None);
    branch_result.instances = PortInstances::UserCreated { min: 0, max: None };
    let mut branch = structural_protocol(
        "pruned_bridge_branch",
        vec![
            control_port("enter", PortDirection::Input),
            data_port("condition", PortDirection::Input, TypeExpr::Unknown, None),
            then_source,
            else_source,
            control_port("true", PortDirection::Output),
            control_port("false", PortDirection::Output),
            branch_result,
        ],
        vec![],
    );
    branch.interface = branch
        .interface
        .with_member_groups(vec![PortMemberGroupSpec {
            templates: vec![key("then_source"), key("else_source"), key("result")]
                .into_boxed_slice(),
            min: 0,
            max: None,
        }])
        .unwrap();
    let source_type = source.type_id.clone();
    let sink_type = sink.type_id.clone();
    let branch_type = branch.type_id.clone();
    let backend = RelationalBackendId::new("test.relational").unwrap();
    let registry = TestRegistry::new(vec![source, sink, condition, branch])
        .structural(&branch_type, StructuralNodeRole::Branch)
        .with_lowerer(
            &source_type,
            FragmentLowerer {
                fragment: LoweredKernel::Relational(RelationalNodeFragment {
                    backend: backend.clone(),
                    fragment: relational::RelationalFragment {
                        id: RelationalFragmentId::new("bridge-source").unwrap(),
                        operators: Box::new([RelationalOperator::Source {
                            resource: ResourceId::new("database.source").unwrap(),
                            relation: "items".into(),
                        }]),
                        root: RelationalOperatorIndex::new(0),
                    },
                    inputs: Box::new([]),
                    metadata: FragmentMetadata::default(),
                }),
            },
        )
        .with_lowerer(
            &sink_type,
            FragmentLowerer {
                fragment: LoweredKernel::Relational(RelationalNodeFragment {
                    backend,
                    fragment: relational::RelationalFragment {
                        id: RelationalFragmentId::new("bridge-sink").unwrap(),
                        operators: Box::new([RelationalOperator::Input {
                            name: "input".into(),
                        }]),
                        root: RelationalOperatorIndex::new(0),
                    },
                    inputs: Box::new([RelationalInputBinding {
                        port: PortAddress::declared(node_id(2), key("in")),
                        operator: RelationalOperatorIndex::new(0),
                    }]),
                    metadata: FragmentMetadata::default(),
                }),
            },
        );
    let mut graph = graph_with_nodes(&[
        (1, "plan_bridge_source"),
        (2, "plan_bridge_sink"),
        (3, "pruned_bridge_condition"),
        (4, "pruned_bridge_branch"),
    ]);
    connect(&mut graph, 10, 1, "out", 2, "in");
    connect(&mut graph, 11, 3, "out", 4, "condition");

    let compiler = GraphCompiler::new(&registry, &Resources);
    let snapshot = compiler.snapshot(GraphResourcePath("events/bridge-demand".into()), &graph);
    let result = compiler
        .compile_snapshot(&snapshot, &CompileCancellationToken::new())
        .unwrap();
    let specialized = result
        .execution_basis
        .as_ref()
        .unwrap_or_else(|| panic!("adapter diagnostics: {:?}", result.analysis.diagnostics))
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([demand_output("events/bridge-demand", 2, "out")]),
            include_default_results: false,
        })
        .expect("retained relational adapter boundary specializes after structured pruning");
    assert_eq!(specialized.operations.len(), 3);
    assert_eq!(specialized.relational_subplans.len(), 2);

    let adapters = specialized
        .operations
        .iter()
        .filter_map(|operation| match &operation.kernel {
            PlannedKernel::Adapter(adapter) => Some((operation, adapter)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(adapters.len(), 1);
    assert!(matches!(adapters[0].1, PlannedAdapter::Collect { .. }));
    assert_eq!(adapters[0].0.workload, WorkloadClass::AdapterIo);
    assert_eq!(adapters[0].0.cache_policy, CachePolicy::Disabled);
    assert_eq!(adapters[0].0.inputs.len(), 1);
    assert_eq!(
        adapters[0].0.inputs[0].consumption,
        InputConsumption::Streaming
    );
    assert_eq!(adapters[0].0.outputs.len(), 1);
    assert_eq!(
        adapters[0].0.outputs[0].production,
        OutputProduction::FullyMaterialized
    );
    assert_eq!(specialized.value_dependencies.len(), 2);
    for operation in &specialized.operations {
        if let crate::node_system::plan::PlannedKernel::Relational(subplan) = operation.kernel {
            assert_eq!(
                operation.outputs.len(),
                specialized.relational_subplans[subplan.index()]
                    .compiled_plan
                    .roots
                    .len(),
                "relational owner outputs and compiled roots keep exact cardinality"
            );
        }
    }
    specialized.validate().unwrap();

    let mut reversed = graph_with_nodes(&[
        (4, "pruned_bridge_branch"),
        (3, "pruned_bridge_condition"),
        (2, "plan_bridge_sink"),
        (1, "plan_bridge_source"),
    ]);
    connect(&mut reversed, 11, 3, "out", 4, "condition");
    connect(&mut reversed, 10, 1, "out", 2, "in");
    let reversed_snapshot =
        compiler.snapshot(GraphResourcePath("events/bridge-demand".into()), &reversed);
    let reversed_result = compiler
        .compile_snapshot(&reversed_snapshot, &CompileCancellationToken::new())
        .unwrap();
    let reversed_specialized = reversed_result
        .execution_basis
        .expect("reversed bridge graph has a specialization basis")
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([demand_output("events/bridge-demand", 2, "out")]),
            include_default_results: false,
        })
        .unwrap();
    assert_eq!(specialized.operations, reversed_specialized.operations);
    assert_eq!(
        specialized.relational_subplans,
        reversed_specialized.relational_subplans
    );
    assert_eq!(specialized.root_region, reversed_specialized.root_region);

    let plan = result.plan.expect("bridge graph should lower");

    assert_eq!(plan.relational_subplans.len(), 2);
    assert_eq!(
        plan.operations
            .iter()
            .filter(|operation| matches!(operation.kernel, PlannedKernel::Adapter(_)))
            .count(),
        1
    );

    let adapter_index = plan
        .operations
        .iter()
        .position(|operation| matches!(operation.kernel, PlannedKernel::Adapter(_)))
        .unwrap();
    let adapter = &plan.operations[adapter_index];
    let incoming = plan
        .value_dependencies
        .iter()
        .find(|dependency| dependency.destination == adapter.inputs[0].value)
        .copied()
        .unwrap();
    let outgoing = plan
        .value_dependencies
        .iter()
        .find(|dependency| dependency.source == adapter.outputs[0].value)
        .copied()
        .unwrap();

    let mut missing = plan.clone();
    missing.operations = missing
        .operations
        .into_vec()
        .into_iter()
        .enumerate()
        .filter_map(|(index, operation)| (index != adapter_index).then_some(operation))
        .collect();
    missing.value_dependencies = Box::new([crate::node_system::plan::ValueDependency {
        source: incoming.source,
        destination: outgoing.destination,
    }]);
    let relational_operations = missing
        .operations
        .iter()
        .enumerate()
        .filter_map(|(index, operation)| {
            matches!(operation.kernel, PlannedKernel::Relational(_)).then_some(
                ControlStep::Operation(crate::node_system::plan::OperationIndex::new(index as u32)),
            )
        })
        .collect::<Vec<_>>();
    missing.root_region =
        StructuredControlRegion::Sequence(relational_operations.into_boxed_slice());
    assert!(
        missing
            .validate()
            .unwrap_err()
            .0
            .iter()
            .any(|error| matches!(
                error,
                PlanValidationError::MissingMaterializationAdapter { .. }
            ))
    );

    let mut extra = plan.clone();
    let mut extra_adapter = extra.operations[adapter_index].clone();
    extra_adapter.stable_id =
        crate::node_system::plan::OperationStableId::new("test.extra.materialization.adapter")
            .unwrap();
    extra_adapter.inputs[0].value = ValueRef::new(extra.value_count);
    extra.value_count += 1;
    extra_adapter.outputs[0].value = ValueRef::new(extra.value_count);
    extra.value_count += 1;
    extra.operations = extra
        .operations
        .into_vec()
        .into_iter()
        .chain([extra_adapter])
        .collect();
    assert!(extra.validate().unwrap_err().0.iter().any(|error| matches!(
        error,
        PlanValidationError::ExtraMaterializationAdapter { .. }
    )));

    let mut incompatible = plan.clone();
    incompatible.operations[adapter_index].kernel =
        PlannedKernel::Adapter(PlannedAdapter::Buffer { capacity: 1 });
    assert!(
        incompatible
            .validate()
            .unwrap_err()
            .0
            .iter()
            .any(|error| matches!(
                error,
                PlanValidationError::IncompatibleMaterializationAdapter { .. }
            ))
    );
}
