use super::*;

pub(super) struct MutationPort<'a> {
    pub(super) spec: &'a PortSpec,
    pub(super) binding: Option<&'a DynamicPortBinding>,
}

pub(super) fn resolve_mutation_port<'a>(
    document: &'a GraphDocument,
    registry: &'a NodeRegistry,
    address: &PortAddress,
) -> Result<MutationPort<'a>, MutationConflict> {
    let node = document.nodes.get(&address.node_id).ok_or_else(|| {
        editor_error(
            EditorMutationErrorCode::GraphPortNotFound,
            format!("endpoint node '{}' does not exist", address.node_id),
        )
    })?;
    let protocol = registry.protocol(&node.node_type).ok_or_else(|| {
        invalid_editor_mutation(format!("unknown node type '{}'", node.node_type))
    })?;
    let template = match &address.port {
        PortRef::Declared { key } => key,
        PortRef::Instance { template, .. } => template,
    };
    let spec = protocol
        .interface
        .ports
        .iter()
        .find(|spec| &spec.key == template)
        .ok_or_else(|| {
            editor_error(
                EditorMutationErrorCode::GraphPortNotFound,
                format!("unknown port '{address}'"),
            )
        })?;
    let binding = match &address.port {
        PortRef::Declared { .. } => {
            if !matches!(spec.instances, PortInstances::Declared) {
                return Err(editor_error(
                    EditorMutationErrorCode::GraphPortNotFound,
                    format!("port '{address}' requires an instance address"),
                ));
            }
            None
        }
        PortRef::Instance { .. } => {
            let binding = document.port_bindings.get(address).ok_or_else(|| {
                editor_error(
                    EditorMutationErrorCode::GraphPortNotFound,
                    format!("instance port '{address}' has no binding"),
                )
            })?;
            let compatible = matches!(
                (&spec.instances, binding),
                (
                    PortInstances::UserCreated { .. },
                    DynamicPortBinding::UserCreated { .. }
                ) | (
                    PortInstances::Derived { .. },
                    DynamicPortBinding::Resolved { .. } | DynamicPortBinding::Orphan { .. }
                )
            );
            if !compatible {
                return Err(invalid_editor_mutation(format!(
                    "port binding kind does not match template '{address}'"
                )));
            }
            Some(binding)
        }
    };
    Ok(MutationPort { spec, binding })
}

pub(crate) fn move_connection_operations(
    document: &GraphDocument,
    registry: &NodeRegistry,
    snapshot: &EditorMutationValidationSnapshot,
    source: PortAddress,
    target: PortAddress,
) -> Result<Vec<GraphDocumentOperation>, MutationConflict> {
    move_connection_operations_with_id_allocator(
        document,
        registry,
        snapshot,
        source,
        target,
        &ConnectionId::new,
    )
}

pub(crate) fn move_connection_operations_with_id_allocator(
    document: &GraphDocument,
    registry: &NodeRegistry,
    snapshot: &EditorMutationValidationSnapshot,
    source: PortAddress,
    target: PortAddress,
    allocate: &dyn Fn() -> ConnectionId,
) -> Result<Vec<GraphDocumentOperation>, MutationConflict> {
    if snapshot.graph_revision != document.revision {
        return Err(MutationConflict::Projection(
            "editor mutation validation snapshot revision does not match the document".into(),
        ));
    }
    let source_port = resolve_mutation_port(document, registry, &source)?;
    let target_port = resolve_mutation_port(document, registry, &target)?;
    validate_move_endpoints(snapshot, &source, &target)?;
    if source == target {
        return Err(editor_error(
            EditorMutationErrorCode::GraphConnectionMoveSamePort,
            "connection source and target ports are identical",
        ));
    }

    let moved = document
        .connections
        .values()
        .filter(|connection| match source_port.spec.direction {
            PortDirection::Output => connection.output == source,
            PortDirection::Input => connection.input == source,
        })
        .cloned()
        .collect::<Vec<_>>();
    if moved.is_empty() {
        return Err(editor_error(
            EditorMutationErrorCode::GraphConnectionMoveSourceEmpty,
            "connection move source has no authoritative connections",
        ));
    }

    let proposals = moved
        .iter()
        .cloned()
        .map(|mut connection| {
            match source_port.spec.direction {
                PortDirection::Output => connection.output = target.clone(),
                PortDirection::Input => connection.input = target.clone(),
            }
            snapshot
                .validate_connection_types(&connection.output, &connection.input)
                .map_err(MutationConflict::Editor)?;
            Ok(connection)
        })
        .collect::<Result<Vec<_>, MutationConflict>>()?;

    let mut removals = moved
        .iter()
        .cloned()
        .map(|connection| (connection.id, connection))
        .collect::<BTreeMap<_, _>>();
    let mut staged = document.clone();
    staged.apply_patch(&GraphDocumentPatch::new(
        removals
            .values()
            .cloned()
            .map(|connection| GraphDocumentOperation::RemoveConnection { connection })
            .collect::<Vec<_>>(),
    ))?;
    match endpoint_capacity(&staged, &target, target_port.spec.connections)? {
        EndpointCapacity::Append => {}
        EndpointCapacity::Replace(incumbents) => {
            for connection in incumbents {
                removals.insert(connection.id, connection);
            }
        }
    }

    let removal_operations = removals
        .values()
        .cloned()
        .map(|connection| GraphDocumentOperation::RemoveConnection { connection })
        .collect::<Vec<_>>();
    staged = document.clone();
    staged.apply_patch(&GraphDocumentPatch::new(removal_operations.clone()))?;
    for proposal in &proposals {
        if staged.connections.values().any(|connection| {
            connection.output == proposal.output && connection.input == proposal.input
        }) {
            return Err(editor_error(
                EditorMutationErrorCode::GraphConnectionAlreadyExists,
                "a moved connection endpoint pair already exists",
            ));
        }
        let output = resolve_mutation_port(&staged, registry, &proposal.output)?;
        let input = resolve_mutation_port(&staged, registry, &proposal.input)?;
        validate_connection_order(input.spec.connections, proposal.order.as_ref())?;
        validate_connection_capacity(&staged, &proposal.output, output.spec.connections)?;
        validate_connection_capacity(&staged, &proposal.input, input.spec.connections)?;
        staged.apply_patch(&GraphDocumentPatch::new(vec![
            GraphDocumentOperation::InsertConnection {
                connection: proposal.clone(),
            },
        ]))?;
    }

    let mut operations = removal_operations;
    operations.extend(proposals.into_iter().map(|mut connection| {
        connection.id = allocate();
        GraphDocumentOperation::InsertConnection { connection }
    }));
    Ok(operations)
}

fn validate_move_endpoints(
    snapshot: &EditorMutationValidationSnapshot,
    source: &PortAddress,
    target: &PortAddress,
) -> Result<(), MutationConflict> {
    let source_port = snapshot.ports.get(source).ok_or_else(|| {
        editor_error(
            EditorMutationErrorCode::GraphPortNotFound,
            format!("move source port '{source}' is absent from validation snapshot"),
        )
    })?;
    let target_port = snapshot.ports.get(target).ok_or_else(|| {
        editor_error(
            EditorMutationErrorCode::GraphPortNotFound,
            format!("move target port '{target}' is absent from validation snapshot"),
        )
    })?;
    if source_port.orphan || target_port.orphan {
        return Err(editor_error(
            EditorMutationErrorCode::GraphPortOrphan,
            "orphan ports cannot be move endpoints",
        ));
    }
    if source_port.direction != target_port.direction {
        return Err(editor_error(
            EditorMutationErrorCode::GraphConnectionDirectionMismatch,
            "connection move endpoints have different directions",
        ));
    }
    if source_port.kind != target_port.kind {
        return Err(editor_error(
            EditorMutationErrorCode::GraphConnectionKindMismatch,
            "connection move endpoints have different kinds",
        ));
    }
    Ok(())
}

pub(super) fn projected_connect_operations(
    graph_path: &GraphResourcePath,
    document: &GraphDocument,
    registry: &NodeRegistry,
    output: PortAddress,
    input: PortAddress,
    order: Option<OrderKey>,
    plan: ProjectedConnectPlan,
) -> Result<Vec<GraphDocumentOperation>, MutationConflict> {
    let projected_is_output = output == plan.projection_address;
    let projected_is_input = input == plan.projection_address;
    if projected_is_output == projected_is_input {
        return Err(invalid_editor_mutation(
            "exactly one connection endpoint must match the projected member",
        ));
    }
    if projected_is_output != (plan.direction == PortDirection::Output)
        || projected_is_input != (plan.direction == PortDirection::Input)
    {
        return Err(invalid_editor_mutation(
            "connection endpoints have invalid directions",
        ));
    }
    let ordinary = if projected_is_output {
        input.clone()
    } else {
        output.clone()
    };
    let ordinary_port = resolve_mutation_port(document, registry, &ordinary)?;
    if matches!(
        ordinary_port.binding,
        Some(DynamicPortBinding::Orphan { .. })
    ) {
        return Err(invalid_editor_mutation("orphan ports cannot be connected"));
    }
    let expected_ordinary_direction = match plan.member.direction() {
        PortDirection::Output => PortDirection::Input,
        PortDirection::Input => PortDirection::Output,
    };
    if ordinary_port.spec.direction != expected_ordinary_direction {
        return Err(invalid_editor_mutation(
            "connection endpoints have invalid directions",
        ));
    }
    if ordinary_port.spec.kind != plan.kind {
        return Err(invalid_editor_mutation(
            "connection endpoint kinds do not match",
        ));
    }
    let ordinary_capacity = endpoint_capacity(document, &ordinary, ordinary_port.spec.connections)?;
    let input_connections = if plan.direction == PortDirection::Input {
        plan.connections
    } else {
        ordinary_port.spec.connections
    };
    validate_connection_order(input_connections, order.as_ref())?;
    let mut operations = match ordinary_capacity {
        EndpointCapacity::Append => Vec::new(),
        EndpointCapacity::Replace(connections) => connections
            .into_iter()
            .map(|connection| GraphDocumentOperation::RemoveConnection { connection })
            .collect(),
    };
    let mut staged = document.clone();
    staged.apply_patch(&GraphDocumentPatch::new(operations.clone()))?;
    validate_connection_capacity(&staged, &ordinary, ordinary_port.spec.connections)?;
    operations.extend(materialize_projected_member_operations(
        graph_path,
        document,
        plan.member,
        plan.authorization,
        ordinary,
        order,
    )?);
    Ok(operations)
}

pub(crate) fn validate_resolved_dynamic_binding_authority(
    protocol: &NodeProtocol,
    spec: &PortSpec,
    parameters: &ParameterValues,
    origin: &DynamicMemberLocator,
    catalog: &crate::graph::compatibility::CatalogMutationValidationSnapshot,
) -> Result<crate::graph::protocol::TypeExpr, MutationConflict> {
    let PortInstances::Derived { resolver } = &spec.instances else {
        return Err(invalid_editor_mutation(
            "resolved dynamic binding requires a derived port template",
        ));
    };
    match origin {
        DynamicMemberLocator::FunctionParameter {
            function,
            parameter,
        } => {
            let target = parameters
                .get(
                    &crate::graph::protocol::ParameterKey::new("target")
                        .expect("function target is a valid parameter key"),
                )
                .and_then(serde_json::Value::as_str);
            if target != Some(function.as_str()) {
                return Err(invalid_editor_mutation(
                    "resolved function member does not match the node target",
                ));
            }
            let path = CatalogResourcePath::new(function.as_str());
            let Some(crate::graph::compatibility::CatalogMutationResource::Function {
                signature,
                ..
            }) = catalog.resources.get(&path)
            else {
                return Err(MutationConflict::ReferencedResourceUnavailable(
                    format!("function resource '{}' is unavailable", function.as_str()).into(),
                ));
            };
            let resolver_id = resolver.as_str();
            let type_name = if resolver_id
                == crate::graph::catalog::project::FUNCTION_CALL_ARGUMENTS_RESOLVER
                && spec.direction == PortDirection::Input
            {
                signature
                    .parameters
                    .iter()
                    .find(|candidate| candidate.id == *parameter)
                    .map(|parameter| parameter.type_name.as_str())
            } else if resolver_id == crate::graph::catalog::project::FUNCTION_CALL_RESULTS_RESOLVER
                && spec.direction == PortDirection::Output
                && parameter.as_str() == "return"
            {
                signature.return_type.as_deref()
            } else {
                None
            };
            let type_name = type_name.ok_or_else(|| {
                invalid_editor_mutation(format!(
                    "function member '{}:{}' is not authoritative for template '{}' on '{}'",
                    function.as_str(),
                    parameter.as_str(),
                    spec.key,
                    protocol.type_id
                ))
            })?;
            crate::graph::compatibility::function_type_expr(type_name).map_err(|error| {
                invalid_editor_mutation(format!(
                    "function member '{}:{}' has invalid authoritative type '{}': {error}",
                    function.as_str(),
                    parameter.as_str(),
                    type_name
                ))
            })
        }
        DynamicMemberLocator::SchemaField { source, field } => {
            let path = CatalogResourcePath::new(source.as_str());
            if !matches!(
                catalog.resources.get(&path),
                Some(crate::graph::compatibility::CatalogMutationResource::Database { .. })
            ) {
                return Err(MutationConflict::ReferencedResourceUnavailable(
                    format!("database resource '{}' is unavailable", source.as_str()).into(),
                ));
            }
            if resolver.as_str() != crate::graph::catalog::dataframe::DATAFRAME_COLUMNS_RESOLVER {
                return Err(invalid_editor_mutation(format!(
                    "schema member '{}:{}' is invalid for template '{}'",
                    source.as_str(),
                    field.as_str(),
                    spec.key
                )));
            }
            Err(MutationConflict::ReferencedResourceUnavailable(
                format!(
                    "current database field authority for '{}:{}' is unavailable",
                    source.as_str(),
                    field.as_str()
                )
                .into(),
            ))
        }
    }
}

pub(crate) fn validate_subgraph_port(
    document: &GraphDocument,
    registry: &NodeRegistry,
    address: &PortAddress,
) -> Result<(), MutationConflict> {
    resolve_mutation_port(document, registry, address).map(|_| ())
}

pub(crate) fn validate_subgraph_connection(
    document: &GraphDocument,
    registry: &NodeRegistry,
    output: &PortAddress,
    input: &PortAddress,
    order: Option<&OrderKey>,
) -> Result<(), MutationConflict> {
    let output_port = resolve_mutation_port(document, registry, output)?;
    let input_port = resolve_mutation_port(document, registry, input)?;
    validate_document_connection_endpoints(&output_port, &input_port)?;
    validate_connection_does_not_exist(document, output, input)?;
    let output_type = authoritative_subgraph_port_type(&output_port);
    let input_type = authoritative_subgraph_port_type(&input_port);
    validate_connection_type_exprs(
        document,
        registry,
        output,
        input,
        output_port.spec.kind,
        output_type,
        input_type,
    )?;
    validate_connection_order(input_port.spec.connections, order)?;
    validate_connection_capacity(document, output, output_port.spec.connections)?;
    validate_connection_capacity(document, input, input_port.spec.connections)
}

pub(super) fn connect_operations(
    document: &GraphDocument,
    registry: &NodeRegistry,
    validation: &EditorMutationValidationSnapshot,
    output: PortAddress,
    input: PortAddress,
    order: Option<OrderKey>,
) -> Result<Vec<GraphDocumentOperation>, MutationConflict> {
    connect_operations_with_id_allocator(
        document,
        registry,
        validation,
        output,
        input,
        order,
        ConnectionId::new,
    )
}

pub(crate) fn connect_operations_with_id_allocator(
    document: &GraphDocument,
    registry: &NodeRegistry,
    validation: &EditorMutationValidationSnapshot,
    output: PortAddress,
    input: PortAddress,
    order: Option<OrderKey>,
    allocate: impl FnOnce() -> ConnectionId,
) -> Result<Vec<GraphDocumentOperation>, MutationConflict> {
    if validation.graph_revision != document.revision {
        return Err(MutationConflict::Projection(
            "editor mutation validation snapshot revision does not match the document".into(),
        ));
    }
    let output_port = resolve_mutation_port(document, registry, &output)?;
    let input_port = resolve_mutation_port(document, registry, &input)?;
    validation
        .validate_connection_endpoints(&output, &input)
        .map_err(MutationConflict::Editor)?;
    validate_connection_does_not_exist(document, &output, &input)?;
    validation
        .validate_connection_types(&output, &input)
        .map_err(MutationConflict::Editor)?;
    plan_connection_operations_after_type_validation(
        document,
        output_port.spec.connections,
        input_port.spec.connections,
        output,
        input,
        order,
        allocate,
    )
}

pub(super) fn connect_operations_prevalidated_type(
    document: &GraphDocument,
    registry: &NodeRegistry,
    output: PortAddress,
    input: PortAddress,
    order: Option<OrderKey>,
) -> Result<Vec<GraphDocumentOperation>, MutationConflict> {
    let output_port = resolve_mutation_port(document, registry, &output)?;
    let input_port = resolve_mutation_port(document, registry, &input)?;
    validate_document_connection_endpoints(&output_port, &input_port)?;
    validate_connection_does_not_exist(document, &output, &input)?;
    validate_static_connection_types(
        document,
        registry,
        &output,
        &input,
        &output_port,
        &input_port,
    )?;
    plan_connection_operations_after_type_validation(
        document,
        output_port.spec.connections,
        input_port.spec.connections,
        output,
        input,
        order,
        ConnectionId::new,
    )
}

fn authoritative_subgraph_port_type<'a>(
    port: &'a MutationPort<'a>,
) -> &'a crate::graph::protocol::TypeExpr {
    match port.binding {
        Some(DynamicPortBinding::Resolved { last_known, .. }) => last_known
            .value_type
            .as_ref()
            .unwrap_or(&port.spec.value_type),
        _ => &port.spec.value_type,
    }
}

fn validate_static_connection_types(
    document: &GraphDocument,
    registry: &NodeRegistry,
    output: &PortAddress,
    input: &PortAddress,
    output_port: &MutationPort<'_>,
    input_port: &MutationPort<'_>,
) -> Result<(), MutationConflict> {
    validate_connection_type_exprs(
        document,
        registry,
        output,
        input,
        output_port.spec.kind,
        &output_port.spec.value_type,
        &input_port.spec.value_type,
    )
}

fn validate_connection_type_exprs(
    document: &GraphDocument,
    registry: &NodeRegistry,
    output: &PortAddress,
    input: &PortAddress,
    kind: PortKind,
    output_type: &crate::graph::protocol::TypeExpr,
    input_type: &crate::graph::protocol::TypeExpr,
) -> Result<(), MutationConflict> {
    if kind != PortKind::Data {
        return Ok(());
    }
    let output_type_parameters = registry
        .protocol(&document.nodes[&output.node_id].node_type)
        .expect("resolved mutation ports have registered protocols")
        .interface
        .type_parameters
        .as_ref();
    let input_type_parameters = registry
        .protocol(&document.nodes[&input.node_id].node_type)
        .expect("resolved mutation ports have registered protocols")
        .interface
        .type_parameters
        .as_ref();
    if crate::graph::protocol::type_exprs_compatibility(
        output_type,
        input_type,
        output_type_parameters,
        input_type_parameters,
    ) == crate::graph::protocol::TypeCompatibility::Incompatible
    {
        Err(invalid_editor_mutation(
            "connection endpoint types are incompatible",
        ))
    } else {
        Ok(())
    }
}

fn validate_connection_does_not_exist(
    document: &GraphDocument,
    output: &PortAddress,
    input: &PortAddress,
) -> Result<(), MutationConflict> {
    if document
        .connections
        .values()
        .any(|connection| connection.output == *output && connection.input == *input)
    {
        Err(editor_error(
            EditorMutationErrorCode::GraphConnectionAlreadyExists,
            "the requested connection already exists",
        ))
    } else {
        Ok(())
    }
}

fn plan_connection_operations_after_type_validation(
    document: &GraphDocument,
    output_connections: ConnectionsPerPort,
    input_connections: ConnectionsPerPort,
    output: PortAddress,
    input: PortAddress,
    order: Option<OrderKey>,
    allocate: impl FnOnce() -> ConnectionId,
) -> Result<Vec<GraphDocumentOperation>, MutationConflict> {
    validate_connection_order(input_connections, order.as_ref())?;
    let output_capacity = endpoint_capacity(document, &output, output_connections)?;
    let input_capacity = endpoint_capacity(document, &input, input_connections)?;
    let mut incumbents = BTreeMap::new();
    for capacity in [output_capacity, input_capacity] {
        if let EndpointCapacity::Replace(connections) = capacity {
            for connection in connections {
                incumbents.insert(connection.id, connection);
            }
        }
    }
    let mut operations = incumbents
        .into_values()
        .map(|connection| GraphDocumentOperation::RemoveConnection { connection })
        .collect::<Vec<_>>();
    let mut staged = document.clone();
    staged.apply_patch(&GraphDocumentPatch::new(operations.clone()))?;
    validate_connection_capacity(&staged, &output, output_connections)?;
    validate_connection_capacity(&staged, &input, input_connections)?;
    operations.push(GraphDocumentOperation::InsertConnection {
        connection: DocumentConnection {
            id: allocate(),
            output,
            input,
            order,
        },
    });
    Ok(operations)
}

fn validate_document_connection_endpoints(
    output: &MutationPort<'_>,
    input: &MutationPort<'_>,
) -> Result<(), MutationConflict> {
    if matches!(output.binding, Some(DynamicPortBinding::Orphan { .. }))
        || matches!(input.binding, Some(DynamicPortBinding::Orphan { .. }))
    {
        return Err(editor_error(
            EditorMutationErrorCode::GraphPortOrphan,
            "orphan ports cannot be connected",
        ));
    }
    if output.spec.direction != PortDirection::Output
        || input.spec.direction != PortDirection::Input
    {
        return Err(editor_error(
            EditorMutationErrorCode::GraphConnectionDirectionMismatch,
            "connection endpoints have invalid directions",
        ));
    }
    if output.spec.kind != input.spec.kind {
        return Err(editor_error(
            EditorMutationErrorCode::GraphConnectionKindMismatch,
            "connection endpoint kinds do not match",
        ));
    }
    Ok(())
}

fn validate_connection_order(
    input_connections: ConnectionsPerPort,
    order: Option<&OrderKey>,
) -> Result<(), MutationConflict> {
    match input_connections {
        ConnectionsPerPort::Multiple { ordered: true, .. } if order.is_none() => Err(editor_error(
            EditorMutationErrorCode::GraphConnectionOrderRequired,
            "ordered input connections require an order key",
        )),
        ConnectionsPerPort::Single | ConnectionsPerPort::Multiple { ordered: false, .. }
            if order.is_some() =>
        {
            Err(editor_error(
                EditorMutationErrorCode::GraphConnectionOrderForbidden,
                "unordered input connections cannot carry an order key",
            ))
        }
        _ => Ok(()),
    }
}

enum EndpointCapacity {
    Append,
    Replace(Vec<DocumentConnection>),
}

fn endpoint_capacity(
    document: &GraphDocument,
    address: &PortAddress,
    capability: ConnectionsPerPort,
) -> Result<EndpointCapacity, MutationConflict> {
    let connections = document
        .connections
        .values()
        .filter(|connection| connection.output == *address || connection.input == *address)
        .cloned()
        .collect::<Vec<_>>();
    match capability {
        ConnectionsPerPort::Single if connections.is_empty() => Ok(EndpointCapacity::Append),
        ConnectionsPerPort::Single => Ok(EndpointCapacity::Replace(connections)),
        ConnectionsPerPort::Multiple { max, .. }
            if max.is_some_and(|maximum| connections.len() >= usize::from(maximum)) =>
        {
            Err(editor_error(
                EditorMutationErrorCode::GraphConnectionLimitReached,
                format!("port '{address}' has reached its connection limit"),
            ))
        }
        ConnectionsPerPort::Multiple { .. } => Ok(EndpointCapacity::Append),
    }
}

fn validate_connection_capacity(
    document: &GraphDocument,
    address: &PortAddress,
    capability: ConnectionsPerPort,
) -> Result<(), MutationConflict> {
    endpoint_capacity(document, address, capability).and_then(|capacity| match capacity {
        EndpointCapacity::Append => Ok(()),
        EndpointCapacity::Replace(_) => Err(editor_error(
            EditorMutationErrorCode::GraphConnectionLimitReached,
            format!("port '{address}' has reached its connection limit"),
        )),
    })
}

fn resolve_literal_target<'a>(
    document: &'a GraphDocument,
    registry: &'a NodeRegistry,
    address: &PortAddress,
) -> Result<MutationPort<'a>, MutationConflict> {
    let port = resolve_mutation_port(document, registry, address)?;
    if matches!(port.binding, Some(DynamicPortBinding::Orphan { .. })) {
        return Err(invalid_editor_mutation(
            "orphan ports cannot carry literal overrides",
        ));
    }
    if port.spec.direction != PortDirection::Input || port.spec.kind != PortKind::Data {
        return Err(invalid_editor_mutation(
            "literal overrides require a data input port",
        ));
    }
    if !matches!(
        port.spec
            .input_binding
            .as_ref()
            .map(|binding| binding.literal_policy),
        Some(LiteralPolicy::Allowed)
    ) {
        return Err(invalid_editor_mutation(
            "the input protocol forbids literal overrides",
        ));
    }
    Ok(port)
}

pub(crate) fn validate_literal_target(
    document: &GraphDocument,
    registry: &NodeRegistry,
    address: &PortAddress,
    literal: Option<&TypedValue>,
) -> Result<(), MutationConflict> {
    let port = resolve_literal_target(document, registry, address)?;
    if let Some(literal) = literal {
        crate::graph::protocol::validate_typed_literal(literal, &port.spec.value_type, registry)
            .map_err(|_| invalid_editor_mutation("literal does not match the input value type"))?;
    }
    Ok(())
}

pub(crate) fn normalize_editor_literal_target(
    document: &GraphDocument,
    registry: &NodeRegistry,
    address: &PortAddress,
    literal: Option<&TypedValue>,
) -> Result<Option<TypedValue>, MutationConflict> {
    let port = resolve_literal_target(document, registry, address)?;
    literal
        .map(|raw| {
            crate::graph::protocol::normalize_json_literal(raw, &port.spec.value_type, registry)
                .map(|literal| {
                    serde_json::to_value(literal).expect("protocol typed values must serialize")
                })
                .map_err(|_| invalid_editor_mutation("literal does not match the input value type"))
        })
        .transpose()
}
