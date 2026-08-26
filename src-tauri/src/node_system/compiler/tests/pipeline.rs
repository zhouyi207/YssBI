use super::*;

#[test]
fn corrupt_registry_snapshot_produces_missing_lowering_blocking_diagnostic() {
    let registry = CorruptCompilerRegistrySnapshot { frozen: registry() };
    let result = GraphCompiler::new(&registry, &Resources)
        .compile(&document(NodeTypeId::new("yssbi.test.constant").unwrap()));

    assert!(result.semantic.is_some());
    assert!(result.plan.is_none());
    assert!(result.analysis.diagnostics.is_empty());
    assert!(matches!(
        result.outcome,
        CompilationOutcome::InternalFailure(ref failure)
            if failure.stage == CompilationStage::Lowering
                && failure.code.as_ref() == "compiler.lowering.implementation_missing"
                && failure.node_id == Some(node_id(1))
    ));
}

#[test]
fn valid_constant_graph_produces_plan_with_same_basis() {
    let registry = registry();
    let result = GraphCompiler::new(&registry, &Resources)
        .compile(&document(NodeTypeId::new("yssbi.test.constant").unwrap()));
    let semantic = result
        .semantic
        .expect("valid graph should retain its semantic graph");
    let plan = result.plan.expect("valid graph should lower");
    assert!(result.analysis.diagnostics.is_empty());
    assert_eq!(plan.operations.len(), 1);
    assert_eq!(semantic.basis, result.analysis.basis);
    assert_eq!(plan.provenance.basis, semantic.basis);
}

#[test]
fn compile_plan_keeps_the_exact_requested_provenance() {
    let registry = registry();
    let session_id = ProjectSessionId::new("project-session-1");
    let compiler =
        GraphCompiler::new(&registry, &Resources).with_project_session_id(session_id.clone());
    let snapshot = compiler.snapshot_with_compile_id(
        CompileId::new(41),
        GraphResourcePath::new("events/main.yssbi-event").unwrap(),
        &document(NodeTypeId::new("yssbi.test.constant").unwrap()),
    );
    let result = compiler
        .compile_snapshot(&snapshot, &CompileCancellationToken::new())
        .unwrap();

    assert_eq!(&snapshot.provenance.project_session_id, &session_id);
    assert_eq!(
        &result.plan.as_ref().expect("valid graph plan").provenance,
        &snapshot.provenance,
    );
    assert_eq!(
        &result
            .execution_basis
            .as_ref()
            .expect("valid graph execution basis")
            .provenance,
        &snapshot.provenance,
    );
}

#[test]
fn unknown_node_returns_analysis_without_plan() {
    let registry = registry();
    let result = GraphCompiler::new(&registry, &Resources)
        .compile(&document(NodeTypeId::new("yssbi.test.missing").unwrap()));
    assert!(result.semantic.is_none());
    assert!(result.plan.is_none());
    assert_eq!(
        result.analysis.diagnostics[0].code.as_str(),
        "compiler.node.unknown"
    );
}

#[test]
fn unknown_port_and_wrong_direction_return_analysis_without_plan() {
    let registry = registry();
    let mut graph = document(NodeTypeId::new("yssbi.test.constant").unwrap());
    let node = node_id(1);
    let other = node_id(2);
    graph.nodes.insert(
        other,
        DocumentNode {
            id: other,
            node_type: NodeTypeId::new("yssbi.test.constant").unwrap(),
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters: BTreeMap::new(),
            user_label: None,
        },
    );
    let unknown = PortAddress::declared(other, key("missing"));
    let output_as_input = PortAddress::declared(other, key("value"));
    graph.connections.insert(
        crate::graph_document::ConnectionId::from_uuid(Uuid::from_u128(3)),
        DocumentConnection {
            id: crate::graph_document::ConnectionId::from_uuid(Uuid::from_u128(3)),
            output: PortAddress::declared(node, key("value")),
            input: unknown,
            order: None,
        },
    );
    graph.connections.insert(
        crate::graph_document::ConnectionId::from_uuid(Uuid::from_u128(4)),
        DocumentConnection {
            id: crate::graph_document::ConnectionId::from_uuid(Uuid::from_u128(4)),
            output: PortAddress::declared(node, key("value")),
            input: output_as_input,
            order: None,
        },
    );
    let result = GraphCompiler::new(&registry, &Resources).compile(&graph);
    assert!(result.plan.is_none());
    let codes: Vec<_> = result
        .analysis
        .diagnostics
        .iter()
        .map(|item| item.code.as_str())
        .collect();
    assert!(codes.contains(&"compiler.port.unknown"));
    assert!(codes.contains(&"compiler.connection.input_direction"));
}

#[test]
fn semantically_identical_documents_serialize_identically() {
    let forward = determinism_fixture(FixtureInsertionOrder::Forward);
    let reverse = determinism_fixture(FixtureInsertionOrder::Reverse);

    assert_reversed_insertions(&forward.node_insertions, &reverse.node_insertions);
    assert_reversed_insertions(
        &forward.connection_insertions,
        &reverse.connection_insertions,
    );
    assert_reversed_insertions(&forward.parameter_insertions, &reverse.parameter_insertions);
    assert_reversed_insertions(
        &forward.port_binding_insertions,
        &reverse.port_binding_insertions,
    );
    assert_reversed_insertions(
        &forward.input_state_insertions,
        &reverse.input_state_insertions,
    );
    assert_eq!(forward.document, reverse.document);

    let registry = determinism_registry(determinism_protocols());
    let compiler = GraphCompiler::new(&registry, &Resources)
        .with_project_session_id(ProjectSessionId::new("determinism-session"));
    let graph_path = GraphResourcePath::new("events/determinism.yssbi-event").unwrap();
    let compile_id = CompileId::new(73);
    let forward_snapshot =
        compiler.snapshot_with_compile_id(compile_id, graph_path.clone(), &forward.document);
    let reverse_snapshot =
        compiler.snapshot_with_compile_id(compile_id, graph_path, &reverse.document);

    assert_eq!(forward_snapshot.provenance, reverse_snapshot.provenance);
    let forward_result = compiler
        .compile_snapshot(&forward_snapshot, &CompileCancellationToken::new())
        .expect("forward fixture should compile");
    let reverse_result = compiler
        .compile_snapshot(&reverse_snapshot, &CompileCancellationToken::new())
        .expect("reverse fixture should compile");

    assert_eq!(
        forward_result.analysis.diagnostics.as_ref(),
        reverse_result.analysis.diagnostics.as_ref()
    );
    assert!(
        forward_result.analysis.diagnostics.is_empty(),
        "fixture diagnostics: {:#?}",
        forward_result.analysis.diagnostics
    );
    assert_eq!(forward_result.analysis.nodes.len(), 4);
    assert_eq!(
        forward_result.analysis.nodes.as_ref(),
        reverse_result.analysis.nodes.as_ref()
    );
    assert_eq!(forward_result.analysis.resolved_interfaces.len(), 4);
    assert_eq!(
        forward_result.analysis.resolved_interfaces.as_ref(),
        reverse_result.analysis.resolved_interfaces.as_ref()
    );
    assert_eq!(forward_result.analysis.partial_types.len(), 7);
    assert_eq!(
        forward_result.analysis.partial_types,
        reverse_result.analysis.partial_types
    );
    assert!(
        forward_result.analysis.partial_schemas.is_empty(),
        "fixture protocols intentionally declare no schema expressions"
    );
    assert_eq!(
        forward_result.analysis.partial_schemas,
        reverse_result.analysis.partial_schemas
    );
    assert_eq!(
        serde_json::to_vec(&forward_result.analysis).unwrap(),
        serde_json::to_vec(&reverse_result.analysis).unwrap()
    );

    let forward_semantic = forward_result
        .semantic
        .expect("forward fixture should produce a semantic graph");
    let reverse_semantic = reverse_result
        .semantic
        .expect("reverse fixture should produce a semantic graph");
    assert_eq!(
        forward_semantic.dependencies.as_ref(),
        reverse_semantic.dependencies.as_ref()
    );
    assert_eq!(forward_semantic.dependencies.len(), 4);
    assert_eq!(
        serde_json::to_vec(&forward_semantic).unwrap(),
        serde_json::to_vec(&reverse_semantic).unwrap()
    );

    let forward_plan = forward_result
        .plan
        .expect("forward fixture should produce an execution plan");
    let reverse_plan = reverse_result
        .plan
        .expect("reverse fixture should produce an execution plan");
    assert_eq!(forward_plan.operations.len(), 5);
    assert_eq!(
        forward_plan
            .operations
            .iter()
            .filter(|operation| matches!(operation.kernel, PlannedKernel::Adapter(_)))
            .count(),
        2
    );

    assert_eq!(
        forward_plan.operations.as_ref(),
        reverse_plan.operations.as_ref()
    );
    assert_eq!(
        forward_plan.relational_subplans.as_ref(),
        reverse_plan.relational_subplans.as_ref()
    );
    assert_eq!(forward_plan.relational_subplans.len(), 2);
    assert!(
        forward_plan
            .relational_subplans
            .iter()
            .any(|subplan| subplan.compiled_plan.fragment_order.len() > 1)
    );
    assert!(
        forward_plan
            .relational_subplans
            .iter()
            .all(|subplan| !subplan.compiled_plan.fragment_roots.is_empty())
    );

    for (forward_subplan, reverse_subplan) in forward_plan
        .relational_subplans
        .iter()
        .zip(reverse_plan.relational_subplans.iter())
    {
        assert_eq!(
            forward_subplan.compiled_plan.fragment_order,
            reverse_subplan.compiled_plan.fragment_order
        );
        assert_eq!(
            forward_subplan.compiled_plan.fragment_roots,
            reverse_subplan.compiled_plan.fragment_roots
        );
        assert_eq!(
            forward_subplan.compiled_plan.roots,
            reverse_subplan.compiled_plan.roots
        );
    }
    assert_eq!(
        serde_json::to_vec(&forward_plan).unwrap(),
        serde_json::to_vec(&reverse_plan).unwrap()
    );
}
