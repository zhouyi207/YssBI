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
    ParameterConstraint, ParameterEditorSpec, ParameterKey, ParameterPresentation, ParameterSchema,
    ParameterSpec, TypeConstructorId, TypeExpr, TypeId, TypeParameterId, Value, data_series_type,
    numeric_data_series_type,
};
use crate::node_system::registry::{TypeConstructorRegistration, TypeRegistration};

mod connection;
mod create;
mod validation;
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
        title: key.into(),
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
            "string_series_out",
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
            "unknown_series_out",
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
            "numeric_series_in",
            PortDirection::Input,
            PortKind::Data,
            PortInstances::Declared,
            ConnectionsPerPort::Single,
            Some(LiteralPolicy::Forbidden),
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
        .find(|port| port.key.as_str() == "string_series_out")
        .unwrap()
        .value_type = data_series_type(TypeExpr::Concrete(TypeId::new("core.string").unwrap()));
    ports
        .iter_mut()
        .find(|port| port.key.as_str() == "unknown_series_out")
        .unwrap()
        .value_type = data_series_type(TypeExpr::Unknown);
    ports
        .iter_mut()
        .find(|port| port.key.as_str() == "numeric_series_in")
        .unwrap()
        .value_type = numeric_data_series_type();
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
            presentation: ParameterPresentation::DetailPanel,
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
            presentation: ParameterPresentation::DetailPanel,
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
            presentation: ParameterPresentation::DetailPanel,
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
            id: TypeId::new("core.float64").unwrap(),
            title_key: I18nKey::new("types.float64.title").unwrap(),
            classes: BTreeSet::new(),
        },
        TypeRegistration {
            id: TypeId::new("core.string").unwrap(),
            title_key: I18nKey::new("types.string.title").unwrap(),
            classes: BTreeSet::new(),
        },
    ]
    .into_boxed_slice();
    provider.type_constructors = vec![TypeConstructorRegistration {
        id: crate::node_system::protocol::TypeConstructorId::new("core.data_series").unwrap(),
        title_key: I18nKey::new("types.data_series.title").unwrap(),
        arity: 1,
    }]
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
            "types.float64.title",
            "types.string.title",
            "types.data_series.title",
            "nodes.test.editor_validation.title",
            "nodes.test.editor_validation.data_out",
            "nodes.test.editor_validation.string_series_out",
            "nodes.test.editor_validation.unknown_series_out",
            "nodes.test.editor_validation.numeric_series_in",
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
                    data_type: crate::data_contract::DataType::Int64,
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

fn assert_editor_error_code(error: MutationConflict, expected: EditorMutationErrorCode) {
    match error {
        MutationConflict::Editor(error) => assert_eq!(error.code, expected),
        other => panic!("expected editor mutation error {expected:?}, got {other:?}"),
    }
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
