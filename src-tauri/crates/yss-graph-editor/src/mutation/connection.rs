use super::*;

type MutationPort<'a> = crate::compatibility::ResolvedEditorPort<'a>;

pub(super) fn resolve_mutation_port<'a>(
    document: &'a GraphDocument,
    registry: &'a NodeRegistry,
    address: &PortAddress,
) -> Result<MutationPort<'a>, MutationConflict> {
    crate::compatibility::resolve_editor_port(document, registry, address)
        .map_err(MutationConflict::Editor)
}

pub(crate) fn move_connection_operations(
    document: &GraphDocument,
    registry: &NodeRegistry,
    catalog: Option<&crate::compatibility::CatalogMutationValidationSnapshot>,
    source: PortAddress,
    target: PortAddress,
) -> Result<Vec<GraphDocumentOperation>, MutationConflict> {
    move_connection_operations_with_id_allocator(
        document,
        registry,
        catalog,
        source,
        target,
        &ConnectionId::new,
    )
}

pub(crate) fn move_connection_operations_with_id_allocator(
    document: &GraphDocument,
    registry: &NodeRegistry,
    catalog: Option<&crate::compatibility::CatalogMutationValidationSnapshot>,
    source: PortAddress,
    target: PortAddress,
    allocate: &dyn Fn() -> ConnectionId,
) -> Result<Vec<GraphDocumentOperation>, MutationConflict> {
    let source_port = resolve_mutation_port(document, registry, &source)?;
    let target_port = resolve_mutation_port(document, registry, &target)?;
    validate_move_endpoints(&source_port, &target_port)?;
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
            crate::compatibility::validate_connection_types(
                document,
                registry,
                catalog,
                &connection.output,
                &connection.input,
            )
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
    apply_graph_document_patch(
        &mut staged,
        &GraphDocumentPatch::new(
            removals
                .values()
                .cloned()
                .map(|connection| GraphDocumentOperation::RemoveConnection { connection })
                .collect::<Vec<_>>(),
        ),
    )?;
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
    apply_graph_document_patch(
        &mut staged,
        &GraphDocumentPatch::new(removal_operations.clone()),
    )?;
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
        apply_graph_document_patch(
            &mut staged,
            &GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertConnection {
                connection: proposal.clone(),
            }]),
        )?;
    }

    let mut operations = removal_operations;
    operations.extend(proposals.into_iter().map(|mut connection| {
        connection.id = allocate();
        GraphDocumentOperation::InsertConnection { connection }
    }));
    Ok(operations)
}

fn validate_move_endpoints(
    source: &MutationPort<'_>,
    target: &MutationPort<'_>,
) -> Result<(), MutationConflict> {
    if matches!(source.binding, Some(DynamicPortBinding::Orphan { .. }))
        || matches!(target.binding, Some(DynamicPortBinding::Orphan { .. }))
    {
        return Err(editor_error(
            EditorMutationErrorCode::GraphPortOrphan,
            "orphan ports cannot be move endpoints",
        ));
    }
    if source.spec.direction != target.spec.direction {
        return Err(editor_error(
            EditorMutationErrorCode::GraphConnectionDirectionMismatch,
            "connection move endpoints have different directions",
        ));
    }
    if source.spec.kind != target.spec.kind {
        return Err(editor_error(
            EditorMutationErrorCode::GraphConnectionKindMismatch,
            "connection move endpoints have different kinds",
        ));
    }
    Ok(())
}

pub(crate) fn validate_resolved_dynamic_binding_authority(
    protocol: &NodeProtocol,
    spec: &PortSpec,
    parameters: &ParameterValues,
    origin: &DynamicMemberLocator,
    catalog: &crate::compatibility::CatalogMutationValidationSnapshot,
) -> Result<yss_graph_protocol::TypeExpr, MutationConflict> {
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
            let resource = authoritative_origin_resource(
                protocol,
                parameters,
                function.as_str(),
                ResourceBoundCreateArgs::Function,
                catalog,
            )?;
            let crate::compatibility::CatalogMutationResource::Function { signature, .. } =
                resource
            else {
                unreachable!("authoritative resource kind was checked before destructuring");
            };
            let resolver_id = resolver.as_str();
            let type_name = if resolver_id == yss_graph_catalog::FUNCTION_CALL_ARGUMENTS_RESOLVER
                && spec.direction == PortDirection::Input
            {
                signature
                    .parameters
                    .iter()
                    .find(|candidate| candidate.id == *parameter)
                    .map(|parameter| parameter.type_name.as_str())
            } else if resolver_id == yss_graph_catalog::FUNCTION_CALL_RESULTS_RESOLVER
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
            crate::compatibility::function_type_expr(type_name).map_err(|error| {
                invalid_editor_mutation(format!(
                    "function member '{}:{}' has invalid authoritative type '{}': {error}",
                    function.as_str(),
                    parameter.as_str(),
                    type_name
                ))
            })
        }
        DynamicMemberLocator::SchemaField { source, field } => {
            authoritative_origin_resource(
                protocol,
                parameters,
                source.as_str(),
                ResourceBoundCreateArgs::Database,
                catalog,
            )?;
            if resolver.as_str() != yss_graph_catalog::DATAFRAME_COLUMNS_RESOLVER {
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

fn authoritative_origin_resource<'a>(
    protocol: &NodeProtocol,
    parameters: &ParameterValues,
    resource_path: &str,
    create_args: ResourceBoundCreateArgs,
    catalog: &'a crate::compatibility::CatalogMutationValidationSnapshot,
) -> Result<&'a crate::compatibility::CatalogMutationResource, MutationConflict> {
    let parameter = crate::compatibility::resource_parameter(protocol, create_args)
        .map_err(invalid_editor_mutation)?;
    if parameters
        .get(parameter)
        .and_then(serde_json::Value::as_str)
        != Some(resource_path)
    {
        return Err(invalid_editor_mutation(
            "resolved dynamic member does not match the node resource binding",
        ));
    }
    let path = CatalogResourcePath::new(resource_path);
    let resource = catalog.resources.get(&path).ok_or_else(|| {
        MutationConflict::ReferencedResourceUnavailable(
            format!("catalog resource '{resource_path}' is unavailable").into(),
        )
    })?;
    if resource.create_args() != create_args {
        return Err(invalid_editor_mutation(
            "resolved dynamic member resource does not match protocol authority",
        ));
    }
    Ok(resource)
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
    crate::compatibility::validate_connection_types(document, registry, None, output, input)
        .map_err(MutationConflict::Editor)?;
    validate_connection_order(input_port.spec.connections, order)?;
    validate_connection_capacity(document, output, output_port.spec.connections)?;
    validate_connection_capacity(document, input, input_port.spec.connections)
}

pub(super) fn connect_operations(
    document: &GraphDocument,
    registry: &NodeRegistry,
    catalog: Option<&crate::compatibility::CatalogMutationValidationSnapshot>,
    output: PortAddress,
    input: PortAddress,
    order: Option<OrderKey>,
) -> Result<Vec<GraphDocumentOperation>, MutationConflict> {
    connect_operations_with_id_allocator(
        document,
        registry,
        catalog,
        output,
        input,
        order,
        ConnectionId::new,
    )
}

pub(crate) fn connect_operations_with_id_allocator(
    document: &GraphDocument,
    registry: &NodeRegistry,
    catalog: Option<&crate::compatibility::CatalogMutationValidationSnapshot>,
    output: PortAddress,
    input: PortAddress,
    order: Option<OrderKey>,
    allocate: impl FnOnce() -> ConnectionId,
) -> Result<Vec<GraphDocumentOperation>, MutationConflict> {
    let output_port = resolve_mutation_port(document, registry, &output)?;
    let input_port = resolve_mutation_port(document, registry, &input)?;
    validate_document_connection_endpoints(&output_port, &input_port)?;
    validate_connection_does_not_exist(document, &output, &input)?;
    crate::compatibility::validate_connection_types(document, registry, catalog, &output, &input)
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
    apply_graph_document_patch(&mut staged, &GraphDocumentPatch::new(operations.clone()))?;
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
        yss_graph_protocol::validate_typed_literal(literal, &port.spec.value_type, registry)
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
            yss_graph_protocol::normalize_json_literal(raw, &port.spec.value_type, registry)
                .map(|literal| {
                    serde_json::to_value(literal).expect("protocol typed values must serialize")
                })
                .map_err(|_| invalid_editor_mutation("literal does not match the input value type"))
        })
        .transpose()
}
