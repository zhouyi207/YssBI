use super::*;

#[test]
fn resource_descriptor_materializes_only_function_variable_and_database_bindings() {
    use crate::node_system::catalog::ResourceBoundCreateArgsDto;

    let variable_id = crate::variable::VariableId::new();
    let snapshot = resource_descriptor_snapshot(variable_id);
    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let document = GraphDocument::default();
    let cases = [
        (
            resource_create(
                "yssbi.project.function.call",
                "functions/Helper.yssbi-function",
                3,
                ResourceBoundCreateArgsDto::Function,
            ),
            "target",
            json!("functions/Helper.yssbi-function"),
        ),
        (
            resource_create(
                "yssbi.project.variable.get",
                &format!("variables/{variable_id}"),
                4,
                ResourceBoundCreateArgsDto::Variable,
            ),
            "variable",
            json!(format!("variables/{variable_id}")),
        ),
        (
            resource_create(
                "yssbi.dataframe.source.get",
                "databases/sales",
                5,
                ResourceBoundCreateArgsDto::Database,
            ),
            "dataframe",
            json!("databases/sales"),
        ),
    ];

    for (mutation, parameter, expected) in cases {
        let patch = mutation
            .into_patch_with_catalog_snapshot(
                &graph_path("events/validation"),
                &document,
                &registry,
                Some(&snapshot),
            )
            .unwrap();
        let GraphDocumentOperation::InsertNode { node } = &patch.operations[0] else {
            panic!("creation must insert a node");
        };
        assert_eq!(node.parameters.len(), 1);
        assert_eq!(
            node.parameters[&ParameterKey::new(parameter).unwrap()],
            expected
        );
    }
}

#[test]
fn resource_descriptor_rejects_invalid_stale_scope_and_parameter_injection() {
    use crate::node_system::catalog::ResourceBoundCreateArgsDto;

    let variable_id = crate::variable::VariableId::new();
    let snapshot = resource_descriptor_snapshot(variable_id);
    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let document = GraphDocument::default();
    let invalid = [
        resource_create(
            "yssbi.project.variable.get",
            "functions/Helper.yssbi-function",
            3,
            ResourceBoundCreateArgsDto::Function,
        ),
        resource_create(
            "yssbi.project.function.call",
            "functions/Helper.yssbi-function",
            3,
            ResourceBoundCreateArgsDto::Variable,
        ),
        resource_create(
            "yssbi.project.function.call",
            "variables/not-a-variable-id",
            3,
            ResourceBoundCreateArgsDto::Function,
        ),
    ];
    for mutation in invalid {
        assert_eq!(
            mutation
                .into_patch_with_catalog_snapshot(
                    &graph_path("events/validation"),
                    &document,
                    &registry,
                    Some(&snapshot),
                )
                .unwrap_err()
                .code(),
            "catalog_descriptor_invalid"
        );
    }

    for mutation in [
        resource_create(
            "yssbi.project.function.call",
            "functions/Helper.yssbi-function",
            2,
            ResourceBoundCreateArgsDto::Function,
        ),
        resource_create(
            "yssbi.project.function.call",
            "functions/Missing.yssbi-function",
            1,
            ResourceBoundCreateArgsDto::Function,
        ),
    ] {
        assert_eq!(
            mutation
                .into_patch_with_catalog_snapshot(
                    &graph_path("events/validation"),
                    &document,
                    &registry,
                    Some(&snapshot),
                )
                .unwrap_err()
                .code(),
            "catalog_resource_stale"
        );
    }

    let out_of_scope = resource_create(
        "yssbi.project.variable.get",
        &format!("variables/{variable_id}"),
        4,
        ResourceBoundCreateArgsDto::Variable,
    )
    .into_patch_with_catalog_snapshot(
        &graph_path("events/other"),
        &document,
        &registry,
        Some(&snapshot),
    )
    .unwrap_err();
    assert_eq!(out_of_scope.code(), "catalog_descriptor_invalid");

    let injected = serde_json::from_value::<EditorGraphMutationDto>(json!({
        "type": "createNode",
        "payload": {
            "descriptor": {
                "kind": "resourceBound",
                "nodeTypeId": "yssbi.project.function.call",
                "resourcePath": "functions/Helper.yssbi-function",
                "resourceRevision": 3,
                "createArgs": { "kind": "function" }
            },
            "position": { "x": 1.0, "y": 2.0 },
            "userLabel": null,
            "parameters": { "target": "functions/Injected.yssbi-function" }
        }
    }));
    assert!(injected.is_err());
}

#[test]
fn resource_descriptor_rejects_noncanonical_paths_before_snapshot_lookup() {
    use crate::node_system::catalog::ResourceBoundCreateArgsDto;

    let variable_id = crate::variable::VariableId::new();
    let snapshot = resource_descriptor_snapshot(variable_id);
    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let document = GraphDocument::default();
    let variable = variable_id.to_string();
    let noncanonical = [
        (
            "yssbi.project.function.call",
            r"functions\Helper.yssbi-function".to_string(),
            ResourceBoundCreateArgsDto::Function,
        ),
        (
            "yssbi.project.function.call",
            "/functions/Helper.yssbi-function".to_string(),
            ResourceBoundCreateArgsDto::Function,
        ),
        (
            "yssbi.project.function.call",
            "functions//Helper.yssbi-function".to_string(),
            ResourceBoundCreateArgsDto::Function,
        ),
        (
            "yssbi.project.function.call",
            "events/Helper.yssbi-event".to_string(),
            ResourceBoundCreateArgsDto::Function,
        ),
        (
            "yssbi.project.variable.get",
            format!("variables/{}", variable.to_uppercase()),
            ResourceBoundCreateArgsDto::Variable,
        ),
        (
            "yssbi.project.variable.get",
            format!("variables/{}", variable.replace('-', "")),
            ResourceBoundCreateArgsDto::Variable,
        ),
        (
            "yssbi.project.variable.get",
            format!("variables//{variable}"),
            ResourceBoundCreateArgsDto::Variable,
        ),
        (
            "yssbi.project.variable.get",
            format!(r"variables\{variable}"),
            ResourceBoundCreateArgsDto::Variable,
        ),
        (
            "yssbi.dataframe.source.get",
            r"databases\sales".to_string(),
            ResourceBoundCreateArgsDto::Database,
        ),
        (
            "yssbi.dataframe.source.get",
            "/databases/sales".to_string(),
            ResourceBoundCreateArgsDto::Database,
        ),
        (
            "yssbi.dataframe.source.get",
            "database/sales".to_string(),
            ResourceBoundCreateArgsDto::Database,
        ),
        (
            "yssbi.dataframe.source.get",
            "databases/".to_string(),
            ResourceBoundCreateArgsDto::Database,
        ),
    ];

    for (node_type, path, create_args) in noncanonical {
        let error = resource_create(node_type, &path, 1, create_args)
            .into_patch_with_catalog_snapshot(
                &graph_path("events/validation"),
                &document,
                &registry,
                Some(&snapshot),
            )
            .unwrap_err();
        assert_eq!(error.code(), "catalog_descriptor_invalid", "path: {path}");
    }

    for missing in [
        resource_create(
            "yssbi.project.function.call",
            "functions/CanonicalMissing.yssbi-function",
            1,
            ResourceBoundCreateArgsDto::Function,
        ),
        resource_create(
            "yssbi.dataframe.source.get",
            "databases/ sales / .. # opaque ",
            1,
            ResourceBoundCreateArgsDto::Database,
        ),
    ] {
        let error = missing
            .into_patch_with_catalog_snapshot(
                &graph_path("events/validation"),
                &document,
                &registry,
                Some(&snapshot),
            )
            .unwrap_err();
        assert_eq!(error.code(), "catalog_resource_stale");
    }
}

#[test]
fn editor_delete_rejects_managed_protocol_and_preserves_required_shell_node() {
    let managed_id = node_id(1_001);
    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let mut document = GraphDocument::default();
    document
        .create_node(DocumentNode {
            id: managed_id,
            node_type: NodeTypeId::new("yssbi.project.event.begin").unwrap(),
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters: ParameterValues::new(),
            user_label: None,
        })
        .unwrap();

    let error = plan(
        EditorGraphMutationDto::DeleteNodes {
            node_ids: vec![managed_id],
        },
        &document,
        &registry,
    )
    .unwrap_err();

    assert_editor_error_code(
        error,
        EditorMutationErrorCode::GraphManagedNodeDeleteForbidden,
    );
    assert!(document.nodes.contains_key(&managed_id));
}

#[test]
fn editor_delete_keeps_normal_protocol_deletable() {
    let ordinary_id = node_id(1_002);
    let registry = validation_registry();
    let document = validation_document(&[ordinary_id]);

    let patch = plan(
        EditorGraphMutationDto::DeleteNodes {
            node_ids: vec![ordinary_id],
        },
        &document,
        &registry,
    )
    .unwrap();

    assert!(patch.operations.iter().any(|operation| matches!(
        operation,
        GraphDocumentOperation::RemoveNode { node } if node.id == ordinary_id
    )));
}

#[test]
fn phase1_collection_delete_nodes_is_deterministic_and_reversible() {
    let registry = validation_registry();
    let first = node_id(2_001);
    let second = node_id(2_002);
    let survivor = node_id(2_003);
    let mut document = validation_document(&[first, second, survivor]);
    let first_input = user_address(first, 2_101);
    let second_input = user_address(second, 2_102);
    document.port_bindings.insert(
        second_input.clone(),
        DynamicPortBinding::UserCreated {
            order: OrderKey("b".into()),
        },
    );
    document.port_bindings.insert(
        first_input.clone(),
        DynamicPortBinding::UserCreated {
            order: OrderKey("a".into()),
        },
    );
    document.input_states.insert(
        second_input.clone(),
        InputState {
            literal_override: Some(json!(2)),
        },
    );
    document.input_states.insert(
        first_input.clone(),
        InputState {
            literal_override: Some(json!(1)),
        },
    );
    for connection in [
        DocumentConnection {
            id: connection_id(30),
            output: declared(survivor, "data_out"),
            input: first_input.clone(),
            order: None,
        },
        DocumentConnection {
            id: connection_id(10),
            output: declared(first, "data_out"),
            input: second_input.clone(),
            order: None,
        },
        DocumentConnection {
            id: connection_id(20),
            output: declared(second, "data_out"),
            input: declared(survivor, "data_in"),
            order: None,
        },
    ] {
        document.connections.insert(connection.id, connection);
    }
    let before = document.clone();

    let patch = plan(
        EditorGraphMutationDto::DeleteNodes {
            node_ids: vec![second, first],
        },
        &document,
        &registry,
    )
    .unwrap();

    let removed_connections = patch
        .operations
        .iter()
        .filter_map(|operation| match operation {
            GraphDocumentOperation::RemoveConnection { connection } => Some(connection.id),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        removed_connections,
        vec![connection_id(10), connection_id(20), connection_id(30)]
    );
    assert_eq!(
        patch.operations[3..5]
            .iter()
            .map(|operation| match operation {
                GraphDocumentOperation::SetInputState { address, .. } => address.clone(),
                other => panic!("unexpected operation: {other:?}"),
            })
            .collect::<Vec<_>>(),
        vec![first_input.clone(), second_input.clone()]
    );
    assert_eq!(
        patch.operations[5..7]
            .iter()
            .map(|operation| match operation {
                GraphDocumentOperation::RemovePortBinding { address, .. } => address.clone(),
                other => panic!("unexpected operation: {other:?}"),
            })
            .collect::<Vec<_>>(),
        vec![first_input.clone(), second_input.clone()]
    );
    assert!(matches!(
        &patch.operations[7],
        GraphDocumentOperation::RemoveNode { node } if node.id == first
    ));
    assert!(matches!(
        &patch.operations[8],
        GraphDocumentOperation::RemoveNode { node } if node.id == second
    ));

    document.apply_patch(&patch).unwrap();
    assert_eq!(
        document.nodes.keys().copied().collect::<Vec<_>>(),
        vec![survivor]
    );
    document.apply_patch(&patch.inverse()).unwrap();
    assert_graph_content_eq(&document, &before);
}

#[test]
fn phase1_collection_delete_nodes_validates_all_direct_targets() {
    let registry = validation_registry();
    let existing = node_id(2_011);
    let missing = node_id(2_012);
    let document = validation_document(&[existing]);

    for (node_ids, expected) in [
        (vec![], EditorMutationErrorCode::GraphMutationEmptyTargets),
        (
            vec![existing, existing],
            EditorMutationErrorCode::GraphMutationDuplicateTarget,
        ),
        (
            vec![existing, missing],
            EditorMutationErrorCode::GraphNodeNotFound,
        ),
    ] {
        let error = plan(
            EditorGraphMutationDto::DeleteNodes { node_ids },
            &document,
            &registry,
        )
        .unwrap_err();
        assert_editor_error_code(error, expected);
    }
}

#[test]
fn phase1_collection_delete_nodes_rejects_mixed_managed_selection() {
    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let ordinary = node_id(2_021);
    let managed = node_id(2_022);
    let mut document = GraphDocument::default();
    for (id, node_type) in [
        (ordinary, "yssbi.project.variable.get"),
        (managed, "yssbi.project.event.begin"),
    ] {
        document
            .create_node(DocumentNode {
                id,
                node_type: NodeTypeId::new(node_type).unwrap(),
                position: NodePosition { x: 0.0, y: 0.0 },
                parameters: ParameterValues::new(),
                user_label: None,
            })
            .unwrap();
    }
    let before = document.clone();

    let error = plan(
        EditorGraphMutationDto::DeleteNodes {
            node_ids: vec![ordinary, managed],
        },
        &document,
        &registry,
    )
    .unwrap_err();

    assert_editor_error_code(
        error,
        EditorMutationErrorCode::GraphManagedNodeDeleteForbidden,
    );
    assert_graph_content_eq(&document, &before);
}

#[test]
fn phase1_collection_move_nodes_sorts_operations_after_full_validation() {
    let registry = validation_registry();
    let first = node_id(2_025);
    let second = node_id(2_026);
    let document = validation_document(&[first, second]);
    let first_target = NodePositionMutationDto {
        node_id: first,
        position: NodePosition { x: 10.0, y: 20.0 },
    };
    let second_target = NodePositionMutationDto {
        node_id: second,
        position: NodePosition { x: 30.0, y: 40.0 },
    };

    let reversed = plan(
        EditorGraphMutationDto::MoveNodes {
            positions: vec![second_target.clone(), first_target.clone()],
        },
        &document,
        &registry,
    )
    .unwrap();
    let forward = plan(
        EditorGraphMutationDto::MoveNodes {
            positions: vec![first_target, second_target],
        },
        &document,
        &registry,
    )
    .unwrap();

    assert_eq!(reversed, forward);
    assert_eq!(
        reversed
            .operations
            .iter()
            .map(|operation| match operation {
                GraphDocumentOperation::UpdateNode { after, .. } => after.id,
                other => panic!("unexpected operation: {other:?}"),
            })
            .collect::<Vec<_>>(),
        vec![first, second]
    );
}

#[test]
fn phase1_collection_disconnect_connections_validates_sorts_and_restores() {
    let registry = validation_registry();
    let first = node_id(2_031);
    let second = node_id(2_032);
    let mut document = validation_document(&[first, second]);
    for id in [connection_id(42), connection_id(41)] {
        document.connections.insert(
            id,
            DocumentConnection {
                id,
                output: declared(first, "data_out"),
                input: declared(second, "ordered_in"),
                order: Some(OrderKey(id.to_string().into())),
            },
        );
    }
    let before = document.clone();

    let patch = plan(
        EditorGraphMutationDto::DisconnectConnections {
            connection_ids: vec![connection_id(42), connection_id(41)],
        },
        &document,
        &registry,
    )
    .unwrap();
    assert_eq!(
        patch
            .operations
            .iter()
            .map(|operation| match operation {
                GraphDocumentOperation::RemoveConnection { connection } => connection.id,
                other => panic!("unexpected operation: {other:?}"),
            })
            .collect::<Vec<_>>(),
        vec![connection_id(41), connection_id(42)]
    );
    document.apply_patch(&patch).unwrap();
    document.apply_patch(&patch.inverse()).unwrap();
    assert_graph_content_eq(&document, &before);

    for (connection_ids, expected) in [
        (vec![], EditorMutationErrorCode::GraphMutationEmptyTargets),
        (
            vec![connection_id(41), connection_id(41)],
            EditorMutationErrorCode::GraphMutationDuplicateTarget,
        ),
    ] {
        let error = plan(
            EditorGraphMutationDto::DisconnectConnections { connection_ids },
            &before,
            &registry,
        )
        .unwrap_err();
        assert_editor_error_code(error, expected);
    }

    let error = plan(
        EditorGraphMutationDto::DisconnectConnections {
            connection_ids: vec![connection_id(41), connection_id(99)],
        },
        &before,
        &registry,
    )
    .unwrap_err();
    assert_editor_error_code(error, EditorMutationErrorCode::GraphConnectionNotFound);
    assert_graph_content_eq(&document, &before);
}

#[test]
fn phase1_collection_disconnect_port_and_node_break_all_incident_links() {
    let registry = validation_registry();
    let first = node_id(2_041);
    let second = node_id(2_042);
    let third = node_id(2_043);
    let mut document = validation_document(&[first, second, third]);
    for connection in [
        DocumentConnection {
            id: connection_id(52),
            output: declared(first, "data_out"),
            input: declared(second, "ordered_in"),
            order: Some(OrderKey("b".into())),
        },
        DocumentConnection {
            id: connection_id(51),
            output: declared(first, "data_out"),
            input: declared(third, "data_in"),
            order: None,
        },
        DocumentConnection {
            id: connection_id(53),
            output: declared(second, "data_out"),
            input: declared(third, "ordered_in"),
            order: Some(OrderKey("c".into())),
        },
    ] {
        document.connections.insert(connection.id, connection);
    }

    let port_patch = plan(
        EditorGraphMutationDto::DisconnectPort {
            address: declared(first, "data_out").into(),
        },
        &document,
        &registry,
    )
    .unwrap();
    assert_eq!(
        port_patch
            .operations
            .iter()
            .map(|operation| match operation {
                GraphDocumentOperation::RemoveConnection { connection } => connection.id,
                other => panic!("unexpected operation: {other:?}"),
            })
            .collect::<Vec<_>>(),
        vec![connection_id(51), connection_id(52)]
    );

    let node_patch = plan(
        EditorGraphMutationDto::DisconnectNode { node_id: third },
        &document,
        &registry,
    )
    .unwrap();
    assert_eq!(
        node_patch
            .operations
            .iter()
            .map(|operation| match operation {
                GraphDocumentOperation::RemoveConnection { connection } => connection.id,
                other => panic!("unexpected operation: {other:?}"),
            })
            .collect::<Vec<_>>(),
        vec![connection_id(51), connection_id(53)]
    );

    let before = document.clone();
    let mut port_document = document.clone();
    port_document.apply_patch(&port_patch).unwrap();
    assert_eq!(
        port_document
            .connections
            .keys()
            .copied()
            .collect::<Vec<_>>(),
        vec![connection_id(53)]
    );
    port_document.apply_patch(&port_patch.inverse()).unwrap();
    assert_graph_content_eq(&port_document, &before);

    let mut node_document = document.clone();
    node_document.apply_patch(&node_patch).unwrap();
    assert_eq!(
        node_document
            .connections
            .keys()
            .copied()
            .collect::<Vec<_>>(),
        vec![connection_id(52)]
    );
    node_document.apply_patch(&node_patch.inverse()).unwrap();
    assert_graph_content_eq(&node_document, &before);

    assert_editor_error_code(
        plan(
            EditorGraphMutationDto::DisconnectPort {
                address: declared(first, "missing").into(),
            },
            &document,
            &registry,
        )
        .unwrap_err(),
        EditorMutationErrorCode::GraphPortNotFound,
    );
    assert_editor_error_code(
        plan(
            EditorGraphMutationDto::DisconnectNode {
                node_id: node_id(2_099),
            },
            &document,
            &registry,
        )
        .unwrap_err(),
        EditorMutationErrorCode::GraphNodeNotFound,
    );
}
