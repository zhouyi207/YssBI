use super::*;

#[test]
fn execution_error_retains_typed_internal_compilation_failure() {
    let failure = crate::node_system::compiler::InternalCompilationFailure {
        stage: crate::node_system::compiler::CompilationStage::Lowering,
        code: "compiler.lowering.internal_invariant".into(),
        node_id: Some(NodeId::from_uuid(uuid::Uuid::from_u128(42))),
    };

    let error = ProjectExecutionError::internal_compilation(failure.clone());

    assert_eq!(error.internal_compilation_failure(), Some(&failure));
    assert!(error.run_error().is_none());
    assert_eq!(
        error.to_string(),
        "internal compilation failure at Lowering: compiler.lowering.internal_invariant (node 00000000-0000-0000-0000-00000000002a)"
    );
}

#[test]
fn project_mutation_rejects_stale_revision_and_records_undo_history() {
    let state = state_with_empty_graph();
    let inserted = node("yssbi.constant.int64");
    let patch = GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode {
        node: inserted.clone(),
    }]);
    let request = MutationRequest::new(
        ResourceKey::Graph(document_path()),
        GraphRevision::INITIAL,
        OperationId::new(),
        patch,
    );

    let event = state
        .apply_graph_patch(&graph_path(), request.clone())
        .unwrap();
    assert_eq!(event.from_revision, GraphRevision::INITIAL);
    assert_eq!(event.to_revision, GraphRevision::new(1));
    assert!(matches!(
        state.apply_graph_patch(&graph_path(), request),
        Err(MutationConflict::StaleRevision { .. })
    ));

    state
        .undo_last_transaction_observed(
            &current_project_instance_id(&state),
            "en-US",
            MutationRequest::new(
                ResourceKey::Graph(document_path()),
                GraphRevision::new(1),
                OperationId::new(),
                HistoryMutation {},
            ),
            |_| {},
        )
        .unwrap();
    let graph = state
        .get_data()
        .unwrap()
        .graphs
        .remove(&graph_path())
        .unwrap();
    assert!(graph.document.nodes.is_empty());
    assert_eq!(graph.document.revision, GraphRevision::new(2));
}

#[test]
fn project_projection_hydrates_localized_editor_dto() {
    let state = state_with_empty_graph();
    let inserted = node("yssbi.constant.int64");
    let patch = GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode {
        node: inserted.clone(),
    }]);
    state
        .apply_graph_patch(
            &graph_path(),
            MutationRequest::new(
                ResourceKey::Graph(document_path()),
                GraphRevision::INITIAL,
                OperationId::new(),
                patch,
            ),
        )
        .unwrap();

    let projection = state.graph_projection(&graph_path(), "zh-CN").unwrap();
    assert_eq!(projection.graph_path.as_ref(), graph_path().as_str());
    assert_eq!(projection.source_revision, 1);
    assert_eq!(projection.nodes.len(), 1);
    assert_eq!(
        projection.nodes[0].node_id.as_ref(),
        inserted.id.to_string()
    );
    assert!(!projection.nodes[0].display.title.is_empty());
}

#[test]
fn project_execution_publishes_persisted_function_plans() {
    let root = std::env::temp_dir().join(format!(
        "yssbi-production-functions-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    crate::project::fixtures::write_project(&ProjectData::new(), root.to_string_lossy().as_ref())
        .unwrap();
    let state = ProjectState::new();
    state.activate_project_fixture(root.to_string_lossy().into_owned(), ProjectData::new());
    let event = state
        .create_graph_resource_fixture("Main", GraphDocumentKind::Event)
        .unwrap();
    load_graph(&state, &event).unwrap();
    let function = state
        .create_graph_resource_fixture("Helper", GraphDocumentKind::Function)
        .unwrap();
    let begin = state.get_data().unwrap().graphs[&event]
        .document
        .nodes
        .values()
        .find(|node| node.node_type.as_str() == "yssbi.project.event.begin")
        .unwrap()
        .id;
    let mut call = node("yssbi.project.function.call");
    call.parameters.insert(
        crate::node_system::protocol::ParameterKey::new("target").unwrap(),
        serde_json::json!(function.as_str()),
    );
    let connection_id = crate::node_system::document::ConnectionId::new();
    let connection = crate::node_system::document::DocumentConnection {
        id: connection_id,
        output: crate::node_system::document::PortAddress::declared(
            begin,
            crate::node_system::protocol::PortKey::new("then").unwrap(),
        ),
        input: crate::node_system::document::PortAddress::declared(
            call.id,
            crate::node_system::protocol::PortKey::new("enter").unwrap(),
        ),
        order: None,
    };
    state
        .apply_graph_patch(
            &event,
            MutationRequest::new(
                crate::node_system::document::ResourceKey::Graph(
                    crate::node_system::document::GraphResourcePath(event.as_str().into()),
                ),
                GraphRevision::INITIAL,
                OperationId::new(),
                GraphDocumentPatch::new(vec![
                    GraphDocumentOperation::InsertNode { node: call },
                    GraphDocumentOperation::InsertConnection { connection },
                ]),
            ),
        )
        .unwrap();

    state
        .execute_graph_for_current_project_for_test(
            &event,
            &crate::node_system::plan::ExecutionDemand::Default,
            &NOOP_RUN_EVENT_SINK,
        )
        .unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn project_execution_uses_replaced_persisted_function_body_and_current_generation() {
    use crate::node_system::analysis::ResourceKey as AnalysisResourceKey;
    use crate::node_system::document::{
        ConnectionId, DocumentConnection, DynamicMemberLocator, DynamicPortBinding,
        FunctionDocument, FunctionParameter, FunctionParameterId, FunctionSignature, OrderKey,
        PortAddress, PortInstanceId,
    };
    use crate::node_system::protocol::{ParameterKey, PortKey};

    let state = ProjectState::new();
    let function_path = GraphResourcePath::new("functions/Current.yssbi-function").unwrap();
    let event_path = GraphResourcePath::new("events/CurrentCaller.yssbi-event").unwrap();
    let parameter_id = FunctionParameterId("amount".into());
    let return_id = FunctionParameterId("return".into());
    let mut input_variable = test_variable("Input");
    input_variable.data_value = crate::graph::value::DataValue::Int64(41);
    let mut first_offset = test_variable("First Offset");
    first_offset.data_value = crate::graph::value::DataValue::Int64(1);
    let mut second_offset = test_variable("Second Offset");
    second_offset.data_value = crate::graph::value::DataValue::Int64(2);
    let mut output_variable = test_variable("Output");
    output_variable.data_value = crate::graph::value::DataValue::Int64(0);
    let mut project = ProjectData::new();
    for variable in [
        input_variable.clone(),
        first_offset.clone(),
        second_offset.clone(),
        output_variable.clone(),
    ] {
        project.variables.insert(variable.id, variable);
    }
    let root = std::env::temp_dir().join(format!(
        "yssbi-structured-control-round2-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    crate::project::fixtures::write_project(&project, root.to_string_lossy().as_ref()).unwrap();
    state.activate_project_fixture(root.to_string_lossy().into_owned(), project);

    let port = |node_id, template: &str, instance: u128| {
        PortAddress::instance(
            node_id,
            PortKey::new(template).unwrap(),
            PortInstanceId::from_uuid(uuid::Uuid::from_u128(instance)),
        )
    };
    let binding = |parameter: &FunctionParameterId, order: &str| DynamicPortBinding::Resolved {
        origin: DynamicMemberLocator::FunctionParameter {
            function: crate::node_system::document::GraphResourcePath(
                function_path.as_str().into(),
            ),
            parameter: parameter.clone(),
        },
        order: OrderKey(order.into()),
        last_known: crate::node_system::document::LastKnownPortMetadata::default(),
    };
    let connection = |output: PortAddress, input: PortAddress| DocumentConnection {
        id: ConnectionId::new(),
        output,
        input,
        order: None,
    };
    let declared = |node_id, key: &str| PortAddress::declared(node_id, PortKey::new(key).unwrap());

    let mut function = GraphResourceDocument::new("Current", GraphDocumentKind::Function);
    function.function = Some(FunctionDocument::new(FunctionSignature {
        parameters: vec![FunctionParameter {
            id: parameter_id.clone(),
            name: "Amount".into(),
            type_name: "Int64".into(),
        }],
        return_type: Some("Int64".into()),
    }));
    let mut entry = node("yssbi.project.function.entry");
    entry.parameters.insert(
        ParameterKey::new("function").unwrap(),
        serde_json::json!(function_path.as_str()),
    );
    let mut return_node = node("yssbi.project.function.return");
    return_node.parameters.insert(
        ParameterKey::new("function").unwrap(),
        serde_json::json!(function_path.as_str()),
    );
    let body = node("yssbi.numeric.add.int64");
    let mut first_offset_source = node("yssbi.project.variable.get");
    first_offset_source.parameters.insert(
        ParameterKey::new("variable").unwrap(),
        serde_json::json!(format!("variables/{}", first_offset.id)),
    );
    let mut second_offset_source = node("yssbi.project.variable.get");
    second_offset_source.parameters.insert(
        ParameterKey::new("variable").unwrap(),
        serde_json::json!(format!("variables/{}", second_offset.id)),
    );
    let entry_parameter = port(entry.id, "parameters", 1);
    let return_result = port(return_node.id, "results", 2);
    function
        .document
        .port_bindings
        .insert(entry_parameter.clone(), binding(&parameter_id, "a"));
    function
        .document
        .port_bindings
        .insert(return_result.clone(), binding(&return_id, "b"));
    let body_offset_connection = connection(
        declared(first_offset_source.id, "value"),
        declared(body.id, "right"),
    );
    function.document.connections = [
        connection(
            declared(entry.id, "then"),
            declared(return_node.id, "enter"),
        ),
        connection(entry_parameter.clone(), declared(body.id, "left")),
        body_offset_connection.clone(),
        connection(declared(body.id, "result"), return_result.clone()),
    ]
    .into_iter()
    .map(|connection| (connection.id, connection))
    .collect();
    function.document.nodes = [
        entry.clone(),
        body.clone(),
        first_offset_source.clone(),
        second_offset_source.clone(),
        return_node.clone(),
    ]
    .into_iter()
    .map(|node| (node.id, node))
    .collect();
    state.insert_graph(function_path.clone(), function).unwrap();

    let mut event = GraphResourceDocument::new("Current Caller", GraphDocumentKind::Event);
    let begin = node("yssbi.project.event.begin");
    let mut call = node("yssbi.project.function.call");
    call.parameters.insert(
        ParameterKey::new("target").unwrap(),
        serde_json::json!(function_path.as_str()),
    );
    let mut source = node("yssbi.project.variable.get");
    source.parameters.insert(
        ParameterKey::new("variable").unwrap(),
        serde_json::json!(format!("variables/{}", input_variable.id)),
    );
    let mut output = node("yssbi.project.variable.set");
    output.parameters.insert(
        ParameterKey::new("variable").unwrap(),
        serde_json::json!(format!("variables/{}", output_variable.id)),
    );
    let call_argument = port(call.id, "arguments", 3);
    let call_result = port(call.id, "results", 4);
    event
        .document
        .port_bindings
        .insert(call_argument.clone(), binding(&parameter_id, "a"));
    event
        .document
        .port_bindings
        .insert(call_result.clone(), binding(&return_id, "b"));
    let event_connections = [
        connection(declared(begin.id, "then"), declared(call.id, "enter")),
        connection(declared(call.id, "then"), declared(output.id, "enter")),
        connection(declared(source.id, "value"), call_argument.clone()),
        connection(call_result, declared(output.id, "value")),
    ];
    event.document.connections = event_connections
        .into_iter()
        .map(|connection| (connection.id, connection))
        .collect();
    event.document.nodes = [begin, source, call, output]
        .into_iter()
        .map(|node| (node.id, node))
        .collect();
    state.insert_graph(event_path.clone(), event).unwrap();
    crate::project::fixtures::write_state_graph(&state, &function_path).unwrap();
    crate::project::fixtures::write_state_graph(&state, &event_path).unwrap();

    let data = state.get_data().unwrap();
    let resources =
        super::project_state::compile_resources_from_data(&data, Default::default()).unwrap();
    let registry = crate::node_system::catalog::build_builtin_node_system()
        .unwrap()
        .registry;
    let compiler = crate::node_system::compiler::GraphCompiler::with_interface_resolvers(
        registry.as_ref(),
        &resources,
        crate::node_system::compiler::build_builtin_interface_resolvers(),
    );
    let function_graph = &data.graphs[&function_path].document;
    let products = compiler
        .compile_snapshot(
            &compiler.snapshot(
                crate::node_system::document::GraphResourcePath(function_path.as_str().into()),
                function_graph,
            ),
            &crate::node_system::compiler::CompileCancellationToken::new(),
        )
        .unwrap();
    let diagnostic_codes = products
        .analysis
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect::<Vec<_>>();
    let plan_error = products
        .execution_basis
        .as_ref()
        .and_then(|basis| basis.derive_full_plan().err());
    assert!(
        products.plan.is_some(),
        "persisted function diagnostics: {diagnostic_codes:?}; outcome: {:?}; plan error: {plan_error:?}",
        products.outcome
    );
    let event_graph = &data.graphs[&event_path].document;
    let event_products = compiler
        .compile_snapshot(
            &compiler.snapshot(
                crate::node_system::document::GraphResourcePath(event_path.as_str().into()),
                event_graph,
            ),
            &crate::node_system::compiler::CompileCancellationToken::new(),
        )
        .unwrap();
    let event_diagnostics = event_products
        .analysis
        .diagnostics
        .iter()
        .map(|diagnostic| format!("{diagnostic:?}"))
        .collect::<Vec<_>>();
    assert!(
        event_products.plan.is_some(),
        "persisted event diagnostics: {event_diagnostics:#?}"
    );
    drop(data);

    let first = state
        .execute_graph_for_current_project_for_test(
            &event_path,
            &crate::node_system::plan::ExecutionDemand::Default,
            &NOOP_RUN_EVENT_SINK,
        )
        .unwrap();
    let first_version = first.provenance.basis.resource_versions
        [&AnalysisResourceKey::new(function_path.as_str())]
        .clone();
    assert_eq!(
        state.get_data().unwrap().variables[&output_variable.id].data_value,
        crate::graph::value::DataValue::Int64(42)
    );
    state
        .apply_graph_patch(
            &function_path,
            MutationRequest::new(
                ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
                    function_path.as_str().into(),
                )),
                GraphRevision::INITIAL,
                OperationId::new(),
                GraphDocumentPatch::new(vec![
                    GraphDocumentOperation::RemoveConnection {
                        connection: body_offset_connection,
                    },
                    GraphDocumentOperation::InsertConnection {
                        connection: connection(
                            declared(second_offset_source.id, "value"),
                            declared(body.id, "right"),
                        ),
                    },
                ]),
            ),
        )
        .unwrap();
    let second = state
        .execute_graph_for_current_project_for_test(
            &event_path,
            &crate::node_system::plan::ExecutionDemand::Default,
            &NOOP_RUN_EVENT_SINK,
        )
        .unwrap();
    let second_version = &second.provenance.basis.resource_versions
        [&AnalysisResourceKey::new(function_path.as_str())];
    assert_ne!(&first_version, second_version);
    assert_ne!(first.provenance.compile_id, second.provenance.compile_id);
    assert_eq!(
        state.get_data().unwrap().variables[&output_variable.id].data_value,
        crate::graph::value::DataValue::Int64(43)
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn reversed_persisted_function_insertion_publishes_equivalent_callable_generation() {
    use crate::node_system::document::{
        ConnectionId, DocumentConnection, DynamicMemberLocator, DynamicPortBinding,
        FunctionDocument, FunctionParameter, FunctionParameterId, FunctionSignature, OrderKey,
        PortAddress, PortInstanceId,
    };
    use crate::node_system::plan::FunctionPlanHandle;
    use crate::node_system::protocol::{ParameterKey, PortKey};
    use crate::node_system::runtime::FunctionPlanProvider;

    let path_a = GraphResourcePath::new("functions/A.yssbi-function").unwrap();
    let path_b = GraphResourcePath::new("functions/B.yssbi-function").unwrap();
    let event_path = GraphResourcePath::new("events/Chain.yssbi-event").unwrap();
    let parameter_id = FunctionParameterId("amount".into());
    let return_id = FunctionParameterId("return".into());
    let port = |node_id, template: &str, instance: u128| {
        PortAddress::instance(
            node_id,
            PortKey::new(template).unwrap(),
            PortInstanceId::from_uuid(uuid::Uuid::from_u128(instance)),
        )
    };
    let declared = |node_id, key: &str| PortAddress::declared(node_id, PortKey::new(key).unwrap());
    let connection = |output: PortAddress, input: PortAddress| DocumentConnection {
        id: ConnectionId::new(),
        output,
        input,
        order: None,
    };
    let binding = |path: &GraphResourcePath, parameter: &FunctionParameterId, order: &str| {
        DynamicPortBinding::Resolved {
            origin: DynamicMemberLocator::FunctionParameter {
                function: crate::node_system::document::GraphResourcePath(path.as_str().into()),
                parameter: parameter.clone(),
            },
            order: OrderKey(order.into()),
            last_known: crate::node_system::document::LastKnownPortMetadata::default(),
        }
    };
    let signature = || FunctionSignature {
        parameters: vec![FunctionParameter {
            id: parameter_id.clone(),
            name: "Amount".into(),
            type_name: "Int64".into(),
        }],
        return_type: Some("Int64".into()),
    };
    let make_function = |path: &GraphResourcePath,
                         target: Option<&GraphResourcePath>,
                         instance_base: u128| {
        let mut resource = GraphResourceDocument::new(path.as_str(), GraphDocumentKind::Function);
        resource.function = Some(FunctionDocument::new(signature()));
        let mut entry = node("yssbi.project.function.entry");
        entry.parameters.insert(
            ParameterKey::new("function").unwrap(),
            serde_json::json!(path.as_str()),
        );
        let mut return_node = node("yssbi.project.function.return");
        return_node.parameters.insert(
            ParameterKey::new("function").unwrap(),
            serde_json::json!(path.as_str()),
        );
        let entry_parameter = port(entry.id, "parameters", instance_base);
        let return_result = port(return_node.id, "results", instance_base + 1);
        resource
            .document
            .port_bindings
            .insert(entry_parameter.clone(), binding(path, &parameter_id, "a"));
        resource
            .document
            .port_bindings
            .insert(return_result.clone(), binding(path, &return_id, "b"));
        let mut nodes = vec![entry.clone(), return_node.clone()];
        let connections = if let Some(target) = target {
            let mut call = node("yssbi.project.function.call");
            call.parameters.insert(
                ParameterKey::new("target").unwrap(),
                serde_json::json!(target.as_str()),
            );
            let call_argument = port(call.id, "arguments", instance_base + 2);
            let call_result = port(call.id, "results", instance_base + 3);
            resource
                .document
                .port_bindings
                .insert(call_argument.clone(), binding(target, &parameter_id, "a"));
            resource
                .document
                .port_bindings
                .insert(call_result.clone(), binding(target, &return_id, "b"));
            let connections = vec![
                connection(declared(entry.id, "then"), declared(call.id, "enter")),
                connection(declared(call.id, "then"), declared(return_node.id, "enter")),
                connection(entry_parameter, call_argument),
                connection(call_result, return_result),
            ];
            nodes.push(call);
            connections
        } else {
            vec![
                connection(
                    declared(entry.id, "then"),
                    declared(return_node.id, "enter"),
                ),
                connection(entry_parameter, return_result),
            ]
        };
        resource.document.nodes = nodes.into_iter().map(|node| (node.id, node)).collect();
        resource.document.connections = connections
            .into_iter()
            .map(|connection| (connection.id, connection))
            .collect();
        resource
    };
    let function_a = make_function(&path_a, Some(&path_b), 100);
    let function_b = make_function(&path_b, None, 200);
    let input_variable = {
        let mut variable = test_variable("Input");
        variable.data_value = crate::graph::value::DataValue::Int64(7);
        variable
    };
    let output_variable = {
        let mut variable = test_variable("Output");
        variable.data_value = crate::graph::value::DataValue::Int64(0);
        variable
    };
    let mut event = GraphResourceDocument::new("Chain", GraphDocumentKind::Event);
    let begin = node("yssbi.project.event.begin");
    let mut source = node("yssbi.project.variable.get");
    source.parameters.insert(
        ParameterKey::new("variable").unwrap(),
        serde_json::json!(format!("variables/{}", input_variable.id)),
    );
    let mut call = node("yssbi.project.function.call");
    call.parameters.insert(
        ParameterKey::new("target").unwrap(),
        serde_json::json!(path_a.as_str()),
    );
    let mut output = node("yssbi.project.variable.set");
    output.parameters.insert(
        ParameterKey::new("variable").unwrap(),
        serde_json::json!(format!("variables/{}", output_variable.id)),
    );
    let argument = port(call.id, "arguments", 300);
    let result = port(call.id, "results", 301);
    event
        .document
        .port_bindings
        .insert(argument.clone(), binding(&path_a, &parameter_id, "a"));
    event
        .document
        .port_bindings
        .insert(result.clone(), binding(&path_a, &return_id, "b"));
    event.document.nodes = [begin.clone(), source.clone(), call.clone(), output.clone()]
        .into_iter()
        .map(|node| (node.id, node))
        .collect();
    event.document.connections = [
        connection(declared(begin.id, "then"), declared(call.id, "enter")),
        connection(declared(call.id, "then"), declared(output.id, "enter")),
        connection(declared(source.id, "value"), argument),
        connection(result, declared(output.id, "value")),
    ]
    .into_iter()
    .map(|connection| (connection.id, connection))
    .collect();

    let run = |reverse: bool| {
        let root = std::env::temp_dir().join(format!(
            "yssbi-structured-control-order-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let mut project = ProjectData::new();
        project
            .variables
            .insert(input_variable.id, input_variable.clone());
        project
            .variables
            .insert(output_variable.id, output_variable.clone());
        crate::project::fixtures::write_project(&project, root.to_string_lossy().as_ref()).unwrap();
        let state = ProjectState::new();
        state.activate_project_fixture(root.to_string_lossy().into_owned(), project);
        let entries = if reverse {
            vec![
                (path_b.clone(), function_b.clone()),
                (path_a.clone(), function_a.clone()),
            ]
        } else {
            vec![
                (path_a.clone(), function_a.clone()),
                (path_b.clone(), function_b.clone()),
            ]
        };
        for (path, function) in entries {
            state.insert_graph(path.clone(), function).unwrap();
            crate::project::fixtures::write_state_graph(&state, &path).unwrap();
        }
        state
            .insert_graph(event_path.clone(), event.clone())
            .unwrap();
        crate::project::fixtures::write_state_graph(&state, &event_path).unwrap();

        let data = state.get_data().unwrap();
        let resources =
            super::project_state::compile_resources_from_data(&data, Default::default()).unwrap();
        drop(data);
        let (registry, store, session) = {
            let store = state.project_store.read().unwrap();
            (
                store.node_registry.clone(),
                store.function_plans.clone(),
                store.project_session_id.clone(),
            )
        };
        let mut parameters = crate::node_system::runtime::CompiledParameterStore::new();
        let generation = super::project_state::publish_function_plans(
            registry.as_ref(),
            store.as_ref(),
            &resources,
            None,
            session,
            &crate::node_system::analysis::NOOP_TRACE_SINK,
            &crate::node_system::compiler::CompileCancellationToken::new(),
            &crate::project::ProjectComputationSettings::default(),
            &mut parameters,
        )
        .unwrap();
        let published = [&path_a, &path_b]
            .into_iter()
            .map(|path| {
                let function = generation
                    .get_function(&FunctionPlanHandle::new(path.as_str()).unwrap())
                    .unwrap()
                    .unwrap();
                assert_eq!(function.plan.provenance, function.abi.provenance);
                (
                    function.plan.provenance.graph_path.clone(),
                    function.plan.provenance.basis.clone(),
                )
            })
            .collect::<Vec<_>>();

        state
            .execute_graph_for_current_project_for_test(
                &event_path,
                &crate::node_system::plan::ExecutionDemand::Default,
                &NOOP_RUN_EVENT_SINK,
            )
            .unwrap();
        let value = state.get_data().unwrap().variables[&output_variable.id]
            .data_value
            .clone();
        std::fs::remove_dir_all(root).unwrap();
        (generation.plan_count(), published, value)
    };

    let forward = run(false);
    let reverse = run(true);
    assert_eq!(forward.0, 2);
    assert_eq!(forward.0, reverse.0);
    assert_eq!(forward.1, reverse.1);
    assert_eq!(forward.2, crate::graph::value::DataValue::Int64(7));
    assert_eq!(forward.2, reverse.2);
}

#[test]
fn production_relational_backend_executes_project_dataframe_source() {
    use crate::node_system::runtime::RelationalBackend;
    let dataframe = polars::df!("value" => [1_i64, 2, 3]).unwrap();
    let resource = crate::node_system::plan::ResourceId::new("databases/main").unwrap();
    let provider = crate::node_system::runtime::ProjectResourceProvider::new(
        crate::node_system::runtime::ProjectResourceSnapshot::new(
            crate::node_system::analysis::ProjectSessionId::new("relational-project"),
            crate::node_system::analysis::ResourceVersionSet::new(),
        )
        .with_database(resource.clone(), std::sync::Arc::new(dataframe)),
    );
    let requirement = crate::node_system::plan::CompiledResourceRequirement {
        resource: resource.clone(),
        kind: crate::node_system::plan::ResourceKind::DatabaseConnection,
        access: crate::node_system::plan::ResourceAccess::Shared,
        optional: false,
    };
    let resources =
        crate::node_system::runtime::RunResourceSet::acquire(&[requirement], &provider).unwrap();
    let cancellation = crate::node_system::runtime::CancellationToken::new();
    let resource_owner = crate::node_system::runtime::RunResourceOwner::new(
        crate::node_system::analysis::RunId::new(1),
        crate::node_system::runtime::RunResourceBudgets::default(),
        cancellation.clone(),
    )
    .unwrap();
    let context = crate::node_system::runtime::RelationalContext {
        run_id: crate::node_system::analysis::RunId::new(1),
        resources: &resources,
        resource_owner: &resource_owner,
        cancellation: &cancellation,
        deadline: None,
    };
    let plan = crate::node_system::plan::CompiledRelationalPlan {
        fragment_order: Box::new([]),
        operators: Box::new([
            crate::node_system::plan::RelationalOperator::Source {
                resource,
                relation: "main".into(),
            },
            crate::node_system::plan::RelationalOperator::Limit {
                input: crate::node_system::plan::RelationalOperatorIndex::new(0),
                rows: 2,
            },
        ]),
        fragment_roots: Box::new([]),
        roots: Box::new([crate::node_system::plan::RelationalOperatorIndex::new(1)]),
        pushdown_hints: Box::new([crate::node_system::plan::RelationalPushdownHint::Limit {
            source: crate::node_system::plan::RelationalOperatorIndex::new(0),
            rows: 2,
        }]),
    };

    let result = crate::node_system::runtime::ProductionRelationalBackend::default()
        .execute(&context, &plan, &[])
        .unwrap();
    let crate::node_system::runtime::RuntimeValue::Scalar(
        crate::node_system::protocol::Value::Object(columns),
    ) = &result.outputs[0]
    else {
        panic!("expected relational dataframe output")
    };
    assert_eq!(
        columns["value"],
        crate::node_system::protocol::Value::List(vec![
            crate::node_system::protocol::Value::Integer(1),
            crate::node_system::protocol::Value::Integer(2),
        ])
    );
}

#[test]
fn project_activation_publishes_declared_duckdb_runtime_and_relational_access() {
    let root = std::env::temp_dir().join(format!(
        "yssbi-production-duckdb-run-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(root.join("database")).unwrap();
    let mut project_data = ProjectData::new();
    project_data.databases.insert(
        "main".into(),
        crate::database::DatabaseDecl {
            id: "main".into(),
            engine: crate::database::DatabaseEngine::DuckDb {
                path: "database/project.duckdb".into(),
                table: "main".into(),
            },
            schema_version: 1,
            required: true,
            name: "Main".into(),
        },
    );
    crate::project::fixtures::write_project(&project_data, root.to_string_lossy().as_ref())
        .unwrap();
    let duckdb = root.join("database/project.duckdb");
    let mut dataframe = polars::df!("value" => [11_i64, 22, 33]).unwrap();
    crate::database::ingest_dataframe_to_duckdb(&mut dataframe, &duckdb, "main").unwrap();

    let state = ProjectState::new();
    state.activate_project_fixture(root.to_string_lossy().into_owned(), project_data);

    let data = state.project_data.read().unwrap();
    let snapshots = crate::project::project_state::snapshot_project_resources(
        &state,
        data.variables.clone(),
        data.databases.clone(),
    )
    .unwrap();
    drop(data);
    let provider = crate::node_system::runtime::ProjectResourceProvider::new(snapshots.runtime);
    use crate::node_system::runtime::ResourceProvider;
    let lease = provider
        .acquire(&crate::node_system::plan::CompiledResourceRequirement {
            resource: crate::node_system::plan::ResourceId::new("databases/main").unwrap(),
            kind: crate::node_system::plan::ResourceKind::DatabaseConnection,
            access: crate::node_system::plan::ResourceAccess::Shared,
            optional: false,
        })
        .unwrap();
    let dataframe = lease
        .as_any()
        .downcast_ref::<crate::node_system::runtime::ProjectResourceLease>()
        .unwrap()
        .load_dataframe()
        .unwrap()
        .unwrap();
    assert_eq!(
        dataframe
            .column("value")
            .unwrap()
            .i64()
            .unwrap()
            .into_no_null_iter()
            .collect::<Vec<_>>(),
        vec![11, 22, 33]
    );
    assert!(
        state
            .project_store
            .read()
            .unwrap()
            .databases
            .contains_key("main")
    );

    drop(lease);
    drop(provider);
    drop(state);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn production_resource_snapshot_supplies_plot_sink() {
    use crate::node_system::runtime::ResourceProvider;
    let provider = crate::node_system::runtime::ProjectResourceProvider::new(
        crate::node_system::runtime::ProjectResourceSnapshot::new(
            crate::node_system::analysis::ProjectSessionId::new("plot-project"),
            crate::node_system::analysis::ResourceVersionSet::new(),
        )
        .with_plot_sink(std::sync::Arc::new(ProductionPlotSink)),
    );
    let lease = provider
        .acquire(&crate::node_system::plan::CompiledResourceRequirement {
            resource: crate::node_system::plan::ResourceId::new("yssbi.runtime.plot_sink").unwrap(),
            kind: crate::node_system::plan::ResourceKind::ExternalArtifact,
            access: crate::node_system::plan::ResourceAccess::Shared,
            optional: false,
        })
        .unwrap();
    let sink = lease
        .as_any()
        .downcast_ref::<crate::node_system::runtime::ProjectResourceLease>()
        .unwrap()
        .plot_sink()
        .unwrap();
    assert_eq!(
        sink.publish(crate::node_system::runtime::PlotKind::Line, "payload")
            .unwrap()
            .as_ref(),
        "payload"
    );
}

#[test]
fn project_execution_refuses_blocking_analysis() {
    let (state, root) = active_state_with_empty_graph("blocking-analysis");
    let invalid = node("yssbi.test.missing");
    let patch = GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode { node: invalid }]);
    state
        .apply_graph_patch(
            &graph_path(),
            MutationRequest::new(
                ResourceKey::Graph(document_path()),
                GraphRevision::INITIAL,
                OperationId::new(),
                patch,
            ),
        )
        .unwrap();

    let error = state
        .execute_graph_for_current_project_for_test(
            &graph_path(),
            &crate::node_system::plan::ExecutionDemand::Default,
            &NOOP_RUN_EVENT_SINK,
        )
        .unwrap_err();
    assert!(error.contains("blocking diagnostics"));
    assert!(error.contains("compiler.node.unknown"));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn project_execution_ignores_an_unreferenced_incomplete_function() {
    let (state, root) = active_state_with_empty_graph("unreferenced-incomplete-function");
    let function_path = GraphResourcePath::new("functions/asd.yssbi-function").unwrap();
    state
        .insert_graph(
            function_path,
            GraphResourceDocument::new("asd", GraphDocumentKind::Function),
        )
        .unwrap();

    state
        .execute_graph_for_current_project_for_test(
            &graph_path(),
            &crate::node_system::plan::ExecutionDemand::Default,
            &NOOP_RUN_EVENT_SINK,
        )
        .expect("an unreferenced incomplete function must not block event execution");

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn project_execution_ignores_unreferenced_local_variable_versions() {
    let (state, root) = active_state_with_empty_graph("unreferenced-local-variable");
    state
        .add_variable(
            "unused",
            crate::graph::value::DataType::Int64,
            crate::graph::value::DataValue::Int64(0),
            "",
            crate::variable::VariableScope::Event {
                event_path: graph_path().as_str().into(),
            },
            Vec::new(),
        )
        .unwrap();

    state
        .execute_graph_for_current_project_for_test(
            &graph_path(),
            &crate::node_system::plan::ExecutionDemand::Default,
            &NOOP_RUN_EVENT_SINK,
        )
        .expect("an unreferenced local variable must not stale the execution basis");

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn project_variable_get_executes_against_authoritative_resource() {
    let project = temp_project_with_empty_graph("project-variable-execution");
    let state = project.state();
    let variable = state
        .add_variable(
            "authoritative",
            crate::graph::value::DataType::Int64,
            crate::graph::value::DataValue::Int64(41),
            "",
            crate::variable::VariableScope::Global,
            Vec::new(),
        )
        .unwrap();
    let mut variable_node = node("yssbi.project.variable.get");
    variable_node.parameters.insert(
        crate::node_system::protocol::ParameterKey::new("variable").unwrap(),
        serde_json::json!(format!("variables/{}", variable.id)),
    );
    state
        .apply_graph_patch(
            &graph_path(),
            MutationRequest::new(
                ResourceKey::Graph(document_path()),
                GraphRevision::INITIAL,
                OperationId::new(),
                GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode {
                    node: variable_node,
                }]),
            ),
        )
        .unwrap();

    let result = state
        .execute_graph_for_current_project_for_test(
            &graph_path(),
            &crate::node_system::plan::ExecutionDemand::Default,
            &NOOP_RUN_EVENT_SINK,
        )
        .unwrap();
    assert!(result.run_id.get() > 0);
}

#[test]
fn demanded_variable_get_preflights_only_its_retained_resource_and_releases_lease() {
    let project = temp_project_with_empty_graph("demanded-variable-resource");
    let state = project.state();
    let first = state
        .add_variable(
            "first",
            crate::graph::value::DataType::Int64,
            crate::graph::value::DataValue::Int64(1),
            "",
            crate::variable::VariableScope::Global,
            Vec::new(),
        )
        .unwrap();
    let second = state
        .add_variable(
            "second",
            crate::graph::value::DataType::Int64,
            crate::graph::value::DataValue::Int64(2),
            "",
            crate::variable::VariableScope::Global,
            Vec::new(),
        )
        .unwrap();
    let mut first_get = node("yssbi.project.variable.get");
    first_get.parameters.insert(
        crate::node_system::protocol::ParameterKey::new("variable").unwrap(),
        serde_json::json!(format!("variables/{}", first.id)),
    );
    let first_node = first_get.id;
    let mut second_get = node("yssbi.project.variable.get");
    let second_node = second_get.id;
    second_get.parameters.insert(
        crate::node_system::protocol::ParameterKey::new("variable").unwrap(),
        serde_json::json!(format!("variables/{}", second.id)),
    );
    state
        .apply_graph_patch(
            &graph_path(),
            MutationRequest::new(
                ResourceKey::Graph(document_path()),
                GraphRevision::INITIAL,
                OperationId::new(),
                GraphDocumentPatch::new(vec![
                    GraphDocumentOperation::InsertNode { node: first_get },
                    GraphDocumentOperation::InsertNode { node: second_get },
                ]),
            ),
        )
        .unwrap();
    let first_resource =
        crate::node_system::plan::ResourceId::new(format!("variables/{}", first.id)).unwrap();
    let second_resource =
        crate::node_system::plan::ResourceId::new(format!("variables/{}", second.id)).unwrap();
    let requirement = |resource| crate::node_system::plan::CompiledResourceRequirement {
        resource,
        kind: crate::node_system::plan::ResourceKind::ExternalArtifact,
        access: crate::node_system::plan::ResourceAccess::Shared,
        optional: false,
    };
    let observer = crate::node_system::runtime::ProjectResourceLeaseObserver::default()
        .with_forced_unavailable(second_resource.clone());
    state.set_project_resource_lease_observer(observer.clone());
    let invalid_demand = crate::node_system::plan::ExecutionDemand::Outputs {
        outputs: Box::new([crate::node_system::plan::GraphOutputRef {
            graph_path: document_path(),
            port: crate::node_system::document::PortAddress::declared(
                crate::node_system::document::NodeId::new(),
                crate::node_system::protocol::PortKey::new("value").unwrap(),
            ),
        }]),
        include_default_results: false,
    };
    let invalid = state
        .execute_graph_for_current_project_for_test(
            &graph_path(),
            &invalid_demand,
            &NOOP_RUN_EVENT_SINK,
        )
        .unwrap_err();
    assert_eq!(
        invalid.kind(),
        crate::project::ProjectExecutionErrorKind::InvalidDemand
    );
    assert_eq!(observer.acquired(), 0);

    let demand = crate::node_system::plan::ExecutionDemand::Outputs {
        outputs: Box::new([crate::node_system::plan::GraphOutputRef {
            graph_path: document_path(),
            port: crate::node_system::document::PortAddress::declared(
                first_node,
                crate::node_system::protocol::PortKey::new("value").unwrap(),
            ),
        }]),
        include_default_results: false,
    };
    let run = state
        .execute_graph_for_current_project_for_test(&graph_path(), &demand, &NOOP_RUN_EVENT_SINK)
        .unwrap();

    assert_eq!(run.result_ids.len(), 1);
    assert_eq!(
        observer.validated_requirements(),
        vec![vec![requirement(first_resource.clone())].into_boxed_slice()]
    );
    assert_eq!(observer.acquire_attempt_ids(), vec![first_resource]);
    assert_eq!(observer.acquired(), 1);
    assert_eq!(observer.dropped(), 1);
    assert_eq!(observer.active(), 0);

    let unavailable_observer = crate::node_system::runtime::ProjectResourceLeaseObserver::default()
        .with_forced_unavailable(second_resource.clone());
    state.set_project_resource_lease_observer(unavailable_observer.clone());
    let unavailable_demand = crate::node_system::plan::ExecutionDemand::Outputs {
        outputs: Box::new([crate::node_system::plan::GraphOutputRef {
            graph_path: document_path(),
            port: crate::node_system::document::PortAddress::declared(
                second_node,
                crate::node_system::protocol::PortKey::new("value").unwrap(),
            ),
        }]),
        include_default_results: false,
    };
    let events = DemandRunEvents::default();

    let unavailable = state
        .execute_graph_for_current_project_for_test(&graph_path(), &unavailable_demand, &events)
        .unwrap_err();

    assert!(unavailable.contains("unavailable"), "{unavailable}");
    assert_eq!(
        unavailable_observer.validated_requirements(),
        vec![vec![requirement(second_resource.clone())].into_boxed_slice()],
    );
    assert_eq!(
        unavailable_observer.acquire_attempt_ids(),
        vec![second_resource],
    );
    assert_eq!(unavailable_observer.acquired(), 0);
    assert_eq!(unavailable_observer.dropped(), 0);
    assert_eq!(unavailable_observer.active(), 0);
    assert!(events.0.lock().unwrap().iter().all(|event| !matches!(
        event.kind,
        crate::node_system::runtime::RunEventKind::OperationStarted { .. }
    )));
}
