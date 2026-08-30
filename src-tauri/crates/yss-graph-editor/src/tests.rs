use crate::{
    CatalogMutationResource, CatalogMutationValidationSnapshot, EditorGraphMutation,
    EditorMutationErrorCode, MutationConflict,
};
use std::collections::BTreeMap;
use yss_data_contract::DataType;
use yss_graph_catalog::{
    CatalogResourcePath, authoritative_static_descriptor, build_builtin_node_system,
};
use yss_graph_document::{
    DocumentConnection, DocumentNode, DynamicMemberLocator, DynamicPortBinding,
    FunctionParameterId, GraphDocument, GraphResourcePath, LastKnownPortMetadata, NodeId,
    NodePosition, OrderKey, ParameterValues, PortAddress, PortInstanceId,
};
use yss_graph_document_edit::apply_graph_document_patch;
use yss_graph_protocol::{NodeTypeId, ParameterKey, PortKey, TypeExpr};
use yss_graph_registry::NodeRegistry;
use yss_graph_resource_contract::{
    GraphResourceId, ResourceCatalogFingerprint, ResourceCatalogSnapshot, VariableValueContract,
};
use yss_variable_contract::VariableScope;

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
fn connect_rejects_incompatible_data_types() {
    let registry = build_builtin_node_system()
        .expect("built-in registry must assemble")
        .registry;
    let mut document = GraphDocument::default();
    let source = insert_node(&mut document, document_node("yssbi.constant.string", 0.0));
    let target = insert_node(
        &mut document,
        document_node("yssbi.numeric.add.int64", 100.0),
    );

    let error = EditorGraphMutation::Connect {
        output: declared(source, "value"),
        input: declared(target, "left"),
        order: None,
    }
    .into_patch(&graph_path(), &document, registry.as_ref())
    .expect_err("String must not connect to Int64");

    let MutationConflict::Editor(error) = error else {
        panic!("expected an editor validation error");
    };
    assert_eq!(
        error.code,
        EditorMutationErrorCode::GraphConnectionTypeMismatch
    );
}

#[test]
fn move_connections_uses_current_document_authority() {
    let registry = build_builtin_node_system()
        .expect("built-in registry must assemble")
        .registry;
    let mut document = GraphDocument::default();
    let source = insert_node(&mut document, document_node("yssbi.constant.int64", 0.0));
    let target = insert_node(
        &mut document,
        document_node("yssbi.numeric.add.int64", 100.0),
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
    let source = insert_node(&mut document, document_node("yssbi.constant.int64", 0.0));
    let source_output = declared(source, "value");
    let catalog = CatalogMutationValidationSnapshot {
        resources: BTreeMap::new(),
    };

    let patch = EditorGraphMutation::CreateNode {
        descriptor: static_descriptor(registry.as_ref(), "yssbi.numeric.add.int64"),
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
    assert_eq!(created.node_type.as_str(), "yssbi.numeric.add.int64");
    assert_eq!(created.user_label.as_deref(), Some("sum"));
}

#[test]
fn port_resolution_rejects_binding_kind_drift() {
    let registry = build_builtin_node_system()
        .expect("built-in registry must assemble")
        .registry;
    let mut document = GraphDocument::default();
    let node = insert_node(
        &mut document,
        document_node("yssbi.numeric.series.add", 0.0),
    );
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
fn resource_type_refinement_uses_the_protocol_binding_parameter() {
    let registry = build_builtin_node_system()
        .expect("built-in registry must assemble")
        .registry;
    let resource_path = CatalogResourcePath::new("variables/00000000-0000-0000-0000-000000000001");
    let mut node = document_node("yssbi.project.variable.get", 0.0);
    node.parameters.insert(
        ParameterKey::new("aaa").expect("fixture key must be valid"),
        serde_json::Value::String("functions/Wrong.yssbi-function".into()),
    );
    node.parameters.insert(
        ParameterKey::new("variable").expect("fixture key must be valid"),
        serde_json::Value::String(resource_path.as_str().into()),
    );
    let mut document = GraphDocument::default();
    let node_id = insert_node(&mut document, node);
    let catalog = CatalogMutationValidationSnapshot {
        resources: BTreeMap::from([(
            resource_path,
            CatalogMutationResource::Variable {
                revision: 1,
                scope: VariableScope::Global,
                data_type: DataType::Int64,
            },
        )]),
    };

    let source = crate::compatibility::source_port(
        &document,
        registry.as_ref(),
        &catalog,
        declared(node_id, "value"),
    )
    .expect("type refinement must use the protocol-owned 'variable' binding");

    assert_eq!(
        source.value_type,
        TypeExpr::Concrete("core.int64".parse().expect("fixture type ID must be valid"))
    );
}

#[test]
fn compatible_catalog_source_uses_the_protocol_binding_parameter() {
    let registry = build_builtin_node_system()
        .expect("built-in registry must assemble")
        .registry;
    let resource_path = "variables/00000000-0000-0000-0000-000000000002";
    let mut node = document_node("yssbi.project.variable.get", 0.0);
    node.parameters.insert(
        ParameterKey::new("aaa").expect("fixture key must be valid"),
        serde_json::Value::String("databases/wrong".into()),
    );
    node.parameters.insert(
        ParameterKey::new("variable").expect("fixture key must be valid"),
        serde_json::Value::String(resource_path.into()),
    );
    let mut document = GraphDocument::default();
    let node_id = insert_node(&mut document, node);
    let catalog = ResourceCatalogSnapshot::new(
        BTreeMap::new(),
        BTreeMap::from([(
            GraphResourceId::new(resource_path),
            VariableValueContract::new(DataType::Int64),
        )]),
        BTreeMap::new(),
        ResourceCatalogFingerprint::from_bytes([7; 32]),
    );

    let source = crate::compatibility::catalog_query_source_port(
        &document,
        registry.as_ref(),
        &declared(node_id, "value"),
        &catalog,
    )
    .expect("compatible catalog source must use the protocol-owned binding");

    assert_eq!(
        source.value_type,
        TypeExpr::Concrete("core.int64".parse().expect("fixture type ID must be valid"))
    );
}
