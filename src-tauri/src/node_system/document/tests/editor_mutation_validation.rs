use super::*;
use crate::node_system::analysis::{
    CompilationOutcomeDto, EditorGraphProjectionDto, EditorNodeProjectionDto, NodeCapabilitiesDto,
    NodeDisplayDto, NodePositionDto, PortConnectionCapabilityDto, PortDirectionDto, PortDisplayDto,
    PortInstanceKindDto, PortKindDto, ProjectionBasis, ResolvedPortDto, ResolvedPortStatusDto,
    TypeSummaryDto,
};
use crate::node_system::catalog::build_builtin_node_system;
use crate::node_system::compatibility::{
    EditorMutationPortType, EditorMutationPortValidation, EditorMutationValidationSnapshot,
};
use crate::node_system::compiler::NodeImplementation;
use crate::node_system::document::mutation::{
    connect_operations_with_id_allocator, move_connection_operations,
    move_connection_operations_with_id_allocator, validate_parameters_with_registry,
};
use crate::node_system::protocol::{
    InputBindingSpec, InterfaceResolverId, LiteralPolicy, NodeInterfaceProtocol, NodeProtocol,
    ParameterConstraint, ParameterEditorSpec, ParameterKey, ParameterSchema, ParameterSpec,
    TypeConstructorId, TypeExpr, TypeId, TypeParameterId, Value,
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

fn validation_snapshot_for_document(
    document: &GraphDocument,
    registry: &NodeRegistry,
) -> EditorMutationValidationSnapshot {
    let mut ports = BTreeMap::new();
    for node in document.nodes.values() {
        let protocol = registry.protocol(&node.node_type).unwrap();
        for spec in protocol.interface.ports.iter() {
            let addresses = match spec.instances {
                PortInstances::Declared => vec![PortAddress::declared(node.id, spec.key.clone())],
                _ => document
                    .port_bindings
                    .keys()
                    .filter(|address| {
                        address.node_id == node.id
                            && matches!(
                                &address.port,
                                PortRef::Instance { template, .. } if template == &spec.key
                            )
                    })
                    .cloned()
                    .collect(),
            };
            for address in addresses {
                let orphan = matches!(
                    document.port_bindings.get(&address),
                    Some(DynamicPortBinding::Orphan { .. })
                );
                let port_type = if spec.kind == PortKind::Data {
                    EditorMutationPortType::Ready {
                        expression: TypeExpr::Concrete(TypeId::new("core.int64").unwrap()),
                        type_parameters: protocol.interface.type_parameters.clone(),
                    }
                } else {
                    EditorMutationPortType::NotApplicable
                };
                ports.insert(
                    address,
                    EditorMutationPortValidation {
                        direction: spec.direction,
                        kind: spec.kind,
                        orphan,
                        port_type,
                    },
                );
            }
        }
    }
    EditorMutationValidationSnapshot {
        graph_revision: document.revision,
        ports,
    }
}

fn plan(
    mutation: EditorGraphMutationDto,
    document: &GraphDocument,
    registry: &NodeRegistry,
) -> Result<GraphDocumentPatch, MutationConflict> {
    mutation.into_patch(&graph_path("events/validation"), document, registry)
}

fn plan_with_validation(
    mutation: EditorGraphMutationDto,
    document: &GraphDocument,
    registry: &NodeRegistry,
    validation: &EditorMutationValidationSnapshot,
) -> Result<GraphDocumentPatch, MutationConflict> {
    mutation.into_patch_with_editor_validation(
        &graph_path("events/validation"),
        document,
        registry,
        None,
        None,
        Some(validation),
    )
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
                    data_type: crate::graph::value::DataType::Int64,
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
        connect_from: None,
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

fn assert_editor_error_code(error: MutationConflict, expected: EditorMutationErrorCode) {
    match error {
        MutationConflict::Editor(error) => assert_eq!(error.code, expected),
        other => panic!("expected editor mutation error {expected:?}, got {other:?}"),
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

fn snapshot_protocol(
    type_id: &str,
    key: &str,
    direction: PortDirection,
    kind: PortKind,
    value_type: TypeExpr,
    type_parameters: Vec<TypeParameterId>,
    connections: ConnectionsPerPort,
) -> NodeProtocol {
    let mut protocol = TestProtocolBuilder::new(type_id, "test")
        .ports(vec![port(
            key,
            direction,
            kind,
            PortInstances::Declared,
            connections,
            None,
        )])
        .execution(EDITOR_MUTATION_EXECUTION)
        .build();
    if kind == PortKind::Data {
        protocol.interface.ports[0].value_type = value_type;
    }
    protocol.interface = NodeInterfaceProtocol::new(
        protocol.interface.ports.into_vec(),
        type_parameters,
        Vec::new(),
    )
    .unwrap();
    protocol
}

fn snapshot_registry(
    source_type: TypeExpr,
    source_parameters: Vec<TypeParameterId>,
    source_kind: PortKind,
    source_connections: ConnectionsPerPort,
    target_type: TypeExpr,
    target_parameters: Vec<TypeParameterId>,
    target_kind: PortKind,
    target_connections: ConnectionsPerPort,
) -> NodeRegistry {
    let protocols = [
        snapshot_protocol(
            "yssbi.test.snapshot_source",
            "out",
            PortDirection::Output,
            source_kind,
            source_type,
            source_parameters,
            source_connections,
        ),
        snapshot_protocol(
            "yssbi.test.snapshot_target",
            "in",
            PortDirection::Input,
            target_kind,
            target_type,
            target_parameters,
            target_connections,
        ),
    ];
    let mut provider = ProviderRegistration::new(ProviderId::new("yssbi").unwrap());
    provider.types = ["core.float64", "core.string"]
        .into_iter()
        .map(|id| TypeRegistration {
            id: TypeId::new(id).unwrap(),
            title_key: I18nKey::new(format!("types.{id}.title")).unwrap(),
            classes: BTreeSet::new(),
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();
    provider.categories = vec![CategoryRegistration {
        id: NodeCategoryId::new("test").unwrap(),
        title_key: I18nKey::new("categories.test.title").unwrap(),
        parent: None,
        order: 0,
    }]
    .into_boxed_slice();
    provider.i18n = I18nManifest {
        keys: [
            "categories.test.title",
            "types.core.float64.title",
            "types.core.string.title",
            "nodes.test.snapshot_source.title",
            "nodes.test.snapshot_target.title",
            "nodes.test.editor_validation.out",
            "nodes.test.editor_validation.in",
        ]
        .into_iter()
        .map(|key| I18nKey::new(key).unwrap())
        .collect(),
    };
    provider.nodes = protocols
        .into_iter()
        .map(|protocol| {
            RegisteredNode::leaf(
                Arc::new(protocol),
                Arc::new(NodeImplementation::new(EditorMutationTestLowerer)),
            )
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let mut builder = NodeRegistryBuilder::new();
    builder.register_provider(provider).unwrap();
    builder.freeze().unwrap()
}

fn snapshot_type(expression: Option<TypeExpr>, resolved: bool) -> Option<TypeSummaryDto> {
    expression.map(|expression| TypeSummaryDto {
        display: "test".into(),
        resolved,
        data_type: None,
        internal_type_expr: Some(expression),
    })
}

fn snapshot_port(
    address: &PortAddress,
    direction: PortDirectionDto,
    kind: PortKindDto,
    orphan: bool,
    resolved_type: Option<TypeSummaryDto>,
) -> ResolvedPortDto {
    ResolvedPortDto {
        address: address.into(),
        template_key: match &address.port {
            PortRef::Declared { key } | PortRef::Instance { template: key, .. } => {
                key.as_str().into()
            }
        },
        display: PortDisplayDto {
            label: "test".into(),
            instance_label: None,
        },
        direction,
        kind,
        instance_kind: PortInstanceKindDto::Declared,
        orphan,
        can_remove: false,
        connections: PortConnectionCapabilityDto {
            current: 0,
            maximum: Some(1),
            ordered: false,
            can_append: true,
            can_replace: false,
            can_move: false,
        },
        input: None,
        resolved_type,
        resolved_schema: None,
        status: if orphan {
            ResolvedPortStatusDto::Orphan
        } else {
            ResolvedPortStatusDto::Resolved
        },
    }
}

fn snapshot_projection(
    registry: &NodeRegistry,
    revision: u64,
    source_id: NodeId,
    target_id: NodeId,
    source_port: ResolvedPortDto,
    target_port: ResolvedPortDto,
) -> EditorGraphProjectionDto {
    let node =
        |id: NodeId, node_type_id: &str, ports: Vec<ResolvedPortDto>| EditorNodeProjectionDto {
            graph_path: "events/validation".into(),
            source_revision: revision,
            node_id: id.to_string().into(),
            node_type_id: node_type_id.into(),
            position: NodePositionDto { x: 0.0, y: 0.0 },
            display: NodeDisplayDto {
                title: "test".into(),
                description: None,
                user_label: None,
                icon_id: None,
                style_id: None,
            },
            ports,
            parameter_editors: Vec::new(),
            capabilities: NodeCapabilitiesDto {
                managed: false,
                can_copy: true,
                can_delete: true,
                can_edit_label: true,
                can_edit_parameters: false,
                has_dynamic_ports: false,
                supports_inline_literals: false,
            },
            diagnostics: Vec::new(),
        };
    EditorGraphProjectionDto {
        basis: ProjectionBasis {
            graph_path: "events/validation".into(),
            graph_revision: revision,
            registry_fingerprint: registry.fingerprint().clone(),
            resource_versions: BTreeMap::new(),
        },
        graph_path: "events/validation".into(),
        source_revision: revision,
        nodes: vec![
            node(source_id, "yssbi.test.snapshot_source", vec![source_port]),
            node(target_id, "yssbi.test.snapshot_target", vec![target_port]),
        ],
        connections: Vec::new(),
        diagnostics: Vec::new(),
        outcome: CompilationOutcomeDto::Success,
        has_blocking_diagnostics: false,
    }
}

fn type_snapshot(
    source_type: Option<TypeExpr>,
    source_resolved: bool,
    source_parameters: Vec<TypeParameterId>,
    source_kind: PortKind,
    target_type: Option<TypeExpr>,
    target_resolved: bool,
    target_parameters: Vec<TypeParameterId>,
    target_kind: PortKind,
) -> (
    NodeRegistry,
    EditorMutationValidationSnapshot,
    PortAddress,
    PortAddress,
) {
    type_snapshot_with_summaries(
        snapshot_type(source_type, source_resolved),
        source_parameters,
        source_kind,
        snapshot_type(target_type, target_resolved),
        target_parameters,
        target_kind,
    )
}

fn type_snapshot_with_summaries(
    source_type: Option<TypeSummaryDto>,
    source_parameters: Vec<TypeParameterId>,
    source_kind: PortKind,
    target_type: Option<TypeSummaryDto>,
    target_parameters: Vec<TypeParameterId>,
    target_kind: PortKind,
) -> (
    NodeRegistry,
    EditorMutationValidationSnapshot,
    PortAddress,
    PortAddress,
) {
    let source_id = node_id(3_001);
    let target_id = node_id(3_002);
    let output = declared(source_id, "out");
    let input = declared(target_id, "in");
    let protocol_type = TypeExpr::Concrete(TypeId::new("core.float64").unwrap());
    let registry = snapshot_registry(
        protocol_type.clone(),
        source_parameters,
        source_kind,
        ConnectionsPerPort::Single,
        protocol_type,
        target_parameters,
        target_kind,
        ConnectionsPerPort::Single,
    );
    let projection = snapshot_projection(
        &registry,
        7,
        source_id,
        target_id,
        snapshot_port(
            &output,
            PortDirectionDto::Output,
            match source_kind {
                PortKind::Data => PortKindDto::Data,
                PortKind::Control => PortKindDto::Control,
                PortKind::Effect => PortKindDto::Effect,
            },
            false,
            source_type,
        ),
        snapshot_port(
            &input,
            PortDirectionDto::Input,
            match target_kind {
                PortKind::Data => PortKindDto::Data,
                PortKind::Control => PortKindDto::Control,
                PortKind::Effect => PortKindDto::Effect,
            },
            false,
            target_type,
        ),
    );
    let snapshot =
        EditorMutationValidationSnapshot::from_projection(&projection, &registry).unwrap();
    (registry, snapshot, output, input)
}

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

struct ConnectFixture {
    registry: NodeRegistry,
    snapshot: EditorMutationValidationSnapshot,
    document: GraphDocument,
    output: PortAddress,
    input: PortAddress,
}

fn connect_fixture(
    source_kind: PortKind,
    source_connections: ConnectionsPerPort,
    source_type: Option<TypeSummaryDto>,
    source_parameters: Vec<TypeParameterId>,
    target_kind: PortKind,
    target_connections: ConnectionsPerPort,
    target_type: Option<TypeSummaryDto>,
    target_parameters: Vec<TypeParameterId>,
) -> ConnectFixture {
    let source_id = node_id(3_101);
    let target_id = node_id(3_102);
    let output = declared(source_id, "out");
    let input = declared(target_id, "in");
    let protocol_type = TypeExpr::Concrete(TypeId::new("core.float64").unwrap());
    let registry = snapshot_registry(
        protocol_type.clone(),
        source_parameters,
        source_kind,
        source_connections,
        protocol_type,
        target_parameters,
        target_kind,
        target_connections,
    );
    let kind_dto = |kind| match kind {
        PortKind::Data => PortKindDto::Data,
        PortKind::Control => PortKindDto::Control,
        PortKind::Effect => PortKindDto::Effect,
    };
    let projection = snapshot_projection(
        &registry,
        7,
        source_id,
        target_id,
        snapshot_port(
            &output,
            PortDirectionDto::Output,
            kind_dto(source_kind),
            false,
            source_type,
        ),
        snapshot_port(
            &input,
            PortDirectionDto::Input,
            kind_dto(target_kind),
            false,
            target_type,
        ),
    );
    let snapshot =
        EditorMutationValidationSnapshot::from_projection(&projection, &registry).unwrap();
    let mut document = GraphDocument::default();
    for (id, node_type) in [
        (source_id, "yssbi.test.snapshot_source"),
        (target_id, "yssbi.test.snapshot_target"),
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
    document.revision = GraphRevision::new(7);
    ConnectFixture {
        registry,
        snapshot,
        document,
        output,
        input,
    }
}

fn ready_summary(expression: TypeExpr) -> Option<TypeSummaryDto> {
    snapshot_type(Some(expression), true)
}

fn assert_connect_failure_unchanged(
    fixture: &ConnectFixture,
    snapshot: &EditorMutationValidationSnapshot,
    output: PortAddress,
    input: PortAddress,
    order: Option<OrderKey>,
    expected: EditorMutationErrorCode,
) {
    let before = fixture.document.clone();
    let before_bytes = serde_json::to_vec(&fixture.document).unwrap();
    let allocations = std::cell::Cell::new(0);
    let error = connect_operations_with_id_allocator(
        &fixture.document,
        &fixture.registry,
        snapshot,
        output,
        input,
        order,
        || {
            allocations.set(allocations.get() + 1);
            connection_id(3_199)
        },
    )
    .unwrap_err();
    assert_editor_error_code(error, expected);
    assert_eq!(allocations.get(), 0);
    assert_eq!(fixture.document, before);
    assert_eq!(serde_json::to_vec(&fixture.document).unwrap(), before_bytes);
    assert_eq!(fixture.document.revision, before.revision);
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
        Some(OrderKey("forbidden".into())),
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

fn assert_move_failure_unchanged(
    document: &GraphDocument,
    registry: &NodeRegistry,
    snapshot: &EditorMutationValidationSnapshot,
    source: PortAddress,
    target: PortAddress,
    expected: EditorMutationErrorCode,
) {
    let before = document.clone();
    let before_bytes = serde_json::to_vec(document).unwrap();
    let error =
        move_connection_operations(document, registry, snapshot, source, target).unwrap_err();

    assert_editor_error_code(error, expected);
    assert_eq!(document, &before);
    assert_eq!(serde_json::to_vec(document).unwrap(), before_bytes);
    assert_eq!(document.revision, before.revision);
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
            order: Some(OrderKey("b".into())),
        },
        DocumentConnection {
            id: connection_id(4_011),
            output: source.clone(),
            input: declared(first_input_node, "ordered_in"),
            order: Some(OrderKey("a".into())),
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
            order: Some(OrderKey("a".into())),
        },
        DocumentConnection {
            id: connection_id(4_072),
            output: declared(second_output, "data_out"),
            input: source.clone(),
            order: Some(OrderKey("b".into())),
        },
        DocumentConnection {
            id: connection_id(4_073),
            output: declared(incumbent_output, "data_out"),
            input: target.clone(),
            order: Some(OrderKey("c".into())),
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
    let validation = validation_snapshot_for_document(&document, &registry);

    assert!(
        plan_with_validation(
            EditorGraphMutationDto::Connect {
                output: declared(first, "data_in").into(),
                input: declared(second, "data_in").into(),
                order: None,
            },
            &document,
            &registry,
            &validation,
        )
        .unwrap_err()
        .to_string()
        .contains("directions")
    );
    assert!(
        plan_with_validation(
            EditorGraphMutationDto::Connect {
                output: declared(first, "data_out").into(),
                input: declared(second, "control_in").into(),
                order: None,
            },
            &document,
            &registry,
            &validation,
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
    let validation = validation_snapshot_for_document(&document, &registry);

    let replacement = plan_with_validation(
        EditorGraphMutationDto::Connect {
            output: declared(third, "data_out").into(),
            input: declared(second, "data_in").into(),
            order: None,
        },
        &document,
        &registry,
        &validation,
    )
    .unwrap();
    assert!(matches!(
        replacement.operations.as_slice(),
        [
            GraphDocumentOperation::RemoveConnection { .. },
            GraphDocumentOperation::InsertConnection { .. }
        ]
    ));
    assert!(
        plan_with_validation(
            EditorGraphMutationDto::Connect {
                output: declared(first, "data_out").into(),
                input: declared(second, "ordered_in").into(),
                order: None,
            },
            &document,
            &registry,
            &validation,
        )
        .unwrap_err()
        .to_string()
        .contains("require an order")
    );
    assert!(
        plan_with_validation(
            EditorGraphMutationDto::Connect {
                output: declared(first, "data_out").into(),
                input: declared(third, "data_in").into(),
                order: Some(OrderKey("a".into())),
            },
            &document,
            &registry,
            &validation,
        )
        .unwrap_err()
        .to_string()
        .contains("cannot carry")
    );
    assert!(
        plan_with_validation(
            EditorGraphMutationDto::Connect {
                output: declared(first, "data_out").into(),
                input: declared(second, "ordered_in").into(),
                order: Some(OrderKey("a".into())),
            },
            &document,
            &registry,
            &validation,
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
    let validation = validation_snapshot_for_document(&document, &registry);
    assert!(
        plan_with_validation(
            EditorGraphMutationDto::Connect {
                output: declared(first, "data_out").into(),
                input: orphan.into(),
                order: None,
            },
            &document,
            &registry,
            &validation,
        )
        .unwrap_err()
        .to_string()
        .contains("orphan")
    );

    let mismatched = user_address(second, 1_024);
    document.port_bindings.insert(mismatched.clone(), binding());
    let validation = validation_snapshot_for_document(&document, &registry);
    assert!(
        plan_with_validation(
            EditorGraphMutationDto::Connect {
                output: declared(first, "data_out").into(),
                input: mismatched.into(),
                order: None,
            },
            &document,
            &registry,
            &validation,
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
