use super::*;
use yss_graph_document_edit::validate_graph_document_connection_candidate;

type MutationPort<'a> = crate::compatibility::ResolvedEditorPort<'a>;

pub(super) fn update_connection_operations(
    document: &GraphDocument,
    registry: &NodeRegistry,
    context: EditorMutationContext<'_>,
    updates: Vec<ConnectionUpdate>,
) -> Result<Vec<GraphDocumentOperation>, MutationConflict> {
    let mut selected = BTreeSet::new();
    let mut proposals = Vec::new();
    for update in updates {
        if !selected.insert(update.connection_id) {
            return Err(invalid_editor_mutation(
                "connection occurs more than once in an update",
            ));
        }
        let original = document
            .connections
            .get(&update.connection_id)
            .ok_or_else(|| {
                editor_error(
                    EditorMutationErrorCode::GraphConnectionNotFound,
                    "connection update target does not exist",
                )
            })?;
        proposals.push(DocumentConnection {
            id: original.id,
            output: update.output,
            input: update.input,
            order: update.order.unwrap_or_else(|| original.order.clone()),
        });
    }
    // Free the whole selection first so swapping occupied endpoints is atomic.
    let mut operations = disconnect_connection_operations(document, selected.iter().copied())?;
    let mut staged = prepare_graph_document_patch(
        document.clone(),
        &GraphDocumentPatch::new(operations.clone()),
    )?;
    for proposal in proposals {
        let mut next = connect_operations(
            &staged,
            registry,
            context,
            proposal.output,
            proposal.input,
            proposal.order,
        )?;
        for operation in &mut next {
            match operation {
                GraphDocumentOperation::RemoveConnection { connection }
                    if selected.contains(&connection.id) =>
                {
                    return Err(invalid_editor_mutation(
                        "updated connections compete for the same limited endpoint",
                    ));
                }
                GraphDocumentOperation::InsertConnection { connection } => {
                    connection.id = proposal.id
                }
                _ => {}
            }
        }
        let patch = GraphDocumentPatch::new(next);
        staged = prepare_graph_document_patch(staged, &patch)?;
        operations.extend(patch.operations);
    }
    Ok(operations)
}

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
    context: EditorMutationContext<'_>,
    source: PortAddress,
    target: PortAddress,
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
                context,
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
    let mut insertions = BTreeMap::new();
    validate_graph_document_connection_candidate(document, &removals, &insertions)?;
    match endpoint_capacity(
        document
            .connections
            .values()
            .filter(|connection| !removals.contains_key(&connection.id)),
        &target,
        target_port.spec.connections,
    )? {
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
    for proposal in &proposals {
        let mut connections = document
            .connections
            .values()
            .filter(|connection| !removals.contains_key(&connection.id))
            .chain(insertions.values());
        if connections.any(|connection| {
            connection.output == proposal.output && connection.input == proposal.input
        }) {
            return Err(editor_error(
                EditorMutationErrorCode::GraphConnectionAlreadyExists,
                "a moved connection endpoint pair already exists",
            ));
        }
        let output = resolve_mutation_port(document, registry, &proposal.output)?;
        let input = resolve_mutation_port(document, registry, &proposal.input)?;
        validate_connection_order(input.spec.connections, proposal.order.as_ref())?;
        validate_connection_capacity(
            document
                .connections
                .values()
                .filter(|connection| !removals.contains_key(&connection.id))
                .chain(insertions.values()),
            &proposal.output,
            output.spec.connections,
        )?;
        validate_connection_capacity(
            document
                .connections
                .values()
                .filter(|connection| !removals.contains_key(&connection.id))
                .chain(insertions.values()),
            &proposal.input,
            input.spec.connections,
        )?;
        insertions.insert(proposal.id, proposal.clone());
        validate_graph_document_connection_candidate(document, &removals, &insertions)?;
    }

    let mut operations = removal_operations;
    operations.extend(proposals.into_iter().map(|mut connection| {
        connection.id = ConnectionId::new();
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
    Ok(())
}

pub(crate) fn validate_resolved_dynamic_binding_authority(
    protocol: &NodeProtocol,
    spec: &PortSpec,
    parameters: &ParameterValues,
    origin: &DynamicMemberLocator,
    catalog: &crate::compatibility::CatalogMutationValidationSnapshot,
) -> Result<yss_node_protocol::TypeExpr, MutationConflict> {
    let PortCardinality::Derived { resolver } = &spec.cardinality else {
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
                ResourceBoundCreateArgs::FunctionGraph,
                catalog,
            )?;
            let crate::compatibility::CatalogMutationResource::Function { signature, .. } =
                resource
            else {
                unreachable!("authoritative resource kind was checked before destructuring");
            };
            let data_type = crate::compatibility::function_member_data_type(
                signature, spec, parameter,
            )
            .ok_or_else(|| {
                invalid_editor_mutation(format!(
                    "function member '{}:{}' is not authoritative for template '{}' on '{}'",
                    function.as_str(),
                    parameter.as_str(),
                    spec.key,
                    protocol.type_id
                ))
            })?;
            crate::compatibility::editor_type_expr(data_type).map_err(|error| {
                invalid_editor_mutation(format!(
                    "function member '{}:{}' has invalid authoritative type '{data_type:?}': {error}",
                    function.as_str(),
                    parameter.as_str(),
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
            if resolver.as_str() != yss_node_catalog::DATAFRAME_COLUMNS_RESOLVER {
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
    crate::compatibility::validate_connection_types(
        document,
        registry,
        EditorMutationContext::default(),
        output,
        input,
    )
    .map_err(MutationConflict::Editor)?;
    validate_connection_order(input_port.spec.connections, order)?;
    validate_connection_capacity(
        document.connections.values(),
        output,
        output_port.spec.connections,
    )?;
    validate_connection_capacity(
        document.connections.values(),
        input,
        input_port.spec.connections,
    )
}

pub(super) fn connect_operations(
    document: &GraphDocument,
    registry: &NodeRegistry,
    context: EditorMutationContext<'_>,
    output: PortAddress,
    input: PortAddress,
    order: Option<OrderKey>,
) -> Result<Vec<GraphDocumentOperation>, MutationConflict> {
    let output_port = resolve_mutation_port(document, registry, &output)?;
    let input_port = resolve_mutation_port(document, registry, &input)?;
    validate_document_connection_endpoints(&output_port, &input_port)?;
    validate_connection_does_not_exist(document, &output, &input)?;
    crate::compatibility::validate_connection_types(document, registry, context, &output, &input)
        .map_err(MutationConflict::Editor)?;
    plan_connection_operations_after_type_validation(
        document,
        output_port.spec.connections,
        input_port.spec.connections,
        output,
        input,
        order,
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
) -> Result<Vec<GraphDocumentOperation>, MutationConflict> {
    validate_connection_order(input_connections, order.as_ref())?;
    let output_capacity =
        endpoint_capacity(document.connections.values(), &output, output_connections)?;
    let input_capacity =
        endpoint_capacity(document.connections.values(), &input, input_connections)?;
    let mut incumbents = BTreeMap::new();
    for capacity in [output_capacity, input_capacity] {
        if let EndpointCapacity::Replace(connections) = capacity {
            for connection in connections {
                incumbents.insert(connection.id, connection);
            }
        }
    }
    validate_graph_document_connection_candidate(document, &incumbents, &BTreeMap::new())?;
    validate_connection_capacity(
        document
            .connections
            .values()
            .filter(|connection| !incumbents.contains_key(&connection.id)),
        &output,
        output_connections,
    )?;
    validate_connection_capacity(
        document
            .connections
            .values()
            .filter(|connection| !incumbents.contains_key(&connection.id)),
        &input,
        input_connections,
    )?;
    let mut operations = incumbents
        .into_values()
        .map(|connection| GraphDocumentOperation::RemoveConnection { connection })
        .collect::<Vec<_>>();
    operations.push(GraphDocumentOperation::InsertConnection {
        connection: DocumentConnection {
            id: ConnectionId::new(),
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

fn endpoint_capacity<'a>(
    connections: impl Iterator<Item = &'a DocumentConnection>,
    address: &PortAddress,
    capability: ConnectionsPerPort,
) -> Result<EndpointCapacity, MutationConflict> {
    match capability {
        ConnectionsPerPort::Single => {
            let connections = connections
                .filter(|connection| connection.output == *address || connection.input == *address)
                .cloned()
                .collect::<Vec<_>>();
            if connections.is_empty() {
                Ok(EndpointCapacity::Append)
            } else {
                Ok(EndpointCapacity::Replace(connections))
            }
        }
        ConnectionsPerPort::Multiple { .. } => {
            validate_connection_capacity(connections, address, capability)
                .map(|()| EndpointCapacity::Append)
        }
    }
}

fn validate_connection_capacity<'a>(
    connections: impl Iterator<Item = &'a DocumentConnection>,
    address: &PortAddress,
    capability: ConnectionsPerPort,
) -> Result<(), MutationConflict> {
    let maximum = match capability {
        ConnectionsPerPort::Single => Some(1),
        ConnectionsPerPort::Multiple { max, .. } => max.map(usize::from),
    };
    if let Some(maximum) = maximum
        && connections
            .filter(|connection| connection.output == *address || connection.input == *address)
            .take(maximum)
            .count()
            >= maximum
    {
        Err(editor_error(
            EditorMutationErrorCode::GraphConnectionLimitReached,
            format!("port '{address}' has reached its connection limit"),
        ))
    } else {
        Ok(())
    }
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
    if port.spec.direction != PortDirection::Input {
        return Err(invalid_editor_mutation(
            "literal overrides require an input port",
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
    literal: Option<&yss_node_protocol::TypedValue>,
) -> Result<(), MutationConflict> {
    let port = resolve_literal_target(document, registry, address)?;
    if let Some(literal) = literal {
        yss_node_protocol::validate_typed_value(literal, &port.spec.value_type, registry)
            .map_err(|_| invalid_editor_mutation("literal does not match the input value type"))?;
    }
    Ok(())
}

pub(crate) fn normalize_editor_literal_target(
    document: &GraphDocument,
    registry: &NodeRegistry,
    address: &PortAddress,
    literal: Option<&JsonValue>,
) -> Result<Option<yss_node_protocol::TypedValue>, MutationConflict> {
    if literal.is_none() && document.input_states.contains_key(address) {
        return Ok(None);
    }
    let port = resolve_literal_target(document, registry, address)?;
    literal
        .map(|raw| {
            yss_node_protocol::normalize_json_literal(raw, &port.spec.value_type, registry)
                .map_err(|_| invalid_editor_mutation("literal does not match the input value type"))
        })
        .transpose()
}
