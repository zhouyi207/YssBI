use super::*;

pub(super) fn include_referenced_orphan_ports(
    document: &GraphDocument,
    nodes: &mut [GraphNodeSemanticFact],
    diagnostics: &mut Vec<GraphDiagnosticFact>,
) {
    let addresses = document
        .connections
        .values()
        .flat_map(|connection| {
            [
                (&connection.output, PortDirection::Output),
                (&connection.input, PortDirection::Input),
            ]
        })
        .chain(
            document
                .input_states
                .keys()
                .map(|address| (address, PortDirection::Input)),
        );
    for (address, direction) in addresses {
        let Some(node) = nodes
            .iter_mut()
            .find(|node| node.node_id == address.node_id)
        else {
            continue;
        };
        if node.ports.iter().any(|port| &port.address == address) {
            continue;
        }
        let key = match &address.port {
            PortRef::Declared { key } => key,
            PortRef::Instance { template, .. } => template,
        };
        let mut ports = node.ports.to_vec();
        ports.push(GraphPortSemanticFact {
            address: address.clone(),
            label: key.as_str().into(),
            instance_label: None,
            direction,
            result_category: GraphResultCategory::Value,
            backing: if address.is_instance() {
                GraphPortBacking::DocumentInstance
            } else {
                GraphPortBacking::Declared
            },
            orphan: true,
            can_remove: false,
            connections: GraphPortConnectionFacts {
                current: document
                    .connections
                    .values()
                    .filter(|connection| {
                        &connection.output == address || &connection.input == address
                    })
                    .count() as u32,
                maximum: None,
                ordered: false,
            },
            editor: GraphPortEditorFact::Hidden,
            protocol_default: None,
            literal_allowed: false,
            accepted_type: TypeExpr::Unknown,
            accepted_domain: None,
            type_state: TypeState::Unknown(TypeUnknownReason::OrphanedPort),
            schema: None,
            schema_state: GraphSchemaState::NotApplicable,
        });
        node.ports = ports.into_boxed_slice();
        if !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.primary == GraphDiagnosticLocation::Port(address.clone()))
        {
            diagnostics.push(graph_problem(
                GraphDiagnosticKind::PortUnknown,
                GraphDiagnosticLocation::Port(address.clone()),
                [("port", address.to_string().into())],
            ));
        }
    }
}

pub(super) fn binding_order(binding: &DynamicPortBinding) -> &OrderKey {
    match binding {
        DynamicPortBinding::UserCreated { order }
        | DynamicPortBinding::Resolved { order, .. }
        | DynamicPortBinding::Orphan { order, .. } => order,
    }
}

pub(super) fn resource_exists(
    resources: &ResourceCatalogSnapshot,
    kind: ResourceDisplayKind,
    identity: &str,
) -> bool {
    match kind {
        ResourceDisplayKind::Function => GraphResourcePath::new(identity)
            .ok()
            .is_some_and(|path| resources.function_signature(&path).is_some()),
        ResourceDisplayKind::Database => resources
            .database_schema(&GraphResourceId::new(identity))
            .is_some(),
    }
}

pub(super) fn binding_origin(binding: &DynamicPortBinding) -> Option<&DynamicMemberLocator> {
    match binding {
        DynamicPortBinding::UserCreated { .. } => None,
        DynamicPortBinding::Resolved { origin, .. } | DynamicPortBinding::Orphan { origin, .. } => {
            Some(origin)
        }
    }
}

pub(super) fn binding_matches_cardinality(
    binding: &DynamicPortBinding,
    cardinality: &PortCardinality,
) -> bool {
    matches!(
        (binding, cardinality),
        (
            DynamicPortBinding::UserCreated { .. },
            PortCardinality::UserCreated { .. }
        ) | (
            DynamicPortBinding::Resolved { .. } | DynamicPortBinding::Orphan { .. },
            PortCardinality::Derived { .. }
        )
    )
}

pub(super) fn binding_kind(binding: &DynamicPortBinding) -> &'static str {
    match binding {
        DynamicPortBinding::UserCreated { .. } => "user_created",
        DynamicPortBinding::Resolved { .. } => "resolved",
        DynamicPortBinding::Orphan { .. } => "orphan",
    }
}

pub(super) fn port_cardinality_kind(cardinality: &PortCardinality) -> &'static str {
    match cardinality {
        PortCardinality::Declared => "declared",
        PortCardinality::UserCreated { .. } => "user_created",
        PortCardinality::Derived { .. } => "derived",
    }
}

pub(super) fn project_declared_port(
    document: &GraphDocument,
    address: PortAddress,
    spec: &yss_graph_protocol::PortSpec,
    resolved_schema: Option<&ResolvedSchemaFact>,
) -> GraphPortSemanticFact {
    debug_assert!(matches!(spec.cardinality, PortCardinality::Declared));
    project_concrete_port(
        document,
        spec,
        ConcretePortProjection {
            address,
            backing: GraphPortBacking::Declared,
            orphan: false,
            can_remove: false,
            instance_label: None,
            value_type: spec.value_type.clone(),
            resolved_schema: resolved_schema.cloned(),
        },
    )
}

pub(super) fn project_bound_port(
    document: &GraphDocument,
    protocol: &yss_graph_protocol::NodeProtocol,
    spec: &yss_graph_protocol::PortSpec,
    node_bindings: &[(&PortAddress, &DynamicPortBinding)],
    projection: BoundPortProjection<'_>,
) -> GraphPortSemanticFact {
    let BoundPortProjection {
        address,
        binding,
        current_member,
        resolved_schema,
    } = projection;
    let (orphan, can_remove, instance_label, value_type) = match binding {
        DynamicPortBinding::UserCreated { .. } => (
            false,
            can_remove_user_created_port(address.node_id, protocol, spec, address, node_bindings),
            None,
            spec.value_type.clone(),
        ),
        DynamicPortBinding::Resolved { last_known, .. }
        | DynamicPortBinding::Orphan { last_known, .. } => current_member.map_or_else(
            || {
                (
                    true,
                    true,
                    Some(last_known.label.clone().into_boxed_str()),
                    last_known
                        .value_type
                        .clone()
                        .unwrap_or_else(|| spec.value_type.clone()),
                )
            },
            |member| {
                (
                    false,
                    false,
                    Some(member.label.clone()),
                    member.value_type.clone(),
                )
            },
        ),
    };
    project_concrete_port(
        document,
        spec,
        ConcretePortProjection {
            address: address.clone(),
            backing: GraphPortBacking::DocumentInstance,
            orphan,
            can_remove,
            instance_label,
            value_type,
            resolved_schema: resolved_schema.cloned(),
        },
    )
}

pub(super) struct BoundPortProjection<'a> {
    pub(super) address: &'a PortAddress,
    pub(super) binding: &'a DynamicPortBinding,
    pub(super) current_member: Option<&'a derived_ports::DerivedPortMember>,
    pub(super) resolved_schema: Option<&'a ResolvedSchemaFact>,
}

pub(super) struct ConcretePortProjection {
    pub(super) address: PortAddress,
    pub(super) backing: GraphPortBacking,
    pub(super) orphan: bool,
    pub(super) can_remove: bool,
    pub(super) instance_label: Option<Box<str>>,
    pub(super) value_type: TypeExpr,
    pub(super) resolved_schema: Option<ResolvedSchemaFact>,
}

pub(super) fn project_concrete_port(
    document: &GraphDocument,
    spec: &yss_graph_protocol::PortSpec,
    projection: ConcretePortProjection,
) -> GraphPortSemanticFact {
    let ConcretePortProjection {
        address,
        backing,
        orphan,
        can_remove,
        instance_label,
        value_type,
        resolved_schema,
    } = projection;
    let connections = document
        .connections
        .values()
        .filter(|connection| connection.input == address || connection.output == address)
        .count() as u32;
    let (maximum, ordered) = match spec.connections {
        ConnectionsPerPort::Single => (Some(1), false),
        ConnectionsPerPort::Multiple { max, ordered } => (max.map(u32::from), ordered),
    };
    let editor = graph_port_editor_fact(&spec.editor);
    GraphPortSemanticFact {
        result_category: result_category::result_category_for_output(
            document.nodes[&address.node_id].node_type.as_str(),
            spec.key.as_str(),
        ),
        address,
        label: instance_label.clone().unwrap_or_else(|| spec.title.clone()),
        instance_label,
        direction: spec.direction,
        backing,
        orphan,
        can_remove,
        connections: GraphPortConnectionFacts {
            current: connections,
            maximum,
            ordered,
        },
        editor,
        protocol_default: spec
            .input_binding
            .as_ref()
            .and_then(|binding| binding.default_value.clone()),
        literal_allowed: spec.input_binding.as_ref().is_some_and(|binding| {
            binding.literal_policy == yss_graph_protocol::LiteralPolicy::Allowed
        }),
        accepted_type: value_type,
        accepted_domain: None,
        type_state: TypeState::Unknown(TypeUnknownReason::UnsupportedDeclaration),
        schema: spec.schema.clone(),
        schema_state: resolved_schema
            .map_or(GraphSchemaState::NotApplicable, GraphSchemaState::Exact),
    }
}

fn can_remove_user_created_port(
    node_id: NodeId,
    protocol: &yss_graph_protocol::NodeProtocol,
    spec: &yss_graph_protocol::PortSpec,
    address: &PortAddress,
    node_bindings: &[(&PortAddress, &DynamicPortBinding)],
) -> bool {
    let PortRef::Instance { instance_id, .. } = address.port else {
        return false;
    };
    if let Some(group) = protocol.interface.member_group_for_template(&spec.key) {
        let state = port_member_group_state(node_id, group, node_bindings.iter().copied());
        return !state.is_complete(instance_id) || state.complete_count() > usize::from(group.min);
    }
    let PortCardinality::UserCreated { min, .. } = spec.cardinality else {
        return false;
    };
    user_created_port_instance_count(node_id, &spec.key, node_bindings.iter().copied())
        > usize::from(min)
}

pub(super) fn project_port_instance_additions(
    node_id: NodeId,
    protocol: &yss_graph_protocol::NodeProtocol,
    node_bindings: &[(&PortAddress, &DynamicPortBinding)],
) -> (Vec<GraphPortInstanceAdditionFact>, bool) {
    let mut minimum_instances_present = true;
    let additions = protocol
        .interface
        .ports
        .iter()
        .filter_map(|spec| {
            let PortCardinality::UserCreated { min, max } = spec.cardinality else {
                return None;
            };
            let (minimum_instances, maximum_instances, current_instances) =
                if let Some(group) = protocol.interface.member_group_for_template(&spec.key) {
                    if group.templates.first() != Some(&spec.key) {
                        return None;
                    }
                    (
                        group.min,
                        group.max,
                        port_member_group_state(node_id, group, node_bindings.iter().copied())
                            .complete_count(),
                    )
                } else {
                    (
                        min,
                        max,
                        user_created_port_instance_count(
                            node_id,
                            &spec.key,
                            node_bindings.iter().copied(),
                        ),
                    )
                };
            minimum_instances_present &= current_instances >= usize::from(minimum_instances);
            Some(GraphPortInstanceAdditionFact {
                template_key: spec.key.clone(),
                label: spec.title.clone(),
                direction: spec.direction,
                can_add: maximum_instances
                    .is_none_or(|maximum| current_instances < usize::from(maximum)),
            })
        })
        .collect();
    (additions, minimum_instances_present)
}

fn graph_port_editor_fact(editor: &PortEditorSpec) -> GraphPortEditorFact {
    match editor {
        PortEditorSpec::Default => GraphPortEditorFact::Default,
        PortEditorSpec::Hidden => GraphPortEditorFact::Hidden,
        PortEditorSpec::InlineLiteral => GraphPortEditorFact::InlineLiteral,
        PortEditorSpec::SchemaColumns { allow_multiple } => GraphPortEditorFact::SchemaColumns {
            allow_multiple: *allow_multiple,
        },
    }
}
