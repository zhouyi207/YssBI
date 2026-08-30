use super::*;

pub(crate) fn instantiate_subgraph(
    graph_path: &GraphResourcePath,
    document: &GraphDocument,
    registry: &NodeRegistry,
    catalog: &CatalogMutationValidationSnapshot,
    snapshot: ValidatedClipboardSubgraph,
    anchor: NodePosition,
) -> Result<GraphDocumentPatch, MutationConflict> {
    let ValidatedClipboardSubgraph(mut snapshot) = snapshot;
    validate_insert_budget(&snapshot)?;
    snapshot
        .nodes
        .sort_by(|left, right| left.local_id.cmp(&right.local_id));
    snapshot
        .port_bindings
        .sort_by(|left, right| left.address.cmp(&right.address));
    snapshot
        .input_states
        .sort_by(|left, right| left.address.cmp(&right.address));
    snapshot.connections.sort_by(|left, right| {
        (&left.output, &left.input, &left.order).cmp(&(&right.output, &right.input, &right.order))
    });

    let node_types = validate_insert_nodes(graph_path, registry, catalog, &snapshot, anchor)?;
    let instance_keys = validate_portable_references(registry, catalog, &snapshot, &node_types)?;

    let temporary_nodes = temporary_node_ids(document, node_types.keys());
    let temporary_instances = temporary_port_instance_ids(document, instance_keys.iter());
    let temporary_connections = temporary_connection_ids(document, snapshot.connections.len());
    plan_instantiation(
        document,
        registry,
        &snapshot,
        anchor,
        &temporary_nodes,
        &temporary_instances,
        &temporary_connections,
    )?;

    let node_ids = fresh_node_ids(document, node_types.keys());
    let instance_ids = fresh_port_instance_ids(document, instance_keys.iter());
    let connection_ids = fresh_connection_ids(document, snapshot.connections.len());
    let patch = plan_instantiation(
        document,
        registry,
        &snapshot,
        anchor,
        &node_ids,
        &instance_ids,
        &connection_ids,
    )?;
    let mut staged = document.clone();
    patch
        .apply_without_revision(&mut staged)
        .map_err(|error| invalid_clipboard(format!("subgraph patch validation failed: {error}")))?;
    Ok(patch)
}

#[cfg(test)]
pub(crate) fn instantiate_subgraph_for_test(
    graph_path: &GraphResourcePath,
    document: &GraphDocument,
    registry: &NodeRegistry,
    catalog: &CatalogMutationValidationSnapshot,
    snapshot: ClipboardSubgraph,
    anchor: NodePosition,
) -> Result<GraphDocumentPatch, MutationConflict> {
    instantiate_subgraph(
        graph_path,
        document,
        registry,
        catalog,
        ValidatedClipboardSubgraph(snapshot),
        anchor,
    )
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct LocalInstanceKey {
    node_id: ClipboardNodeId,
    scope: PortKey,
    local_instance_id: ClipboardPortInstanceId,
}

fn validate_insert_budget(snapshot: &ClipboardSubgraph) -> Result<(), MutationConflict> {
    if snapshot.schema_version != CLIPBOARD_SUBGRAPH_SCHEMA_VERSION {
        return Err(invalid_clipboard(format!(
            "unsupported clipboard subgraph schema version {}",
            snapshot.schema_version
        )));
    }
    if snapshot.nodes.is_empty() {
        return Err(invalid_clipboard("clipboard subgraph contains no nodes"));
    }
    enforce_insert_limit("nodes", snapshot.nodes.len(), MAX_CLIPBOARD_NODES)?;
    enforce_insert_limit(
        "connections",
        snapshot.connections.len(),
        MAX_CLIPBOARD_CONNECTIONS,
    )?;
    enforce_insert_limit(
        "port bindings",
        snapshot.port_bindings.len(),
        MAX_CLIPBOARD_PORT_BINDINGS,
    )?;
    enforce_insert_limit(
        "input states",
        snapshot.input_states.len(),
        MAX_CLIPBOARD_INPUT_STATES,
    )?;

    let serialized = serde_json::to_vec(snapshot)
        .map_err(|error| invalid_clipboard(format!("clipboard serialization failed: {error}")))?;
    enforce_insert_limit(
        "serialized bytes",
        serialized.len(),
        MAX_CLIPBOARD_SERIALIZED_BYTES,
    )?;
    let mut parameter_bytes = 0usize;
    for state in &snapshot.input_states {
        if state
            .state
            .literal_override
            .as_ref()
            .is_some_and(|value| json_depth(value) > MAX_CLIPBOARD_VALUE_DEPTH)
        {
            return Err(invalid_clipboard(
                "clipboard input literal exceeds depth limit",
            ));
        }
    }
    for node in &snapshot.nodes {
        let bytes = serde_json::to_vec(&node.parameters).map_err(|error| {
            invalid_clipboard(format!("clipboard parameter serialization failed: {error}"))
        })?;
        parameter_bytes = parameter_bytes
            .checked_add(bytes.len())
            .ok_or_else(|| invalid_clipboard("clipboard parameter size overflow"))?;
        for value in node.parameters.values() {
            if json_depth(value) > MAX_CLIPBOARD_VALUE_DEPTH {
                return Err(invalid_clipboard(
                    "clipboard parameter value exceeds depth limit",
                ));
            }
        }
    }
    enforce_insert_limit(
        "parameter bytes",
        parameter_bytes,
        MAX_CLIPBOARD_PARAMETER_BYTES,
    )
}

fn validate_insert_nodes(
    graph_path: &GraphResourcePath,
    registry: &NodeRegistry,
    catalog: &CatalogMutationValidationSnapshot,
    snapshot: &ClipboardSubgraph,
    anchor: NodePosition,
) -> Result<BTreeMap<ClipboardNodeId, NodeTypeId>, MutationConflict> {
    if !anchor.x.is_finite() || !anchor.y.is_finite() {
        return Err(invalid_clipboard("subgraph anchor must be finite"));
    }
    let mut node_types = BTreeMap::new();
    for node in &snapshot.nodes {
        validate_local_identity("node", node.local_id.0.as_ref())?;
        let node_type = creation_node_type(&node.creation).clone();
        if node_types
            .insert(node.local_id.clone(), node_type.clone())
            .is_some()
        {
            return Err(invalid_clipboard(format!(
                "duplicate clipboard node ID '{}'",
                node.local_id.0
            )));
        }
        let position = NodePosition {
            x: anchor.x + node.relative_position.x,
            y: anchor.y + node.relative_position.y,
        };
        if !node.relative_position.x.is_finite()
            || !node.relative_position.y.is_finite()
            || !position.x.is_finite()
            || !position.y.is_finite()
        {
            return Err(invalid_clipboard(format!(
                "clipboard node '{}' has a non-finite target position",
                node.local_id.0
            )));
        }
        validate_node_creation(graph_path, registry, catalog, node)?;
    }
    Ok(node_types)
}

fn validate_node_creation(
    graph_path: &GraphResourcePath,
    registry: &NodeRegistry,
    catalog: &CatalogMutationValidationSnapshot,
    node: &ClipboardNode,
) -> Result<(), MutationConflict> {
    let node_type = creation_node_type(&node.creation);
    let protocol = registry.protocol(node_type).ok_or_else(|| {
        invalid_clipboard(format!("clipboard node type '{node_type}' is unavailable"))
    })?;
    if protocol.managed_role.is_some() {
        return Err(invalid_clipboard(format!(
            "clipboard node type '{node_type}' is managed"
        )));
    }
    validate_node_scope(graph_path, protocol)
        .map_err(|error| invalid_clipboard(error.to_string()))?;

    match &node.creation {
        ClipboardNodeCreation::Static { .. } => {
            if matches!(
                protocol.instance_display,
                NodeInstanceDisplaySpec::ResourceParameter { .. }
            ) || !matches!(
                authoritative_static_descriptor(registry, protocol),
                Some(NodeCreation::Static { .. }) | Some(NodeCreation::ParameterizedStatic { .. })
            ) {
                return Err(invalid_clipboard(format!(
                    "clipboard static identity does not match registry authority for '{node_type}'"
                )));
            }
        }
        ClipboardNodeCreation::ResourceBound {
            resource_path,
            create_args,
            ..
        } => validate_resource_creation(
            graph_path,
            protocol,
            resource_path,
            *create_args,
            catalog,
            &node.parameters,
        )?,
    }
    validate_parameters_with_registry(registry, protocol, &node.parameters)
        .map_err(|error| invalid_clipboard(error.to_string()))
}

fn validate_resource_creation(
    graph_path: &GraphResourcePath,
    protocol: &yss_graph_protocol::NodeProtocol,
    resource_path: &CatalogResourcePath,
    create_args: ResourceBoundCreateArgs,
    catalog: &CatalogMutationValidationSnapshot,
    parameters: &ParameterValues,
) -> Result<(), MutationConflict> {
    let resource = catalog.resources.get(resource_path).ok_or_else(|| {
        unavailable_resource(format!(
            "referenced resource '{}' is unavailable",
            resource_path.as_str()
        ))
    })?;
    validate_resource_path(resource_path, create_args)?;
    let (allowed, binding, kind, in_scope) = match (create_args, resource) {
        (
            ResourceBoundCreateArgs::Function,
            CatalogMutationResource::Function {
                allowed_node_type_id,
                parameter_binding,
                ..
            },
        ) => (
            allowed_node_type_id == &protocol.type_id,
            parameter_binding.as_ref(),
            ResourceDisplayKind::Function,
            true,
        ),
        (
            ResourceBoundCreateArgs::Variable,
            CatalogMutationResource::Variable {
                allowed_node_type_ids,
                parameter_binding,
                scope,
                ..
            },
        ) => (
            allowed_node_type_ids.contains(&protocol.type_id),
            parameter_binding.as_ref(),
            ResourceDisplayKind::Variable,
            variable_in_scope(graph_path, scope),
        ),
        (
            ResourceBoundCreateArgs::Database,
            CatalogMutationResource::Database {
                allowed_node_type_id,
                parameter_binding,
                ..
            },
        ) => (
            allowed_node_type_id == &protocol.type_id,
            parameter_binding.as_ref(),
            ResourceDisplayKind::Database,
            true,
        ),
        _ => {
            return Err(invalid_clipboard(format!(
                "resource '{}' kind does not match clipboard creation arguments",
                resource_path.as_str()
            )));
        }
    };
    if !allowed || !in_scope {
        return Err(unavailable_resource(format!(
            "resource '{}' is unavailable for this graph and node type",
            resource_path.as_str()
        )));
    }
    let NodeInstanceDisplaySpec::ResourceParameter {
        parameter,
        kind: expected_kind,
    } = &protocol.instance_display
    else {
        return Err(invalid_clipboard(format!(
            "node type '{}' is not resource-bound",
            protocol.type_id
        )));
    };
    if parameter.as_str() != binding || *expected_kind != kind {
        return Err(invalid_clipboard(format!(
            "resource '{}' binding does not match protocol authority",
            resource_path.as_str()
        )));
    }
    if parameters.get(parameter)
        != Some(&serde_json::Value::String(
            resource_path.as_str().to_owned(),
        ))
    {
        return Err(invalid_clipboard(format!(
            "resource '{}' is not bound by the clipboard node parameters",
            resource_path.as_str()
        )));
    }
    Ok(())
}

fn validate_resource_path(
    resource_path: &CatalogResourcePath,
    create_args: ResourceBoundCreateArgs,
) -> Result<(), MutationConflict> {
    let path = resource_path.as_str();
    let valid = match create_args {
        ResourceBoundCreateArgs::Function => yss_graph_document::GraphResourcePath::new(path)
            .is_ok_and(|canonical| {
                canonical.as_str() == path && canonical.as_str().starts_with("functions/")
            }),
        ResourceBoundCreateArgs::Variable => path
            .strip_prefix("variables/")
            .and_then(|id| uuid::Uuid::parse_str(id).ok())
            .is_some_and(|id| format!("variables/{id}") == path),
        ResourceBoundCreateArgs::Database => path
            .strip_prefix("databases/")
            .is_some_and(|id| !id.is_empty()),
    };
    if valid {
        Ok(())
    } else {
        Err(invalid_clipboard(format!(
            "resource path '{path}' is malformed for its creation arguments"
        )))
    }
}

fn validate_portable_references(
    registry: &NodeRegistry,
    catalog: &CatalogMutationValidationSnapshot,
    snapshot: &ClipboardSubgraph,
    node_types: &BTreeMap<ClipboardNodeId, NodeTypeId>,
) -> Result<BTreeSet<LocalInstanceKey>, MutationConflict> {
    let mut binding_addresses = BTreeSet::new();
    let mut instance_keys = BTreeSet::new();
    for entry in &snapshot.port_bindings {
        let ClipboardPortRef::Instance {
            local_instance_id, ..
        } = &entry.address.port
        else {
            return Err(invalid_clipboard(
                "clipboard port bindings require instance addresses",
            ));
        };
        validate_local_identity("port instance", local_instance_id.0.as_ref())?;
        if !binding_addresses.insert(entry.address.clone()) {
            return Err(invalid_clipboard(format!(
                "duplicate clipboard port binding at {:?}",
                entry.address
            )));
        }
        let spec = portable_port_spec(&entry.address, node_types, registry)?;
        let compatible = matches!(
            (&spec.instances, &entry.binding),
            (
                PortInstances::UserCreated { .. },
                ClipboardDynamicPortBinding::UserCreated { .. }
            ) | (
                PortInstances::Derived { .. },
                ClipboardDynamicPortBinding::Resolved { .. }
                    | ClipboardDynamicPortBinding::Orphan { .. }
            )
        );
        if !compatible {
            return Err(invalid_clipboard(format!(
                "clipboard binding kind does not match port template '{}'",
                spec.key
            )));
        }
        instance_keys.insert(local_instance_key(&entry.address, node_types, registry)?);
        let binding = DynamicPortBinding::from(entry.binding.clone());
        validate_dynamic_origin(&entry.address, &binding, snapshot, registry, catalog)?;
    }
    validate_instance_cardinality(snapshot, node_types, registry)?;

    let mut state_addresses = BTreeSet::new();
    for entry in &snapshot.input_states {
        if !state_addresses.insert(entry.address.clone()) {
            return Err(invalid_clipboard(format!(
                "duplicate clipboard input state at {:?}",
                entry.address
            )));
        }
        validate_endpoint(&entry.address, node_types, registry, &binding_addresses)?;
    }

    let mut connections = BTreeSet::new();
    for connection in &snapshot.connections {
        validate_endpoint(&connection.output, node_types, registry, &binding_addresses)?;
        validate_endpoint(&connection.input, node_types, registry, &binding_addresses)?;
        if !connections.insert((
            connection.output.clone(),
            connection.input.clone(),
            connection.order.clone(),
        )) {
            return Err(invalid_clipboard(
                "clipboard subgraph contains a duplicate connection",
            ));
        }
    }
    Ok(instance_keys)
}

fn portable_port_spec<'a>(
    address: &ClipboardPortAddress,
    node_types: &BTreeMap<ClipboardNodeId, NodeTypeId>,
    registry: &'a NodeRegistry,
) -> Result<&'a yss_graph_protocol::PortSpec, MutationConflict> {
    let node_type = node_types.get(&address.node_id).ok_or_else(|| {
        invalid_clipboard(format!(
            "clipboard address references missing node '{}'",
            address.node_id.0
        ))
    })?;
    let protocol = registry
        .protocol(node_type)
        .ok_or_else(|| invalid_clipboard(format!("node type '{node_type}' is unavailable")))?;
    let key = match &address.port {
        ClipboardPortRef::Declared { key } => key,
        ClipboardPortRef::Instance { template, .. } => template,
    };
    protocol
        .interface
        .ports
        .iter()
        .find(|spec| &spec.key == key)
        .ok_or_else(|| {
            invalid_clipboard(format!(
                "clipboard address references unknown port '{key}' on node '{}'",
                address.node_id.0
            ))
        })
}

fn validate_endpoint(
    address: &ClipboardPortAddress,
    node_types: &BTreeMap<ClipboardNodeId, NodeTypeId>,
    registry: &NodeRegistry,
    binding_addresses: &BTreeSet<ClipboardPortAddress>,
) -> Result<(), MutationConflict> {
    let spec = portable_port_spec(address, node_types, registry)?;
    match &address.port {
        ClipboardPortRef::Declared { .. } if matches!(spec.instances, PortInstances::Declared) => {
            Ok(())
        }
        ClipboardPortRef::Instance {
            local_instance_id, ..
        } if !local_instance_id.0.is_empty() && binding_addresses.contains(address) => Ok(()),
        ClipboardPortRef::Declared { .. } => Err(invalid_clipboard(format!(
            "port '{}' requires an instance address",
            spec.key
        ))),
        ClipboardPortRef::Instance { .. } => Err(invalid_clipboard(format!(
            "instance port '{}' has no clipboard binding",
            spec.key
        ))),
    }
}

fn validate_instance_cardinality(
    snapshot: &ClipboardSubgraph,
    node_types: &BTreeMap<ClipboardNodeId, NodeTypeId>,
    registry: &NodeRegistry,
) -> Result<(), MutationConflict> {
    for (local_node, node_type) in node_types {
        let protocol = registry
            .protocol(node_type)
            .expect("validated clipboard node protocols remain registered");
        for group in &protocol.interface.member_groups {
            let mut members = BTreeMap::<ClipboardPortInstanceId, BTreeSet<PortKey>>::new();
            for entry in snapshot
                .port_bindings
                .iter()
                .filter(|entry| &entry.address.node_id == local_node)
            {
                let ClipboardPortRef::Instance {
                    template,
                    local_instance_id,
                } = &entry.address.port
                else {
                    continue;
                };
                if group.templates.contains(template) {
                    members
                        .entry(local_instance_id.clone())
                        .or_default()
                        .insert(template.clone());
                }
            }
            let required = group.templates.iter().cloned().collect::<BTreeSet<_>>();
            if members.values().any(|templates| templates != &required)
                || members.len() < usize::from(group.min)
                || group
                    .max
                    .is_some_and(|maximum| members.len() > usize::from(maximum))
            {
                return Err(invalid_clipboard(format!(
                    "clipboard node '{}' has invalid grouped port cardinality",
                    local_node.0
                )));
            }
        }
        for spec in protocol.interface.ports.iter() {
            if protocol
                .interface
                .member_group_for_template(&spec.key)
                .is_some()
            {
                continue;
            }
            let PortInstances::UserCreated { min, max } = spec.instances else {
                continue;
            };
            let count = snapshot
                .port_bindings
                .iter()
                .filter_map(|entry| {
                    if &entry.address.node_id != local_node {
                        return None;
                    }
                    match &entry.address.port {
                        ClipboardPortRef::Instance {
                            template,
                            local_instance_id,
                        } if template == &spec.key => Some(local_instance_id),
                        _ => None,
                    }
                })
                .collect::<BTreeSet<_>>()
                .len();
            if count < usize::from(min) || max.is_some_and(|maximum| count > usize::from(maximum)) {
                return Err(invalid_clipboard(format!(
                    "clipboard node '{}' has invalid cardinality for port '{}'",
                    local_node.0, spec.key
                )));
            }
        }
    }
    Ok(())
}

fn validate_dynamic_origin(
    address: &ClipboardPortAddress,
    binding: &DynamicPortBinding,
    snapshot: &ClipboardSubgraph,
    registry: &NodeRegistry,
    catalog: &CatalogMutationValidationSnapshot,
) -> Result<(), MutationConflict> {
    let DynamicPortBinding::Resolved {
        origin, last_known, ..
    } = binding
    else {
        return Ok(());
    };
    let node = snapshot
        .nodes
        .iter()
        .find(|node| node.local_id == address.node_id)
        .expect("portable addresses reference validated nodes");
    let protocol = registry
        .protocol(creation_node_type(&node.creation))
        .expect("clipboard node protocols were validated");
    let template = match &address.port {
        ClipboardPortRef::Instance { template, .. } => template,
        ClipboardPortRef::Declared { .. } => {
            return Err(invalid_clipboard(
                "resolved dynamic binding requires an instance address",
            ));
        }
    };
    let spec = protocol
        .interface
        .ports
        .iter()
        .find(|spec| &spec.key == template)
        .expect("clipboard port templates were validated");
    let authoritative_type = validate_resolved_dynamic_binding_authority(
        protocol,
        spec,
        &node.parameters,
        origin,
        catalog,
    )
    .map_err(|error| match error {
        MutationConflict::ReferencedResourceUnavailable(message) => {
            MutationConflict::ReferencedResourceUnavailable(message)
        }
        other => invalid_clipboard(other.to_string()),
    })?;
    if last_known.value_type.as_ref() != Some(&authoritative_type) {
        return Err(invalid_clipboard(format!(
            "resolved dynamic binding for template '{}' has forged last-known type",
            spec.key
        )));
    }
    Ok(())
}

fn local_instance_key(
    address: &ClipboardPortAddress,
    node_types: &BTreeMap<ClipboardNodeId, NodeTypeId>,
    registry: &NodeRegistry,
) -> Result<LocalInstanceKey, MutationConflict> {
    let ClipboardPortRef::Instance {
        template,
        local_instance_id,
    } = &address.port
    else {
        return Err(invalid_clipboard("declared port has no local instance key"));
    };
    let node_type = &node_types[&address.node_id];
    let protocol = registry
        .protocol(node_type)
        .expect("validated clipboard node protocols remain registered");
    let scope = protocol
        .interface
        .member_group_for_template(template)
        .and_then(|group| group.templates.first())
        .unwrap_or(template)
        .clone();
    Ok(LocalInstanceKey {
        node_id: address.node_id.clone(),
        scope,
        local_instance_id: local_instance_id.clone(),
    })
}

fn plan_instantiation(
    document: &GraphDocument,
    registry: &NodeRegistry,
    snapshot: &ClipboardSubgraph,
    anchor: NodePosition,
    node_ids: &BTreeMap<ClipboardNodeId, NodeId>,
    instance_ids: &BTreeMap<LocalInstanceKey, PortInstanceId>,
    connection_ids: &[ConnectionId],
) -> Result<GraphDocumentPatch, MutationConflict> {
    let node_types = snapshot
        .nodes
        .iter()
        .map(|node| {
            (
                node.local_id.clone(),
                creation_node_type(&node.creation).clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut operations = snapshot
        .nodes
        .iter()
        .map(|node| GraphDocumentOperation::InsertNode {
            node: DocumentNode {
                id: node_ids[&node.local_id],
                node_type: creation_node_type(&node.creation).clone(),
                position: NodePosition {
                    x: anchor.x + node.relative_position.x,
                    y: anchor.y + node.relative_position.y,
                },
                parameters: node.parameters.clone(),
                user_label: node.user_label.clone(),
            },
        })
        .collect::<Vec<_>>();
    for entry in &snapshot.port_bindings {
        operations.push(GraphDocumentOperation::InsertPortBinding {
            address: instantiate_address(
                &entry.address,
                &node_types,
                registry,
                node_ids,
                instance_ids,
            )?,
            binding: entry.binding.clone().into(),
        });
    }

    let mut staged = document.clone();
    GraphDocumentPatch::new(operations.clone())
        .apply_without_revision(&mut staged)
        .map_err(|error| invalid_clipboard(format!("node and port staging failed: {error}")))?;
    for operation in &operations {
        if let GraphDocumentOperation::InsertPortBinding { address, .. } = operation {
            validate_subgraph_port(&staged, registry, address)
                .map_err(|error| invalid_clipboard(error.to_string()))?;
        }
    }

    for entry in &snapshot.input_states {
        let address = instantiate_address(
            &entry.address,
            &node_types,
            registry,
            node_ids,
            instance_ids,
        )?;
        validate_literal_target(
            &staged,
            registry,
            &address,
            entry.state.literal_override.as_ref(),
        )
        .map_err(|error| invalid_clipboard(error.to_string()))?;
        let operation = GraphDocumentOperation::SetInputState {
            before: staged.input_states.get(&address).cloned(),
            address,
            after: Some(entry.state.clone()),
        };
        GraphDocumentPatch::new(vec![operation.clone()])
            .apply_without_revision(&mut staged)
            .map_err(|error| invalid_clipboard(format!("input state staging failed: {error}")))?;
        operations.push(operation);
    }

    for (index, entry) in snapshot.connections.iter().enumerate() {
        let output =
            instantiate_address(&entry.output, &node_types, registry, node_ids, instance_ids)?;
        let input =
            instantiate_address(&entry.input, &node_types, registry, node_ids, instance_ids)?;
        validate_subgraph_connection(&staged, registry, &output, &input, entry.order.as_ref())
            .map_err(|error| invalid_clipboard(error.to_string()))?;
        let operation = GraphDocumentOperation::InsertConnection {
            connection: DocumentConnection {
                id: connection_ids[index],
                output,
                input,
                order: entry.order.clone(),
            },
        };
        GraphDocumentPatch::new(vec![operation.clone()])
            .apply_without_revision(&mut staged)
            .map_err(|error| invalid_clipboard(format!("connection staging failed: {error}")))?;
        operations.push(operation);
    }
    Ok(GraphDocumentPatch::new(operations))
}

fn instantiate_address(
    address: &ClipboardPortAddress,
    node_types: &BTreeMap<ClipboardNodeId, NodeTypeId>,
    registry: &NodeRegistry,
    node_ids: &BTreeMap<ClipboardNodeId, NodeId>,
    instance_ids: &BTreeMap<LocalInstanceKey, PortInstanceId>,
) -> Result<PortAddress, MutationConflict> {
    let node_id = node_ids[&address.node_id];
    let port = match &address.port {
        ClipboardPortRef::Declared { key } => PortRef::Declared { key: key.clone() },
        ClipboardPortRef::Instance { template, .. } => PortRef::Instance {
            template: template.clone(),
            instance_id: instance_ids[&local_instance_key(address, node_types, registry)?],
        },
    };
    Ok(PortAddress { node_id, port })
}

fn temporary_node_ids<'a>(
    document: &GraphDocument,
    local_ids: impl Iterator<Item = &'a ClipboardNodeId>,
) -> BTreeMap<ClipboardNodeId, NodeId> {
    let mut used = document.nodes.keys().copied().collect::<BTreeSet<_>>();
    local_ids
        .enumerate()
        .map(|(index, local_id)| {
            let mut value = u128::MAX - index as u128;
            let id = loop {
                let candidate = NodeId::from_uuid(uuid::Uuid::from_u128(value));
                if used.insert(candidate) {
                    break candidate;
                }
                value -= 1;
            };
            (local_id.clone(), id)
        })
        .collect()
}

fn temporary_port_instance_ids<'a>(
    document: &GraphDocument,
    keys: impl Iterator<Item = &'a LocalInstanceKey>,
) -> BTreeMap<LocalInstanceKey, PortInstanceId> {
    let mut used = document
        .port_bindings
        .keys()
        .filter_map(|address| match address.port {
            PortRef::Instance { instance_id, .. } => Some(instance_id),
            PortRef::Declared { .. } => None,
        })
        .collect::<BTreeSet<_>>();
    keys.enumerate()
        .map(|(index, key)| {
            let mut value = u128::MAX / 2 - index as u128;
            let id = loop {
                let candidate = PortInstanceId::from_uuid(uuid::Uuid::from_u128(value));
                if used.insert(candidate) {
                    break candidate;
                }
                value -= 1;
            };
            (key.clone(), id)
        })
        .collect()
}

fn temporary_connection_ids(document: &GraphDocument, count: usize) -> Vec<ConnectionId> {
    let mut used = document
        .connections
        .keys()
        .copied()
        .collect::<BTreeSet<_>>();
    (0..count)
        .map(|index| {
            let mut value = u128::MAX / 4 - index as u128;
            loop {
                let candidate = ConnectionId::from_uuid(uuid::Uuid::from_u128(value));
                if used.insert(candidate) {
                    break candidate;
                }
                value -= 1;
            }
        })
        .collect()
}

fn fresh_node_ids<'a>(
    document: &GraphDocument,
    local_ids: impl Iterator<Item = &'a ClipboardNodeId>,
) -> BTreeMap<ClipboardNodeId, NodeId> {
    let mut used = document.nodes.keys().copied().collect::<BTreeSet<_>>();
    local_ids
        .map(|local_id| {
            let id = loop {
                let candidate = NodeId::new();
                if used.insert(candidate) {
                    break candidate;
                }
            };
            (local_id.clone(), id)
        })
        .collect()
}

fn fresh_port_instance_ids<'a>(
    document: &GraphDocument,
    keys: impl Iterator<Item = &'a LocalInstanceKey>,
) -> BTreeMap<LocalInstanceKey, PortInstanceId> {
    let mut used = document
        .port_bindings
        .keys()
        .filter_map(|address| match address.port {
            PortRef::Instance { instance_id, .. } => Some(instance_id),
            PortRef::Declared { .. } => None,
        })
        .collect::<BTreeSet<_>>();
    keys.map(|key| {
        let id = loop {
            let candidate = PortInstanceId::new();
            if used.insert(candidate) {
                break candidate;
            }
        };
        (key.clone(), id)
    })
    .collect()
}

fn fresh_connection_ids(document: &GraphDocument, count: usize) -> Vec<ConnectionId> {
    let mut used = document
        .connections
        .keys()
        .copied()
        .collect::<BTreeSet<_>>();
    (0..count)
        .map(|_| {
            loop {
                let candidate = ConnectionId::new();
                if used.insert(candidate) {
                    break candidate;
                }
            }
        })
        .collect()
}

fn creation_node_type(creation: &ClipboardNodeCreation) -> &NodeTypeId {
    match creation {
        ClipboardNodeCreation::Static { node_type_id }
        | ClipboardNodeCreation::ResourceBound { node_type_id, .. } => node_type_id,
    }
}

fn validate_local_identity(kind: &str, value: &str) -> Result<(), MutationConflict> {
    if value.is_empty() {
        Err(invalid_clipboard(format!(
            "clipboard {kind} identity must not be empty"
        )))
    } else {
        Ok(())
    }
}

fn enforce_insert_limit(name: &str, actual: usize, limit: usize) -> Result<(), MutationConflict> {
    if actual > limit {
        Err(invalid_clipboard(format!(
            "clipboard subgraph {name} limit exceeded ({actual} > {limit})"
        )))
    } else {
        Ok(())
    }
}
