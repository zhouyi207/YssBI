use super::*;
use crate::node_system::catalog::build_builtin_node_system;
use crate::node_system::compiler::NodeImplementation;
use crate::node_system::document::mutation::validate_parameters_with_registry;
use crate::node_system::protocol::{
    InputBindingSpec, InterfaceResolverId, LiteralPolicy, ParameterConstraint, ParameterEditorSpec,
    ParameterKey, ParameterSchema, ParameterSpec, TypeExpr, TypeId, Value,
};
use crate::node_system::registry::TypeRegistration;

const NODE_TYPE: &str = "yssbi.test.editor_validation";

fn port(
    key: &str,
    direction: PortDirection,
    kind: PortKind,
    instances: PortInstances,
    connections: ConnectionsPerPort,
    literal_policy: Option<LiteralPolicy>,
) -> PortSpec {
    PortSpec {
        key: PortKey::new(key).unwrap(),
        label_key: I18nKey::new(format!("nodes.test.editor_validation.{key}")).unwrap(),
        direction,
        kind,
        value_type: TypeExpr::Unknown,
        instances,
        connections,
        input_binding: literal_policy.map(|literal_policy| InputBindingSpec {
            literal_policy,
            default_value: None,
        }),
        consumption: None,
        production: None,
        editor: PortEditorSpec::Default,
        schema: None,
    }
}

fn validation_registry() -> NodeRegistry {
    let derived_resolver = InterfaceResolverId::new("test.derived").unwrap();
    let mut ports = vec![
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
    ports
        .iter_mut()
        .find(|port| port.key.as_str() == "data_in")
        .unwrap()
        .value_type = TypeExpr::Concrete(TypeId::new("core.int64").unwrap());
    let parameters = vec![
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
    ];
    let protocol = TestProtocolBuilder::new(NODE_TYPE, "test")
        .style("test")
        .ports(ports)
        .parameters(parameters)
        .execution(EDITOR_MUTATION_EXECUTION)
        .build();

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

#[test]
fn editor_parameter_validation_applies_registered_nominal_codec() {
    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let mut protocol = registry
        .protocol(&NodeTypeId::new("yssbi.dataframe.rename").unwrap())
        .unwrap()
        .clone();
    let columns = ParameterKey::new("columns").unwrap();
    protocol.parameters = ParameterSchema::new(vec![ParameterSpec {
        key: columns.clone(),
        title_key: I18nKey::new("parameters.columns.title").unwrap(),
        description_key: None,
        value_type: TypeExpr::Concrete(
            TypeId::new(crate::node_system::protocol::dataframe::PROJECT_COLUMNS_TYPE_ID).unwrap(),
        ),
        default_value: None,
        constraints: vec![ParameterConstraint::Required],
        editor: ParameterEditorSpec::Auto,
    }])
    .unwrap();

    assert!(
        validate_parameters_with_registry(
            &registry,
            &protocol,
            &ParameterValues::from([(columns.clone(), serde_json::json!(["b", "a"]))]),
        )
        .is_ok()
    );
    assert!(
        validate_parameters_with_registry(
            &registry,
            &protocol,
            &ParameterValues::from([(columns, serde_json::json!([]))]),
        )
        .is_err()
    );
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

fn plan(
    mutation: EditorGraphMutationDto,
    document: &GraphDocument,
    registry: &NodeRegistry,
) -> Result<GraphDocumentPatch, MutationConflict> {
    mutation.into_patch(&graph_path("events/validation"), document, registry)
}

fn resource_descriptor_snapshot(
    variable_id: crate::variable::VariableId,
) -> crate::project::CatalogMutationValidationSnapshot {
    use crate::node_system::catalog::CatalogResourcePath;
    use crate::project::CatalogMutationResource;
    use crate::variable::VariableScope;

    crate::project::CatalogMutationValidationSnapshot {
        project_instance_id: crate::project::ProjectInstanceId::from_existing("project-1".into()),
        authority_generation: 7,
        resources: BTreeMap::from([
            (
                CatalogResourcePath::new("functions/Helper.yssbi-function"),
                CatalogMutationResource::Function {
                    revision: ResourceRevision::new(3),
                    signature: FunctionSignature::default(),
                    allowed_node_type_id: NodeTypeId::new("yssbi.project.function.call").unwrap(),
                    parameter_binding: "target".into(),
                },
            ),
            (
                CatalogResourcePath::new(format!("variables/{variable_id}")),
                CatalogMutationResource::Variable {
                    revision: ResourceRevision::new(4),
                    scope: VariableScope::Event {
                        event_path: "events/validation".into(),
                    },
                    allowed_node_type_ids: [
                        NodeTypeId::new("yssbi.project.variable.get").unwrap(),
                        NodeTypeId::new("yssbi.project.variable.set").unwrap(),
                    ],
                    parameter_binding: "variable".into(),
                },
            ),
            (
                CatalogResourcePath::new("databases/sales"),
                CatalogMutationResource::Database {
                    authority_revision: ResourceRevision::new(5),
                    allowed_node_type_id: NodeTypeId::new("yssbi.dataframe.source.get").unwrap(),
                    parameter_binding: "dataframe".into(),
                },
            ),
        ]),
    }
}

fn resource_create(
    node_type_id: &str,
    resource_path: &str,
    resource_revision: u64,
    create_args: crate::node_system::catalog::ResourceBoundCreateArgsDto,
) -> EditorGraphMutationDto {
    EditorGraphMutationDto::CreateNode {
        descriptor: crate::node_system::catalog::NodeCreationDescriptor::ResourceBound {
            node_type_id: NodeTypeId::new(node_type_id).unwrap(),
            resource_path: crate::node_system::catalog::CatalogResourcePath::new(resource_path),
            resource_revision: ResourceRevision::new(resource_revision),
            create_args,
        },
        position: NodePosition { x: 1.0, y: 2.0 },
        user_label: None,
    }
}

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
    let protocol = registry
        .protocol(&NodeTypeId::new(NODE_TYPE).unwrap())
        .unwrap();
    let validate = |parameters: ParameterValues| validate_parameters(protocol, &parameters);
    let count = ParameterKey::new("count").unwrap();
    let mode = ParameterKey::new("mode").unwrap();
    let label = ParameterKey::new("label").unwrap();

    assert!(validate(ParameterValues::new()).is_err());
    assert!(
        validate(BTreeMap::from([(count.clone(), json!("two"))]))
            .unwrap_err()
            .to_string()
            .contains("declared type")
    );
    assert!(
        validate(BTreeMap::from([(count.clone(), json!(4))]))
            .unwrap_err()
            .to_string()
            .contains("constraints")
    );
    assert!(
        validate(BTreeMap::from([
            (count.clone(), json!(2)),
            (mode.clone(), json!("gamma")),
        ]))
        .unwrap_err()
        .to_string()
        .contains("constraints")
    );
    assert!(
        validate(BTreeMap::from([
            (count.clone(), json!(2)),
            (label.clone(), json!("x")),
        ]))
        .unwrap_err()
        .to_string()
        .contains("constraints")
    );
    assert!(
        validate(BTreeMap::from([
            (count, json!(2)),
            (mode, json!("alpha")),
            (label, json!("good")),
        ]))
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
    let literal = serde_json::to_value(crate::node_system::protocol::TypedValue {
        value_type: TypeExpr::Concrete(TypeId::new("core.int64").unwrap()),
        value: Value::Integer(1),
    })
    .unwrap();

    assert!(
        plan(
            EditorGraphMutationDto::SetLiteral {
                address: declared(owner, "data_in").into(),
                literal: Some(literal.clone()),
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
                literal: Some(literal.clone()),
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
                literal: Some(literal),
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
fn editor_mutation_rejects_legal_typed_literal_for_the_wrong_port_type() {
    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let owner = node_id(1_035);
    let mut document = GraphDocument::default();
    document
        .create_node(DocumentNode {
            id: owner,
            node_type: NodeTypeId::new("yssbi.control.branch").unwrap(),
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters: BTreeMap::new(),
            user_label: None,
        })
        .unwrap();
    let literal = serde_json::to_value(crate::node_system::protocol::TypedValue {
        value_type: TypeExpr::Concrete(TypeId::new("core.string").unwrap()),
        value: Value::String("not-a-bool".into()),
    })
    .unwrap();

    let error = plan(
        EditorGraphMutationDto::SetLiteral {
            address: declared(owner, "condition").into(),
            literal: Some(literal),
        },
        &document,
        &registry,
    )
    .unwrap_err();

    assert!(error.to_string().contains("literal does not match"));
}

#[test]
fn editor_mutation_rejects_nested_literal_element_mismatch() {
    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let owner = node_id(1_036);
    let mut document = GraphDocument::default();
    document
        .create_node(DocumentNode {
            id: owner,
            node_type: NodeTypeId::new("yssbi.data_series.convert.int64_to_string").unwrap(),
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters: BTreeMap::new(),
            user_label: None,
        })
        .unwrap();
    let series = TypeExpr::Applied {
        constructor: crate::node_system::protocol::TypeConstructorId::new("core.data_series")
            .unwrap(),
        arguments: vec![TypeExpr::Concrete(TypeId::new("core.int64").unwrap())],
    };
    let literal = serde_json::to_value(crate::node_system::protocol::TypedValue {
        value_type: series,
        value: Value::List(vec![Value::Integer(1), Value::String("wrong".into())]),
    })
    .unwrap();

    let error = plan(
        EditorGraphMutationDto::SetLiteral {
            address: declared(owner, "input").into(),
            literal: Some(literal),
        },
        &document,
        &registry,
    )
    .unwrap_err();

    assert!(error.to_string().contains("literal does not match"));
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
