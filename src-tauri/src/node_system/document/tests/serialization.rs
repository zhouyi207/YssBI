use super::*;

#[test]
fn resolved_binding_serde_requires_and_serializes_metadata() {
    let missing_metadata = serde_json::json!({
        "kind": "resolved",
        "origin": {
            "kind": "schema_field",
            "source": "databases/main",
            "field": "customer_id"
        },
        "order": "a"
    });
    assert!(serde_json::from_value::<DynamicPortBinding>(missing_metadata).is_err());

    let value = serde_json::to_value(DynamicPortBinding::Resolved {
        origin: DynamicMemberLocator::SchemaField {
            source: SchemaSourceIdentity::new("databases/main"),
            field: SchemaFieldIdentity::new("customer_id"),
        },
        order: OrderKey::new("a"),
        last_known: LastKnownPortMetadata {
            label: "customer_id".into(),
            value_type: Some(TypeExpr::Concrete(TypeId::new("core.int64").unwrap())),
        },
    })
    .unwrap();
    assert_eq!(
        value,
        serde_json::json!({
            "kind": "resolved",
            "origin": {
                "kind": "schema_field",
                "source": "databases/main",
                "field": "customer_id"
            },
            "order": "a",
            "last_known": {
                "label": "customer_id",
                "value_type": { "Concrete": "core.int64" }
            }
        })
    );
}

#[test]
fn canonical_editor_mutation_address_wire_declared() {
    let mutation = EditorGraphMutationDto::SetLiteral {
        address: declared(node_id(901), "value").into(),
        literal: Some(json!(42)),
    };
    let expected = json!({
        "type": "setLiteral",
        "payload": {
            "address": {
                "kind": "declared",
                "nodeId": "00000000-0000-0000-0000-000000000385",
                "portKey": "value"
            },
            "literal": 42
        }
    });

    assert_eq!(serde_json::to_value(&mutation).unwrap(), expected);
    assert_eq!(
        serde_json::from_value::<EditorGraphMutationDto>(expected).unwrap(),
        mutation
    );
}

#[test]
fn canonical_editor_mutation_address_wire_instance() {
    let mutation = EditorGraphMutationDto::RemovePortInstance {
        address: PortAddress::instance(
            node_id(902),
            PortKey::new("inputs").unwrap(),
            instance_id(904),
        )
        .into(),
    };
    let expected = json!({
        "type": "removePortInstance",
        "payload": {
            "address": {
                "kind": "instance",
                "nodeId": "00000000-0000-0000-0000-000000000386",
                "templateKey": "inputs",
                "instanceId": "00000000-0000-0000-0000-000000000388"
            }
        }
    });

    assert_eq!(serde_json::to_value(&mutation).unwrap(), expected);
    assert_eq!(
        serde_json::from_value::<EditorGraphMutationDto>(expected).unwrap(),
        mutation
    );
}

#[test]
fn phase1_move_connections_wire_contains_only_source_and_target() {
    let source = declared(node_id(905), "data_out");
    let target = declared(node_id(906), "data_out");
    let mutation = EditorGraphMutationDto::MoveConnections {
        source: source.clone().into(),
        target: target.clone().into(),
    };
    let expected = json!({
        "type": "moveConnections",
        "payload": {
            "source": PortAddressDto::from(source),
            "target": PortAddressDto::from(target),
        }
    });

    assert_eq!(serde_json::to_value(&mutation).unwrap(), expected);
    assert_eq!(
        serde_json::from_value::<EditorGraphMutationDto>(expected).unwrap(),
        mutation
    );
}

#[test]
fn phase1_error_protocol_domain_codes_are_stable() {
    use crate::node_system::document::EditorMutationErrorCode;

    let cases = [
        (
            EditorMutationErrorCode::GraphPortNotFound,
            "graph_port_not_found",
        ),
        (
            EditorMutationErrorCode::GraphNodeNotFound,
            "graph_node_not_found",
        ),
        (
            EditorMutationErrorCode::GraphConnectionNotFound,
            "graph_connection_not_found",
        ),
        (
            EditorMutationErrorCode::GraphPortOrphan,
            "graph_port_orphan",
        ),
        (
            EditorMutationErrorCode::GraphConnectionDirectionMismatch,
            "graph_connection_direction_mismatch",
        ),
        (
            EditorMutationErrorCode::GraphConnectionKindMismatch,
            "graph_connection_kind_mismatch",
        ),
        (
            EditorMutationErrorCode::GraphConnectionTypeMismatch,
            "graph_connection_type_mismatch",
        ),
        (
            EditorMutationErrorCode::GraphConnectionTypeUnavailable,
            "graph_connection_type_unavailable",
        ),
        (
            EditorMutationErrorCode::GraphConnectionTypeUnresolved,
            "graph_connection_type_unresolved",
        ),
        (
            EditorMutationErrorCode::GraphConnectionLimitReached,
            "graph_connection_limit_reached",
        ),
        (
            EditorMutationErrorCode::GraphConnectionOrderRequired,
            "graph_connection_order_required",
        ),
        (
            EditorMutationErrorCode::GraphConnectionOrderForbidden,
            "graph_connection_order_forbidden",
        ),
        (
            EditorMutationErrorCode::GraphConnectionAlreadyExists,
            "graph_connection_already_exists",
        ),
        (
            EditorMutationErrorCode::GraphConnectionMoveSourceEmpty,
            "graph_connection_move_source_empty",
        ),
        (
            EditorMutationErrorCode::GraphConnectionMoveSamePort,
            "graph_connection_move_same_port",
        ),
        (
            EditorMutationErrorCode::GraphMutationEmptyTargets,
            "graph_mutation_empty_targets",
        ),
        (
            EditorMutationErrorCode::GraphMutationDuplicateTarget,
            "graph_mutation_duplicate_target",
        ),
        (
            EditorMutationErrorCode::GraphManagedNodeDeleteForbidden,
            "graph_managed_node_delete_forbidden",
        ),
    ];

    for (code, expected) in cases {
        assert_eq!(code.as_str(), expected);
    }
}

#[test]
fn phase1_collection_editor_mutation_wire_is_stable_and_camel_case() {
    let first = node_id(901);
    let second = node_id(902);
    let connection = connection_id(903);
    let instance = instance_id(904);
    let output = PortAddressDto::from(declared(first, "output"));
    let input = PortAddressDto::from(PortAddress::instance(
        second,
        PortKey::new("inputs").unwrap(),
        instance,
    ));
    let cases = [
        (
            EditorGraphMutationDto::CreateNode {
                descriptor: crate::node_system::catalog::NodeCreationDescriptor::Static {
                    node_type_id: NodeTypeId::new("yssbi.test.editor_mutation").unwrap(),
                },
                position: NodePosition { x: 1.0, y: 2.0 },
                user_label: Some("Created".to_owned()),
                connect_from: None,
            },
            json!({
                "type": "createNode",
                "payload": {
                    "descriptor": {
                        "kind": "static",
                        "nodeTypeId": "yssbi.test.editor_mutation"
                    },
                    "position": { "x": 1.0, "y": 2.0 },
                    "userLabel": "Created",
                    "connectFrom": null
                }
            }),
        ),
        (
            EditorGraphMutationDto::DeleteNodes {
                node_ids: vec![first],
            },
            json!({ "type": "deleteNodes", "payload": { "nodeIds": [first] } }),
        ),
        (
            EditorGraphMutationDto::MoveNodes {
                positions: vec![NodePositionMutationDto {
                    node_id: first,
                    position: NodePosition { x: 3.0, y: 4.0 },
                }],
            },
            json!({
                "type": "moveNodes",
                "payload": {
                    "positions": [{ "nodeId": first, "position": { "x": 3.0, "y": 4.0 } }]
                }
            }),
        ),
        (
            EditorGraphMutationDto::Connect {
                output: output.clone(),
                input: input.clone(),
                order: Some(OrderKey::new("a")),
            },
            json!({
                "type": "connect",
                "payload": { "output": output, "input": input.clone(), "order": "a" }
            }),
        ),
        (
            EditorGraphMutationDto::DisconnectConnections {
                connection_ids: vec![connection],
            },
            json!({
                "type": "disconnectConnections",
                "payload": { "connectionIds": [connection] }
            }),
        ),
        (
            EditorGraphMutationDto::DisconnectPort {
                address: input.clone(),
            },
            json!({ "type": "disconnectPort", "payload": { "address": input.clone() } }),
        ),
        (
            EditorGraphMutationDto::DisconnectNode { node_id: second },
            json!({ "type": "disconnectNode", "payload": { "nodeId": second } }),
        ),
        (
            EditorGraphMutationDto::SetLiteral {
                address: input.clone(),
                literal: Some(json!(42)),
            },
            json!({
                "type": "setLiteral",
                "payload": { "address": input.clone(), "literal": 42 }
            }),
        ),
        (
            EditorGraphMutationDto::AddPortInstance {
                node_id: second,
                template: PortKey::new("inputs").unwrap(),
                order: None,
            },
            json!({
                "type": "addPortInstance",
                "payload": { "nodeId": second, "template": "inputs", "order": null }
            }),
        ),
        (
            EditorGraphMutationDto::RemovePortInstance {
                address: input.clone(),
            },
            json!({
                "type": "removePortInstance",
                "payload": { "address": input }
            }),
        ),
    ];

    for (mutation, expected) in cases {
        let serialized = serde_json::to_value(&mutation).unwrap();
        assert_eq!(serialized, expected);
        assert_eq!(
            serde_json::from_value::<EditorGraphMutationDto>(serialized).unwrap(),
            mutation
        );
    }
}

#[test]
fn history_transaction_rejects_missing_persistence() {
    let transaction = ProjectHistoryTransaction::graph(
        operation_id(629),
        graph_path("events/strict-history"),
        GraphRevision::INITIAL,
        GraphDocumentPatch::new(Vec::new()),
    );
    let mut missing_persistence = serde_json::to_value(&transaction).unwrap();
    missing_persistence
        .as_object_mut()
        .unwrap()
        .remove("persistence");

    let error = serde_json::from_value::<ProjectHistoryTransaction>(missing_persistence)
        .expect_err("history persistence is required on the wire");

    assert!(error.to_string().contains("missing field `persistence`"));
}

#[test]
fn worksheet_document_patch_round_trips_and_inverts() {
    let before = worksheet_state("before", "histogram");
    let after = worksheet_state("after", "scatter");
    let patch = ResourceDocumentPatch::Worksheet(WorksheetDocumentPatch {
        before: before.clone(),
        after: after.clone(),
    });
    let encoded = serde_json::to_value(&patch).unwrap();

    assert_eq!(patch.kind(), ResourceKind::Worksheet);
    assert_eq!(
        serde_json::from_value::<ResourceDocumentPatch>(encoded).unwrap(),
        patch
    );
    assert_eq!(
        patch.inverse(),
        ResourceDocumentPatch::Worksheet(WorksheetDocumentPatch {
            before: after,
            after: before
        })
    );
}

#[test]
fn history_persistence_policies_round_trip() {
    let transactions = [
        ProjectHistoryTransaction::graph(
            operation_id(630),
            graph_path("events/in-memory-history"),
            GraphRevision::INITIAL,
            GraphDocumentPatch::new(Vec::new()),
        ),
        ProjectHistoryTransaction::durable_variable_effects(
            operation_id(631),
            Vec::new(),
            VariableEffectHistorySnapshots::default(),
        ),
        ProjectHistoryTransaction::graph_move(
            operation_id(632),
            graph_path("events/before-move"),
            graph_path("events/after-move"),
            json!({}),
        ),
    ];

    for transaction in transactions {
        let encoded = serde_json::to_value(&transaction).unwrap();
        let decoded: ProjectHistoryTransaction = serde_json::from_value(encoded).unwrap();
        assert_eq!(decoded, transaction);
    }
}
