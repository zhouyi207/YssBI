use super::*;

#[test]
fn phase1_type_snapshot_validates_concrete_and_separate_generic_assignability() {
    let float = TypeExpr::Concrete(TypeId::new("core.float64").unwrap());
    let string = TypeExpr::Concrete(TypeId::new("core.string").unwrap());
    let item = TypeParameterId::new("item").unwrap();
    let target_item = TypeParameterId::new("target_item").unwrap();
    for (source, source_parameters, target, target_parameters) in [
        (float.clone(), vec![], float.clone(), vec![]),
        (
            TypeExpr::Generic(item.clone()),
            vec![item.clone()],
            TypeExpr::Generic(item.clone()),
            vec![item.clone()],
        ),
        (
            TypeExpr::Generic(item.clone()),
            vec![item.clone()],
            TypeExpr::Generic(target_item.clone()),
            vec![target_item.clone()],
        ),
        (
            TypeExpr::Applied {
                constructor: TypeConstructorId::new("core.data_series").unwrap(),
                arguments: vec![TypeExpr::Generic(item.clone())],
            },
            vec![item.clone()],
            TypeExpr::Applied {
                constructor: TypeConstructorId::new("core.data_series").unwrap(),
                arguments: vec![float.clone()],
            },
            vec![],
        ),
    ] {
        let (_, snapshot, output, input) = type_snapshot(
            Some(source),
            false,
            source_parameters,
            PortKind::Data,
            Some(target),
            false,
            target_parameters,
            PortKind::Data,
        );
        assert_eq!(snapshot.graph_revision, GraphRevision::new(7));
        assert!(snapshot.validate_connection_types(&output, &input).is_ok());
    }

    let (_, captured, generic_output, generic_input) = type_snapshot(
        Some(TypeExpr::Generic(item.clone())),
        false,
        vec![item.clone()],
        PortKind::Data,
        Some(TypeExpr::Generic(target_item.clone())),
        false,
        vec![target_item.clone()],
        PortKind::Data,
    );
    assert!(matches!(
        &captured.ports[&generic_output].port_type,
        EditorMutationPortType::Ready { type_parameters, .. }
            if type_parameters.as_ref() == [item]
    ));
    assert!(matches!(
        &captured.ports[&generic_input].port_type,
        EditorMutationPortType::Ready { type_parameters, .. }
            if type_parameters.as_ref() == [target_item]
    ));

    let (_, snapshot, output, input) = type_snapshot(
        Some(float),
        true,
        vec![],
        PortKind::Data,
        Some(string),
        true,
        vec![],
        PortKind::Data,
    );
    assert_editor_error_code(
        MutationConflict::Editor(
            snapshot
                .validate_connection_types(&output, &input)
                .unwrap_err(),
        ),
        EditorMutationErrorCode::GraphConnectionTypeMismatch,
    );
}

#[test]
fn phase1_type_snapshot_rejects_unavailable_and_recursively_unresolved_types() {
    let missing = TypeParameterId::new("missing").unwrap();
    let float = TypeExpr::Concrete(TypeId::new("core.float64").unwrap());
    for source in [
        None,
        Some(TypeExpr::Unknown),
        Some(TypeExpr::Generic(missing.clone())),
        Some(TypeExpr::Applied {
            constructor: TypeConstructorId::new("core.data_series").unwrap(),
            arguments: vec![TypeExpr::Unknown],
        }),
        Some(TypeExpr::Union(vec![
            float.clone(),
            TypeExpr::Generic(missing.clone()),
        ])),
    ] {
        let expected = if source.is_none() {
            EditorMutationErrorCode::GraphConnectionTypeUnavailable
        } else {
            EditorMutationErrorCode::GraphConnectionTypeUnresolved
        };
        let (_, snapshot, output, input) = type_snapshot(
            source,
            true,
            vec![],
            PortKind::Data,
            Some(float.clone()),
            true,
            vec![],
            PortKind::Data,
        );
        assert_editor_error_code(
            MutationConflict::Editor(
                snapshot
                    .validate_connection_types(&output, &input)
                    .unwrap_err(),
            ),
            expected,
        );
    }

    let missing_internal = TypeSummaryDto {
        display: "missing".into(),
        resolved: false,
        data_type: None,
        internal_type_expr: None,
    };
    let (_, snapshot, output, input) = type_snapshot_with_summaries(
        Some(missing_internal),
        vec![],
        PortKind::Data,
        snapshot_type(Some(float), true),
        vec![],
        PortKind::Data,
    );
    assert_editor_error_code(
        MutationConflict::Editor(
            snapshot
                .validate_connection_types(&output, &input)
                .unwrap_err(),
        ),
        EditorMutationErrorCode::GraphConnectionTypeUnavailable,
    );
}

#[test]
fn phase1_type_snapshot_bypasses_data_types_only_for_equal_non_data_kinds() {
    for kind in [PortKind::Control, PortKind::Effect] {
        let (_, snapshot, output, input) =
            type_snapshot(None, false, vec![], kind, None, false, vec![], kind);
        assert!(snapshot.validate_connection_types(&output, &input).is_ok());
    }
    let (_, snapshot, output, input) = type_snapshot(
        None,
        false,
        vec![],
        PortKind::Control,
        None,
        false,
        vec![],
        PortKind::Effect,
    );
    assert_editor_error_code(
        MutationConflict::Editor(
            snapshot
                .validate_connection_types(&output, &input)
                .unwrap_err(),
        ),
        EditorMutationErrorCode::GraphConnectionKindMismatch,
    );
}

#[test]
fn phase1_connect_replaces_independently_occupied_single_endpoints_atomically() {
    let float = TypeExpr::Concrete(TypeId::new("core.float64").unwrap());
    let (registry, snapshot, output, input) = type_snapshot(
        Some(float.clone()),
        true,
        vec![],
        PortKind::Data,
        Some(float),
        true,
        vec![],
        PortKind::Data,
    );
    let source_id = output.node_id;
    let target_id = input.node_id;
    let other_source = node_id(3_003);
    let other_target = node_id(3_004);
    let mut document = GraphDocument::default();
    document.revision = GraphRevision::new(7);
    for (id, node_type) in [
        (source_id, "yssbi.test.snapshot_source"),
        (target_id, "yssbi.test.snapshot_target"),
        (other_source, "yssbi.test.snapshot_source"),
        (other_target, "yssbi.test.snapshot_target"),
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
    let incumbents = [
        DocumentConnection {
            id: connection_id(61),
            output: output.clone(),
            input: declared(other_target, "in"),
            order: None,
        },
        DocumentConnection {
            id: connection_id(60),
            output: declared(other_source, "out"),
            input: input.clone(),
            order: None,
        },
    ];
    for connection in incumbents.clone() {
        document.connections.insert(connection.id, connection);
    }
    document.revision = GraphRevision::new(7);
    let before = document.clone();
    let allocated = connection_id(62);

    let operations = connect_operations_with_id_allocator(
        &document,
        &registry,
        &snapshot,
        output.clone(),
        input.clone(),
        None,
        || allocated,
    )
    .unwrap();

    assert_eq!(
        operations,
        vec![
            GraphDocumentOperation::RemoveConnection {
                connection: incumbents[1].clone(),
            },
            GraphDocumentOperation::RemoveConnection {
                connection: incumbents[0].clone(),
            },
            GraphDocumentOperation::InsertConnection {
                connection: DocumentConnection {
                    id: allocated,
                    output,
                    input,
                    order: None,
                },
            },
        ]
    );
    assert_graph_content_eq(&document, &before);
}

#[test]
fn phase1_connect_duplicate_endpoint_precedes_replacement_and_id_allocation() {
    let float = TypeExpr::Concrete(TypeId::new("core.float64").unwrap());
    let (registry, snapshot, output, input) = type_snapshot(
        Some(float.clone()),
        true,
        vec![],
        PortKind::Data,
        Some(float),
        true,
        vec![],
        PortKind::Data,
    );
    let mut document = GraphDocument::default();
    document.revision = GraphRevision::new(7);
    for (id, node_type) in [
        (output.node_id, "yssbi.test.snapshot_source"),
        (input.node_id, "yssbi.test.snapshot_target"),
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
    let existing = DocumentConnection {
        id: connection_id(70),
        output: output.clone(),
        input: input.clone(),
        order: None,
    };
    document.connections.insert(existing.id, existing);
    document.revision = GraphRevision::new(7);
    let before = document.clone();
    let allocations = std::cell::Cell::new(0);

    let error = connect_operations_with_id_allocator(
        &document,
        &registry,
        &snapshot,
        output,
        input,
        None,
        || {
            allocations.set(allocations.get() + 1);
            connection_id(71)
        },
    )
    .unwrap_err();

    assert_editor_error_code(error, EditorMutationErrorCode::GraphConnectionAlreadyExists);
    assert_eq!(allocations.get(), 0);
    assert_graph_content_eq(&document, &before);
    assert_eq!(document.revision, before.revision);
}

#[test]
fn phase1_connect_replaces_only_an_occupied_single_output() {
    let float = TypeExpr::Concrete(TypeId::new("core.float64").unwrap());
    let mut fixture = connect_fixture(
        PortKind::Data,
        ConnectionsPerPort::Single,
        ready_summary(float.clone()),
        vec![],
        PortKind::Data,
        ConnectionsPerPort::Multiple {
            max: None,
            ordered: false,
        },
        ready_summary(float),
        vec![],
    );
    let other_target = node_id(3_103);
    fixture
        .document
        .create_node(DocumentNode {
            id: other_target,
            node_type: NodeTypeId::new("yssbi.test.snapshot_target").unwrap(),
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters: ParameterValues::new(),
            user_label: None,
        })
        .unwrap();
    let incumbent = DocumentConnection {
        id: connection_id(3_110),
        output: fixture.output.clone(),
        input: declared(other_target, "in"),
        order: None,
    };
    fixture
        .document
        .connections
        .insert(incumbent.id, incumbent.clone());
    fixture.document.revision = GraphRevision::new(7);
    let before = fixture.document.clone();
    let allocated = connection_id(3_111);

    let operations = connect_operations_with_id_allocator(
        &fixture.document,
        &fixture.registry,
        &fixture.snapshot,
        fixture.output.clone(),
        fixture.input.clone(),
        None,
        || allocated,
    )
    .unwrap();

    assert_eq!(
        operations,
        vec![
            GraphDocumentOperation::RemoveConnection {
                connection: incumbent,
            },
            GraphDocumentOperation::InsertConnection {
                connection: DocumentConnection {
                    id: allocated,
                    output: fixture.output,
                    input: fixture.input,
                    order: None,
                },
            },
        ]
    );
    assert_eq!(fixture.document, before);
}

#[test]
fn phase1_connect_rejects_full_bounded_multiple_without_eviction_or_allocation() {
    let float = TypeExpr::Concrete(TypeId::new("core.float64").unwrap());
    let mut fixture = connect_fixture(
        PortKind::Data,
        ConnectionsPerPort::Multiple {
            max: None,
            ordered: false,
        },
        ready_summary(float.clone()),
        vec![],
        PortKind::Data,
        ConnectionsPerPort::Multiple {
            max: Some(1),
            ordered: false,
        },
        ready_summary(float),
        vec![],
    );
    let other_source = node_id(3_120);
    fixture
        .document
        .create_node(DocumentNode {
            id: other_source,
            node_type: NodeTypeId::new("yssbi.test.snapshot_source").unwrap(),
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters: ParameterValues::new(),
            user_label: None,
        })
        .unwrap();
    let incumbent = DocumentConnection {
        id: connection_id(3_121),
        output: declared(other_source, "out"),
        input: fixture.input.clone(),
        order: None,
    };
    fixture.document.connections.insert(incumbent.id, incumbent);
    fixture.document.revision = GraphRevision::new(7);

    assert_connect_failure_unchanged(
        &fixture,
        &fixture.snapshot,
        fixture.output.clone(),
        fixture.input.clone(),
        None,
        EditorMutationErrorCode::GraphConnectionLimitReached,
    );
}

#[test]
fn phase1_connect_rejects_endpoint_direction_kind_and_order_errors_atomically() {
    let float = TypeExpr::Concrete(TypeId::new("core.float64").unwrap());
    let fixture = connect_fixture(
        PortKind::Data,
        ConnectionsPerPort::Single,
        ready_summary(float.clone()),
        vec![],
        PortKind::Data,
        ConnectionsPerPort::Single,
        ready_summary(float.clone()),
        vec![],
    );
    assert_connect_failure_unchanged(
        &fixture,
        &fixture.snapshot,
        fixture.output.clone(),
        declared(node_id(3_999), "missing"),
        None,
        EditorMutationErrorCode::GraphPortNotFound,
    );

    let mut orphan = fixture.snapshot.clone();
    orphan.ports.get_mut(&fixture.input).unwrap().orphan = true;
    assert_connect_failure_unchanged(
        &fixture,
        &orphan,
        fixture.output.clone(),
        fixture.input.clone(),
        None,
        EditorMutationErrorCode::GraphPortOrphan,
    );

    let mut direction = fixture.snapshot.clone();
    direction.ports.get_mut(&fixture.output).unwrap().direction = PortDirection::Input;
    assert_connect_failure_unchanged(
        &fixture,
        &direction,
        fixture.output.clone(),
        fixture.input.clone(),
        None,
        EditorMutationErrorCode::GraphConnectionDirectionMismatch,
    );

    let mut kind = fixture.snapshot.clone();
    let target = kind.ports.get_mut(&fixture.input).unwrap();
    target.kind = PortKind::Control;
    target.port_type = EditorMutationPortType::NotApplicable;
    assert_connect_failure_unchanged(
        &fixture,
        &kind,
        fixture.output.clone(),
        fixture.input.clone(),
        None,
        EditorMutationErrorCode::GraphConnectionKindMismatch,
    );

    assert_connect_failure_unchanged(
        &fixture,
        &fixture.snapshot,
        fixture.output.clone(),
        fixture.input.clone(),
        Some(OrderKey::new("forbidden")),
        EditorMutationErrorCode::GraphConnectionOrderForbidden,
    );

    let ordered = connect_fixture(
        PortKind::Data,
        ConnectionsPerPort::Multiple {
            max: None,
            ordered: false,
        },
        ready_summary(float.clone()),
        vec![],
        PortKind::Data,
        ConnectionsPerPort::Multiple {
            max: None,
            ordered: true,
        },
        ready_summary(float),
        vec![],
    );
    assert_connect_failure_unchanged(
        &ordered,
        &ordered.snapshot,
        ordered.output.clone(),
        ordered.input.clone(),
        None,
        EditorMutationErrorCode::GraphConnectionOrderRequired,
    );
}

#[test]
fn phase1_connect_rejects_authoritative_type_states_atomically() {
    let float = TypeExpr::Concrete(TypeId::new("core.float64").unwrap());
    let string = TypeExpr::Concrete(TypeId::new("core.string").unwrap());
    let cases = [
        (
            ready_summary(float.clone()),
            ready_summary(string),
            vec![],
            EditorMutationErrorCode::GraphConnectionTypeMismatch,
        ),
        (
            None,
            ready_summary(float.clone()),
            vec![],
            EditorMutationErrorCode::GraphConnectionTypeUnavailable,
        ),
        (
            Some(TypeSummaryDto {
                display: "missing".into(),
                resolved: false,
                data_type: None,
                internal_type_expr: None,
            }),
            ready_summary(float.clone()),
            vec![],
            EditorMutationErrorCode::GraphConnectionTypeUnavailable,
        ),
        (
            ready_summary(TypeExpr::Unknown),
            ready_summary(float.clone()),
            vec![],
            EditorMutationErrorCode::GraphConnectionTypeUnresolved,
        ),
        (
            ready_summary(TypeExpr::Generic(TypeParameterId::new("missing").unwrap())),
            ready_summary(float.clone()),
            vec![],
            EditorMutationErrorCode::GraphConnectionTypeUnresolved,
        ),
    ];
    for (source_type, target_type, source_parameters, expected) in cases {
        let fixture = connect_fixture(
            PortKind::Data,
            ConnectionsPerPort::Multiple {
                max: None,
                ordered: false,
            },
            source_type,
            source_parameters,
            PortKind::Data,
            ConnectionsPerPort::Single,
            target_type,
            vec![],
        );
        assert_connect_failure_unchanged(
            &fixture,
            &fixture.snapshot,
            fixture.output.clone(),
            fixture.input.clone(),
            None,
            expected,
        );
    }
}

#[test]
fn phase1_move_connections_allocates_new_ids_in_old_source_id_order_and_inverse_restores_old_ids() {
    let registry = validation_registry();
    let source_node = node_id(4_001);
    let target_node = node_id(4_002);
    let first_input_node = node_id(4_003);
    let second_input_node = node_id(4_004);
    let mut document = validation_document(&[
        source_node,
        target_node,
        first_input_node,
        second_input_node,
    ]);
    let source = declared(source_node, "data_out");
    let target = declared(target_node, "data_out");
    let originals = [
        DocumentConnection {
            id: connection_id(4_012),
            output: source.clone(),
            input: declared(second_input_node, "ordered_in"),
            order: Some(OrderKey::new("b")),
        },
        DocumentConnection {
            id: connection_id(4_011),
            output: source.clone(),
            input: declared(first_input_node, "ordered_in"),
            order: Some(OrderKey::new("a")),
        },
    ];
    for connection in originals.clone() {
        document.connections.insert(connection.id, connection);
    }
    let before = document.clone();
    let snapshot = validation_snapshot_for_document(&document, &registry);

    let allocated = [connection_id(4_101), connection_id(4_102)];
    let allocation_count = std::cell::Cell::new(0usize);
    let patch = GraphDocumentPatch::new(
        move_connection_operations_with_id_allocator(
            &document,
            &registry,
            &snapshot,
            source,
            target.clone(),
            &|| {
                let index = allocation_count.get();
                allocation_count.set(index + 1);
                allocated[index]
            },
        )
        .unwrap(),
    );
    assert_eq!(allocation_count.get(), 2);
    let first_moved = DocumentConnection {
        id: allocated[0],
        output: target.clone(),
        input: originals[1].input.clone(),
        order: originals[1].order.clone(),
    };
    let second_moved = DocumentConnection {
        id: allocated[1],
        output: target.clone(),
        input: originals[0].input.clone(),
        order: originals[0].order.clone(),
    };
    assert_eq!(
        patch.operations,
        vec![
            GraphDocumentOperation::RemoveConnection {
                connection: originals[1].clone(),
            },
            GraphDocumentOperation::RemoveConnection {
                connection: originals[0].clone(),
            },
            GraphDocumentOperation::InsertConnection {
                connection: first_moved.clone(),
            },
            GraphDocumentOperation::InsertConnection {
                connection: second_moved.clone(),
            },
        ]
    );
    assert_eq!(
        patch.inverse().operations,
        vec![
            GraphDocumentOperation::RemoveConnection {
                connection: second_moved,
            },
            GraphDocumentOperation::RemoveConnection {
                connection: first_moved,
            },
            GraphDocumentOperation::InsertConnection {
                connection: originals[0].clone(),
            },
            GraphDocumentOperation::InsertConnection {
                connection: originals[1].clone(),
            },
        ]
    );

    document.apply_patch(&patch).unwrap();
    assert!(
        document
            .connections
            .values()
            .all(|connection| connection.output == target)
    );
    document.apply_patch(&patch.inverse()).unwrap();
    assert_graph_content_eq(&document, &before);
}

#[test]
fn phase1_move_connections_moves_input_and_replaces_single_target_once() {
    let registry = validation_registry();
    let moved_output_node = node_id(4_021);
    let source_node = node_id(4_022);
    let target_node = node_id(4_023);
    let incumbent_output_node = node_id(4_024);
    let mut document = validation_document(&[
        moved_output_node,
        source_node,
        target_node,
        incumbent_output_node,
    ]);
    let source = declared(source_node, "data_in");
    let target = declared(target_node, "data_in");
    let moved = DocumentConnection {
        id: connection_id(4_032),
        output: declared(moved_output_node, "data_out"),
        input: source.clone(),
        order: None,
    };
    let incumbent = DocumentConnection {
        id: connection_id(4_031),
        output: declared(incumbent_output_node, "data_out"),
        input: target.clone(),
        order: None,
    };
    for connection in [moved.clone(), incumbent.clone()] {
        document.connections.insert(connection.id, connection);
    }
    let before = document.clone();
    let snapshot = validation_snapshot_for_document(&document, &registry);

    let allocated = connection_id(4_101);
    let inserted = DocumentConnection {
        id: allocated,
        output: moved.output.clone(),
        input: target,
        order: None,
    };
    let patch = GraphDocumentPatch::new(
        move_connection_operations_with_id_allocator(
            &document,
            &registry,
            &snapshot,
            source,
            inserted.input.clone(),
            &|| allocated,
        )
        .unwrap(),
    );

    assert_eq!(
        patch.operations,
        vec![
            GraphDocumentOperation::RemoveConnection {
                connection: incumbent.clone(),
            },
            GraphDocumentOperation::RemoveConnection {
                connection: moved.clone(),
            },
            GraphDocumentOperation::InsertConnection {
                connection: inserted.clone(),
            },
        ]
    );
    assert_eq!(
        patch.inverse().operations,
        vec![
            GraphDocumentOperation::RemoveConnection {
                connection: inserted,
            },
            GraphDocumentOperation::InsertConnection { connection: moved },
            GraphDocumentOperation::InsertConnection {
                connection: incumbent,
            },
        ]
    );

    document.apply_patch(&patch).unwrap();
    assert_eq!(document.connections.len(), 1);
    document.apply_patch(&patch.inverse()).unwrap();
    assert_graph_content_eq(&document, &before);
}

#[test]
fn phase1_move_connections_rejects_same_empty_missing_or_invalid_target_atomically() {
    let registry = validation_registry();
    let source_node = node_id(4_041);
    let input_node = node_id(4_042);
    let target_node = node_id(4_043);
    let empty_node = node_id(4_044);
    let mut document = validation_document(&[source_node, input_node, target_node, empty_node]);
    let source = declared(source_node, "data_out");
    let target = declared(target_node, "data_out");
    let original = DocumentConnection {
        id: connection_id(4_051),
        output: source.clone(),
        input: declared(input_node, "data_in"),
        order: None,
    };
    document.connections.insert(original.id, original);
    let snapshot = validation_snapshot_for_document(&document, &registry);

    assert_move_failure_unchanged(
        &document,
        &registry,
        &snapshot,
        source.clone(),
        source.clone(),
        EditorMutationErrorCode::GraphConnectionMoveSamePort,
    );
    assert_move_failure_unchanged(
        &document,
        &registry,
        &snapshot,
        declared(empty_node, "data_out"),
        target.clone(),
        EditorMutationErrorCode::GraphConnectionMoveSourceEmpty,
    );
    assert_move_failure_unchanged(
        &document,
        &registry,
        &snapshot,
        source.clone(),
        declared(node_id(4_999), "data_out"),
        EditorMutationErrorCode::GraphPortNotFound,
    );

    let mut orphan = snapshot.clone();
    orphan.ports.get_mut(&target).unwrap().orphan = true;
    assert_move_failure_unchanged(
        &document,
        &registry,
        &orphan,
        source.clone(),
        target.clone(),
        EditorMutationErrorCode::GraphPortOrphan,
    );

    let mut direction = snapshot.clone();
    direction.ports.get_mut(&target).unwrap().direction = PortDirection::Input;
    assert_move_failure_unchanged(
        &document,
        &registry,
        &direction,
        source.clone(),
        target.clone(),
        EditorMutationErrorCode::GraphConnectionDirectionMismatch,
    );

    let mut kind = snapshot.clone();
    let target_validation = kind.ports.get_mut(&target).unwrap();
    target_validation.kind = PortKind::Control;
    target_validation.port_type = EditorMutationPortType::NotApplicable;
    assert_move_failure_unchanged(
        &document,
        &registry,
        &kind,
        source.clone(),
        target.clone(),
        EditorMutationErrorCode::GraphConnectionKindMismatch,
    );

    let mut incompatible = snapshot.clone();
    incompatible.ports.get_mut(&target).unwrap().port_type = EditorMutationPortType::Ready {
        expression: TypeExpr::Concrete(TypeId::new("core.string").unwrap()),
        type_parameters: Box::new([]),
    };
    assert_move_failure_unchanged(
        &document,
        &registry,
        &incompatible,
        source,
        target,
        EditorMutationErrorCode::GraphConnectionTypeMismatch,
    );
}

#[test]
fn phase1_move_connections_rejects_aggregate_capacity_and_order_atomically() {
    let registry = validation_registry();
    let first_output = node_id(4_061);
    let second_output = node_id(4_062);
    let incumbent_output = node_id(4_063);
    let source_node = node_id(4_064);
    let target_node = node_id(4_065);
    let mut document = validation_document(&[
        first_output,
        second_output,
        incumbent_output,
        source_node,
        target_node,
    ]);
    let source = declared(source_node, "ordered_in");
    let target = declared(target_node, "ordered_in");
    for connection in [
        DocumentConnection {
            id: connection_id(4_071),
            output: declared(first_output, "data_out"),
            input: source.clone(),
            order: Some(OrderKey::new("a")),
        },
        DocumentConnection {
            id: connection_id(4_072),
            output: declared(second_output, "data_out"),
            input: source.clone(),
            order: Some(OrderKey::new("b")),
        },
        DocumentConnection {
            id: connection_id(4_073),
            output: declared(incumbent_output, "data_out"),
            input: target.clone(),
            order: Some(OrderKey::new("c")),
        },
    ] {
        document.connections.insert(connection.id, connection);
    }
    let snapshot = validation_snapshot_for_document(&document, &registry);
    assert_move_failure_unchanged(
        &document,
        &registry,
        &snapshot,
        source,
        target,
        EditorMutationErrorCode::GraphConnectionLimitReached,
    );

    let order_source = node_id(4_081);
    let order_input = node_id(4_082);
    let order_target = node_id(4_083);
    let mut order_document = validation_document(&[order_source, order_input, order_target]);
    let source = declared(order_input, "data_in");
    let target = declared(order_target, "ordered_in");
    let connection = DocumentConnection {
        id: connection_id(4_084),
        output: declared(order_source, "data_out"),
        input: source.clone(),
        order: None,
    };
    order_document.connections.insert(connection.id, connection);
    let order_snapshot = validation_snapshot_for_document(&order_document, &registry);
    assert_move_failure_unchanged(
        &order_document,
        &registry,
        &order_snapshot,
        source,
        target,
        EditorMutationErrorCode::GraphConnectionOrderRequired,
    );
}
