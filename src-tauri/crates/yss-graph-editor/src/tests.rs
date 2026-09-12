use crate::{
    CatalogMutationValidationSnapshot, EditorGraphMutation, EditorMutationErrorCode,
};
use std::collections::BTreeMap;
use yss_data_contract::DataType;
use yss_graph_catalog::{authoritative_static_descriptor, build_builtin_node_system};
use yss_graph_document::{
    DocumentConnection, DocumentNode, DynamicMemberLocator, DynamicPortBinding,
    FunctionParameterId, GraphDocument, GraphResourcePath, LastKnownPortMetadata, NodeId,
    NodePosition, OrderKey, ParameterValues, PortAddress, PortInstanceId,
};
use yss_graph_document_edit::apply_graph_document_patch;
use yss_graph_protocol::{NodeTypeId, ParameterKey, PortKey, TypeExpr};
use yss_graph_registry::NodeRegistry;

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

fn static_descriptor(registry: &NodeRegistry, node_type: &str) -> yss_graph_catalog::NodeCreation {
    let node_type = NodeTypeId::new(node_type).expect("fixture node type must be valid");
    let protocol = registry
        .protocol(&node_type)
        .expect("fixture protocol must exist");
    authoritative_static_descriptor(registry, protocol)
        .expect("fixture protocol must have a catalog creation descriptor")
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
            DataType::Int64,
            yss_data_contract::DataValue::Int64(1),
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
}

#[test]
fn connect_preserves_a_structurally_valid_draft_for_semantic_analysis() {
    let registry = build_builtin_node_system()
        .expect("built-in registry must assemble")
        .registry;
    let mut document = GraphDocument::default();
    let source = insert_node(&mut document, document_node("yssbi.constant.get", 0.0));
    set_constant(
        &mut document,
        source,
        DataType::String,
        yss_data_contract::DataValue::String(String::new()),
    );
    let target = insert_node(
        &mut document,
        document_node("yssbi.numeric.subtract", 100.0),
    );

    let patch = EditorGraphMutation::Connect {
        output: declared(source, "value"),
        input: declared(target, "left"),
        order: None,
    }
    .into_patch(&graph_path(), &document, registry.as_ref())
    .expect("structural editing must leave authoritative type diagnostics to graph analysis");
    apply_graph_document_patch(&mut document, &patch).expect("planned connection must be valid");

    assert_eq!(document.connections.len(), 1);
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
        DataType::Int64,
        yss_data_contract::DataValue::Int64(0),
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
        DataType::Int64,
        yss_data_contract::DataValue::Int64(0),
    );
    let source_output = declared(source, "value");
    let catalog = CatalogMutationValidationSnapshot {
        resources: BTreeMap::new(),
    };

    let patch = EditorGraphMutation::CreateNode {
        descriptor: static_descriptor(registry.as_ref(), "yssbi.numeric.add"),
        position: NodePosition { x: 100.0, y: 0.0 },
        user_label: Some("sum".into()),
        connect_from: Some(source_output.clone()),
    }
    .into_patch_with_catalog_snapshot(&graph_path(), &document, registry.as_ref(), Some(&catalog))
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
                    "core.int64".parse().expect("fixture type ID must be valid"),
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
                data_type: DataType::Int64,
                data_value: yss_data_contract::DataValue::Int64(42),
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
    data_type: yss_data_contract::DataType,
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
    let registry = build_builtin_node_system().unwrap().registry;
    let catalog = CatalogMutationValidationSnapshot {
        resources: BTreeMap::new(),
    };
    let mut source = GraphDocument::default();
    let node = insert_node(&mut source, document_node("yssbi.constant.get", 0.0));
    set_constant(
        &mut source,
        node,
        DataType::Int64,
        yss_data_contract::DataValue::Int64(42),
    );
    let snapshot = crate::export_subgraph(&source, &registry, &catalog, vec![node]).unwrap();
    let bytes = serde_json::to_vec(&snapshot).unwrap();
    let snapshot = crate::deserialize_clipboard_subgraph(&bytes).unwrap();
    assert_eq!(snapshot.constants.len(), 1);
    let mut target = source.clone();
    target.constants.values_mut().next().unwrap().data_value =
        yss_data_contract::DataValue::Int64(99);
    let before = target.clone();
    let patch = EditorGraphMutation::InsertSubgraph {
        snapshot,
        anchor: NodePosition { x: 200.0, y: 0.0 },
    }
    .into_patch_with_catalog_snapshot(&graph_path(), &target, &registry, Some(&catalog))
    .unwrap();
    apply_graph_document_patch(&mut target, &patch).unwrap();
    assert_eq!(target.constants.len(), 2);
    let pasted = target
        .nodes
        .values()
        .find(|candidate| candidate.id != node)
        .unwrap();
    let id: yss_graph_document::ConstantId = pasted.parameters
        [&ParameterKey::new("constant").unwrap()]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(
        target.constants[&id].data_value,
        yss_data_contract::DataValue::Int64(42)
    );
    assert_ne!(
        target.constants[&id].name,
        source.constants.values().next().unwrap().name
    );
    apply_graph_document_patch(&mut target, &patch.inverse()).unwrap();
    assert_eq!(target, before);
    let patch = EditorGraphMutation::DuplicateSubgraph {
        node_ids: vec![node],
        offset: NodePosition { x: 200.0, y: 0.0 },
    }
    .into_patch_with_catalog_snapshot(&graph_path(), &source, &registry, Some(&catalog))
    .unwrap();
    apply_graph_document_patch(&mut source, &patch).unwrap();
    assert_eq!(source.constants.len(), 1);
}
