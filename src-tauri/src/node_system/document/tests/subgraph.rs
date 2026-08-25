use super::*;
use crate::node_system::catalog::{CatalogResourcePath, ResourceBoundCreateArgsDto};
use crate::node_system::document::{
    ClipboardConnectionDto, ClipboardDynamicMemberOriginDto, ClipboardDynamicPortBindingDto,
    ClipboardLastKnownPortMetadataDto, ClipboardNodeCreationDto, ClipboardNodeDto, ClipboardNodeId,
    ClipboardPortAddressDto, ClipboardPortBindingDto, ClipboardPortInstanceId, ClipboardPortRefDto,
    ClipboardSubgraphDto, GraphDocumentOperation, MutationConflict, deserialize_clipboard_subgraph,
    duplicate_subgraph, export_subgraph, instantiate_subgraph_for_test as instantiate_subgraph,
};
use crate::project::{
    CatalogMutationResource, CatalogMutationValidationSnapshot, ProjectInstanceId,
};

struct SubgraphTestLowerer;

impl NodeLowerer for SubgraphTestLowerer {
    fn lower(&self, _: &LoweringContext<'_>) -> Result<LoweredNode, LoweringError> {
        Ok(LoweredNode {
            kernel: crate::node_system::compiler::LoweredKernel::Native(
                crate::node_system::plan::KernelHandle::new("testing.subgraph").unwrap(),
            ),
            parameters: crate::node_system::plan::CompiledParameterHandle::new(
                "testing.subgraph.parameters",
            )
            .unwrap(),
        })
    }
}

struct ExportFixture {
    graph_path: GraphResourcePath,
    document: GraphDocument,
    registry: NodeRegistry,
    catalog: CatalogMutationValidationSnapshot,
    first: NodeId,
    second: NodeId,
    external: NodeId,
    first_input_instance: PortInstanceId,
    second_input_instance: PortInstanceId,
    external_input_instance: PortInstanceId,
    internal_connection: ConnectionId,
    outgoing_connection: ConnectionId,
    incoming_connection: ConnectionId,
}

fn empty_catalog_snapshot() -> CatalogMutationValidationSnapshot {
    CatalogMutationValidationSnapshot {
        project_instance_id: ProjectInstanceId::new(),
        authority_generation: 0,
        resources: BTreeMap::new(),
    }
}

fn subgraph_registry() -> NodeRegistry {
    let value_type = TypeExpr::Concrete(TypeId::new("core.int64").unwrap());
    let protocol = TestProtocolBuilder::new("yssbi.test.editor_mutation", "test")
        .style("test")
        .ports(vec![
            PortSpec {
                key: PortKey::new("output").unwrap(),
                title: "Output".into(),
                direction: PortDirection::Output,
                kind: PortKind::Data,
                value_type: value_type.clone(),
                instances: PortInstances::Declared,
                connections: ConnectionsPerPort::Multiple {
                    max: None,
                    ordered: false,
                },
                input_binding: None,
                consumption: None,
                production: None,
                editor: PortEditorSpec::Default,
                schema: None,
            },
            PortSpec {
                key: PortKey::new("inputs").unwrap(),
                title: "Inputs".into(),
                direction: PortDirection::Input,
                kind: PortKind::Data,
                value_type,
                instances: PortInstances::UserCreated {
                    min: 1,
                    max: Some(2),
                },
                connections: ConnectionsPerPort::Multiple {
                    max: None,
                    ordered: true,
                },
                input_binding: Some(InputBindingSpec {
                    literal_policy: LiteralPolicy::Allowed,
                    default_value: None,
                }),
                consumption: None,
                production: None,
                editor: PortEditorSpec::Default,
                schema: None,
            },
        ])
        .parameters(vec![crate::node_system::protocol::ParameterSpec {
            key: ParameterKey::new("export_marker").unwrap(),
            title_key: I18nKey::new("nodes.test.editor_mutation.export_marker.title").unwrap(),
            description_key: None,
            value_type: TypeExpr::Concrete(TypeId::new("core.int64").unwrap()),
            default_value: None,
            constraints: Vec::new(),
            editor: crate::node_system::protocol::ParameterEditorSpec::Number,
            presentation: crate::node_system::protocol::ParameterPresentation::DetailPanel,
        }])
        .execution(EDITOR_MUTATION_EXECUTION)
        .scope(NodeScope::Any)
        .build();
    let (mut provider, _, _) = builtin_bundle_parts_for_test().unwrap();
    let mut categories = provider.categories.into_vec();
    categories.push(CategoryRegistration {
        id: NodeCategoryId::new("test").unwrap(),
        title_key: I18nKey::new("categories.test.title").unwrap(),
        parent: None,
        order: 0,
    });
    provider.categories = categories.into_boxed_slice();
    provider.i18n.keys.extend([
        I18nKey::new("categories.test.title").unwrap(),
        I18nKey::new("nodes.test.editor_mutation.title").unwrap(),
        I18nKey::new("nodes.test.editor_mutation.output").unwrap(),
        I18nKey::new("nodes.test.editor_mutation.inputs").unwrap(),
        I18nKey::new("nodes.test.editor_mutation.export_marker.title").unwrap(),
    ]);
    let mut nodes = provider.nodes.into_vec();
    nodes.push(RegisteredNode::leaf(
        Arc::new(protocol),
        Arc::new(NodeImplementation::new(SubgraphTestLowerer)),
    ));
    provider.nodes = nodes.into_boxed_slice();
    let mut builder = NodeRegistryBuilder::new();
    crate::node_system::catalog::register_builtin_nominal_validators_for_test(&mut builder)
        .unwrap();
    builder.register_provider(provider).unwrap();
    builder.freeze().unwrap()
}

fn int64_literal(value: i64) -> serde_json::Value {
    serde_json::to_value(crate::node_system::protocol::TypedValue {
        value_type: TypeExpr::Concrete(TypeId::new("core.int64").unwrap()),
        value: crate::node_system::protocol::Value::Integer(value),
    })
    .unwrap()
}

fn export_test_node(
    id: NodeId,
    position: NodePosition,
    user_label: Option<&str>,
    parameter_value: i64,
) -> DocumentNode {
    let mut parameters = ParameterValues::new();
    parameters.insert(
        ParameterKey::new("export_marker").unwrap(),
        json!(parameter_value),
    );
    DocumentNode {
        id,
        node_type: NodeTypeId::new("yssbi.test.editor_mutation").unwrap(),
        position,
        parameters,
        user_label: user_label.map(str::to_owned),
    }
}

fn user_input_address(node_id: NodeId, instance_id: PortInstanceId) -> PortAddress {
    PortAddress::instance(node_id, PortKey::new("inputs").unwrap(), instance_id)
}

fn declared_output_address(node_id: NodeId) -> PortAddress {
    PortAddress::declared(node_id, PortKey::new("output").unwrap())
}

fn insert_user_input(
    document: &mut GraphDocument,
    node_id: NodeId,
    instance_id: PortInstanceId,
    order: &str,
) -> PortAddress {
    let address = user_input_address(node_id, instance_id);
    document.port_bindings.insert(
        address.clone(),
        DynamicPortBinding::UserCreated {
            order: OrderKey(order.into()),
        },
    );
    address
}

fn insert_connection(
    document: &mut GraphDocument,
    id: ConnectionId,
    output_node: NodeId,
    input: PortAddress,
    order: Option<&str>,
) {
    document.connections.insert(
        id,
        DocumentConnection {
            id,
            output: declared_output_address(output_node),
            input,
            order: order.map(|value| OrderKey(value.into())),
        },
    );
}

fn export_fixture() -> ExportFixture {
    let first = node_id(0x101);
    let second = node_id(0x102);
    let external = node_id(0x103);
    let first_input_instance = instance_id(0x201);
    let second_input_instance = instance_id(0x202);
    let external_input_instance = instance_id(0x203);
    let internal_connection = connection_id(0x301);
    let outgoing_connection = connection_id(0x302);
    let incoming_connection = connection_id(0x303);
    let mut document = GraphDocument::default();

    document.nodes.insert(
        first,
        export_test_node(first, NodePosition { x: 20.0, y: 30.0 }, Some("Source"), 11),
    );
    document.nodes.insert(
        second,
        export_test_node(
            second,
            NodePosition { x: 80.0, y: 90.0 },
            Some("Reroute"),
            22,
        ),
    );
    document.nodes.insert(
        external,
        export_test_node(
            external,
            NodePosition { x: 160.0, y: 180.0 },
            Some("External"),
            33,
        ),
    );

    let first_input = insert_user_input(&mut document, first, first_input_instance, "first-input");
    let second_input =
        insert_user_input(&mut document, second, second_input_instance, "second-input");
    let external_input = insert_user_input(
        &mut document,
        external,
        external_input_instance,
        "external-input",
    );
    document.input_states.insert(
        second_input.clone(),
        InputState {
            literal_override: Some(int64_literal(42)),
        },
    );

    insert_connection(
        &mut document,
        internal_connection,
        first,
        second_input,
        Some("internal-order"),
    );
    insert_connection(
        &mut document,
        outgoing_connection,
        second,
        external_input,
        None,
    );
    insert_connection(
        &mut document,
        incoming_connection,
        external,
        first_input,
        None,
    );

    ExportFixture {
        graph_path: graph_path("events/export.yssbi-event"),
        document,
        registry: subgraph_registry(),
        catalog: empty_catalog_snapshot(),
        first,
        second,
        external,
        first_input_instance,
        second_input_instance,
        external_input_instance,
        internal_connection,
        outgoing_connection,
        incoming_connection,
    }
}

fn export_selected(
    fixture: &ExportFixture,
    node_ids: Vec<NodeId>,
) -> Result<ClipboardSubgraphDto, MutationConflict> {
    export_subgraph(
        &fixture.graph_path,
        &fixture.document,
        &fixture.registry,
        &fixture.catalog,
        node_ids,
    )
}

#[test]
fn subgraph_export_uses_relative_positions_and_omits_authority_ids() {
    let fixture = export_fixture();
    let snapshot = export_selected(&fixture, vec![fixture.second, fixture.first]).unwrap();

    assert_eq!(snapshot.nodes.len(), 2);
    assert_eq!(snapshot.nodes[0].local_id.0.as_ref(), "node/0");
    assert_eq!(snapshot.nodes[1].local_id.0.as_ref(), "node/1");
    assert_eq!(
        snapshot.nodes[0].relative_position,
        NodePosition { x: 0.0, y: 0.0 },
    );
    assert_eq!(
        snapshot.nodes[1].relative_position,
        NodePosition { x: 60.0, y: 60.0 },
    );

    let wire = serde_json::to_string(&snapshot).unwrap();
    for authority_id in [
        fixture.first.to_string(),
        fixture.second.to_string(),
        fixture.internal_connection.to_string(),
        fixture.first_input_instance.to_string(),
        fixture.second_input_instance.to_string(),
    ] {
        assert!(!wire.contains(&authority_id));
    }
}

#[test]
fn subgraph_export_preserves_parameters_labels_bindings_and_literals() {
    let fixture = export_fixture();
    let snapshot = export_selected(&fixture, vec![fixture.first, fixture.second]).unwrap();

    assert_eq!(snapshot.schema_version, 1);
    assert_eq!(snapshot.nodes.len(), 2);
    assert_eq!(snapshot.nodes[0].user_label.as_deref(), Some("Source"));
    assert_eq!(snapshot.nodes[1].user_label.as_deref(), Some("Reroute"));
    assert_eq!(
        snapshot.nodes[0]
            .parameters
            .get(&ParameterKey::new("export_marker").unwrap()),
        Some(&json!(11)),
    );
    assert!(matches!(
        snapshot.nodes[0].creation,
        ClipboardNodeCreationDto::Static { .. }
    ));
    assert_eq!(snapshot.port_bindings.len(), 2);
    assert!(
        snapshot
            .port_bindings
            .iter()
            .all(|entry| matches!(entry.address.port, ClipboardPortRefDto::Instance { .. }))
    );
    assert_eq!(snapshot.input_states.len(), 1);
    assert_eq!(
        snapshot.input_states[0].state.literal_override,
        Some(int64_literal(42)),
    );
}

#[test]
fn subgraph_export_keeps_only_internal_connections() {
    let fixture = export_fixture();
    let snapshot = export_selected(&fixture, vec![fixture.first, fixture.second]).unwrap();

    assert_eq!(snapshot.connections.len(), 1);
    assert_eq!(
        snapshot.connections[0].order,
        Some(OrderKey("internal-order".into())),
    );
    let wire = serde_json::to_string(&snapshot).unwrap();
    assert!(!wire.contains(&fixture.outgoing_connection.to_string()));
    assert!(!wire.contains(&fixture.incoming_connection.to_string()));
    assert!(!wire.contains(&fixture.external.to_string()));
    assert!(!wire.contains(&fixture.external_input_instance.to_string()));
}

#[test]
fn subgraph_export_rejects_empty_duplicate_and_missing_targets() {
    let fixture = export_fixture();
    let missing = node_id(0x999);

    for result in [
        export_selected(&fixture, Vec::new()),
        export_selected(&fixture, vec![fixture.first, fixture.first]),
        export_selected(&fixture, vec![fixture.first, missing]),
    ] {
        let error = result.unwrap_err();
        assert!(!error.code().is_empty());
        assert!(!error.to_string().is_empty());
    }
}

fn insert_snapshot(fixture: &ExportFixture) -> ClipboardSubgraphDto {
    let mut snapshot = export_selected(fixture, vec![fixture.first, fixture.second]).unwrap();
    for node in &mut snapshot.nodes {
        node.parameters.clear();
    }
    snapshot
}

fn instantiate(
    fixture: &ExportFixture,
    document: &GraphDocument,
    snapshot: ClipboardSubgraphDto,
) -> Result<GraphDocumentPatch, MutationConflict> {
    instantiate_subgraph(
        &fixture.graph_path,
        document,
        &fixture.registry,
        &fixture.catalog,
        snapshot,
        NodePosition { x: 100.0, y: 200.0 },
    )
}

fn assert_clipboard_invalid(error: MutationConflict) {
    assert_eq!(error.code(), "clipboard_subgraph_invalid");
    assert!(!error.to_string().is_empty());
}

fn dangling_address() -> ClipboardPortAddressDto {
    ClipboardPortAddressDto {
        node_id: ClipboardNodeId("node/missing".into()),
        port: ClipboardPortRefDto::Declared {
            key: PortKey::new("output").unwrap(),
        },
    }
}

#[test]
fn subgraph_insert_allocates_fresh_document_ids() {
    let fixture = export_fixture();
    let snapshot = insert_snapshot(&fixture);
    let patch = instantiate(&fixture, &GraphDocument::default(), snapshot).unwrap();

    let nodes = patch
        .operations
        .iter()
        .filter_map(|operation| match operation {
            GraphDocumentOperation::InsertNode { node } => Some(node),
            _ => None,
        })
        .collect::<Vec<_>>();
    let bindings = patch
        .operations
        .iter()
        .filter_map(|operation| match operation {
            GraphDocumentOperation::InsertPortBinding { address, .. } => Some(address),
            _ => None,
        })
        .collect::<Vec<_>>();
    let connections = patch
        .operations
        .iter()
        .filter_map(|operation| match operation {
            GraphDocumentOperation::InsertConnection { connection } => Some(connection),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(nodes.len(), 2);
    assert_eq!(nodes[0].position, NodePosition { x: 100.0, y: 200.0 });
    assert_eq!(nodes[1].position, NodePosition { x: 160.0, y: 260.0 });
    assert!(
        nodes
            .iter()
            .all(|node| ![fixture.first, fixture.second].contains(&node.id))
    );
    assert!(bindings.iter().all(|address| match address.port {
        PortRef::Instance { instance_id, .. } =>
            ![fixture.first_input_instance, fixture.second_input_instance,].contains(&instance_id),
        PortRef::Declared { .. } => false,
    }));
    assert_eq!(connections.len(), 1);
    assert_ne!(connections[0].id, fixture.internal_connection);
    assert!(patch.operations.iter().all(|operation| {
        let wire = serde_json::to_string(operation).unwrap();
        !wire.contains("node/0") && !wire.contains("node/1") && !wire.contains("port/")
    }));
}

#[test]
fn subgraph_insert_restores_dynamic_instances_literals_and_ordered_edges() {
    let fixture = export_fixture();
    let patch = instantiate(
        &fixture,
        &GraphDocument::default(),
        insert_snapshot(&fixture),
    )
    .unwrap();
    let mut document = GraphDocument::default();
    document.apply_patch(&patch).unwrap();

    assert_eq!(document.nodes.len(), 2);
    assert_eq!(document.port_bindings.len(), 2);
    assert_eq!(document.input_states.len(), 1);
    assert_eq!(
        document
            .input_states
            .values()
            .next()
            .unwrap()
            .literal_override,
        Some(int64_literal(42)),
    );
    assert_eq!(document.connections.len(), 1);
    assert_eq!(
        document.connections.values().next().unwrap().order,
        Some(OrderKey("internal-order".into())),
    );
}

#[test]
fn subgraph_insert_rejects_wrong_schema_version() {
    let fixture = export_fixture();
    let mut snapshot = insert_snapshot(&fixture);
    snapshot.schema_version += 1;

    assert_clipboard_invalid(
        instantiate(&fixture, &GraphDocument::default(), snapshot).unwrap_err(),
    );
}

#[test]
fn subgraph_insert_rejects_duplicate_local_ids() {
    let fixture = export_fixture();
    let mut duplicate_node = insert_snapshot(&fixture);
    duplicate_node.nodes[1].local_id = duplicate_node.nodes[0].local_id.clone();
    assert_clipboard_invalid(
        instantiate(&fixture, &GraphDocument::default(), duplicate_node).unwrap_err(),
    );

    let mut duplicate_instance = insert_snapshot(&fixture);
    duplicate_instance
        .port_bindings
        .push(duplicate_instance.port_bindings[0].clone());
    assert_clipboard_invalid(
        instantiate(&fixture, &GraphDocument::default(), duplicate_instance).unwrap_err(),
    );
}

#[test]
fn subgraph_insert_rejects_dangling_local_references() {
    let fixture = export_fixture();
    let mut dangling_node = insert_snapshot(&fixture);
    dangling_node.connections[0].output = dangling_address();
    assert_clipboard_invalid(
        instantiate(&fixture, &GraphDocument::default(), dangling_node).unwrap_err(),
    );

    let mut dangling_instance = insert_snapshot(&fixture);
    dangling_instance.connections[0].input.port = ClipboardPortRefDto::Instance {
        template: PortKey::new("inputs").unwrap(),
        local_instance_id: ClipboardPortInstanceId("port/missing".into()),
    };
    assert_clipboard_invalid(
        instantiate(&fixture, &GraphDocument::default(), dangling_instance).unwrap_err(),
    );
}

#[test]
fn subgraph_insert_rejects_non_finite_positions() {
    let fixture = export_fixture();
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut snapshot = insert_snapshot(&fixture);
        snapshot.nodes[0].relative_position.x = invalid;
        assert_clipboard_invalid(
            instantiate(&fixture, &GraphDocument::default(), snapshot).unwrap_err(),
        );
    }
}

#[test]
fn subgraph_insert_rejects_missing_protocol() {
    let fixture = export_fixture();
    let mut snapshot = insert_snapshot(&fixture);
    snapshot.nodes[0].creation = ClipboardNodeCreationDto::Static {
        node_type_id: NodeTypeId::new("yssbi.test.missing").unwrap(),
    };

    assert_clipboard_invalid(
        instantiate(&fixture, &GraphDocument::default(), snapshot).unwrap_err(),
    );
}

#[test]
fn subgraph_insert_rejects_missing_resource() {
    let fixture = export_fixture();
    let mut snapshot = insert_snapshot(&fixture);
    snapshot.nodes[0].creation = ClipboardNodeCreationDto::ResourceBound {
        node_type_id: NodeTypeId::new("yssbi.test.editor_mutation").unwrap(),
        resource_path: CatalogResourcePath::new("functions/missing"),
        create_args: ResourceBoundCreateArgsDto::Function,
    };

    let error = instantiate(&fixture, &GraphDocument::default(), snapshot).unwrap_err();
    assert_eq!(error.code(), "referenced_resource_unavailable");
    assert!(!error.to_string().is_empty());
}

#[test]
fn subgraph_insert_rejects_each_limit_plus_one() {
    let fixture = export_fixture();
    let base = insert_snapshot(&fixture);
    let invalid_node = || ClipboardNodeDto {
        local_id: ClipboardNodeId("limit".into()),
        creation: ClipboardNodeCreationDto::Static {
            node_type_id: NodeTypeId::new("yssbi.test.editor_mutation").unwrap(),
        },
        parameters: ParameterValues::new(),
        user_label: None,
        relative_position: NodePosition { x: 0.0, y: 0.0 },
    };
    let invalid_binding = || ClipboardPortBindingDto {
        address: dangling_address(),
        binding: ClipboardDynamicPortBindingDto::UserCreated {
            order: OrderKey("limit".into()),
        },
    };
    let invalid_connection = || ClipboardConnectionDto {
        output: dangling_address(),
        input: dangling_address(),
        order: None,
    };

    let mut cases = Vec::new();
    let mut nodes = base.clone();
    nodes.nodes = (0..=crate::node_system::document::subgraph::MAX_CLIPBOARD_NODES)
        .map(|index| {
            let mut node = invalid_node();
            node.local_id = ClipboardNodeId(format!("node/{index}").into());
            node
        })
        .collect();
    cases.push(nodes);

    let mut connections = base.clone();
    connections.connections = (0
        ..=crate::node_system::document::subgraph::MAX_CLIPBOARD_CONNECTIONS)
        .map(|_| invalid_connection())
        .collect();
    cases.push(connections);

    let mut bindings = base.clone();
    bindings.port_bindings = (0
        ..=crate::node_system::document::subgraph::MAX_CLIPBOARD_PORT_BINDINGS)
        .map(|_| invalid_binding())
        .collect();
    cases.push(bindings);

    let mut states = base.clone();
    states.input_states = (0..=crate::node_system::document::subgraph::MAX_CLIPBOARD_INPUT_STATES)
        .map(|_| crate::node_system::document::ClipboardInputStateDto {
            address: dangling_address(),
            state: InputState {
                literal_override: None,
            },
        })
        .collect();
    cases.push(states);

    let mut parameters = base.clone();
    parameters.nodes[0].parameters.insert(
        ParameterKey::new("oversized").unwrap(),
        json!(
            "x".repeat(crate::node_system::document::subgraph::MAX_CLIPBOARD_PARAMETER_BYTES + 1)
        ),
    );
    cases.push(parameters);

    let mut depth = base.clone();
    let mut value = json!(null);
    for _ in 0..=crate::node_system::document::subgraph::MAX_CLIPBOARD_VALUE_DEPTH {
        value = json!([value]);
    }
    depth.nodes[0]
        .parameters
        .insert(ParameterKey::new("deep").unwrap(), value.clone());
    cases.push(depth);

    let mut literal_depth = base.clone();
    literal_depth.input_states[0].state.literal_override = Some(value);
    cases.push(literal_depth);

    let mut serialized = base;
    serialized.nodes[0].user_label = Some(
        "x".repeat(crate::node_system::document::subgraph::MAX_CLIPBOARD_SERIALIZED_BYTES + 1),
    );
    cases.push(serialized);

    for snapshot in cases {
        assert_clipboard_invalid(
            instantiate(&fixture, &GraphDocument::default(), snapshot).unwrap_err(),
        );
    }
}

#[test]
fn subgraph_insert_has_zero_staged_effects_on_validation_failure() {
    let fixture = export_fixture();
    let mut document = GraphDocument::default();
    document.nodes.insert(
        fixture.external,
        fixture.document.nodes[&fixture.external].clone(),
    );
    let before = document.clone();

    let mut snapshots = Vec::new();
    let mut wrong_schema = insert_snapshot(&fixture);
    wrong_schema.schema_version = 99;
    snapshots.push(wrong_schema);
    let mut duplicate = insert_snapshot(&fixture);
    duplicate.nodes[1].local_id = duplicate.nodes[0].local_id.clone();
    snapshots.push(duplicate);
    let mut dangling = insert_snapshot(&fixture);
    dangling.connections[0].input = dangling_address();
    snapshots.push(dangling);
    let mut position = insert_snapshot(&fixture);
    position.nodes[0].relative_position.y = f64::NAN;
    snapshots.push(position);

    for snapshot in snapshots {
        assert!(instantiate(&fixture, &document, snapshot).is_err());
        assert_eq!(document, before);
    }
}

#[test]
fn subgraph_insert_deserialization_rejects_collection_limit_plus_one() {
    let fixture = export_fixture();
    let snapshot = insert_snapshot(&fixture);
    let mut wire = serde_json::to_value(&snapshot).unwrap();
    let node = wire["nodes"][0].clone();
    wire["nodes"] = serde_json::Value::Array(
        (0..=crate::node_system::document::subgraph::MAX_CLIPBOARD_NODES)
            .map(|_| node.clone())
            .collect(),
    );

    let error = deserialize_clipboard_subgraph(&serde_json::to_vec(&wire).unwrap()).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("clipboard nodes exceeds entry limit")
    );
}

#[test]
fn subgraph_insert_deserialization_rejects_value_depth_before_instantiation() {
    let fixture = export_fixture();
    let mut wire = serde_json::to_value(insert_snapshot(&fixture)).unwrap();
    let mut value = json!(null);
    for _ in 0..=crate::node_system::document::subgraph::MAX_CLIPBOARD_VALUE_DEPTH {
        value = json!([value]);
    }
    wire["nodes"][0]["parameters"] = json!({ "deep": value });

    let error = deserialize_clipboard_subgraph(&serde_json::to_vec(&wire).unwrap()).unwrap_err();
    assert!(error.to_string().contains("exceeds depth limit"));
}

#[test]
fn subgraph_insert_raw_decoder_rejects_oversized_payload_before_serde() {
    let bytes =
        vec![b' '; crate::node_system::document::subgraph::MAX_CLIPBOARD_SERIALIZED_BYTES + 1];
    let error = deserialize_clipboard_subgraph(&bytes).unwrap_err();
    assert_eq!(error.code(), "clipboard_subgraph_invalid");
    assert!(error.to_string().contains("payload byte limit exceeded"));
}

fn duplicate_json_field(wire: &serde_json::Value, field: &str) -> Vec<u8> {
    let serialized = serde_json::to_string(wire).unwrap();
    let value = wire
        .pointer(field)
        .unwrap_or_else(|| panic!("missing duplicate field fixture at {field}"));
    let field_name = field.rsplit('/').next().unwrap();
    let encoded_value = serde_json::to_string(value).unwrap();
    let encoded_field = format!("\"{field_name}\":{encoded_value}");
    serialized
        .replacen(
            &encoded_field,
            &format!("{encoded_field},{encoded_field}"),
            1,
        )
        .into_bytes()
}

#[test]
fn subgraph_insert_node_creation_and_port_ref_wire_is_camel_case_and_strict() {
    let fixture = export_fixture();
    let static_wire = serde_json::to_value(insert_snapshot(&fixture)).unwrap();
    let static_creation = &static_wire["nodes"][0]["creation"];
    assert!(static_creation.get("nodeTypeId").is_some());
    assert!(static_creation.get("node_type_id").is_none());
    let instance_port = &static_wire["portBindings"][0]["address"]["port"];
    assert!(instance_port.get("localInstanceId").is_some());
    assert!(instance_port.get("local_instance_id").is_none());

    let (_, _, _, resource_bound) = function_binding_fixture();
    let resource_wire = serde_json::to_value(resource_bound).unwrap();
    let creation = &resource_wire["nodes"][0]["creation"];
    for field in ["nodeTypeId", "resourcePath", "createArgs"] {
        assert!(creation.get(field).is_some(), "missing {field}");
    }
    let port = &resource_wire["portBindings"][0]["address"]["port"];
    assert!(port.get("localInstanceId").is_some());

    for (pointer, snake_case) in [
        ("/nodes/0/creation/nodeTypeId", "node_type_id"),
        ("/nodes/0/creation/resourcePath", "resource_path"),
        ("/nodes/0/creation/createArgs", "create_args"),
        (
            "/portBindings/0/address/port/localInstanceId",
            "local_instance_id",
        ),
    ] {
        let mut invalid = resource_wire.clone();
        let (parent, camel_case) = pointer.rsplit_once('/').unwrap();
        let object = invalid
            .pointer_mut(parent)
            .unwrap()
            .as_object_mut()
            .unwrap();
        let value = object.remove(camel_case).unwrap();
        object.insert(snake_case.into(), value);
        assert!(
            deserialize_clipboard_subgraph(&serde_json::to_vec(&invalid).unwrap()).is_err(),
            "snake_case field {snake_case} must be rejected"
        );
        assert!(
            deserialize_clipboard_subgraph(&duplicate_json_field(&resource_wire, pointer)).is_err(),
            "duplicate field at {pointer} must be rejected"
        );
    }

    for pointer in ["/nodes/0/creation", "/portBindings/0/address/port"] {
        let mut invalid = resource_wire.clone();
        invalid
            .pointer_mut(pointer)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("unknown".into(), json!(true));
        assert!(
            deserialize_clipboard_subgraph(&serde_json::to_vec(&invalid).unwrap()).is_err(),
            "unknown field at {pointer} must be rejected"
        );
    }
}

#[test]
fn subgraph_insert_portable_binding_wire_is_camel_case_and_strict_at_every_level() {
    let fixture = export_fixture();
    let snapshot = insert_snapshot(&fixture);
    let wire = serde_json::to_value(&snapshot).unwrap();
    assert_eq!(wire["portBindings"][0]["binding"]["kind"], "userCreated");

    let (_, _, _, resolved) = function_binding_fixture();
    let resolved_wire = serde_json::to_value(&resolved).unwrap();
    let binding = &resolved_wire["portBindings"][0]["binding"];
    assert_eq!(binding["kind"], "resolved");
    assert_eq!(binding["origin"]["kind"], "functionParameter");
    assert!(binding.get("lastKnown").is_some());
    assert!(binding.get("last_known").is_none());

    let mut orphan = resolved.clone();
    let ClipboardDynamicPortBindingDto::Resolved {
        origin,
        order,
        last_known,
    } = orphan.port_bindings[0].binding.clone()
    else {
        unreachable!()
    };
    orphan.port_bindings[0].binding = ClipboardDynamicPortBindingDto::Orphan {
        origin,
        order,
        last_known,
    };
    assert_eq!(
        serde_json::to_value(orphan).unwrap()["portBindings"][0]["binding"]["kind"],
        "orphan"
    );

    for path in [
        "/portBindings/0/binding",
        "/portBindings/0/binding/origin",
        "/portBindings/0/binding/lastKnown",
    ] {
        let mut invalid = resolved_wire.clone();
        invalid
            .pointer_mut(path)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("unknown".into(), json!(true));
        assert!(
            deserialize_clipboard_subgraph(&serde_json::to_vec(&invalid).unwrap()).is_err(),
            "unknown field at {path} must be rejected"
        );
    }
}

#[test]
fn subgraph_insert_input_state_wire_is_camel_case_and_strict() {
    let fixture = export_fixture();
    let snapshot = insert_snapshot(&fixture);
    let entry = snapshot.input_states[0].clone();
    let wire = serde_json::to_value(&entry).unwrap();
    assert!(wire["state"].get("literalOverride").is_some());
    assert!(wire["state"].get("literal_override").is_none());
    assert_eq!(
        serde_json::from_value::<crate::node_system::document::ClipboardInputStateDto>(
            wire.clone()
        )
        .unwrap(),
        entry
    );

    for invalid_state in [
        json!({ "literal_override": null }),
        json!({ "literalOverride": null, "unknown": true }),
    ] {
        let invalid = json!({ "address": wire["address"].clone(), "state": invalid_state });
        assert!(
            serde_json::from_value::<crate::node_system::document::ClipboardInputStateDto>(invalid)
                .is_err()
        );
    }

    let address = serde_json::to_string(&wire["address"]).unwrap();
    let duplicate = format!(
        "{{\"address\":{address},\"state\":{{\"literalOverride\":null,\"literalOverride\":null}}}}"
    );
    assert!(
        serde_json::from_str::<crate::node_system::document::ClipboardInputStateDto>(&duplicate)
            .is_err()
    );
}

#[test]
fn subgraph_insert_unknown_port_rejects_malformed_typed_literal() {
    let mut fixture = export_fixture();
    fixture.registry = editor_mutation_registry();
    let mut snapshot = insert_snapshot(&fixture);
    snapshot.connections[0].order = None;
    snapshot.input_states[0].state.literal_override = Some(json!(42));

    assert_clipboard_invalid(
        instantiate(&fixture, &GraphDocument::default(), snapshot).unwrap_err(),
    );
}

#[test]
fn subgraph_insert_single_input_rejects_connection_order() {
    let mut fixture = export_fixture();
    fixture.registry = editor_mutation_registry();
    let mut snapshot = insert_snapshot(&fixture);
    snapshot.input_states.clear();

    assert_clipboard_invalid(
        instantiate(&fixture, &GraphDocument::default(), snapshot).unwrap_err(),
    );
}

fn function_binding_fixture() -> (
    GraphResourcePath,
    NodeRegistry,
    CatalogMutationValidationSnapshot,
    ClipboardSubgraphDto,
) {
    let graph_path = graph_path("events/function-binding.yssbi-event");
    let function_path = CatalogResourcePath::new("functions/callee.yssbi-function");
    let node_type = NodeTypeId::new("yssbi.project.function.call").unwrap();
    let parameter = FunctionParameter {
        id: FunctionParameterId("amount".into()),
        name: "Amount".into(),
        type_name: "Int64".into(),
    };
    let catalog = CatalogMutationValidationSnapshot {
        project_instance_id: ProjectInstanceId::new(),
        authority_generation: 0,
        resources: BTreeMap::from([(
            function_path.clone(),
            CatalogMutationResource::Function {
                revision: ResourceRevision::new(7),
                signature: FunctionSignature {
                    parameters: vec![parameter.clone()],
                    return_type: Some("Int64".into()),
                },
                allowed_node_type_id: node_type.clone(),
                parameter_binding: "target".into(),
            },
        )]),
    };
    let local_node = ClipboardNodeId("node/0".into());
    let snapshot = ClipboardSubgraphDto {
        schema_version: 1,
        nodes: vec![ClipboardNodeDto {
            local_id: local_node.clone(),
            creation: ClipboardNodeCreationDto::ResourceBound {
                node_type_id: node_type,
                resource_path: function_path,
                create_args: ResourceBoundCreateArgsDto::Function,
            },
            parameters: BTreeMap::from([(
                ParameterKey::new("target").unwrap(),
                json!("functions/callee.yssbi-function"),
            )]),
            user_label: None,
            relative_position: NodePosition { x: 0.0, y: 0.0 },
        }],
        port_bindings: vec![ClipboardPortBindingDto {
            address: ClipboardPortAddressDto {
                node_id: local_node,
                port: ClipboardPortRefDto::Instance {
                    template: PortKey::new("arguments").unwrap(),
                    local_instance_id: ClipboardPortInstanceId("port/0".into()),
                },
            },
            binding: ClipboardDynamicPortBindingDto::Resolved {
                origin: ClipboardDynamicMemberOriginDto::FunctionParameter {
                    function: GraphResourcePath("functions/callee.yssbi-function".into()),
                    parameter: parameter.id,
                },
                order: OrderKey("00000".into()),
                last_known: ClipboardLastKnownPortMetadataDto {
                    label: "Amount".into(),
                    value_type: Some(TypeExpr::Concrete(TypeId::new("core.int64").unwrap())),
                },
            },
        }],
        input_states: Vec::new(),
        connections: Vec::new(),
    };
    (
        graph_path,
        std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry),
        catalog,
        snapshot,
    )
}

#[test]
fn subgraph_insert_rejects_invalid_authoritative_function_type() {
    let (graph_path, registry, mut catalog, snapshot) = function_binding_fixture();
    let CatalogMutationResource::Function { signature, .. } =
        catalog.resources.values_mut().next().unwrap()
    else {
        unreachable!()
    };
    signature.parameters[0].type_name = "not-a-function-type".into();

    assert_clipboard_invalid(
        instantiate_subgraph(
            &graph_path,
            &GraphDocument::default(),
            &registry,
            &catalog,
            snapshot,
            NodePosition { x: 0.0, y: 0.0 },
        )
        .unwrap_err(),
    );
}

#[test]
fn subgraph_insert_rejects_forged_resolved_function_last_known_type() {
    let (graph_path, registry, catalog, mut snapshot) = function_binding_fixture();
    let ClipboardDynamicPortBindingDto::Resolved { last_known, .. } =
        &mut snapshot.port_bindings[0].binding
    else {
        unreachable!()
    };
    last_known.value_type = Some(TypeExpr::Concrete(TypeId::new("core.string").unwrap()));

    assert_clipboard_invalid(
        instantiate_subgraph(
            &graph_path,
            &GraphDocument::default(),
            &registry,
            &catalog,
            snapshot,
            NodePosition { x: 0.0, y: 0.0 },
        )
        .unwrap_err(),
    );
}

#[test]
fn subgraph_insert_rejects_connection_with_incompatible_authoritative_function_types() {
    let (graph_path, registry, mut catalog, mut snapshot) = function_binding_fixture();
    let CatalogMutationResource::Function { signature, .. } =
        catalog.resources.values_mut().next().unwrap()
    else {
        unreachable!()
    };
    signature.parameters[0].type_name = "String".into();

    let mut input_node = snapshot.nodes[0].clone();
    input_node.local_id = ClipboardNodeId("node/1".into());
    snapshot.nodes.push(input_node);
    snapshot.port_bindings[0].address.node_id = ClipboardNodeId("node/1".into());
    let ClipboardDynamicPortBindingDto::Resolved { last_known, .. } =
        &mut snapshot.port_bindings[0].binding
    else {
        unreachable!()
    };
    last_known.value_type = Some(TypeExpr::Concrete(TypeId::new("core.string").unwrap()));

    let output_address = ClipboardPortAddressDto {
        node_id: ClipboardNodeId("node/0".into()),
        port: ClipboardPortRefDto::Instance {
            template: PortKey::new("results").unwrap(),
            local_instance_id: ClipboardPortInstanceId("port/result".into()),
        },
    };
    let input_address = snapshot.port_bindings[0].address.clone();
    snapshot.port_bindings.push(ClipboardPortBindingDto {
        address: output_address.clone(),
        binding: ClipboardDynamicPortBindingDto::Resolved {
            origin: ClipboardDynamicMemberOriginDto::FunctionParameter {
                function: GraphResourcePath("functions/callee.yssbi-function".into()),
                parameter: FunctionParameterId("return".into()),
            },
            order: OrderKey("00000".into()),
            last_known: ClipboardLastKnownPortMetadataDto {
                label: "Int64".into(),
                value_type: Some(TypeExpr::Concrete(TypeId::new("core.int64").unwrap())),
            },
        },
    });
    snapshot.connections.push(ClipboardConnectionDto {
        output: output_address,
        input: input_address,
        order: None,
    });

    assert_clipboard_invalid(
        instantiate_subgraph(
            &graph_path,
            &GraphDocument::default(),
            &registry,
            &catalog,
            snapshot,
            NodePosition { x: 0.0, y: 0.0 },
        )
        .unwrap_err(),
    );
}

#[test]
fn subgraph_insert_accepts_authoritative_resolved_function_member() {
    let (graph_path, registry, catalog, snapshot) = function_binding_fixture();
    instantiate_subgraph(
        &graph_path,
        &GraphDocument::default(),
        &registry,
        &catalog,
        snapshot,
        NodePosition { x: 0.0, y: 0.0 },
    )
    .unwrap();
}

#[test]
fn subgraph_insert_rejects_resolved_function_member_for_wrong_template() {
    let (graph_path, registry, catalog, mut snapshot) = function_binding_fixture();
    let ClipboardPortRefDto::Instance { template, .. } =
        &mut snapshot.port_bindings[0].address.port
    else {
        unreachable!()
    };
    *template = PortKey::new("results").unwrap();

    assert_clipboard_invalid(
        instantiate_subgraph(
            &graph_path,
            &GraphDocument::default(),
            &registry,
            &catalog,
            snapshot,
            NodePosition { x: 0.0, y: 0.0 },
        )
        .unwrap_err(),
    );
}

#[test]
fn subgraph_insert_rejects_resolved_function_member_absent_from_signature() {
    let (graph_path, registry, catalog, mut snapshot) = function_binding_fixture();
    let ClipboardDynamicPortBindingDto::Resolved { origin, .. } =
        &mut snapshot.port_bindings[0].binding
    else {
        unreachable!()
    };
    let ClipboardDynamicMemberOriginDto::FunctionParameter { parameter, .. } = origin else {
        unreachable!()
    };
    *parameter = FunctionParameterId("missing".into());

    assert_clipboard_invalid(
        instantiate_subgraph(
            &graph_path,
            &GraphDocument::default(),
            &registry,
            &catalog,
            snapshot,
            NodePosition { x: 0.0, y: 0.0 },
        )
        .unwrap_err(),
    );
}

#[test]
fn subgraph_insert_rejects_resolved_database_field_without_field_authority() {
    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let node_type = NodeTypeId::new("yssbi.dataframe.decompose").unwrap();
    let database_path = CatalogResourcePath::new("databases/main");
    let catalog = CatalogMutationValidationSnapshot {
        project_instance_id: ProjectInstanceId::new(),
        authority_generation: 0,
        resources: BTreeMap::from([(
            database_path,
            CatalogMutationResource::Database {
                authority_revision: ResourceRevision::new(3),
                allowed_node_type_id: NodeTypeId::new("yssbi.dataframe.source.get").unwrap(),
                parameter_binding: "dataframe".into(),
            },
        )]),
    };
    let snapshot = ClipboardSubgraphDto {
        schema_version: 1,
        nodes: vec![ClipboardNodeDto {
            local_id: ClipboardNodeId("node/0".into()),
            creation: ClipboardNodeCreationDto::Static {
                node_type_id: node_type,
            },
            parameters: ParameterValues::new(),
            user_label: None,
            relative_position: NodePosition { x: 0.0, y: 0.0 },
        }],
        port_bindings: vec![ClipboardPortBindingDto {
            address: ClipboardPortAddressDto {
                node_id: ClipboardNodeId("node/0".into()),
                port: ClipboardPortRefDto::Instance {
                    template: PortKey::new("columns").unwrap(),
                    local_instance_id: ClipboardPortInstanceId("port/0".into()),
                },
            },
            binding: ClipboardDynamicPortBindingDto::Resolved {
                origin: ClipboardDynamicMemberOriginDto::SchemaField {
                    source: SchemaSourceIdentity("databases/main".into()),
                    field: SchemaFieldIdentity("customer_id".into()),
                },
                order: OrderKey("00000".into()),
                last_known: ClipboardLastKnownPortMetadataDto::default(),
            },
        }],
        input_states: Vec::new(),
        connections: Vec::new(),
    };

    let error = instantiate_subgraph(
        &graph_path("events/database-binding.yssbi-event"),
        &GraphDocument::default(),
        &registry,
        &catalog,
        snapshot,
        NodePosition { x: 0.0, y: 0.0 },
    )
    .unwrap_err();
    assert_eq!(error.code(), "referenced_resource_unavailable");
}

#[test]
fn duplicate_subgraph_offsets_every_node_and_excludes_external_edges() {
    let fixture = export_fixture();
    let patch = duplicate_subgraph(
        &fixture.graph_path,
        &fixture.document,
        &fixture.registry,
        &fixture.catalog,
        vec![fixture.second, fixture.first],
        NodePosition { x: 15.0, y: 25.0 },
    )
    .unwrap();

    let nodes = patch
        .operations
        .iter()
        .filter_map(|operation| match operation {
            GraphDocumentOperation::InsertNode { node } => Some(node),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(nodes.len(), 2);
    assert_eq!(nodes[0].position, NodePosition { x: 35.0, y: 55.0 });
    assert_eq!(nodes[1].position, NodePosition { x: 95.0, y: 115.0 });
    assert_eq!(
        patch
            .operations
            .iter()
            .filter(|operation| matches!(
                operation,
                GraphDocumentOperation::InsertConnection { .. }
            ))
            .count(),
        1
    );
    let wire = serde_json::to_string(&patch).unwrap();
    assert!(!wire.contains(&fixture.external.to_string()));
    assert!(!wire.contains(&fixture.outgoing_connection.to_string()));
    assert!(!wire.contains(&fixture.incoming_connection.to_string()));
}

#[test]
fn duplicate_subgraph_rejects_empty_and_duplicate_node_ids() {
    let fixture = export_fixture();
    for node_ids in [Vec::new(), vec![fixture.first, fixture.first]] {
        assert!(
            duplicate_subgraph(
                &fixture.graph_path,
                &fixture.document,
                &fixture.registry,
                &fixture.catalog,
                node_ids,
                NodePosition { x: 10.0, y: 10.0 },
            )
            .is_err()
        );
    }
}

#[test]
fn project_state_insert_subgraph_raw_json_preserves_complex_patch_projection_and_history() {
    let export = export_fixture();
    let snapshot = export_selected(&export, vec![export.first, export.second]).unwrap();
    let snapshot_json = serde_json::to_string(&snapshot).unwrap();
    let project = crate::project::fixtures::TempProject::activate(
        "task4-complex-raw-insert",
        crate::project::ProjectData::new(),
    );
    let state = project.state();
    state.project_store.write().unwrap().node_registry = Arc::new(subgraph_registry());
    let project_graph_path =
        crate::project::GraphResourcePath::new(export.graph_path.0.as_ref()).unwrap();
    state
        .insert_graph(
            project_graph_path.clone(),
            crate::project::GraphResourceDocument::new(
                "Complex raw insert",
                crate::project::GraphDocumentKind::Event,
            ),
        )
        .unwrap();
    let project_instance_id = state.capture_project_session().unwrap().instance_id;
    let original = state.get_data().unwrap().graphs[&project_graph_path]
        .document
        .clone();

    let result = state
        .apply_editor_graph_mutation(
            &project_instance_id,
            &project_graph_path,
            "en-US",
            MutationRequest::new(
                ResourceKey::Graph(export.graph_path.clone()),
                GraphRevision::INITIAL,
                OperationId::new(),
                EditorGraphMutationDto::InsertSubgraph {
                    snapshot_json,
                    anchor: NodePosition { x: 300.0, y: 400.0 },
                },
            ),
        )
        .unwrap();
    let committed = state.get_data().unwrap().graphs[&project_graph_path]
        .document
        .clone();
    assert_eq!(committed.nodes.len(), 2);
    assert_eq!(committed.port_bindings.len(), 2);
    assert_eq!(committed.input_states.len(), 1);
    assert_eq!(
        committed
            .input_states
            .values()
            .next()
            .unwrap()
            .literal_override,
        Some(int64_literal(42))
    );
    assert_eq!(committed.connections.len(), 1);
    assert_eq!(
        committed.connections.values().next().unwrap().order,
        Some(OrderKey("internal-order".into()))
    );
    let mut reconstructed = original.clone();
    reconstructed.apply_patch(&result.delta.payload).unwrap();
    assert_eq!(reconstructed, committed);
    assert_eq!(
        result.projection_replacement.projection,
        state
            .graph_projection(&project_graph_path, "en-US")
            .unwrap()
    );

    state
        .undo_last_transaction_observed(
            &project_instance_id,
            "en-US",
            MutationRequest::new(
                ResourceKey::Graph(export.graph_path.clone()),
                GraphRevision::new(1),
                OperationId::new(),
                HistoryMutation {},
            ),
            |_| {},
        )
        .unwrap();
    assert_graph_content_eq(
        &state.get_data().unwrap().graphs[&project_graph_path].document,
        &original,
    );

    state
        .redo_last_transaction_observed(
            &project_instance_id,
            "en-US",
            MutationRequest::new(
                ResourceKey::Graph(export.graph_path),
                GraphRevision::new(2),
                OperationId::new(),
                HistoryMutation {},
            ),
            |_| {},
        )
        .unwrap();
    assert_graph_content_eq(
        &state.get_data().unwrap().graphs[&project_graph_path].document,
        &committed,
    );
}

#[test]
fn insert_subgraph_mutation_wire_carries_only_raw_snapshot_json() {
    let fixture = export_fixture();
    let snapshot_json = serde_json::to_string(&insert_snapshot(&fixture)).unwrap();
    let mutation = EditorGraphMutationDto::InsertSubgraph {
        snapshot_json: snapshot_json.clone(),
        anchor: NodePosition { x: 1.0, y: 2.0 },
    };

    let wire = serde_json::to_value(&mutation).unwrap();
    assert_eq!(wire["type"], "insertSubgraph");
    assert_eq!(wire["payload"]["snapshotJson"], snapshot_json);
    assert!(wire["payload"].get("snapshot").is_none());
    assert_eq!(
        serde_json::from_value::<EditorGraphMutationDto>(wire).unwrap(),
        mutation
    );
    assert!(
        serde_json::from_value::<EditorGraphMutationDto>(json!({
            "type": "insertSubgraph",
            "payload": {
                "snapshot": insert_snapshot(&fixture),
                "anchor": { "x": 1.0, "y": 2.0 }
            }
        }))
        .is_err()
    );
}
