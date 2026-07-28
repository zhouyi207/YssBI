use super::*;
use crate::node_system::catalog::build_builtin_registry;
use crate::node_system::compiler::NodeImplementation;
use crate::node_system::protocol::{
    InputBindingSpec, InterfaceResolverId, LiteralPolicy, ParameterConstraint, ParameterEditorSpec,
    ParameterKey, ParameterSchema, ParameterSpec, TypeExpr, TypeId, Value,
};
use crate::node_system::registry::TypeRegistration;

const NODE_TYPE: &str = "yssbi.test.editor_validation";

fn port(
    key: &'static str,
    direction: PortDirection,
    kind: PortKind,
    instances: PortInstances,
    connections: ConnectionsPerPort,
    literal_policy: Option<LiteralPolicy>,
) -> StaticPortSpec {
    StaticPortSpec {
        key,
        label_key: Box::leak(format!("nodes.test.editor_validation.{key}").into_boxed_str()),
        direction,
        kind,
        instances,
        connections,
        input_binding: literal_policy.map(|literal_policy| InputBindingSpec {
            literal_policy,
            default_value: None,
        }),
    }
}

fn validation_registry() -> NodeRegistry {
    let derived_resolver = InterfaceResolverId::new("test.derived").unwrap();
    let ports = vec![
        port(
            "data_out",
            PortDirection::Output,
            PortKind::Data,
            PortInstances::Declared,
            ConnectionsPerPort::Multiple {
                max: None,
                ordered: false,
            },
            None,
        ),
        port(
            "data_in",
            PortDirection::Input,
            PortKind::Data,
            PortInstances::Declared,
            ConnectionsPerPort::Single,
            Some(LiteralPolicy::Allowed),
        ),
        port(
            "forbidden_in",
            PortDirection::Input,
            PortKind::Data,
            PortInstances::Declared,
            ConnectionsPerPort::Single,
            Some(LiteralPolicy::Forbidden),
        ),
        port(
            "control_in",
            PortDirection::Input,
            PortKind::Control,
            PortInstances::Declared,
            ConnectionsPerPort::Single,
            None,
        ),
        port(
            "ordered_in",
            PortDirection::Input,
            PortKind::Data,
            PortInstances::Declared,
            ConnectionsPerPort::Multiple {
                max: Some(2),
                ordered: true,
            },
            Some(LiteralPolicy::Allowed),
        ),
        port(
            "user_inputs",
            PortDirection::Input,
            PortKind::Data,
            PortInstances::UserCreated {
                min: 0,
                max: Some(2),
            },
            ConnectionsPerPort::Single,
            Some(LiteralPolicy::Allowed),
        ),
        port(
            "derived_inputs",
            PortDirection::Input,
            PortKind::Data,
            PortInstances::Derived {
                resolver: derived_resolver.clone(),
            },
            ConnectionsPerPort::Single,
            Some(LiteralPolicy::Allowed),
        ),
    ];
    let mut protocol = crate::node_system::protocol::NodeProtocol::from_static(Box::leak(
        Box::new(StaticNodeProtocol {
            type_id: NODE_TYPE,
            catalog: StaticNodeCatalogProtocol {
                title_key: "nodes.test.editor_validation.title",
                description_key: None,
                documentation_key: None,
                aliases_key: None,
                category_id: "test",
                icon_id: "test",
                style_id: "test",
                hidden: false,
            },
            ports: Box::leak(ports.into_boxed_slice()),
            execution: EDITOR_MUTATION_EXECUTION,
            scope: NodeScope::Any,
            managed_role: None,
        }),
    ))
    .unwrap();
    protocol.parameters = ParameterSchema::new(vec![
        ParameterSpec {
            key: ParameterKey::new("count").unwrap(),
            title_key: I18nKey::new("nodes.test.editor_validation.count").unwrap(),
            description_key: None,
            value_type: TypeExpr::Concrete(TypeId::new("core.int64").unwrap()),
            default_value: None,
            constraints: vec![
                ParameterConstraint::Required,
                ParameterConstraint::IntegerRange {
                    min: Some(1),
                    max: Some(3),
                },
            ],
            editor: ParameterEditorSpec::Number,
        },
        ParameterSpec {
            key: ParameterKey::new("mode").unwrap(),
            title_key: I18nKey::new("nodes.test.editor_validation.mode").unwrap(),
            description_key: None,
            value_type: TypeExpr::Concrete(TypeId::new("core.string").unwrap()),
            default_value: None,
            constraints: vec![ParameterConstraint::OneOf(vec![
                Value::String("alpha".into()),
                Value::String("beta".into()),
            ])],
            editor: ParameterEditorSpec::Select,
        },
        ParameterSpec {
            key: ParameterKey::new("label").unwrap(),
            title_key: I18nKey::new("nodes.test.editor_validation.label").unwrap(),
            description_key: None,
            value_type: TypeExpr::Concrete(TypeId::new("core.string").unwrap()),
            default_value: None,
            constraints: vec![ParameterConstraint::Length {
                min: Some(2),
                max: Some(4),
            }],
            editor: ParameterEditorSpec::Text { multiline: false },
        },
    ])
    .unwrap();

    let mut provider = ProviderRegistration::new(ProviderId::new("yssbi").unwrap());
    provider.types = vec![
        TypeRegistration {
            id: TypeId::new("core.int64").unwrap(),
            title_key: I18nKey::new("types.int64.title").unwrap(),
            classes: BTreeSet::new(),
        },
        TypeRegistration {
            id: TypeId::new("core.string").unwrap(),
            title_key: I18nKey::new("types.string.title").unwrap(),
            classes: BTreeSet::new(),
        },
    ]
    .into_boxed_slice();
    provider.categories = vec![CategoryRegistration {
        id: NodeCategoryId::new("test").unwrap(),
        title_key: I18nKey::new("categories.test.title").unwrap(),
        parent: None,
        order: 0,
    }]
    .into_boxed_slice();
    provider.interface_resolvers = vec![derived_resolver].into_boxed_slice();
    provider.i18n = I18nManifest {
        keys: [
            "categories.test.title",
            "types.int64.title",
            "types.string.title",
            "nodes.test.editor_validation.title",
            "nodes.test.editor_validation.data_out",
            "nodes.test.editor_validation.data_in",
            "nodes.test.editor_validation.forbidden_in",
            "nodes.test.editor_validation.control_in",
            "nodes.test.editor_validation.ordered_in",
            "nodes.test.editor_validation.user_inputs",
            "nodes.test.editor_validation.derived_inputs",
            "nodes.test.editor_validation.count",
            "nodes.test.editor_validation.mode",
            "nodes.test.editor_validation.label",
        ]
        .into_iter()
        .map(|key| I18nKey::new(key).unwrap())
        .collect(),
    };
    provider.nodes = vec![RegisteredNode::leaf(
        Arc::new(protocol),
        Arc::new(NodeImplementation::new(EditorMutationTestLowerer)),
    )]
    .into_boxed_slice();

    let mut builder = NodeRegistryBuilder::new();
    builder.register_provider(provider).unwrap();
    builder.freeze().unwrap()
}

fn validation_node(id: NodeId) -> DocumentNode {
    DocumentNode {
        id,
        node_type: NodeTypeId::new(NODE_TYPE).unwrap(),
        position: NodePosition { x: 0.0, y: 0.0 },
        parameters: ParameterValues::new(),
        user_label: None,
    }
}

fn validation_document(ids: &[NodeId]) -> GraphDocument {
    let mut document = GraphDocument::default();
    for &id in ids {
        document.create_node(validation_node(id)).unwrap();
    }
    document
}

fn create_with(parameters: ParameterValues) -> EditorGraphMutationDto {
    EditorGraphMutationDto::CreateNode {
        node_type_id: NodeTypeId::new(NODE_TYPE).unwrap(),
        position: NodePosition { x: 0.0, y: 0.0 },
        parameters,
        user_label: None,
    }
}

fn plan(
    mutation: EditorGraphMutationDto,
    document: &GraphDocument,
    registry: &NodeRegistry,
) -> Result<GraphDocumentPatch, MutationConflict> {
    mutation.into_patch(&graph_path("events/validation"), document, registry)
}

#[test]
fn editor_delete_rejects_managed_protocol_and_preserves_required_shell_node() {
    let managed_id = node_id(1_001);
    let registry = build_builtin_registry();
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
        EditorGraphMutationDto::DeleteNode {
            node_id: managed_id,
        },
        &document,
        &registry,
    )
    .unwrap_err();

    assert!(matches!(error, MutationConflict::InvalidEditorMutation(_)));
    assert!(error.to_string().contains("managed"));
    assert!(document.nodes.contains_key(&managed_id));
}

#[test]
fn editor_delete_keeps_normal_protocol_deletable() {
    let ordinary_id = node_id(1_002);
    let registry = validation_registry();
    let document = validation_document(&[ordinary_id]);

    let patch = plan(
        EditorGraphMutationDto::DeleteNode {
            node_id: ordinary_id,
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

fn user_address(node_id: NodeId, value: u128) -> PortAddress {
    PortAddress::instance(
        node_id,
        PortKey::new("user_inputs").unwrap(),
        instance_id(value),
    )
}

fn derived_address(node_id: NodeId, value: u128) -> PortAddress {
    PortAddress::instance(
        node_id,
        PortKey::new("derived_inputs").unwrap(),
        instance_id(value),
    )
}

#[test]
fn editor_mutation_validates_parameter_type_and_constraints() {
    let registry = validation_registry();
    let document = GraphDocument::default();
    let count = ParameterKey::new("count").unwrap();
    let mode = ParameterKey::new("mode").unwrap();
    let label = ParameterKey::new("label").unwrap();

    assert!(plan(create_with(ParameterValues::new()), &document, &registry).is_err());
    assert!(
        plan(
            create_with(BTreeMap::from([(count.clone(), json!("two"))])),
            &document,
            &registry,
        )
        .unwrap_err()
        .to_string()
        .contains("declared type")
    );
    assert!(
        plan(
            create_with(BTreeMap::from([(count.clone(), json!(4))])),
            &document,
            &registry,
        )
        .unwrap_err()
        .to_string()
        .contains("constraints")
    );
    assert!(
        plan(
            create_with(BTreeMap::from([
                (count.clone(), json!(2)),
                (mode.clone(), json!("gamma")),
            ])),
            &document,
            &registry,
        )
        .unwrap_err()
        .to_string()
        .contains("constraints")
    );
    assert!(
        plan(
            create_with(BTreeMap::from([
                (count.clone(), json!(2)),
                (label.clone(), json!("x")),
            ])),
            &document,
            &registry,
        )
        .unwrap_err()
        .to_string()
        .contains("constraints")
    );
    assert!(
        plan(
            create_with(BTreeMap::from([
                (count, json!(2)),
                (mode, json!("alpha")),
                (label, json!("good")),
            ])),
            &document,
            &registry,
        )
        .is_ok()
    );
}

#[test]
fn editor_mutation_rejects_connection_direction_and_kind_mismatches() {
    let registry = validation_registry();
    let first = node_id(1_001);
    let second = node_id(1_002);
    let document = validation_document(&[first, second]);

    assert!(
        plan(
            EditorGraphMutationDto::Connect {
                output: declared(first, "data_in").into(),
                input: declared(second, "data_in").into(),
                order: None,
            },
            &document,
            &registry,
        )
        .unwrap_err()
        .to_string()
        .contains("directions")
    );
    assert!(
        plan(
            EditorGraphMutationDto::Connect {
                output: declared(first, "data_out").into(),
                input: declared(second, "control_in").into(),
                order: None,
            },
            &document,
            &registry,
        )
        .unwrap_err()
        .to_string()
        .contains("kinds")
    );
}

#[test]
fn editor_mutation_enforces_connection_capacity_and_order_policy() {
    let registry = validation_registry();
    let first = node_id(1_011);
    let second = node_id(1_012);
    let third = node_id(1_013);
    let mut document = validation_document(&[first, second, third]);
    document
        .connect(
            declared(first, "data_out"),
            declared(second, "data_in"),
            None,
        )
        .unwrap();

    assert!(
        plan(
            EditorGraphMutationDto::Connect {
                output: declared(third, "data_out").into(),
                input: declared(second, "data_in").into(),
                order: None,
            },
            &document,
            &registry,
        )
        .unwrap_err()
        .to_string()
        .contains("connection limit")
    );
    assert!(
        plan(
            EditorGraphMutationDto::Connect {
                output: declared(first, "data_out").into(),
                input: declared(second, "ordered_in").into(),
                order: None,
            },
            &document,
            &registry,
        )
        .unwrap_err()
        .to_string()
        .contains("require an order")
    );
    assert!(
        plan(
            EditorGraphMutationDto::Connect {
                output: declared(first, "data_out").into(),
                input: declared(third, "data_in").into(),
                order: Some(OrderKey("a".into())),
            },
            &document,
            &registry,
        )
        .unwrap_err()
        .to_string()
        .contains("cannot carry")
    );
    assert!(
        plan(
            EditorGraphMutationDto::Connect {
                output: declared(first, "data_out").into(),
                input: declared(second, "ordered_in").into(),
                order: Some(OrderKey("a".into())),
            },
            &document,
            &registry,
        )
        .is_ok()
    );
}

#[test]
fn editor_mutation_rejects_orphan_and_binding_policy_mismatches() {
    let registry = validation_registry();
    let first = node_id(1_021);
    let second = node_id(1_022);
    let mut document = validation_document(&[first, second]);
    let orphan = derived_address(second, 1_023);
    document.port_bindings.insert(
        orphan.clone(),
        DynamicPortBinding::Orphan {
            origin: DynamicMemberLocator::SchemaField {
                source: SchemaSourceIdentity("source".into()),
                field: SchemaFieldIdentity("field".into()),
            },
            order: OrderKey("a".into()),
            last_known: LastKnownPortMetadata {
                label: "Field".to_owned(),
            },
        },
    );
    assert!(
        plan(
            EditorGraphMutationDto::Connect {
                output: declared(first, "data_out").into(),
                input: orphan.into(),
                order: None,
            },
            &document,
            &registry,
        )
        .unwrap_err()
        .to_string()
        .contains("orphan")
    );

    let mismatched = user_address(second, 1_024);
    document.port_bindings.insert(mismatched.clone(), binding());
    assert!(
        plan(
            EditorGraphMutationDto::Connect {
                output: declared(first, "data_out").into(),
                input: mismatched.into(),
                order: None,
            },
            &document,
            &registry,
        )
        .unwrap_err()
        .to_string()
        .contains("binding kind")
    );
}

#[test]
fn editor_mutation_enforces_literal_policy() {
    let registry = validation_registry();
    let owner = node_id(1_031);
    let document = validation_document(&[owner]);

    assert!(
        plan(
            EditorGraphMutationDto::SetLiteral {
                address: declared(owner, "data_in").into(),
                literal: Some(json!(1)),
            },
            &document,
            &registry,
        )
        .is_ok()
    );
    assert!(
        plan(
            EditorGraphMutationDto::SetLiteral {
                address: declared(owner, "forbidden_in").into(),
                literal: Some(json!(1)),
            },
            &document,
            &registry,
        )
        .unwrap_err()
        .to_string()
        .contains("forbids")
    );
    assert!(
        plan(
            EditorGraphMutationDto::SetLiteral {
                address: declared(owner, "data_out").into(),
                literal: Some(json!(1)),
            },
            &document,
            &registry,
        )
        .unwrap_err()
        .to_string()
        .contains("data input")
    );
}

#[test]
fn editor_mutation_rejects_non_user_created_port_templates() {
    let registry = validation_registry();
    let owner = node_id(1_041);
    let document = validation_document(&[owner]);

    for template in ["data_in", "derived_inputs", "not_owned"] {
        assert!(
            plan(
                EditorGraphMutationDto::AddPortInstance {
                    node_id: owner,
                    template: PortKey::new(template).unwrap(),
                    order: None,
                },
                &document,
                &registry,
            )
            .is_err()
        );
    }
    assert!(
        plan(
            EditorGraphMutationDto::RemovePortInstance {
                address: declared(owner, "data_in").into(),
            },
            &document,
            &registry,
        )
        .is_err()
    );
    let derived = derived_address(owner, 1_042);
    let mut with_derived = document.clone();
    with_derived
        .port_bindings
        .insert(derived.clone(), binding());
    assert!(
        plan(
            EditorGraphMutationDto::RemovePortInstance {
                address: derived.into(),
            },
            &with_derived,
            &registry,
        )
        .is_err()
    );
}

#[test]
fn editor_mutation_remove_instance_cleanup_is_reversible() {
    let registry = validation_registry();
    let source = node_id(1_051);
    let owner = node_id(1_052);
    let address = user_address(owner, 1_053);
    let mut document = validation_document(&[source, owner]);
    document
        .bind_port(
            address.clone(),
            DynamicPortBinding::UserCreated {
                order: OrderKey("a".into()),
            },
        )
        .unwrap();
    document
        .connect(declared(source, "data_out"), address.clone(), None)
        .unwrap();
    document
        .set_literal(address.clone(), Some(json!(42)))
        .unwrap();
    let before = document.clone();

    let patch = plan(
        EditorGraphMutationDto::RemovePortInstance {
            address: address.clone().into(),
        },
        &document,
        &registry,
    )
    .unwrap();
    document.apply_patch(&patch).unwrap();
    assert!(!document.port_bindings.contains_key(&address));
    assert!(!document.input_states.contains_key(&address));
    assert!(document.connections.is_empty());

    document.apply_patch(&patch.inverse()).unwrap();
    assert_graph_content_eq(&document, &before);
}
