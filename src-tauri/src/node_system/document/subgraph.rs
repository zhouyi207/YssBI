use super::mutation::{
    validate_literal_target, validate_node_scope, validate_parameters_with_registry,
    validate_resolved_dynamic_binding_authority, validate_subgraph_connection,
    validate_subgraph_port,
};
use super::{
    ConnectionId, DocumentConnection, DocumentNode, DynamicMemberLocator, DynamicPortBinding,
    FunctionParameterId, GraphDocument, GraphDocumentOperation, GraphDocumentPatch,
    GraphResourcePath, InputState, LastKnownPortMetadata, MutationConflict, NodeId, NodePosition,
    OrderKey, ParameterValues, PortAddress, PortInstanceId, PortRef, SchemaFieldIdentity,
    SchemaSourceIdentity,
};
use crate::node_system::catalog::{
    CatalogResourcePath, NodeCreationDescriptor, ResourceBoundCreateArgsDto,
    authoritative_static_descriptor,
};
use crate::node_system::protocol::{
    NodeInstanceDisplaySpec, NodeTypeId, ParameterKey, PortInstances, PortKey, ResourceDisplayKind,
    TypeExpr,
};
use crate::node_system::registry::NodeRegistry;
use crate::project::{CatalogMutationResource, CatalogMutationValidationSnapshot};
use serde::de::{DeserializeSeed, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

#[path = "subgraph/clipboard.rs"]
mod clipboard;
pub use clipboard::{
    CLIPBOARD_SUBGRAPH_SCHEMA_VERSION, ClipboardConnectionDto, ClipboardDynamicMemberOriginDto,
    ClipboardDynamicPortBindingDto, ClipboardInputStateDto, ClipboardLastKnownPortMetadataDto,
    ClipboardNodeCreationDto, ClipboardNodeDto, ClipboardNodeId, ClipboardPortAddressDto,
    ClipboardPortBindingDto, ClipboardPortInstanceId, ClipboardPortRefDto, ClipboardSubgraphDto,
    MAX_CLIPBOARD_CONNECTIONS, MAX_CLIPBOARD_INPUT_STATES, MAX_CLIPBOARD_NODES,
    MAX_CLIPBOARD_PARAMETER_BYTES, MAX_CLIPBOARD_PORT_BINDINGS, MAX_CLIPBOARD_SERIALIZED_BYTES,
    MAX_CLIPBOARD_VALUE_DEPTH,
};
pub(crate) use clipboard::{ValidatedClipboardSubgraph, deserialize_clipboard_subgraph};
#[path = "subgraph/instantiate.rs"]
mod instantiate;
pub(crate) use instantiate::instantiate_subgraph;
#[cfg(test)]
pub(crate) use instantiate::instantiate_subgraph_for_test;

pub fn export_subgraph(
    graph_path: &GraphResourcePath,
    document: &GraphDocument,
    registry: &NodeRegistry,
    catalog: &CatalogMutationValidationSnapshot,
    node_ids: Vec<NodeId>,
) -> Result<ClipboardSubgraphDto, MutationConflict> {
    let selected = validate_targets(document, node_ids)?;
    enforce_limit("nodes", selected.len(), MAX_CLIPBOARD_NODES)?;

    let node_ids = selected.iter().copied().collect::<Vec<_>>();
    let local_nodes = node_ids
        .iter()
        .enumerate()
        .map(|(index, node_id)| (*node_id, ClipboardNodeId(format!("node/{index}").into())))
        .collect::<BTreeMap<_, _>>();
    let min_x = node_ids
        .iter()
        .map(|node_id| document.nodes[node_id].position.x)
        .reduce(f64::min)
        .expect("validated targets are non-empty");
    let min_y = node_ids
        .iter()
        .map(|node_id| document.nodes[node_id].position.y)
        .reduce(f64::min)
        .expect("validated targets are non-empty");

    let selected_bindings = document
        .port_bindings
        .iter()
        .filter(|(address, _)| selected.contains(&address.node_id))
        .collect::<Vec<_>>();
    let selected_states = document
        .input_states
        .iter()
        .filter(|(address, _)| selected.contains(&address.node_id))
        .collect::<Vec<_>>();
    let selected_connections = document
        .connections
        .values()
        .filter(|connection| {
            selected.contains(&connection.output.node_id)
                && selected.contains(&connection.input.node_id)
        })
        .collect::<Vec<_>>();
    enforce_limit(
        "port bindings",
        selected_bindings.len(),
        MAX_CLIPBOARD_PORT_BINDINGS,
    )?;
    enforce_limit(
        "input states",
        selected_states.len(),
        MAX_CLIPBOARD_INPUT_STATES,
    )?;
    enforce_limit(
        "connections",
        selected_connections.len(),
        MAX_CLIPBOARD_CONNECTIONS,
    )?;

    let local_instances = local_instance_ids(
        selected_bindings.iter().map(|(address, _)| *address),
        selected_states.iter().map(|(address, _)| *address),
        selected_connections
            .iter()
            .flat_map(|connection| [&connection.output, &connection.input]),
    );

    let mut parameter_bytes = 0usize;
    let mut nodes = Vec::with_capacity(node_ids.len());
    for node_id in node_ids {
        let node = &document.nodes[&node_id];
        validate_parameter_values(&node.parameters, &mut parameter_bytes)?;
        let creation = authoritative_creation(graph_path, node, registry, catalog)?;
        nodes.push(ClipboardNodeDto {
            local_id: local_nodes[&node_id].clone(),
            creation,
            parameters: node.parameters.clone(),
            user_label: node.user_label.clone(),
            relative_position: NodePosition {
                x: node.position.x - min_x,
                y: node.position.y - min_y,
            },
        });
    }

    let mut port_bindings = selected_bindings
        .into_iter()
        .map(|(address, binding)| {
            Ok(ClipboardPortBindingDto {
                address: rewrite_address(address, &local_nodes, &local_instances)?,
                binding: binding.into(),
            })
        })
        .collect::<Result<Vec<_>, MutationConflict>>()?;
    port_bindings.sort_by(|left, right| left.address.cmp(&right.address));

    let mut input_states = selected_states
        .into_iter()
        .map(|(address, state)| {
            Ok(ClipboardInputStateDto {
                address: rewrite_address(address, &local_nodes, &local_instances)?,
                state: state.clone(),
            })
        })
        .collect::<Result<Vec<_>, MutationConflict>>()?;
    input_states.sort_by(|left, right| left.address.cmp(&right.address));

    let mut connections = selected_connections
        .into_iter()
        .map(|connection| {
            Ok(ClipboardConnectionDto {
                output: rewrite_address(&connection.output, &local_nodes, &local_instances)?,
                input: rewrite_address(&connection.input, &local_nodes, &local_instances)?,
                order: connection.order.clone(),
            })
        })
        .collect::<Result<Vec<_>, MutationConflict>>()?;
    connections.sort_by(|left, right| {
        (&left.output, &left.input, &left.order).cmp(&(&right.output, &right.input, &right.order))
    });

    let snapshot = ClipboardSubgraphDto {
        schema_version: CLIPBOARD_SUBGRAPH_SCHEMA_VERSION,
        nodes,
        port_bindings,
        input_states,
        connections,
    };
    let serialized_bytes = serde_json::to_vec(&snapshot)
        .map_err(|error| invalid_export(format!("subgraph serialization failed: {error}")))?
        .len();
    enforce_limit(
        "serialized bytes",
        serialized_bytes,
        MAX_CLIPBOARD_SERIALIZED_BYTES,
    )?;
    Ok(snapshot)
}

pub fn duplicate_subgraph(
    graph_path: &GraphResourcePath,
    document: &GraphDocument,
    registry: &NodeRegistry,
    catalog: &CatalogMutationValidationSnapshot,
    node_ids: Vec<NodeId>,
    offset: NodePosition,
) -> Result<GraphDocumentPatch, MutationConflict> {
    let snapshot = export_subgraph(graph_path, document, registry, catalog, node_ids.clone())?;
    let origin_x = node_ids
        .iter()
        .map(|node_id| document.nodes[node_id].position.x)
        .reduce(f64::min)
        .expect("subgraph export validates non-empty targets");
    let origin_y = node_ids
        .iter()
        .map(|node_id| document.nodes[node_id].position.y)
        .reduce(f64::min)
        .expect("subgraph export validates non-empty targets");
    instantiate_subgraph(
        graph_path,
        document,
        registry,
        catalog,
        ValidatedClipboardSubgraph(snapshot),
        NodePosition {
            x: origin_x + offset.x,
            y: origin_y + offset.y,
        },
    )
}

fn invalid_clipboard(message: impl Into<Box<str>>) -> MutationConflict {
    MutationConflict::ClipboardSubgraphInvalid(message.into())
}

fn unavailable_resource(message: impl Into<Box<str>>) -> MutationConflict {
    MutationConflict::ReferencedResourceUnavailable(message.into())
}

fn validate_targets(
    document: &GraphDocument,
    node_ids: Vec<NodeId>,
) -> Result<BTreeSet<NodeId>, MutationConflict> {
    if node_ids.is_empty() {
        return Err(invalid_export("subgraph export requires at least one node"));
    }
    let selected = node_ids.iter().copied().collect::<BTreeSet<_>>();
    if selected.len() != node_ids.len() {
        return Err(invalid_export(
            "subgraph export contains a duplicate direct target",
        ));
    }
    if let Some(missing) = selected
        .iter()
        .find(|node_id| !document.nodes.contains_key(node_id))
    {
        return Err(invalid_export(format!(
            "subgraph export node '{missing}' does not exist"
        )));
    }
    Ok(selected)
}

fn authoritative_creation(
    graph_path: &GraphResourcePath,
    node: &super::DocumentNode,
    registry: &NodeRegistry,
    catalog: &CatalogMutationValidationSnapshot,
) -> Result<ClipboardNodeCreationDto, MutationConflict> {
    let protocol = registry.protocol(&node.node_type).ok_or_else(|| {
        invalid_export(format!(
            "subgraph export references unknown node type '{}'",
            node.node_type
        ))
    })?;
    if protocol.managed_role.is_some() {
        return Err(invalid_export(format!(
            "managed node '{}' cannot be exported",
            node.id
        )));
    }

    let matches = matching_resources(graph_path, node, catalog)?;
    if matches.len() > 1 {
        return Err(invalid_export(format!(
            "node '{}' matches multiple authoritative resources",
            node.id
        )));
    }
    if let Some((resource_path, create_args)) = matches.into_iter().next() {
        return Ok(ClipboardNodeCreationDto::ResourceBound {
            node_type_id: node.node_type.clone(),
            resource_path,
            create_args,
        });
    }
    if matches!(
        protocol.instance_display,
        NodeInstanceDisplaySpec::ResourceParameter { .. }
    ) {
        return Err(invalid_export(format!(
            "node '{}' has no authoritative catalog resource",
            node.id
        )));
    }
    match authoritative_static_descriptor(registry, protocol) {
        Some(NodeCreationDescriptor::Static { .. })
        | Some(NodeCreationDescriptor::ParameterizedStatic { .. }) => {
            Ok(ClipboardNodeCreationDto::Static {
                node_type_id: node.node_type.clone(),
            })
        }
        _ => Err(invalid_export(format!(
            "node '{}' has no authoritative creation identity",
            node.id
        ))),
    }
}

fn matching_resources(
    graph_path: &GraphResourcePath,
    node: &super::DocumentNode,
    catalog: &CatalogMutationValidationSnapshot,
) -> Result<Vec<(CatalogResourcePath, ResourceBoundCreateArgsDto)>, MutationConflict> {
    let mut matches = Vec::new();
    for (resource_path, resource) in &catalog.resources {
        let (allowed, parameter_binding, create_args, in_scope) = match resource {
            CatalogMutationResource::Function {
                allowed_node_type_id,
                parameter_binding,
                ..
            } => (
                allowed_node_type_id == &node.node_type,
                parameter_binding.as_ref(),
                ResourceBoundCreateArgsDto::Function,
                true,
            ),
            CatalogMutationResource::Variable {
                allowed_node_type_ids,
                parameter_binding,
                scope,
                ..
            } => (
                allowed_node_type_ids.contains(&node.node_type),
                parameter_binding.as_ref(),
                ResourceBoundCreateArgsDto::Variable,
                variable_in_scope(graph_path, scope),
            ),
            CatalogMutationResource::Database {
                allowed_node_type_id,
                parameter_binding,
                ..
            } => (
                allowed_node_type_id == &node.node_type,
                parameter_binding.as_ref(),
                ResourceBoundCreateArgsDto::Database,
                true,
            ),
        };
        if !allowed || !in_scope {
            continue;
        }
        let key = ParameterKey::new(parameter_binding).map_err(|error| {
            invalid_export(format!(
                "catalog resource '{}' has an invalid parameter binding: {error}",
                resource_path.as_str()
            ))
        })?;
        if node.parameters.get(&key)
            == Some(&serde_json::Value::String(
                resource_path.as_str().to_owned(),
            ))
        {
            matches.push((resource_path.clone(), create_args));
        }
    }
    Ok(matches)
}

fn variable_in_scope(
    graph_path: &GraphResourcePath,
    scope: &crate::variable::VariableScope,
) -> bool {
    match scope {
        crate::variable::VariableScope::Global => true,
        crate::variable::VariableScope::Event { event_path } => {
            event_path.as_str() == graph_path.as_str()
        }
        crate::variable::VariableScope::Function { function_path } => {
            function_path.as_str() == graph_path.as_str()
        }
    }
}

fn local_instance_ids<'a>(
    bindings: impl Iterator<Item = &'a PortAddress>,
    states: impl Iterator<Item = &'a PortAddress>,
    connections: impl Iterator<Item = &'a PortAddress>,
) -> BTreeMap<PortInstanceId, ClipboardPortInstanceId> {
    let addresses = bindings
        .chain(states)
        .chain(connections)
        .collect::<BTreeSet<_>>();
    let mut instances = BTreeMap::new();
    for address in addresses {
        if let PortRef::Instance { instance_id, .. } = address.port {
            let next = instances.len();
            instances
                .entry(instance_id)
                .or_insert_with(|| ClipboardPortInstanceId(format!("port/{next}").into()));
        }
    }
    instances
}

fn rewrite_address(
    address: &PortAddress,
    nodes: &BTreeMap<NodeId, ClipboardNodeId>,
    instances: &BTreeMap<PortInstanceId, ClipboardPortInstanceId>,
) -> Result<ClipboardPortAddressDto, MutationConflict> {
    let node_id = nodes
        .get(&address.node_id)
        .cloned()
        .ok_or_else(|| invalid_export("subgraph address references an unselected node"))?;
    let port = match &address.port {
        PortRef::Declared { key } => ClipboardPortRefDto::Declared { key: key.clone() },
        PortRef::Instance {
            template,
            instance_id,
        } => ClipboardPortRefDto::Instance {
            template: template.clone(),
            local_instance_id: instances
                .get(instance_id)
                .cloned()
                .ok_or_else(|| invalid_export("subgraph address has no local port identity"))?,
        },
    };
    Ok(ClipboardPortAddressDto { node_id, port })
}

fn validate_parameter_values(
    parameters: &ParameterValues,
    total_bytes: &mut usize,
) -> Result<(), MutationConflict> {
    for value in parameters.values() {
        if json_depth(value) > MAX_CLIPBOARD_VALUE_DEPTH {
            return Err(invalid_export(
                "subgraph parameter value exceeds depth limit",
            ));
        }
    }
    let bytes = serde_json::to_vec(parameters)
        .map_err(|error| invalid_export(format!("parameter serialization failed: {error}")))?
        .len();
    *total_bytes = total_bytes
        .checked_add(bytes)
        .ok_or_else(|| invalid_export("subgraph parameter size overflow"))?;
    enforce_limit(
        "parameter bytes",
        *total_bytes,
        MAX_CLIPBOARD_PARAMETER_BYTES,
    )
}

fn json_depth(value: &serde_json::Value) -> usize {
    match value {
        serde_json::Value::Array(values) => {
            1 + values.iter().map(json_depth).max().unwrap_or_default()
        }
        serde_json::Value::Object(values) => {
            1 + values.values().map(json_depth).max().unwrap_or_default()
        }
        _ => 1,
    }
}

fn enforce_limit(name: &str, actual: usize, limit: usize) -> Result<(), MutationConflict> {
    if actual > limit {
        Err(invalid_export(format!(
            "subgraph export {name} limit exceeded ({actual} > {limit})"
        )))
    } else {
        Ok(())
    }
}

fn invalid_export(message: impl Into<Box<str>>) -> MutationConflict {
    MutationConflict::InvalidEditorMutation(message.into())
}
