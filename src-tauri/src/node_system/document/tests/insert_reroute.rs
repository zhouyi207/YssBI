use super::*;
use crate::node_system::catalog::{
    CONTROL_REROUTE_NODE_TYPE, DATA_REROUTE_NODE_TYPE, EFFECT_REROUTE_NODE_TYPE,
    REROUTE_INPUT_PORT, REROUTE_OUTPUT_PORT, build_builtin_node_system,
    builtin_bundle_parts_for_test, validate_builtin_bundle_for_test,
};
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

const ORIGINAL_CONNECTION: u128 = 0x101;
const SOURCE_NODE: u128 = 0x201;
const TARGET_NODE: u128 = 0x202;

struct EmptyResources;

impl ResourceSnapshot for EmptyResources {
    fn versions(&self) -> ResourceVersionSet {
        ResourceVersionSet::new()
    }
}

#[test]
fn phase2_insert_reroute_document_wire_is_exact_and_closed() {
    let mutation = EditorGraphMutationDto::InsertReroute {
        connection_id: connection_id(ORIGINAL_CONNECTION),
        position: NodePosition { x: 120.5, y: -30.0 },
    };
    let expected = json!({
        "type": "insertReroute",
        "payload": {
            "connectionId": "00000000-0000-0000-0000-000000000101",
            "position": { "x": 120.5, "y": -30.0 }
        }
    });
    assert_eq!(serde_json::to_value(&mutation).unwrap(), expected);
    assert_eq!(
        serde_json::from_value::<EditorGraphMutationDto>(expected).unwrap(),
        mutation
    );

    for invalid in [
        json!({
            "type": "insertReroute",
            "payload": {
                "connectionId": "00000000-0000-0000-0000-000000000101",
                "position": { "x": 120.5, "y": -30.0 },
                "unexpected": true
            }
        }),
        json!({
            "type": "insertReroute",
            "payload": {
                "connectionId": "00000000-0000-0000-0000-000000000101",
                "position": { "x": 120.5, "y": -30.0, "z": 1.0 }
            }
        }),
        json!({ "type": "unknownReroute", "payload": {} }),
    ] {
        assert!(serde_json::from_value::<EditorGraphMutationDto>(invalid).is_err());
    }
}

#[test]
fn phase2_insert_reroute_document_wire_rejects_envelope_extra_and_nonfinite_json() {
    let envelope_extra = r#"{
        "resource":{"kind":"graph","key":"events/Reroute.yssbi-event"},
        "baseRevision":0,
        "operationId":"00000000-0000-0000-0000-000000000001",
        "payload":{"type":"insertReroute","payload":{"connectionId":"00000000-0000-0000-0000-000000000101","position":{"x":1.0,"y":2.0}}},
        "unexpected":true
    }"#;
    assert!(
        serde_json::from_str::<MutationRequest<EditorGraphMutationDto>>(envelope_extra).is_err()
    );
    for raw in [
        r#"{"type":"insertReroute","payload":{"connectionId":"00000000-0000-0000-0000-000000000101","position":{"x":NaN,"y":2.0}}}"#,
        r#"{"type":"insertReroute","payload":{"connectionId":"00000000-0000-0000-0000-000000000101","position":{"x":1.0,"y":Infinity}}}"#,
    ] {
        assert!(serde_json::from_str::<EditorGraphMutationDto>(raw).is_err());
    }
}

#[test]
fn phase2_insert_reroute_document_allocator_assigns_all_ids_after_validation() {
    let registry = build_builtin_node_system().unwrap().registry;
    let (document, original) = Case::data(None).document();
    let node_allocations = AtomicUsize::new(0);
    let connection_allocations = AtomicUsize::new(0);
    let patch = EditorGraphMutationDto::InsertReroute {
        connection_id: original.id,
        position: NodePosition { x: 40.0, y: 20.0 },
    }
    .into_patch_with_editor_validation_and_allocators(
        &graph_path("events/Reroute.yssbi-event"),
        &document,
        registry.as_ref(),
        None,
        None,
        None,
        &|| {
            node_allocations.fetch_add(1, Ordering::SeqCst);
            node_id(0xa01)
        },
        &|| {
            let ordinal = connection_allocations.fetch_add(1, Ordering::SeqCst);
            connection_id(0xb01 + ordinal as u128)
        },
    )
    .unwrap();
    assert_eq!(node_allocations.load(Ordering::SeqCst), 1);
    assert_eq!(connection_allocations.load(Ordering::SeqCst), 2);
    assert!(
        matches!(&patch.operations[1], GraphDocumentOperation::InsertNode { node } if node.id == node_id(0xa01))
    );
    assert!(
        matches!(&patch.operations[2], GraphDocumentOperation::InsertConnection { connection } if connection.id == connection_id(0xb01))
    );
    assert!(
        matches!(&patch.operations[3], GraphDocumentOperation::InsertConnection { connection } if connection.id == connection_id(0xb02))
    );

    for invalid in [
        EditorGraphMutationDto::InsertReroute {
            connection_id: connection_id(0xffff),
            position: NodePosition { x: 1.0, y: 2.0 },
        },
        EditorGraphMutationDto::InsertReroute {
            connection_id: original.id,
            position: NodePosition {
                x: f64::NAN,
                y: 2.0,
            },
        },
    ] {
        node_allocations.store(0, Ordering::SeqCst);
        connection_allocations.store(0, Ordering::SeqCst);
        let before = document.clone();
        assert!(
            invalid
                .into_patch_with_editor_validation_and_allocators(
                    &graph_path("events/Reroute.yssbi-event"),
                    &document,
                    registry.as_ref(),
                    None,
                    None,
                    None,
                    &|| {
                        node_allocations.fetch_add(1, Ordering::SeqCst);
                        node_id(0xa02)
                    },
                    &|| {
                        connection_allocations.fetch_add(1, Ordering::SeqCst);
                        connection_id(0xb03)
                    },
                )
                .is_err()
        );
        assert_eq!(node_allocations.load(Ordering::SeqCst), 0);
        assert_eq!(connection_allocations.load(Ordering::SeqCst), 0);
        assert_graph_content_eq(&document, &before);
    }
}

#[test]
fn phase2_insert_reroute_document_invalid_exact_protocol_allocates_zero_ids() {
    let (mut provider, catalog, aliases) = builtin_bundle_parts_for_test().unwrap();
    let registered = provider
        .nodes
        .iter_mut()
        .find(|node| node.protocol().type_id.as_str() == DATA_REROUTE_NODE_TYPE)
        .unwrap();
    let mut protocol = registered.protocol().clone();
    protocol.interface.ports[0].editor =
        crate::node_system::protocol::PortEditorSpec::InlineLiteral;
    *registered = crate::node_system::registry::RegisteredNode::transparent(
        Arc::new(protocol),
        crate::node_system::registry::TransparentNodeRole::Reroute,
    );
    let registry = validate_builtin_bundle_for_test(provider, catalog, aliases)
        .unwrap()
        .registry;
    let (document, original) = Case::data(None).document();
    let node_allocations = AtomicUsize::new(0);
    let connection_allocations = AtomicUsize::new(0);

    let error = EditorGraphMutationDto::InsertReroute {
        connection_id: original.id,
        position: NodePosition { x: 40.0, y: 20.0 },
    }
    .into_patch_with_editor_validation_and_allocators(
        &graph_path("events/Reroute.yssbi-event"),
        &document,
        registry.as_ref(),
        None,
        None,
        None,
        &|| {
            node_allocations.fetch_add(1, Ordering::SeqCst);
            node_id(0xa03)
        },
        &|| {
            let ordinal = connection_allocations.fetch_add(1, Ordering::SeqCst);
            connection_id(0xb04 + ordinal as u128)
        },
    )
    .unwrap_err();

    assert!(matches!(error, MutationConflict::Projection(_)));
    assert_eq!(node_allocations.load(Ordering::SeqCst), 0);
    assert_eq!(connection_allocations.load(Ordering::SeqCst), 0);
}

#[test]
fn phase2_insert_reroute_document_selects_protocol_and_builds_exact_patch() {
    for case in [
        Case::data(None),
        Case::control(None),
        Case::effect(Some("original-order")),
    ] {
        let (document, original) = case.document();
        let registry = build_builtin_node_system().unwrap().registry;
        let patch = EditorGraphMutationDto::InsertReroute {
            connection_id: original.id,
            position: NodePosition { x: 40.0, y: 20.0 },
        }
        .into_patch(
            &graph_path("events/Reroute.yssbi-event"),
            &document,
            registry.as_ref(),
        )
        .unwrap();

        assert_eq!(patch.operations.len(), 4);
        let GraphDocumentOperation::RemoveConnection {
            connection: removed,
        } = &patch.operations[0]
        else {
            panic!("first operation must remove the original connection");
        };
        let GraphDocumentOperation::InsertNode { node: reroute } = &patch.operations[1] else {
            panic!("second operation must insert the reroute node");
        };
        let GraphDocumentOperation::InsertConnection {
            connection: source_side,
        } = &patch.operations[2]
        else {
            panic!("third operation must insert the source-side connection");
        };
        let GraphDocumentOperation::InsertConnection {
            connection: target_side,
        } = &patch.operations[3]
        else {
            panic!("fourth operation must insert the target-side connection");
        };

        assert_eq!(*removed, original);
        assert_eq!(reroute.node_type.as_str(), case.reroute_type);
        assert_eq!(reroute.position, NodePosition { x: 40.0, y: 20.0 });
        assert!(reroute.parameters.is_empty());
        assert_eq!(reroute.user_label, None);
        assert_ne!(reroute.id, original.output.node_id);
        assert_ne!(reroute.id, original.input.node_id);
        assert_ne!(source_side.id, original.id);
        assert_ne!(target_side.id, original.id);
        assert_ne!(source_side.id, target_side.id);
        assert_eq!(source_side.output, original.output);
        assert_eq!(source_side.input, declared(reroute.id, REROUTE_INPUT_PORT));
        assert_eq!(source_side.order, None);
        assert_eq!(
            target_side.output,
            declared(reroute.id, REROUTE_OUTPUT_PORT)
        );
        assert_eq!(target_side.input, original.input);
        assert_eq!(target_side.order, original.order);

        let mut applied = document.clone();
        applied.apply_patch(&patch).unwrap();
        assert!(!applied.connections.contains_key(&original.id));
        assert!(applied.nodes.contains_key(&reroute.id));
        assert_eq!(applied.connections.len(), 2);
        applied.apply_patch(&patch.inverse()).unwrap();
        assert_graph_content_eq(&applied, &document);
        assert_eq!(
            serde_json::to_vec(&applied).unwrap(),
            serde_json::to_vec(&document).unwrap()
        );
    }
}

#[test]
fn phase2_insert_reroute_document_roundtrip_preserves_persisted_reroute_and_projection() {
    let registry_bundle = build_builtin_node_system().unwrap();
    let registry = registry_bundle.registry;
    let catalog = registry_bundle.catalog;
    let (document, original) = Case::effect(Some("original-order")).document();
    let patch = EditorGraphMutationDto::InsertReroute {
        connection_id: original.id,
        position: NodePosition { x: 120.5, y: -30.0 },
    }
    .into_patch_with_editor_validation_and_allocators(
        &graph_path("events/Reroute.yssbi-event"),
        &document,
        registry.as_ref(),
        None,
        None,
        None,
        &|| node_id(0xa11),
        &|| {
            static NEXT: AtomicUsize = AtomicUsize::new(0xb11);
            connection_id(NEXT.fetch_add(1, Ordering::SeqCst) as u128)
        },
    )
    .unwrap();
    let mut applied = document.clone();
    applied.apply_patch(&patch).unwrap();

    let json = serde_json::to_vec(&applied).unwrap();
    let restored: GraphDocument = serde_json::from_slice(&json).unwrap();
    let reroute = restored.nodes.get(&node_id(0xa11)).unwrap();
    assert_eq!(reroute.node_type.as_str(), EFFECT_REROUTE_NODE_TYPE);
    assert_eq!(reroute.position, NodePosition { x: 120.5, y: -30.0 });
    assert_eq!(restored.connections.len(), 2);
    let source_side = restored.connections.get(&connection_id(0xb11)).unwrap();
    let target_side = restored.connections.get(&connection_id(0xb12)).unwrap();
    assert_eq!(source_side.output, original.output);
    assert_eq!(source_side.input, declared(reroute.id, REROUTE_INPUT_PORT));
    assert_eq!(source_side.order, None);
    assert_eq!(
        target_side.output,
        declared(reroute.id, REROUTE_OUTPUT_PORT)
    );
    assert_eq!(target_side.input, original.input);
    assert_eq!(target_side.order, original.order);

    let analysis = GraphCompiler::new(registry.as_ref(), &EmptyResources)
        .compile(&restored)
        .analysis;
    let localization = catalog.localization("en-US");
    let projection = crate::node_system::analysis::EditorGraphProjectionDto::from_sources(
        "events/Reroute.yssbi-event",
        &analysis,
        &restored,
        registry.as_ref(),
        &localization,
    )
    .unwrap();
    let projected = projection
        .nodes
        .iter()
        .find(|node| node.node_id.as_ref() == reroute.id.to_string())
        .unwrap();
    assert_eq!(
        projected.display.style_id.as_deref(),
        Some("builtin.reroute")
    );
    assert_eq!(projected.position.x, 120.5);
    assert_eq!(projected.position.y, -30.0);
    assert_eq!(projected.ports.len(), 2);
    for port in &projected.ports {
        match &port.address {
            crate::node_system::document::PortAddressDto::Declared { node_id, .. }
            | crate::node_system::document::PortAddressDto::Instance { node_id, .. } => {
                assert_eq!(node_id.as_ref(), reroute.id.to_string().as_str());
            }
        }
    }
}

#[test]
fn phase2_insert_reroute_document_rejects_invalid_inputs_without_allocated_patch() {
    let registry = build_builtin_node_system().unwrap().registry;
    let (document, original) = Case::data(None).document();
    let before = serde_json::to_vec(&document).unwrap();

    for mutation in [
        EditorGraphMutationDto::InsertReroute {
            connection_id: connection_id(0xffff),
            position: NodePosition { x: 1.0, y: 2.0 },
        },
        EditorGraphMutationDto::InsertReroute {
            connection_id: original.id,
            position: NodePosition {
                x: f64::NAN,
                y: 2.0,
            },
        },
        EditorGraphMutationDto::InsertReroute {
            connection_id: original.id,
            position: NodePosition {
                x: 1.0,
                y: f64::INFINITY,
            },
        },
    ] {
        assert!(
            mutation
                .into_patch(
                    &graph_path("events/Reroute.yssbi-event"),
                    &document,
                    registry.as_ref()
                )
                .is_err()
        );
        assert_eq!(serde_json::to_vec(&document).unwrap(), before);
    }
}

#[test]
fn phase2_insert_reroute_document_rejects_orphan_malformed_and_mismatched_endpoints() {
    let registry = build_builtin_node_system().unwrap().registry;
    let (base, original) = Case::data(None).document();
    let mut malformed_documents = Vec::new();

    let mut missing_node = base.clone();
    missing_node.nodes.remove(&original.input.node_id);
    malformed_documents.push(missing_node);

    let mut wrong_direction = base.clone();
    wrong_direction
        .connections
        .get_mut(&original.id)
        .unwrap()
        .output = declared(original.input.node_id, "data");
    malformed_documents.push(wrong_direction);

    let mut kind_mismatch = base.clone();
    kind_mismatch
        .connections
        .get_mut(&original.id)
        .unwrap()
        .input = declared(original.input.node_id, "enter");
    malformed_documents.push(kind_mismatch);

    let mut orphan = base.clone();
    let orphan_address = PortAddress::instance(
        original.input.node_id,
        PortKey::new("data").unwrap(),
        instance_id(0x999),
    );
    orphan.connections.get_mut(&original.id).unwrap().input = orphan_address.clone();
    orphan.port_bindings.insert(
        orphan_address,
        DynamicPortBinding::Orphan {
            origin: DynamicMemberLocator::SchemaField {
                source: SchemaSourceIdentity("source".into()),
                field: SchemaFieldIdentity("field".into()),
            },
            order: OrderKey("orphan".into()),
            last_known: LastKnownPortMetadata {
                label: "Orphan".into(),
                value_type: None,
            },
        },
    );
    malformed_documents.push(orphan);

    for document in malformed_documents {
        assert!(
            EditorGraphMutationDto::InsertReroute {
                connection_id: original.id,
                position: NodePosition { x: 1.0, y: 2.0 },
            }
            .into_patch(
                &graph_path("events/Reroute.yssbi-event"),
                &document,
                registry.as_ref()
            )
            .is_err()
        );
    }
}

#[derive(Clone, Copy)]
struct Case {
    source_type: &'static str,
    source_port: &'static str,
    target_type: &'static str,
    target_port: &'static str,
    reroute_type: &'static str,
    order: Option<&'static str>,
}

impl Case {
    const fn data(order: Option<&'static str>) -> Self {
        Self {
            source_type: "yssbi.constant.int64",
            source_port: "value",
            target_type: "yssbi.debug.view",
            target_port: "data",
            reroute_type: DATA_REROUTE_NODE_TYPE,
            order,
        }
    }

    const fn control(order: Option<&'static str>) -> Self {
        Self {
            source_type: "yssbi.debug.view",
            source_port: "then",
            target_type: "yssbi.debug.view",
            target_port: "enter",
            reroute_type: CONTROL_REROUTE_NODE_TYPE,
            order,
        }
    }

    const fn effect(order: Option<&'static str>) -> Self {
        Self {
            source_type: "yssbi.control.do",
            source_port: "effect_out",
            target_type: "yssbi.control.do",
            target_port: "effect_in",
            reroute_type: EFFECT_REROUTE_NODE_TYPE,
            order,
        }
    }

    fn document(self) -> (GraphDocument, DocumentConnection) {
        let mut document = GraphDocument::default();
        document.nodes.insert(
            node_id(SOURCE_NODE),
            DocumentNode {
                id: node_id(SOURCE_NODE),
                node_type: NodeTypeId::new(self.source_type).unwrap(),
                position: NodePosition { x: 0.0, y: 0.0 },
                parameters: ParameterValues::new(),
                user_label: None,
            },
        );
        document.nodes.insert(
            node_id(TARGET_NODE),
            DocumentNode {
                id: node_id(TARGET_NODE),
                node_type: NodeTypeId::new(self.target_type).unwrap(),
                position: NodePosition { x: 200.0, y: 0.0 },
                parameters: ParameterValues::new(),
                user_label: None,
            },
        );
        let connection = DocumentConnection {
            id: connection_id(ORIGINAL_CONNECTION),
            output: declared(node_id(SOURCE_NODE), self.source_port),
            input: declared(node_id(TARGET_NODE), self.target_port),
            order: self.order.map(|value| OrderKey(value.into())),
        };
        document
            .connections
            .insert(connection.id, connection.clone());
        (document, connection)
    }
}
