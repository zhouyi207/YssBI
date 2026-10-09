use crate::{
    CatalogMutationValidationSnapshot, EditorGraphMutation, EditorMutationContext,
    EditorMutationErrorCode,
};
use std::collections::BTreeMap;
use yss_data_contract::ValueType;
use yss_graph_document::{
    DocumentConnection, DocumentNode, DynamicMemberLocator, DynamicPortBinding,
    FunctionParameterId, GraphDocument, GraphResourcePath, LastKnownPortMetadata, NodeId,
    NodePosition, OrderKey, ParameterValues, PortAddress, PortInstanceId,
};
use yss_graph_document_edit::{GraphDocumentRead, apply_graph_document_patch};
use yss_node_catalog::{authoritative_static_descriptor, build_builtin_node_system};
use yss_node_protocol::{NodeTypeId, PortKey, TypeExpr};
use yss_node_registry::NodeRegistry;

fn graph_path() -> GraphResourcePath {
    GraphResourcePath::new("events/Main.yssbi-event").expect("fixture graph path must be valid")
}

fn document_node(node_type: &str, x: f64) -> DocumentNode {
    DocumentNode {
        id: NodeId::new(),
        node_type: NodeTypeId::new(node_type).expect("fixture node type must be valid"),
        position: NodePosition { x, y: 0.0 },
        parameters: ParameterValues::new(),
        user_label: None,
    }
}

fn declared(node: NodeId, port: &str) -> PortAddress {
    PortAddress::declared(
        node,
        PortKey::new(port).expect("fixture port key must be valid"),
    )
}

fn insert_node(document: &mut GraphDocument, node: DocumentNode) -> NodeId {
    let id = node.id;
    assert!(document.nodes.insert(id, node).is_none());
    id
}

fn static_descriptor(registry: &NodeRegistry, node_type: &str) -> yss_node_catalog::NodeCreation {
    let node_type = NodeTypeId::new(node_type).expect("fixture node type must be valid");
    let protocol = registry
        .protocol(&node_type)
        .expect("fixture protocol must exist");
    authoritative_static_descriptor(protocol)
        .expect("fixture protocol must have a catalog creation descriptor")
}

#[test]
fn creation_and_partial_edits_share_parameter_rules_without_requiring_complete_values() {
    let registry = build_builtin_node_system().unwrap().registry;
    let mut document = GraphDocument::default();
    let create = |values| EditorGraphMutation::CreateNode {
        port_counts: Default::default(),
        descriptor: static_descriptor(&registry, "yssbi.dataframe.groupby"),
        position: NodePosition { x: 0.0, y: 0.0 },
        parameters: serde_json::from_value(values).unwrap(),
        user_label: None,
        connect_from: None,
    };
    let values = serde_json::json!({"mean": [" sales "]});
    let patch = create(values.clone())
        .into_patch(&graph_path(), &document, &registry)
        .unwrap();
    apply_graph_document_patch(&mut document, &patch).unwrap();
    let node = document.nodes.values().next().unwrap();
    let node_id = node.id;
    assert_eq!(serde_json::to_value(&node.parameters).unwrap(), values);
    let protocol = registry.protocol(&node.node_type).unwrap();
    let issues = yss_node_protocol::validate_parameter_values(
        protocol,
        &node.parameters,
        &|id: &yss_node_protocol::TypeId, value: &serde_json::Value| {
            registry.validate_nominal_parameter(id, value)
        },
    );
    assert!(issues.iter().any(|issue| issue.key.as_str() == "keys"
        && matches!(issue.kind, yss_node_protocol::ParameterIssueKind::Required)));

    let edit = |values| EditorGraphMutation::SetParameters {
        node_id,
        parameters: serde_json::from_value(values).unwrap(),
    };
    let key_patch = edit(serde_json::json!({"keys": ["industry"]}))
        .into_patch(&graph_path(), &document, &registry)
        .unwrap();
    apply_graph_document_patch(&mut document, &key_patch).unwrap();
    assert_eq!(
        serde_json::to_value(&document.nodes[&node_id].parameters).unwrap(),
        serde_json::json!({"keys": ["industry"], "mean": [" sales "]})
    );
    let before = document.clone();
    for invalid in [
        serde_json::json!({"keys": [42]}),
        serde_json::json!({"mean": [42]}),
        serde_json::json!({"mean": [""]}),
        serde_json::json!({"mean": ["sales", "sales"]}),
        serde_json::json!({"unknown": true}),
    ] {
        assert!(
            create(invalid.clone())
                .into_patch(&graph_path(), &document, &registry)
                .is_err()
        );
        assert!(
            edit(invalid)
                .into_patch(&graph_path(), &document, &registry)
                .is_err()
        );
        assert_eq!(document, before);
    }
    let clear = edit(serde_json::json!({"keys": null}))
        .into_patch(&graph_path(), &document, &registry)
        .unwrap();
    apply_graph_document_patch(&mut document, &clear).unwrap();
    assert_eq!(
        serde_json::to_value(&document.nodes[&node_id].parameters).unwrap(),
        values
    );
    apply_graph_document_patch(&mut document, &clear.inverse()).unwrap();
    assert_eq!(document, before);
    apply_graph_document_patch(&mut document, &key_patch.inverse()).unwrap();
    apply_graph_document_patch(&mut document, &patch.inverse()).unwrap();
    assert_eq!(document, GraphDocument::default());
    let rename = EditorGraphMutation::CreateNode {
        port_counts: Default::default(),
        descriptor: static_descriptor(&registry, "yssbi.dataframe.rename"),
        position: NodePosition { x: 0.0, y: 0.0 },
        parameters: serde_json::from_value(serde_json::json!({"from": "amount"})).unwrap(),
        user_label: None,
        connect_from: None,
    }
    .into_patch(&graph_path(), &document, &registry)
    .unwrap();
    apply_graph_document_patch(&mut document, &rename).unwrap();
    assert_eq!(
        serde_json::to_value(&document.nodes.values().next().unwrap().parameters).unwrap(),
        serde_json::json!({"from": "amount"})
    );
}

#[test]
fn grouped_parameters_merge_reset_and_undo_atomically() {
    let system = build_builtin_node_system().unwrap();
    let mut document = GraphDocument::default();
    let node_id = insert_node(
        &mut document,
        document_node("yssbi.statistics.linear.fit", 0.0),
    );
    let edit = |document: &GraphDocument, values: serde_json::Value| {
        EditorGraphMutation::SetParameters {
            node_id,
            parameters: serde_json::from_value(values).unwrap(),
        }
        .into_patch(&graph_path(), document, &system.registry)
    };
    let patch = edit(
        &document,
        serde_json::json!({"constant": false, "covariance": "HAC", "bandwidth": 5}),
    )
    .unwrap();
    apply_graph_document_patch(&mut document, &patch).unwrap();
    assert_eq!(
        serde_json::to_value(&document.nodes[&node_id].parameters).unwrap(),
        serde_json::json!({"constant": false, "covariance": "HAC", "bandwidth": 5})
    );
    let before = document.clone();
    assert!(
        edit(
            &document,
            serde_json::json!({"constant": true, "covariance": "fixed scale", "scale": -1})
        )
        .is_err()
    );
    assert_eq!(document, before);
    let patch = edit(&document, serde_json::json!({"covariance": "HC1"})).unwrap();
    apply_graph_document_patch(&mut document, &patch).unwrap();
    assert_eq!(
        serde_json::to_value(&document.nodes[&node_id].parameters).unwrap(),
        serde_json::json!({"constant": false, "covariance": "HC1"})
    );
    apply_graph_document_patch(&mut document, &patch.inverse()).unwrap();
    assert_eq!(document, before);
    let patch = edit(&document, serde_json::json!({"bandwidth": null})).unwrap();
    apply_graph_document_patch(&mut document, &patch).unwrap();
    let protocol = system
        .registry
        .protocol(&document.nodes[&node_id].node_type)
        .unwrap();
    assert_eq!(
        protocol
            .parameters
            .get(&"bandwidth".parse().unwrap())
            .unwrap()
            .default_json(),
        Some(1.into())
    );
    assert!(
        !document.nodes[&node_id]
            .parameters
            .contains_key(&"bandwidth".parse().unwrap())
    );
}

#[test]
fn output_fan_out_preserves_other_branches_when_an_input_is_replaced_and_undone() {
    let system = build_builtin_node_system().unwrap();
    let mut document = GraphDocument::default();
    let source = insert_node(&mut document, document_node("yssbi.constant.get", 0.0));
    let replacement = insert_node(&mut document, document_node("yssbi.constant.get", 0.0));
    for node in [source, replacement] {
        set_constant(
            &mut document,
            node,
            ValueType::Scalar(yss_data_contract::SemanticType::Numeric),
            yss_data_contract::DataValue::Integer(1),
        );
    }
    let first = insert_node(&mut document, document_node("yssbi.debug.view", 200.0));
    let second = insert_node(&mut document, document_node("yssbi.debug.view", 400.0));
    let connect = |document: &GraphDocument, source, target| {
        EditorGraphMutation::Connect {
            output: declared(source, "value"),
            input: declared(target, "data"),
            order: None,
        }
        .into_patch(&graph_path(), document, &system.registry)
    };
    for target in [first, second] {
        let patch = connect(&document, source, target).unwrap();
        apply_graph_document_patch(&mut document, &patch).unwrap();
    }
    assert_eq!(
        document.connections.len(),
        2,
        "the second consumer must not disconnect the first"
    );
    let before = document.clone();
    assert!(
        matches!(connect(&document, source, second), Err(crate::MutationConflict::Editor(error)) if error.code == EditorMutationErrorCode::GraphConnectionAlreadyExists)
    );
    let patch = connect(&document, replacement, first).unwrap();
    apply_graph_document_patch(&mut document, &patch).unwrap();
    assert_eq!(document.connections.len(), 2);
    assert!(
        document
            .connections
            .values()
            .any(|connection| connection.output == declared(source, "value")
                && connection.input == declared(second, "data"))
    );
    assert!(
        document
            .connections
            .values()
            .any(
                |connection| connection.output == declared(replacement, "value")
                    && connection.input == declared(first, "data")
            )
    );
    apply_graph_document_patch(&mut document, &patch.inverse()).unwrap();
    assert_eq!(document, before);

    let unused = insert_node(&mut document, document_node("yssbi.debug.view", 600.0));
    connect(&document, replacement, unused).unwrap();
    let mut invalid = document.clone();
    let missing = NodeId::new();
    invalid.input_states.insert(
        declared(missing, "input"),
        yss_graph_document::InputState {
            literal_override: None,
        },
    );
    assert!(matches!(
        connect(&invalid, replacement, unused),
        Err(crate::MutationConflict::Document(
            yss_graph_document_edit::DocumentError::EndpointNodeNotFound(node)
        )) if node == missing
    ));
}

#[test]
fn bounded_connection_limits_reject_overflow_and_reopen_after_disconnect() {
    use std::sync::Arc;
    use yss_node_protocol::{ConnectionsPerPort, NodeTypingSpec};
    use yss_node_registry::{
        LeafImplementation, NodeRegistryBuilder, ProviderRegistration, RegisteredNode,
    };
    let builtin = build_builtin_node_system().unwrap();
    let mut protocol = builtin
        .registry
        .protocol(&"yssbi.logic.not".parse().unwrap())
        .unwrap()
        .clone();
    protocol.type_id = "tests.connections.bounded".parse().unwrap();
    protocol.typing = NodeTypingSpec::Fixed;
    for port in &mut protocol.interface.ports {
        port.value_type = TypeExpr::Concrete("core.binary".parse().unwrap());
        port.connections = ConnectionsPerPort::Multiple {
            max: Some(2),
            ordered: false,
        };
    }
    let mut builder = NodeRegistryBuilder::new();
    yss_node_catalog::register_builtin_nodes(&mut builder).unwrap();
    let mut provider = ProviderRegistration::new("tests.connections".parse().unwrap());
    provider.nodes = [RegisteredNode::leaf(
        Arc::new(protocol),
        LeafImplementation::new("tests.connections.bounded"),
    )]
    .into();
    builder.register_provider(provider).unwrap();
    let registry = builder.freeze().unwrap();
    let mut document = GraphDocument::default();
    let sources = std::array::from_fn::<_, 3, _>(|_| {
        insert_node(
            &mut document,
            document_node("tests.connections.bounded", 0.0),
        )
    });
    let targets = std::array::from_fn::<_, 3, _>(|_| {
        insert_node(
            &mut document,
            document_node("tests.connections.bounded", 100.0),
        )
    });
    let connect = |document: &GraphDocument, source, target| {
        EditorGraphMutation::Connect {
            output: declared(source, "result"),
            input: declared(target, "input"),
            order: None,
        }
        .into_patch(&graph_path(), document, &registry)
    };
    for (source, target) in [
        (sources[0], targets[0]),
        (sources[0], targets[1]),
        (sources[1], targets[0]),
    ] {
        let patch = connect(&document, source, target).unwrap();
        apply_graph_document_patch(&mut document, &patch).unwrap();
    }
    let full = document.clone();
    for (source, target) in [(sources[0], targets[2]), (sources[2], targets[0])] {
        assert!(matches!(
            connect(&document, source, target),
            Err(crate::MutationConflict::Editor(error))
                if error.code == EditorMutationErrorCode::GraphConnectionLimitReached
        ));
    }
    assert_eq!(document, full);
    let released = document
        .connections
        .values()
        .find(|connection| connection.input.node_id == targets[1])
        .unwrap()
        .id;
    let disconnect = EditorGraphMutation::DisconnectConnections {
        connection_ids: vec![released],
    }
    .into_patch(&graph_path(), &document, &registry)
    .unwrap();
    apply_graph_document_patch(&mut document, &disconnect).unwrap();
    let replacement = connect(&document, sources[0], targets[2]).unwrap();
    apply_graph_document_patch(&mut document, &replacement).unwrap();
    apply_graph_document_patch(&mut document, &replacement.inverse()).unwrap();
    apply_graph_document_patch(&mut document, &disconnect.inverse()).unwrap();
    assert_eq!(document, full);
}

#[test]
fn connect_rejects_a_known_type_outside_the_input_class() {
    use std::sync::Arc;
    use yss_node_protocol::{NodeTypingSpec, ParameterCondition, ParameterEditorSpec, TypedValue};
    use yss_node_registry::{
        LeafImplementation, NodeRegistryBuilder, ProviderRegistration, RegisteredNode,
    };
    let builtin = build_builtin_node_system()
        .expect("built-in registry must assemble")
        .registry;
    let mut document = GraphDocument::default();
    let source = insert_node(&mut document, document_node("yssbi.constant.get", 0.0));
    set_constant(
        &mut document,
        source,
        ValueType::Scalar(yss_data_contract::SemanticType::Text),
        yss_data_contract::DataValue::String("".into()),
    );
    let constant_id = *document.constants.keys().next().unwrap();
    let mut protocol = builtin
        .protocol(&"yssbi.constant.get".parse().unwrap())
        .unwrap()
        .clone();
    protocol.type_id = "tests.constant.default".parse().unwrap();
    let mut reference = protocol.parameters.groups[0].parameters[0].clone();
    reference.key = "selection".parse().unwrap();
    reference.default_value = Some(TypedValue {
        value_type: reference.value_type.clone(),
        value: yss_data_contract::DataValue::String(constant_id.to_string().into()),
    });
    reference.visible_when = Some(ParameterCondition {
        key: "mode".parse().unwrap(),
        values: [yss_data_contract::DataValue::String("active".into())].into(),
    });
    let mut mode = reference.clone();
    mode.key = "mode".parse().unwrap();
    mode.editor = ParameterEditorSpec::Text { multiline: false };
    mode.visible_when = None;
    mode.default_value.as_mut().unwrap().value =
        yss_data_contract::DataValue::String("active".into());
    protocol.parameters.groups[0].parameters = [reference, mode].into();
    protocol.typing = NodeTypingSpec::ConstantOutput {
        parameter: "selection".parse().unwrap(),
        output: "value".parse().unwrap(),
    };
    let mut builder = NodeRegistryBuilder::new();
    yss_node_catalog::register_builtin_nodes(&mut builder).unwrap();
    let mut provider = ProviderRegistration::new("tests.constants".parse().unwrap());
    provider.nodes = [RegisteredNode::leaf(
        Arc::new(protocol),
        LeafImplementation::new("tests.constant"),
    )]
    .into();
    builder.register_provider(provider).unwrap();
    let registry = builder.freeze().unwrap();
    let target = insert_node(
        &mut document,
        document_node("yssbi.numeric.subtract", 100.0),
    );

    let connect = || EditorGraphMutation::Connect {
        output: declared(source, "value"),
        input: declared(target, "left"),
        order: None,
    };
    let assert_mismatch = |document: &GraphDocument| {
        let error = connect()
            .into_patch(&graph_path(), document, &registry)
            .expect_err("a known text value cannot connect to a numeric input");
        assert!(matches!(error, crate::MutationConflict::Editor(error)
            if error.code == EditorMutationErrorCode::GraphConnectionTypeMismatch));
    };
    assert_mismatch(&document);

    let source_node = document.nodes.get_mut(&source).unwrap();
    source_node.node_type = "tests.constant.default".parse().unwrap();
    source_node.parameters.clear();
    assert_mismatch(&document);
    assert!(document.nodes[&source].parameters.is_empty());
    let source_node = document.nodes.get_mut(&source).unwrap();
    source_node.parameters.insert(
        "selection".parse().unwrap(),
        serde_json::json!(constant_id.to_string()),
    );
    source_node
        .parameters
        .insert("mode".parse().unwrap(), serde_json::json!("inactive"));
    connect()
        .into_patch(&graph_path(), &document, &registry)
        .expect("an inactive stored reference must not narrow generic preflight");
    document
        .nodes
        .get_mut(&source)
        .unwrap()
        .parameters
        .remove(&"mode".parse().unwrap());
    assert_mismatch(&document);
    assert!(document.connections.is_empty());
}

#[test]
fn polymorphic_output_preflight_does_not_reject_a_possible_exact_target() {
    let registry = build_builtin_node_system()
        .expect("built-in registry must assemble")
        .registry;
    let mut document = GraphDocument::default();
    let source = insert_node(&mut document, document_node("yssbi.numeric.add", 0.0));
    let target = insert_node(
        &mut document,
        document_node("yssbi.dataframe.series.inverse_standardize", 100.0),
    );

    let patch = EditorGraphMutation::Connect {
        output: declared(source, "result"),
        input: declared(target, "mean"),
        order: None,
    }
    .into_patch(&graph_path(), &document, registry.as_ref())
    .expect("a polymorphic output that can resolve to Float64 must pass cheap preflight");
    apply_graph_document_patch(&mut document, &patch).expect("planned connection must be valid");

    assert_eq!(document.connections.len(), 1);
}

#[test]
fn move_connections_uses_current_document_authority() {
    let registry = build_builtin_node_system()
        .expect("built-in registry must assemble")
        .registry;
    let mut document = GraphDocument::default();
    let source = insert_node(&mut document, document_node("yssbi.constant.get", 0.0));
    set_constant(
        &mut document,
        source,
        ValueType::Scalar(yss_data_contract::SemanticType::Numeric),
        yss_data_contract::DataValue::Integer(0),
    );
    let target = insert_node(
        &mut document,
        document_node("yssbi.numeric.subtract", 100.0),
    );
    let output = declared(source, "value");
    let left = declared(target, "left");
    let right = declared(target, "right");
    let connection = DocumentConnection {
        id: yss_graph_document::ConnectionId::new(),
        output: output.clone(),
        input: left.clone(),
        order: None,
    };
    assert!(
        document
            .connections
            .insert(connection.id, connection)
            .is_none()
    );

    let patch = EditorGraphMutation::MoveConnections {
        source: left,
        target: right.clone(),
    }
    .into_patch(&graph_path(), &document, registry.as_ref())
    .expect("moving compatible connections must not require an external snapshot");
    apply_graph_document_patch(&mut document, &patch).expect("planned move must be valid");

    assert_eq!(document.connections.len(), 1);
    let moved = document
        .connections
        .values()
        .next()
        .expect("moved connection must exist");
    assert_eq!(moved.output, output);
    assert_eq!(moved.input, right);
}

fn fan_out_move_document() -> (GraphDocument, PortAddress, PortAddress, [NodeId; 2]) {
    let mut document = GraphDocument::default();
    let source = insert_node(&mut document, document_node("yssbi.constant.pi", 0.0));
    let target = insert_node(&mut document, document_node("yssbi.constant.pi", 100.0));
    let inputs = [
        insert_node(
            &mut document,
            document_node("yssbi.numeric.subtract", 200.0),
        ),
        insert_node(
            &mut document,
            document_node("yssbi.numeric.subtract", 300.0),
        ),
    ];
    let source = declared(source, "value");
    let target = declared(target, "value");
    for (index, node) in inputs.iter().enumerate() {
        let id = yss_graph_document::ConnectionId::from_bytes((index as u128 + 1).to_be_bytes());
        document.connections.insert(
            id,
            DocumentConnection {
                id,
                output: source.clone(),
                input: declared(*node, "left"),
                order: None,
            },
        );
    }
    (document, source, target, inputs)
}

#[test]
fn output_move_preserves_all_branches_and_existing_target_links_with_exact_undo() {
    let registry = build_builtin_node_system().unwrap().registry;
    let (mut document, source, target, inputs) = fan_out_move_document();
    let retained = DocumentConnection {
        id: yss_graph_document::ConnectionId::new(),
        output: target.clone(),
        input: declared(inputs[0], "right"),
        order: None,
    };
    document.connections.insert(retained.id, retained.clone());
    let before = document.clone();
    let patch = EditorGraphMutation::MoveConnections {
        source,
        target: target.clone(),
    }
    .into_patch(&graph_path(), &document, registry.as_ref())
    .unwrap();
    assert_eq!(document, before);
    apply_graph_document_patch(&mut document, &patch).unwrap();

    assert_eq!(document.connections.len(), 3);
    assert_eq!(document.connections.get(&retained.id), Some(&retained));
    for node in inputs {
        assert!(document.connections.values().any(|connection| {
            connection.output == target && connection.input == declared(node, "left")
        }));
    }
    apply_graph_document_patch(&mut document, &patch.inverse()).unwrap();
    assert_eq!(document, before);
}

#[test]
fn output_move_rejects_a_conflict_on_a_later_branch_without_editing() {
    let registry = build_builtin_node_system().unwrap().registry;
    let (mut document, source, target, inputs) = fan_out_move_document();
    let id = yss_graph_document::ConnectionId::new();
    document.connections.insert(
        id,
        DocumentConnection {
            id,
            output: target.clone(),
            input: declared(inputs[1], "left"),
            order: None,
        },
    );
    let before = document.clone();
    let error = EditorGraphMutation::MoveConnections { source, target }
        .into_patch(&graph_path(), &document, registry.as_ref())
        .unwrap_err();
    assert!(matches!(error, crate::MutationConflict::Editor(error)
        if error.code == EditorMutationErrorCode::GraphConnectionAlreadyExists));
    assert_eq!(document, before);
}

#[test]
fn grouped_initial_ports_are_offered_and_created_as_complete_members() {
    use std::sync::Arc;
    use yss_data_contract::{DataValue, SemanticType};
    use yss_graph_resource_contract::ResourceCatalogSnapshot;
    use yss_node_protocol::{NodeTypingSpec, PortCardinality, PortDirection, PortMemberGroupSpec};
    use yss_node_registry::{
        LeafImplementation, NodeRegistryBuilder, ProviderRegistration, RegisteredNode,
    };

    let builtin = build_builtin_node_system().unwrap();
    let mut builder = NodeRegistryBuilder::new();
    yss_node_catalog::register_builtin_nodes(&mut builder).unwrap();
    let mut provider = ProviderRegistration::new("tests".parse().unwrap());
    provider.nodes = [("tests.group.required", 2), ("tests.group.optional", 0)]
        .into_iter()
        .map(|(id, min)| {
            let mut protocol = builtin
                .registry
                .protocol(&"yssbi.numeric.subtract".parse().unwrap())
                .unwrap()
                .clone();
            protocol.type_id = id.parse().unwrap();
            protocol.typing = NodeTypingSpec::Fixed;
            for port in &mut protocol.interface.ports {
                port.value_type = TypeExpr::Concrete("core.numeric".parse().unwrap());
                if port.direction == PortDirection::Input {
                    port.cardinality = PortCardinality::UserCreated { min: 0, max: None };
                }
            }
            protocol.interface = protocol
                .interface
                .with_member_groups(vec![PortMemberGroupSpec {
                    templates: ["left".parse().unwrap(), "right".parse().unwrap()].into(),
                    min,
                    max: Some(4),
                }])
                .unwrap();
            RegisteredNode::leaf(Arc::new(protocol), LeafImplementation::new(id))
        })
        .collect();
    builder.register_provider(provider).unwrap();
    let registry = builder.freeze().unwrap();
    let mut document = GraphDocument::default();
    let source = insert_node(&mut document, document_node("yssbi.constant.get", 0.0));
    set_constant(
        &mut document,
        source,
        ValueType::Scalar(SemanticType::Numeric),
        DataValue::Integer(1),
    );
    let before = document.clone();
    let source_port = crate::SourcePort {
        address: declared(source, "value"),
        direction: PortDirection::Output,
        value_type: TypeExpr::Concrete("core.numeric".parse().unwrap()),
    };
    let catalog = crate::filter_compatible_catalog(
        &graph_path(),
        &registry,
        &source_port,
        &ResourceCatalogSnapshot::new(BTreeMap::new(), BTreeMap::new()),
        &[],
        builtin.catalog.localize(&registry, "en-US"),
    );
    let item = catalog
        .items
        .iter()
        .find(|item| item.node_type_id.as_ref() == "tests.group.required")
        .expect(
            "required group members exist immediately and must be connectable from the catalog",
        );
    assert!(
        !catalog
            .items
            .iter()
            .any(|item| item.node_type_id.as_ref() == "tests.group.optional")
    );
    let authority = CatalogMutationValidationSnapshot {
        resources: BTreeMap::new(),
    };
    let patch = EditorGraphMutation::CreateNode {
        port_counts: Default::default(),
        parameters: Default::default(),
        descriptor: item.creation.clone(),
        position: NodePosition { x: 100.0, y: 0.0 },
        user_label: None,
        connect_from: Some(source_port.address.clone()),
    }
    .into_patch_with_context(
        &graph_path(),
        GraphDocumentRead::new(&document),
        &registry,
        EditorMutationContext {
            catalog: Some(&authority),
            semantics: None,
        },
    )
    .unwrap();
    assert_eq!(document, before);
    apply_graph_document_patch(&mut document, &patch).unwrap();
    assert_eq!(document.nodes.len(), 2);
    assert_eq!(document.connections.len(), 1);
    assert_eq!(document.port_bindings.len(), 4);
    let connection = document.connections.values().next().unwrap();
    assert_eq!(connection.output, source_port.address);
    assert!(document.port_bindings.contains_key(&connection.input));
    let protocol = registry
        .protocol(&document.nodes[&connection.input.node_id].node_type)
        .unwrap();
    let members = yss_graph_document_edit::port_member_group_state(
        connection.input.node_id,
        &protocol.interface.member_groups[0],
        &document.port_bindings,
    );
    assert_eq!(members.complete_count(), 2);
    apply_graph_document_patch(&mut document, &patch.inverse()).unwrap();
    assert_eq!(document, before);

    for (kind, count) in [
        ("tests.group.required", 3),
        ("tests.group.optional", 3),
        ("tests.group.optional", 0),
    ] {
        let patch = EditorGraphMutation::CreateNode {
            descriptor: static_descriptor(&registry, kind),
            position: NodePosition { x: 0., y: 0. },
            parameters: Default::default(),
            port_counts: [("left".parse().unwrap(), count)].into(),
            user_label: None,
            connect_from: (count > 0).then(|| source_port.address.clone()),
        }
        .into_patch_with_context(
            &graph_path(),
            GraphDocumentRead::new(&document),
            &registry,
            EditorMutationContext {
                catalog: Some(&authority),
                semantics: None,
            },
        )
        .unwrap();
        apply_graph_document_patch(&mut document, &patch).unwrap();
        assert_eq!(document.port_bindings.len(), usize::from(count) * 2);
        assert_eq!(document.connections.len(), usize::from(count > 0));
        apply_graph_document_patch(&mut document, &patch.inverse()).unwrap();
        assert_eq!(document, before);
    }
    for counts in [
        vec![("left", 1)],
        vec![("left", 5)],
        vec![("result", 2)],
        vec![("missing", 1)],
        vec![("left", 2), ("right", 3)],
    ] {
        assert!(
            EditorGraphMutation::CreateNode {
                descriptor: item.creation.clone(),
                position: NodePosition { x: 0., y: 0. },
                parameters: Default::default(),
                port_counts: counts
                    .into_iter()
                    .map(|(key, count)| (key.parse().unwrap(), count))
                    .collect(),
                user_label: None,
                connect_from: None,
            }
            .into_patch(&graph_path(), &document, &registry)
            .is_err()
        );
        assert_eq!(document, before);
    }
    let decompose = registry
        .protocol(&"yssbi.dataframe.decompose".parse().unwrap())
        .unwrap();
    let derived = decompose
        .interface
        .ports
        .iter()
        .find(|port| matches!(port.cardinality, PortCardinality::Derived { .. }))
        .unwrap();
    assert!(
        EditorGraphMutation::CreateNode {
            descriptor: static_descriptor(&registry, decompose.type_id.as_str()),
            position: NodePosition { x: 0., y: 0. },
            parameters: Default::default(),
            port_counts: [(derived.key.clone(), 4)].into(),
            user_label: None,
            connect_from: None,
        }
        .into_patch(&graph_path(), &document, &registry)
        .is_err()
    );
}

#[test]
fn function_connection_fallback_checks_the_member_resolver_before_its_type() {
    use yss_data_contract::{DataValue, SemanticType};
    use yss_graph_resource_contract::{FunctionParameterContract, FunctionSignature};
    use yss_node_catalog::CatalogResourcePath;

    let registry = build_builtin_node_system().unwrap().registry;
    let function = GraphResourcePath::new("functions/Measure.yssbi-function").unwrap();
    let numeric = ValueType::Scalar(SemanticType::Numeric);
    let catalog = CatalogMutationValidationSnapshot {
        resources: BTreeMap::from([(
            CatalogResourcePath::new(function.as_str()),
            crate::CatalogMutationResource::Function {
                revision: 1,
                signature: FunctionSignature::new(
                    vec![FunctionParameterContract::new(
                        FunctionParameterId::new("amount"),
                        "Amount",
                        numeric.clone(),
                    )],
                    Some(numeric.clone()),
                ),
            },
        )]),
    };
    let mut original = GraphDocument::default();
    let source = insert_node(&mut original, document_node("yssbi.constant.get", 0.0));
    set_constant(&mut original, source, numeric, DataValue::Integer(1));
    let target = insert_node(
        &mut original,
        document_node("yssbi.numeric.subtract", 200.0),
    );
    let mut call = document_node("yssbi.project.function.call", 100.0);
    call.parameters.insert(
        "target".parse().unwrap(),
        serde_json::json!(function.as_str()),
    );
    let call = insert_node(&mut original, call);
    for (template, wrong, correct) in [
        ("arguments", "return", "amount"),
        ("results", "amount", "return"),
    ] {
        let mut document = original.clone();
        let member = PortAddress::instance(call, template.parse().unwrap(), PortInstanceId::new());
        let binding = |parameter| DynamicPortBinding::Resolved {
            origin: DynamicMemberLocator::FunctionParameter {
                function: function.clone(),
                parameter: FunctionParameterId::new(parameter),
            },
            order: OrderKey::new("00000"),
            last_known: LastKnownPortMetadata::default(),
        };
        document
            .port_bindings
            .insert(member.clone(), binding(wrong));
        let (output, input) = if template == "arguments" {
            (declared(source, "value"), member.clone())
        } else {
            (member.clone(), declared(target, "left"))
        };
        let connect = EditorGraphMutation::Connect {
            output,
            input,
            order: None,
        };
        let context = EditorMutationContext {
            catalog: Some(&catalog),
            semantics: None,
        };
        let error = connect
            .clone()
            .into_patch_with_context(
                &graph_path(),
                GraphDocumentRead::new(&document),
                &registry,
                context,
            )
            .expect_err("matching numeric types cannot authorize a member from the wrong resolver");
        assert!(matches!(error, crate::MutationConflict::Editor(error)
            if error.code == EditorMutationErrorCode::GraphConnectionTypeUnavailable));
        assert!(document.connections.is_empty());
        document.port_bindings.insert(member, binding(correct));
        let before = document.clone();
        let patch = connect
            .into_patch_with_context(
                &graph_path(),
                GraphDocumentRead::new(&document),
                &registry,
                context,
            )
            .unwrap();
        apply_graph_document_patch(&mut document, &patch).unwrap();
        assert_eq!(document.connections.len(), 1);
        apply_graph_document_patch(&mut document, &patch.inverse()).unwrap();
        assert_eq!(document, before);
    }
}

#[test]
fn create_and_connect_plans_one_atomic_patch() {
    let registry = build_builtin_node_system()
        .expect("built-in registry must assemble")
        .registry;
    let mut document = GraphDocument::default();
    let source = insert_node(&mut document, document_node("yssbi.constant.get", 0.0));
    set_constant(
        &mut document,
        source,
        ValueType::Scalar(yss_data_contract::SemanticType::Numeric),
        yss_data_contract::DataValue::Integer(0),
    );
    let source_output = declared(source, "value");
    let catalog = CatalogMutationValidationSnapshot {
        resources: BTreeMap::new(),
    };

    let patch = EditorGraphMutation::CreateNode {
        port_counts: Default::default(),
        parameters: Default::default(),
        descriptor: static_descriptor(registry.as_ref(), "yssbi.numeric.add"),
        position: NodePosition { x: 100.0, y: 0.0 },
        user_label: Some("sum".into()),
        connect_from: Some(source_output.clone()),
    }
    .into_patch_with_context(
        &graph_path(),
        GraphDocumentRead::new(&document),
        registry.as_ref(),
        EditorMutationContext {
            catalog: Some(&catalog),
            semantics: None,
        },
    )
    .expect("creation must derive a compatible target port from catalog authority");
    apply_graph_document_patch(&mut document, &patch).expect("atomic creation patch must apply");

    assert_eq!(document.nodes.len(), 2);
    assert_eq!(document.connections.len(), 1);
    let connection = document
        .connections
        .values()
        .next()
        .expect("created node must be connected");
    assert_eq!(connection.output, source_output);
    let created = &document.nodes[&connection.input.node_id];
    assert_eq!(created.node_type.as_str(), "yssbi.numeric.add");
    assert_eq!(created.user_label.as_deref(), Some("sum"));
    let input = connection.input.clone();
    let before = document.clone();
    let parameters = document.nodes[&source].parameters.clone();
    let configured = EditorGraphMutation::CreateNode {
        port_counts: Default::default(),
        descriptor: static_descriptor(registry.as_ref(), "yssbi.constant.get"),
        position: NodePosition { x: 50.0, y: 0.0 },
        parameters: parameters.clone(),
        user_label: None,
        connect_from: Some(input),
    }
    .into_patch_with_context(
        &graph_path(),
        GraphDocumentRead::new(&document),
        &registry,
        EditorMutationContext {
            catalog: Some(&catalog),
            semantics: None,
        },
    )
    .unwrap();
    apply_graph_document_patch(&mut document, &configured).unwrap();
    let connection = document.connections.values().next().unwrap();
    assert_ne!(connection.output.node_id, source);
    assert_eq!(
        document.nodes[&connection.output.node_id].parameters,
        parameters
    );
    assert_eq!(document.nodes.len(), 3);
    assert_eq!(document.connections.len(), 1);
    apply_graph_document_patch(&mut document, &configured.inverse()).unwrap();
    assert_eq!(document, before);
}

#[test]
fn port_resolution_rejects_binding_kind_drift() {
    let registry = build_builtin_node_system()
        .expect("built-in registry must assemble")
        .registry;
    let mut document = GraphDocument::default();
    let node = insert_node(&mut document, document_node("yssbi.numeric.add", 0.0));
    let address = PortAddress::instance(
        node,
        PortKey::new("operands").expect("fixture port key must be valid"),
        PortInstanceId::new(),
    );
    document.port_bindings.insert(
        address.clone(),
        DynamicPortBinding::Resolved {
            origin: DynamicMemberLocator::FunctionParameter {
                function: GraphResourcePath::new("functions/F.yssbi-function")
                    .expect("fixture resource path must be valid"),
                parameter: FunctionParameterId::new("value"),
            },
            order: OrderKey::new("00000"),
            last_known: LastKnownPortMetadata::default(),
        },
    );

    let error = crate::compatibility::resolve_editor_port(&document, registry.as_ref(), &address)
        .expect_err("a user-created template must reject a resolved binding");

    assert_eq!(error.code, EditorMutationErrorCode::GraphPortNotFound);
    assert!(error.detail.contains("binding kind"));
}

#[test]
fn remove_port_instance_cleans_up_an_orphaned_derived_port() {
    let registry = build_builtin_node_system()
        .expect("built-in registry must assemble")
        .registry;
    let mut document = GraphDocument::default();
    let node_id = insert_node(
        &mut document,
        document_node("yssbi.project.function.call", 0.0),
    );
    let address = PortAddress::instance(
        node_id,
        PortKey::new("arguments").expect("fixture port key must be valid"),
        PortInstanceId::new(),
    );
    document.port_bindings.insert(
        address.clone(),
        DynamicPortBinding::Orphan {
            origin: DynamicMemberLocator::FunctionParameter {
                function: GraphResourcePath::new("functions/Missing.yssbi-function")
                    .expect("fixture resource path must be valid"),
                parameter: FunctionParameterId::new("value"),
            },
            order: OrderKey::new("00000"),
            last_known: LastKnownPortMetadata {
                label: "Value".into(),
                value_type: Some(TypeExpr::Concrete(
                    "core.numeric"
                        .parse()
                        .expect("fixture type ID must be valid"),
                )),
            },
        },
    );

    let patch = EditorGraphMutation::RemovePortInstance {
        address: address.clone(),
    }
    .into_patch(&graph_path(), &document, registry.as_ref())
    .expect("orphaned derived ports must be removable");
    apply_graph_document_patch(&mut document, &patch)
        .expect("orphan cleanup patch must remain structurally valid");

    assert!(!document.port_bindings.contains_key(&address));
}

#[test]
fn constant_edits_preserve_reference_identity_and_reject_duplicate_names_atomically() {
    use yss_graph_document::{ConstantId, GraphConstant};
    let registry = build_builtin_node_system().unwrap().registry;
    let mut document = GraphDocument::default();
    let apply = |document: &mut GraphDocument, mutation: EditorGraphMutation| {
        let patch = mutation
            .into_patch(&graph_path(), document, &registry)
            .unwrap();
        apply_graph_document_patch(document, &patch).unwrap();
        patch
    };
    let id = ConstantId::new();
    apply(
        &mut document,
        EditorGraphMutation::SetConstant {
            id,
            constant: Some(GraphConstant {
                id,
                name: " Count ".into(),
                data_type: ValueType::Scalar(yss_data_contract::SemanticType::Numeric),
                data_value: yss_data_contract::DataValue::Integer(42),
                tabular: None,
                description: String::new(),
                tags: vec![],
            }),
        },
    );
    assert_eq!(document.constants[&id].name, "Count");
    apply(
        &mut document,
        EditorGraphMutation::InsertConstantReference {
            id,
            position: NodePosition { x: 10.0, y: 20.0 },
        },
    );
    let references = document.nodes.clone();
    let mut renamed = document.constants[&id].clone();
    renamed.name = "Renamed".into();
    apply(
        &mut document,
        EditorGraphMutation::SetConstant {
            id,
            constant: Some(renamed.clone()),
        },
    );
    assert_eq!(document.nodes, references);
    let before = document.clone();
    let duplicate = renamed.copy_with_id(ConstantId::new());
    let invalid = EditorGraphMutation::SetConstant {
        id: duplicate.id,
        constant: Some(duplicate),
    }
    .into_patch(&graph_path(), &document, &registry)
    .unwrap();
    assert!(apply_graph_document_patch(&mut document, &invalid).is_err());
    assert_eq!(document, before);
    let delete = apply(
        &mut document,
        EditorGraphMutation::SetConstant { id, constant: None },
    );
    assert!(document.constants.is_empty());
    assert_eq!(document.nodes, references);
    apply_graph_document_patch(&mut document, &delete.inverse()).unwrap();
    assert_eq!(document, before);
}

fn set_constant(
    document: &mut GraphDocument,
    node: NodeId,
    data_type: yss_data_contract::ValueType,
    data_value: yss_data_contract::DataValue,
) {
    let id = yss_graph_document::ConstantId::from_uuid(node.as_uuid());
    document.constants.insert(
        id,
        yss_graph_document::GraphConstant {
            id,
            name: id.to_string(),
            data_type,
            data_value,
            tabular: None,
            description: String::new(),
            tags: vec![],
        },
    );
    let node = document.nodes.get_mut(&node).unwrap();
    node.node_type = "yssbi.constant.get".parse().unwrap();
    node.parameters = ParameterValues::from([(
        "constant".parse().unwrap(),
        serde_json::json!(id.to_string()),
    )]);
}

#[test]
fn clipboard_constants_preserve_values_resolve_collisions_and_undo_atomically() {
    use std::sync::Arc;
    use yss_node_protocol::{NodeTypingSpec, ParameterEditorSpec, TypedValue};
    use yss_node_registry::{
        LeafImplementation, NodeRegistryBuilder, ProviderRegistration, RegisteredNode,
    };
    let builtin = build_builtin_node_system().unwrap();
    let catalog = CatalogMutationValidationSnapshot {
        resources: BTreeMap::new(),
    };
    let mut source = GraphDocument::default();
    let node = insert_node(&mut source, document_node("yssbi.constant.get", 0.0));
    set_constant(
        &mut source,
        node,
        ValueType::Scalar(yss_data_contract::SemanticType::Numeric),
        yss_data_contract::DataValue::Integer(42),
    );
    let constant_id = *source.constants.keys().next().unwrap();
    let mut protocol = builtin
        .registry
        .protocol(&"yssbi.constant.get".parse().unwrap())
        .unwrap()
        .clone();
    protocol.type_id = "tests.constant.reference".parse().unwrap();
    let mut reference = protocol.parameters.groups[0].parameters[0].clone();
    reference.key = "selection".parse().unwrap();
    reference.default_value = Some(TypedValue {
        value_type: reference.value_type.clone(),
        value: yss_data_contract::DataValue::String(constant_id.to_string().into()),
    });
    let mut note = reference.clone();
    note.key = "note".parse().unwrap();
    note.editor = ParameterEditorSpec::Text { multiline: false };
    note.default_value = None;
    note.constraints.clear();
    protocol.parameters.groups[0].parameters = [reference, note].into();
    protocol.typing = NodeTypingSpec::ConstantOutput {
        parameter: "selection".parse().unwrap(),
        output: "value".parse().unwrap(),
    };
    let mut builder = NodeRegistryBuilder::new();
    yss_node_catalog::register_builtin_nodes(&mut builder).unwrap();
    let mut provider = ProviderRegistration::new("tests.constants".parse().unwrap());
    provider.nodes = [RegisteredNode::leaf(
        Arc::new(protocol),
        LeafImplementation::new("tests.constant"),
    )]
    .into();
    builder.register_provider(provider).unwrap();
    let registry = builder.freeze().unwrap();
    let mut selected = vec![node];
    for explicit in [true, false] {
        let mut custom = document_node("tests.constant.reference", 0.0);
        custom.parameters.insert(
            "note".parse().unwrap(),
            serde_json::json!(constant_id.to_string()),
        );
        if explicit {
            custom.parameters.insert(
                "selection".parse().unwrap(),
                serde_json::json!(constant_id.to_string()),
            );
        }
        let custom = insert_node(&mut source, custom);
        let exported = crate::export_subgraph(&source, &registry, &catalog, vec![custom]).unwrap();
        assert_eq!(exported.constants.len(), 1);
        assert_eq!(
            exported.nodes[0].parameters[&"selection".parse().unwrap()],
            serde_json::json!(constant_id.to_string())
        );
        assert_eq!(
            source.nodes[&custom]
                .parameters
                .contains_key(&"selection".parse().unwrap()),
            explicit
        );
        selected.push(custom);
    }
    let snapshot = crate::export_subgraph(&source, &registry, &catalog, selected.clone()).unwrap();
    let bytes = serde_json::to_vec(&snapshot).unwrap();
    let snapshot = crate::deserialize_clipboard_subgraph(&bytes).unwrap();
    assert_eq!(snapshot.constants.len(), 1);
    let mut target = source.clone();
    target.constants.values_mut().next().unwrap().data_value =
        yss_data_contract::DataValue::Integer(99);
    let before = target.clone();
    let patch = EditorGraphMutation::InsertSubgraph {
        snapshot,
        anchor: NodePosition { x: 200.0, y: 0.0 },
    }
    .into_patch_with_context(
        &graph_path(),
        GraphDocumentRead::new(&target),
        &registry,
        EditorMutationContext {
            catalog: Some(&catalog),
            semantics: None,
        },
    )
    .unwrap();
    apply_graph_document_patch(&mut target, &patch).unwrap();
    assert_eq!(target.constants.len(), 2);
    let pasted = target
        .nodes
        .values()
        .filter(|candidate| !source.nodes.contains_key(&candidate.id))
        .collect::<Vec<_>>();
    assert_eq!(pasted.len(), selected.len());
    for pasted in pasted {
        let protocol = registry.protocol(&pasted.node_type).unwrap();
        let NodeTypingSpec::ConstantOutput { parameter, .. } = &protocol.typing else {
            panic!("constant protocol")
        };
        let id: yss_graph_document::ConstantId = pasted.parameters[parameter]
            .as_str()
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(
            target.constants[&id].data_value,
            yss_data_contract::DataValue::Integer(42)
        );
        assert_ne!(
            target.constants[&id].name,
            source.constants[&constant_id].name
        );
        if pasted.node_type.as_str() == "tests.constant.reference" {
            assert_eq!(
                pasted.parameters[&"note".parse().unwrap()],
                serde_json::json!(constant_id.to_string())
            );
        }
    }
    apply_graph_document_patch(&mut target, &patch.inverse()).unwrap();
    assert_eq!(target, before);
    let patch = EditorGraphMutation::DuplicateSubgraph {
        node_ids: selected,
        offset: NodePosition { x: 200.0, y: 0.0 },
    }
    .into_patch_with_context(
        &graph_path(),
        GraphDocumentRead::new(&source),
        &registry,
        EditorMutationContext {
            catalog: Some(&catalog),
            semantics: None,
        },
    )
    .unwrap();
    apply_graph_document_patch(&mut source, &patch).unwrap();
    assert_eq!(source.constants.len(), 1);
}

#[test]
fn clipboard_connection_limits_reject_partial_imports_and_allow_undo() {
    let builtin = build_builtin_node_system().unwrap();
    let catalog = CatalogMutationValidationSnapshot {
        resources: BTreeMap::new(),
    };
    let mut source = GraphDocument::default();
    let nodes = std::array::from_fn::<_, 3, _>(|_| {
        insert_node(&mut source, document_node("yssbi.logic.not", 0.0))
    });
    let input = declared(nodes[2], "input");
    source.input_states.insert(
        input.clone(),
        yss_graph_document::InputState {
            literal_override: Some(yss_node_protocol::TypedValue {
                value_type: TypeExpr::Concrete("core.binary".parse().unwrap()),
                value: yss_data_contract::DataValue::Bool(false),
            }),
        },
    );
    for node in &nodes[..2] {
        let id = yss_graph_document::ConnectionId::new();
        source.connections.insert(
            id,
            DocumentConnection {
                id,
                output: declared(*node, "result"),
                input: input.clone(),
                order: None,
            },
        );
    }
    let mut snapshot =
        crate::export_subgraph(&source, &builtin.registry, &catalog, nodes.to_vec()).unwrap();
    let mut target = GraphDocument::default();
    let existing = source.nodes[&nodes[0]].clone();
    target.nodes.insert(existing.id, existing);
    let before = target.clone();
    let plan = |snapshot| {
        EditorGraphMutation::InsertSubgraph {
            snapshot,
            anchor: NodePosition { x: 200.0, y: 0.0 },
        }
        .into_patch_with_context(
            &graph_path(),
            GraphDocumentRead::new(&before),
            &builtin.registry,
            EditorMutationContext {
                catalog: Some(&catalog),
                semantics: None,
            },
        )
    };
    assert!(matches!(
        plan(snapshot.clone()),
        Err(crate::MutationConflict::ClipboardSubgraphInvalid(detail))
            if detail.contains("connection limit")
    ));
    assert_eq!(target, before);
    snapshot.connections.pop().unwrap();
    let patch = plan(snapshot).unwrap();
    apply_graph_document_patch(&mut target, &patch).unwrap();
    assert_eq!(target.nodes.len(), 4);
    assert_eq!(target.connections.len(), 1);
    assert_eq!(target.input_states.len(), 1);
    apply_graph_document_patch(&mut target, &patch.inverse()).unwrap();
    assert_eq!(target, before);
}
