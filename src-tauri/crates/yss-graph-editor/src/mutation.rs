use yss_graph_catalog::reroute_node_type;
use yss_graph_catalog::{CatalogResourcePath, NodeCreation, ResourceBoundCreateArgs};
use yss_graph_document::{
    ConnectionId, DocumentConnection, DocumentNode, DynamicMemberLocator, DynamicPortBinding,
    GraphDocument, GraphResourceKind, GraphResourcePath, InputState, NodeId, NodePosition,
    OrderKey, ParameterValues, PortAddress, PortInstanceId, PortRef, TypedValue,
};
use yss_graph_document_edit::{
    DocumentError, GraphDocumentOperation, GraphDocumentPatch, apply_graph_document_patch,
    port_member_group_state, user_created_port_instance_count,
};
use yss_graph_protocol::{
    ConnectionsPerPort, LiteralPolicy, NodeProtocol, NodeScope, NodeTypeId, PortDirection,
    PortInstances, PortKey, PortMemberGroupSpec, PortSpec,
};
use yss_graph_registry::NodeRegistry;

use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

#[path = "mutation/connection.rs"]
mod connection;
use connection::{
    connect_operations, move_connection_operations, normalize_editor_literal_target,
    resolve_mutation_port,
};
pub(super) use connection::{
    validate_literal_target, validate_resolved_dynamic_binding_authority,
    validate_subgraph_connection, validate_subgraph_port,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum EditorMutationErrorCode {
    GraphPortNotFound,
    GraphNodeNotFound,
    GraphConnectionNotFound,
    GraphPortOrphan,
    GraphConnectionDirectionMismatch,
    GraphConnectionTypeMismatch,
    GraphConnectionTypeUnavailable,
    GraphConnectionTypeUnresolved,
    GraphConnectionLimitReached,
    GraphConnectionOrderRequired,
    GraphConnectionOrderForbidden,
    GraphConnectionAlreadyExists,
    GraphConnectionMoveSourceEmpty,
    GraphConnectionMoveSamePort,
    GraphMutationEmptyTargets,
    GraphMutationDuplicateTarget,
    GraphManagedNodeDeleteForbidden,
}

impl EditorMutationErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GraphPortNotFound => "graph_port_not_found",
            Self::GraphNodeNotFound => "graph_node_not_found",
            Self::GraphConnectionNotFound => "graph_connection_not_found",
            Self::GraphPortOrphan => "graph_port_orphan",
            Self::GraphConnectionDirectionMismatch => "graph_connection_direction_mismatch",
            Self::GraphConnectionTypeMismatch => "graph_connection_type_mismatch",
            Self::GraphConnectionTypeUnavailable => "graph_connection_type_unavailable",
            Self::GraphConnectionTypeUnresolved => "graph_connection_type_unresolved",
            Self::GraphConnectionLimitReached => "graph_connection_limit_reached",
            Self::GraphConnectionOrderRequired => "graph_connection_order_required",
            Self::GraphConnectionOrderForbidden => "graph_connection_order_forbidden",
            Self::GraphConnectionAlreadyExists => "graph_connection_already_exists",
            Self::GraphConnectionMoveSourceEmpty => "graph_connection_move_source_empty",
            Self::GraphConnectionMoveSamePort => "graph_connection_move_same_port",
            Self::GraphMutationEmptyTargets => "graph_mutation_empty_targets",
            Self::GraphMutationDuplicateTarget => "graph_mutation_duplicate_target",
            Self::GraphManagedNodeDeleteForbidden => "graph_managed_node_delete_forbidden",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorMutationError {
    pub code: EditorMutationErrorCode,
    pub detail: Box<str>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MutationConflict {
    CatalogResourceStale(Box<str>),
    CatalogDescriptorInvalid(Box<str>),
    ClipboardSubgraphInvalid(Box<str>),
    ReferencedResourceUnavailable(Box<str>),
    Editor(EditorMutationError),
    InvalidEditorMutation(Box<str>),
    RegistryInvariant(Box<str>),
    Document(DocumentError),
}

impl MutationConflict {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::CatalogResourceStale(_) => "catalog_resource_stale",
            Self::CatalogDescriptorInvalid(_) => "catalog_descriptor_invalid",
            Self::ClipboardSubgraphInvalid(_) => "clipboard_subgraph_invalid",
            Self::ReferencedResourceUnavailable(_) => "referenced_resource_unavailable",
            Self::Editor(error) => error.code.as_str(),
            _ => "mutation_conflict",
        }
    }
}

impl fmt::Display for MutationConflict {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CatalogResourceStale(message)
            | Self::CatalogDescriptorInvalid(message)
            | Self::ClipboardSubgraphInvalid(message)
            | Self::ReferencedResourceUnavailable(message)
            | Self::InvalidEditorMutation(message) => formatter.write_str(message),
            Self::Editor(error) => formatter.write_str(&error.detail),
            Self::RegistryInvariant(message) => {
                write!(formatter, "graph registry invariant failed: {message}")
            }
            Self::Document(source) => write!(formatter, "mutation patch failed: {source}"),
        }
    }
}

impl std::error::Error for MutationConflict {}

impl From<DocumentError> for MutationConflict {
    fn from(source: DocumentError) -> Self {
        Self::Document(source)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum EditorGraphMutation {
    CreateNode {
        descriptor: NodeCreation,
        position: NodePosition,
        user_label: Option<String>,
        connect_from: Option<PortAddress>,
    },
    DeleteNodes {
        node_ids: Vec<NodeId>,
    },
    SetParameters {
        node_id: NodeId,
        parameters: ParameterValues,
    },
    MoveNodes {
        positions: Vec<NodePositionMutation>,
    },
    Connect {
        output: PortAddress,
        input: PortAddress,
        order: Option<OrderKey>,
    },
    MoveConnections {
        source: PortAddress,
        target: PortAddress,
    },
    DisconnectConnections {
        connection_ids: Vec<ConnectionId>,
    },
    InsertReroute {
        connection_id: ConnectionId,
        position: NodePosition,
    },
    DisconnectPort {
        address: PortAddress,
    },
    DisconnectNode {
        node_id: NodeId,
    },
    SetLiteral {
        address: PortAddress,
        literal: Option<TypedValue>,
    },
    AddPortInstance {
        node_id: NodeId,
        template_key: PortKey,
        order: Option<OrderKey>,
    },
    RemovePortInstance {
        address: PortAddress,
    },
    DuplicateSubgraph {
        node_ids: Vec<NodeId>,
        offset: NodePosition,
    },
    InsertSubgraph {
        snapshot: crate::ClipboardSubgraph,
        anchor: NodePosition,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct NodePositionMutation {
    pub node_id: NodeId,
    pub position: NodePosition,
}

impl EditorGraphMutation {
    pub fn into_patch(
        self,
        graph_path: &GraphResourcePath,
        document: &GraphDocument,
        registry: &NodeRegistry,
    ) -> Result<GraphDocumentPatch, MutationConflict> {
        self.into_patch_with_catalog_snapshot(graph_path, document, registry, None)
    }

    pub fn into_patch_with_catalog_snapshot(
        self,
        graph_path: &GraphResourcePath,
        document: &GraphDocument,
        registry: &NodeRegistry,
        catalog_validation: Option<&crate::compatibility::CatalogMutationValidationSnapshot>,
    ) -> Result<GraphDocumentPatch, MutationConflict> {
        let operations = match self {
            Self::CreateNode {
                descriptor,
                position,
                user_label,
                connect_from,
            } => {
                validate_position(position)?;
                let connection_descriptor = descriptor.clone();
                let (node_type_id, protocol, parameters, resource_bound, allow_missing_parameters) =
                    match descriptor {
                        descriptor @ NodeCreation::Static { .. }
                        | descriptor @ NodeCreation::ParameterizedStatic { .. } => {
                            let node_type_id = match &descriptor {
                                NodeCreation::Static { node_type_id }
                                | NodeCreation::ParameterizedStatic { node_type_id, .. } => {
                                    node_type_id.clone()
                                }
                                NodeCreation::ResourceBound { .. } => unreachable!(),
                            };
                            let protocol = registry.protocol(&node_type_id).ok_or_else(|| {
                                catalog_descriptor_invalid(format!(
                                    "unknown node type '{node_type_id}'"
                                ))
                            })?;
                            let authoritative = yss_graph_catalog::authoritative_static_descriptor(
                                registry, protocol,
                            );
                            if authoritative.as_ref() != Some(&descriptor) {
                                return Err(catalog_descriptor_invalid(
                                    "catalog creation descriptor does not match registry authority",
                                ));
                            }
                            let allow_missing =
                                matches!(descriptor, NodeCreation::ParameterizedStatic { .. });
                            (
                                node_type_id,
                                protocol,
                                ParameterValues::new(),
                                false,
                                allow_missing,
                            )
                        }
                        NodeCreation::ResourceBound {
                            node_type_id,
                            resource_path,
                            resource_revision,
                            create_args,
                        } => {
                            let protocol = registry.protocol(&node_type_id).ok_or_else(|| {
                                catalog_descriptor_invalid(format!(
                                    "unknown node type '{node_type_id}'"
                                ))
                            })?;
                            let parameters = materialize_resource_descriptor(
                                graph_path,
                                protocol,
                                &resource_path,
                                resource_revision,
                                create_args,
                                catalog_validation.ok_or_else(|| {
                                    catalog_resource_stale(
                                        "resource validation snapshot is unavailable",
                                    )
                                })?,
                            )?;
                            (node_type_id, protocol, parameters, true, false)
                        }
                    };
                if resource_bound {
                    validate_node_scope(graph_path, protocol)
                        .map_err(catalog_descriptor_validation_error)?;
                    validate_parameters_with_registry(registry, protocol, &parameters)
                        .map_err(catalog_descriptor_validation_error)?;
                } else {
                    validate_node_scope(graph_path, protocol)?;
                    if !allow_missing_parameters {
                        validate_parameters_with_registry(registry, protocol, &parameters)?;
                    }
                }
                let mut operations = create_node_operations(
                    protocol,
                    node_type_id,
                    position,
                    parameters,
                    user_label,
                );
                if let Some(connect_from) = connect_from {
                    let source_address = connect_from;
                    let resources = catalog_validation.ok_or_else(|| {
                        invalid_editor_mutation("catalog compatibility snapshot is unavailable")
                    })?;
                    let source = crate::compatibility::source_port(
                        document,
                        registry,
                        resources,
                        source_address,
                    )
                    .map_err(MutationConflict::Editor)?;
                    let candidate = crate::compatibility::connection_candidates(
                        graph_path,
                        &connection_descriptor,
                        registry,
                        resources,
                        &source,
                    )
                    .map_err(invalid_editor_mutation)?
                    .into_iter()
                    .next()
                    .expect("compatible candidate lookup returns a non-empty result");
                    append_atomic_connection(
                        document,
                        registry,
                        resources,
                        &mut operations,
                        &source,
                        candidate,
                    )?;
                }
                operations
            }
            Self::DeleteNodes { node_ids } => {
                delete_editor_node_operations(document, registry, node_ids)?
            }
            Self::SetParameters {
                node_id,
                parameters,
            } => {
                let before = document.nodes.get(&node_id).cloned().ok_or_else(|| {
                    editor_error(
                        EditorMutationErrorCode::GraphNodeNotFound,
                        format!("node '{node_id}' does not exist"),
                    )
                })?;
                let protocol = registry.protocol(&before.node_type).ok_or_else(|| {
                    invalid_editor_mutation(format!("unknown node type '{}'", before.node_type))
                })?;
                if protocol.managed_role.is_some() {
                    return Err(invalid_editor_mutation(
                        "managed node parameters cannot be edited",
                    ));
                }
                validate_parameters_with_registry(registry, protocol, &parameters)?;
                let mut after = before.clone();
                after.parameters = parameters;
                vec![GraphDocumentOperation::UpdateNode { before, after }]
            }
            Self::MoveNodes { positions } => move_node_operations(document, positions)?,
            Self::Connect {
                output,
                input,
                order,
            } => connect_operations(document, registry, catalog_validation, output, input, order)?,
            Self::MoveConnections { source, target } => {
                move_connection_operations(document, registry, catalog_validation, source, target)?
            }
            Self::DisconnectConnections { connection_ids } => {
                validate_direct_targets(&connection_ids)?;
                disconnect_connection_operations(document, connection_ids)?
            }
            Self::InsertReroute {
                connection_id,
                position,
            } => insert_reroute_operations(document, registry, connection_id, position)?,
            Self::DisconnectPort { address } => {
                resolve_mutation_port(document, registry, &address)?;
                let connection_ids = document.connections.values().filter_map(|connection| {
                    (connection.output == address || connection.input == address)
                        .then_some(connection.id)
                });
                disconnect_connection_operations(document, connection_ids)?
            }
            Self::DisconnectNode { node_id } => {
                if !document.nodes.contains_key(&node_id) {
                    return Err(editor_error(
                        EditorMutationErrorCode::GraphNodeNotFound,
                        format!("node '{node_id}' does not exist"),
                    ));
                }
                let connection_ids = document.connections.values().filter_map(|connection| {
                    (connection.output.node_id == node_id || connection.input.node_id == node_id)
                        .then_some(connection.id)
                });
                disconnect_connection_operations(document, connection_ids)?
            }
            Self::SetLiteral { address, literal } => {
                let literal = normalize_editor_literal_target(
                    document,
                    registry,
                    &address,
                    literal.as_ref(),
                )?;
                let before = document.input_states.get(&address).cloned();
                vec![GraphDocumentOperation::SetInputState {
                    address,
                    before,
                    after: literal.map(|value| InputState {
                        literal_override: Some(value),
                    }),
                }]
            }
            Self::AddPortInstance {
                node_id,
                template_key,
                order,
            } => add_port_instance_operations(document, registry, node_id, template_key, order)?,
            Self::RemovePortInstance { address } => {
                remove_port_instance_operations(document, registry, address)?
            }
            Self::DuplicateSubgraph { node_ids, offset } => {
                return crate::subgraph::duplicate_subgraph(
                    graph_path,
                    document,
                    registry,
                    catalog_validation.ok_or_else(|| {
                        catalog_resource_stale("subgraph catalog snapshot is unavailable")
                    })?,
                    node_ids,
                    offset,
                );
            }
            Self::InsertSubgraph { snapshot, anchor } => {
                return crate::subgraph::instantiate_subgraph(
                    graph_path,
                    document,
                    registry,
                    catalog_validation.ok_or_else(|| {
                        catalog_resource_stale("subgraph catalog snapshot is unavailable")
                    })?,
                    crate::subgraph::ValidatedClipboardSubgraph(snapshot),
                    anchor,
                );
            }
        };
        Ok(GraphDocumentPatch::new(operations))
    }
}

fn editor_error(code: EditorMutationErrorCode, detail: impl Into<Box<str>>) -> MutationConflict {
    MutationConflict::Editor(EditorMutationError {
        code,
        detail: detail.into(),
    })
}

fn invalid_editor_mutation(message: impl Into<Box<str>>) -> MutationConflict {
    MutationConflict::InvalidEditorMutation(message.into())
}

fn catalog_resource_stale(message: impl Into<Box<str>>) -> MutationConflict {
    MutationConflict::CatalogResourceStale(message.into())
}

fn catalog_descriptor_invalid(message: impl Into<Box<str>>) -> MutationConflict {
    MutationConflict::CatalogDescriptorInvalid(message.into())
}

fn catalog_descriptor_validation_error(error: MutationConflict) -> MutationConflict {
    catalog_descriptor_invalid(error.to_string())
}

fn materialize_resource_descriptor(
    graph_path: &GraphResourcePath,
    protocol: &NodeProtocol,
    resource_path: &CatalogResourcePath,
    resource_revision: u64,
    create_args: ResourceBoundCreateArgs,
    snapshot: &crate::compatibility::CatalogMutationValidationSnapshot,
) -> Result<ParameterValues, MutationConflict> {
    validate_resource_path(resource_path, create_args)?;
    let resource = snapshot.resources.get(resource_path).ok_or_else(|| {
        catalog_resource_stale(format!(
            "catalog resource '{}' is unavailable",
            resource_path.as_str()
        ))
    })?;
    if resource.create_args() != create_args {
        return Err(catalog_descriptor_invalid(
            "resource descriptor create arguments do not match the resource kind",
        ));
    }
    if resource.revision() != resource_revision {
        return Err(catalog_resource_stale(format!(
            "catalog resource '{}' revision is stale",
            resource_path.as_str()
        )));
    }
    if let Some(scope) = resource.variable_scope() {
        validate_variable_scope(graph_path, scope)?;
    }
    let binding = crate::compatibility::resource_parameter(protocol, create_args)
        .map_err(catalog_descriptor_invalid)?;
    Ok(BTreeMap::from([(
        binding.clone(),
        serde_json::Value::String(resource_path.as_str().to_owned()),
    )]))
}

fn validate_resource_path(
    resource_path: &CatalogResourcePath,
    create_args: ResourceBoundCreateArgs,
) -> Result<(), MutationConflict> {
    let path = resource_path.as_str();
    if crate::compatibility::resource_path_is_valid(resource_path, create_args) {
        Ok(())
    } else {
        Err(catalog_descriptor_invalid(format!(
            "catalog resource path '{path}' is malformed for its create arguments"
        )))
    }
}

fn validate_variable_scope(
    graph_path: &GraphResourcePath,
    scope: &yss_variable_contract::VariableScope,
) -> Result<(), MutationConflict> {
    if crate::compatibility::variable_in_scope(graph_path, scope) {
        Ok(())
    } else {
        Err(catalog_descriptor_invalid(format!(
            "variable resource is out of scope for graph '{}'",
            graph_path.as_str()
        )))
    }
}

fn validate_direct_targets<T: Ord + Copy>(targets: &[T]) -> Result<BTreeSet<T>, MutationConflict> {
    if targets.is_empty() {
        return Err(editor_error(
            EditorMutationErrorCode::GraphMutationEmptyTargets,
            "graph mutation requires at least one target",
        ));
    }
    let selected = targets.iter().copied().collect::<BTreeSet<_>>();
    if selected.len() != targets.len() {
        return Err(editor_error(
            EditorMutationErrorCode::GraphMutationDuplicateTarget,
            "graph mutation contains a duplicate direct target",
        ));
    }
    Ok(selected)
}

fn delete_editor_node_operations(
    document: &GraphDocument,
    registry: &NodeRegistry,
    node_ids: Vec<NodeId>,
) -> Result<Vec<GraphDocumentOperation>, MutationConflict> {
    let selected = validate_direct_targets(&node_ids)?;
    for node_id in &selected {
        let node = document.nodes.get(node_id).ok_or_else(|| {
            editor_error(
                EditorMutationErrorCode::GraphNodeNotFound,
                format!("node '{node_id}' does not exist"),
            )
        })?;
        let protocol = registry.protocol(&node.node_type).ok_or_else(|| {
            invalid_editor_mutation(format!("unknown node type '{}'", node.node_type))
        })?;
        if protocol.managed_role.is_some() {
            return Err(editor_error(
                EditorMutationErrorCode::GraphManagedNodeDeleteForbidden,
                format!(
                    "managed node '{}' cannot be deleted by an editor mutation",
                    node.node_type
                ),
            ));
        }
    }

    let connection_ids = document.connections.values().filter_map(|connection| {
        (selected.contains(&connection.output.node_id)
            || selected.contains(&connection.input.node_id))
        .then_some(connection.id)
    });
    let mut operations = disconnect_connection_operations(document, connection_ids)?;
    operations.extend(
        document
            .input_states
            .iter()
            .filter(|(address, _)| selected.contains(&address.node_id))
            .map(|(address, state)| GraphDocumentOperation::SetInputState {
                address: address.clone(),
                before: Some(state.clone()),
                after: None,
            }),
    );
    operations.extend(
        document
            .port_bindings
            .iter()
            .filter(|(address, _)| selected.contains(&address.node_id))
            .map(
                |(address, binding)| GraphDocumentOperation::RemovePortBinding {
                    address: address.clone(),
                    binding: binding.clone(),
                },
            ),
    );
    operations.extend(
        selected
            .into_iter()
            .map(|node_id| GraphDocumentOperation::RemoveNode {
                node: document.nodes[&node_id].clone(),
            }),
    );
    Ok(operations)
}

fn insert_reroute_operations(
    document: &GraphDocument,
    registry: &NodeRegistry,
    connection_id: ConnectionId,
    position: NodePosition,
) -> Result<Vec<GraphDocumentOperation>, MutationConflict> {
    insert_reroute_operations_with_allocators(
        document,
        registry,
        connection_id,
        position,
        &NodeId::new,
        &ConnectionId::new,
    )
}

fn insert_reroute_operations_with_allocators(
    document: &GraphDocument,
    registry: &NodeRegistry,
    connection_id: ConnectionId,
    position: NodePosition,
    allocate_node_id: &dyn Fn() -> NodeId,
    allocate_connection_id: &dyn Fn() -> ConnectionId,
) -> Result<Vec<GraphDocumentOperation>, MutationConflict> {
    validate_position(position)?;
    let original = document
        .connections
        .get(&connection_id)
        .cloned()
        .ok_or(DocumentError::ConnectionNotFound(connection_id))?;
    let output = resolve_mutation_port(document, registry, &original.output)?;
    let input = resolve_mutation_port(document, registry, &original.input)?;
    if matches!(output.binding, Some(DynamicPortBinding::Orphan { .. }))
        || matches!(input.binding, Some(DynamicPortBinding::Orphan { .. }))
    {
        return Err(invalid_editor_mutation(
            "reroute endpoints must not be orphaned",
        ));
    }
    if output.spec.direction != PortDirection::Output
        || input.spec.direction != PortDirection::Input
    {
        return Err(invalid_editor_mutation(
            "reroute connection endpoints have invalid directions",
        ));
    }
    let reroute_type = reroute_node_type();
    let registered = registry.get(&reroute_type).ok_or_else(|| {
        invalid_editor_mutation(format!("unknown reroute node type '{reroute_type}'"))
    })?;
    let contract = yss_graph_catalog::validate_reroute_protocol_contract(registered)
        .map_err(|detail| MutationConflict::RegistryInvariant(detail.into()))?;

    let reroute_id = allocate_node_id();
    let source_connection_id = allocate_connection_id();
    let target_connection_id = allocate_connection_id();
    let operations = vec![
        GraphDocumentOperation::RemoveConnection {
            connection: original.clone(),
        },
        GraphDocumentOperation::InsertNode {
            node: DocumentNode {
                id: reroute_id,
                node_type: reroute_type,
                position,
                parameters: ParameterValues::new(),
                user_label: None,
            },
        },
        GraphDocumentOperation::InsertConnection {
            connection: DocumentConnection {
                id: source_connection_id,
                output: original.output.clone(),
                input: PortAddress::declared(reroute_id, contract.input_key),
                order: None,
            },
        },
        GraphDocumentOperation::InsertConnection {
            connection: DocumentConnection {
                id: target_connection_id,
                output: PortAddress::declared(reroute_id, contract.output_key),
                input: original.input.clone(),
                order: original.order.clone(),
            },
        },
    ];
    let mut staged = document.clone();
    apply_graph_document_patch(&mut staged, &GraphDocumentPatch::new(operations.clone()))?;
    Ok(operations)
}

fn disconnect_connection_operations(
    document: &GraphDocument,
    connection_ids: impl IntoIterator<Item = ConnectionId>,
) -> Result<Vec<GraphDocumentOperation>, MutationConflict> {
    let connection_ids = connection_ids.into_iter().collect::<BTreeSet<_>>();
    let connections = connection_ids
        .into_iter()
        .map(|connection_id| {
            document
                .connections
                .get(&connection_id)
                .cloned()
                .ok_or_else(|| {
                    editor_error(
                        EditorMutationErrorCode::GraphConnectionNotFound,
                        format!("connection '{connection_id}' does not exist"),
                    )
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(connections
        .into_iter()
        .map(|connection| GraphDocumentOperation::RemoveConnection { connection })
        .collect())
}

pub(super) fn validate_node_scope(
    graph_path: &GraphResourcePath,
    protocol: &NodeProtocol,
) -> Result<(), MutationConflict> {
    let allowed = match protocol.scope {
        NodeScope::Any => true,
        NodeScope::Function => graph_path.kind() == GraphResourceKind::Function,
    };
    if !allowed {
        Err(invalid_editor_mutation(format!(
            "node scope {:?} is invalid for graph '{}'",
            protocol.scope,
            graph_path.as_str()
        )))
    } else {
        Ok(())
    }
}

fn append_atomic_connection(
    document: &GraphDocument,
    registry: &NodeRegistry,
    catalog: &crate::compatibility::CatalogMutationValidationSnapshot,
    operations: &mut Vec<GraphDocumentOperation>,
    source: &crate::compatibility::SourcePort,
    candidate: crate::compatibility::CandidatePort,
) -> Result<(), MutationConflict> {
    let node_id = operations
        .iter()
        .find_map(|operation| match operation {
            GraphDocumentOperation::InsertNode { node } => Some(node.id),
            _ => None,
        })
        .expect("create node operations always begin with node insertion");
    let candidate_address = if let Some(dynamic) = candidate.dynamic {
        let address =
            PortAddress::instance(node_id, candidate.template.clone(), PortInstanceId::new());
        operations.push(GraphDocumentOperation::InsertPortBinding {
            address: address.clone(),
            binding: DynamicPortBinding::Resolved {
                origin: dynamic.origin,
                order: dynamic.order,
                last_known: dynamic.last_known,
            },
        });
        address
    } else if let Some(address) = operations.iter().find_map(|operation| match operation {
        GraphDocumentOperation::InsertPortBinding { address, .. }
            if address.node_id == node_id
                && matches!(
                    &address.port,
                    PortRef::Instance { template, .. } if *template == candidate.template
                ) =>
        {
            Some(address.clone())
        }
        _ => None,
    }) {
        address
    } else {
        PortAddress::declared(node_id, candidate.template.clone())
    };
    let (output, input, input_connections) = match source.direction {
        PortDirection::Output => (
            source.address.clone(),
            candidate_address,
            candidate.connections,
        ),
        PortDirection::Input => {
            let source_port = resolve_mutation_port(document, registry, &source.address)?;
            (
                candidate_address,
                source.address.clone(),
                source_port.spec.connections,
            )
        }
    };
    let order = match input_connections {
        ConnectionsPerPort::Multiple { ordered: true, .. } => {
            Some(OrderKey::new(format!("{:05}", document.connections.len())))
        }
        ConnectionsPerPort::Single | ConnectionsPerPort::Multiple { ordered: false, .. } => None,
    };
    let mut staged = document.clone();
    apply_graph_document_patch(&mut staged, &GraphDocumentPatch::new(operations.clone()))?;
    operations.extend(connect_operations(
        &staged,
        registry,
        Some(catalog),
        output,
        input,
        order,
    )?);
    Ok(())
}

pub(super) fn create_node_operations(
    protocol: &NodeProtocol,
    node_type: NodeTypeId,
    position: NodePosition,
    parameters: ParameterValues,
    user_label: Option<String>,
) -> Vec<GraphDocumentOperation> {
    let node_id = NodeId::new();
    let mut operations = vec![GraphDocumentOperation::InsertNode {
        node: DocumentNode {
            id: node_id,
            node_type,
            position,
            parameters,
            user_label,
        },
    }];
    for group in &protocol.interface.member_groups {
        for index in 0..group.min {
            let instance_id = PortInstanceId::new();
            let order = OrderKey::new(format!("{index:05}"));
            for template in &group.templates {
                operations.push(GraphDocumentOperation::InsertPortBinding {
                    address: PortAddress::instance(node_id, template.clone(), instance_id),
                    binding: DynamicPortBinding::UserCreated {
                        order: order.clone(),
                    },
                });
            }
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
        let PortInstances::UserCreated { min, .. } = spec.instances else {
            continue;
        };
        for index in 0..min {
            let instance_id = PortInstanceId::new();
            operations.push(GraphDocumentOperation::InsertPortBinding {
                address: PortAddress::instance(node_id, spec.key.clone(), instance_id),
                binding: DynamicPortBinding::UserCreated {
                    order: OrderKey::new(format!("{index:05}")),
                },
            });
        }
    }
    operations
}

fn validate_position(position: NodePosition) -> Result<(), MutationConflict> {
    if position.x.is_finite() && position.y.is_finite() {
        Ok(())
    } else {
        Err(invalid_editor_mutation("node position must be finite"))
    }
}

pub(super) fn validate_parameters_with_registry(
    registry: &NodeRegistry,
    protocol: &NodeProtocol,
    parameters: &ParameterValues,
) -> Result<(), MutationConflict> {
    let nominal = |type_id: &yss_graph_protocol::TypeId, value: &serde_json::Value| {
        registry.validate_nominal_parameter(type_id, value)
    };
    validate_shared_parameters(protocol, parameters, &nominal)
}

fn validate_shared_parameters(
    protocol: &NodeProtocol,
    parameters: &ParameterValues,
    nominal: &impl yss_graph_protocol::validation::NominalParameterValidator,
) -> Result<(), MutationConflict> {
    let Some(issue) = yss_graph_protocol::validate_parameter_values(protocol, parameters, nominal)
        .into_iter()
        .next()
    else {
        return Ok(());
    };
    use yss_graph_protocol::ParameterIssueKind;
    let detail = match issue.kind {
        ParameterIssueKind::Unknown => format!(
            "unknown parameter '{}' for node type '{}'",
            issue.key, protocol.type_id
        ),
        ParameterIssueKind::Required => {
            format!("required parameter '{}' is missing or null", issue.key)
        }
        ParameterIssueKind::InvalidType => {
            format!("parameter '{}' does not match its declared type", issue.key)
        }
        ParameterIssueKind::Constraint => {
            format!(
                "parameter '{}' violates its protocol constraints",
                issue.key
            )
        }
        ParameterIssueKind::InvalidNominal(detail) => {
            format!("parameter '{}' is invalid: {detail}", issue.key)
        }
        ParameterIssueKind::InvalidResourceId => {
            format!("parameter '{}' is not a valid resource id", issue.key)
        }
    };
    Err(invalid_editor_mutation(detail))
}

fn move_node_operations(
    document: &GraphDocument,
    positions: Vec<NodePositionMutation>,
) -> Result<Vec<GraphDocumentOperation>, MutationConflict> {
    if positions.is_empty() {
        return Err(editor_error(
            EditorMutationErrorCode::GraphMutationEmptyTargets,
            "node move requires at least one target",
        ));
    }
    let mut targets = BTreeMap::new();
    for target in positions {
        validate_position(target.position)?;
        if targets.contains_key(&target.node_id) {
            return Err(editor_error(
                EditorMutationErrorCode::GraphMutationDuplicateTarget,
                format!("node '{}' appears more than once in a move", target.node_id),
            ));
        }
        if !document.nodes.contains_key(&target.node_id) {
            return Err(editor_error(
                EditorMutationErrorCode::GraphNodeNotFound,
                format!("node '{}' does not exist", target.node_id),
            ));
        }
        targets.insert(target.node_id, target.position);
    }
    Ok(targets
        .into_iter()
        .map(|(node_id, position)| {
            let before = document.nodes[&node_id].clone();
            let mut after = before.clone();
            after.position = position;
            GraphDocumentOperation::UpdateNode { before, after }
        })
        .collect())
}

fn add_port_instance_operations(
    document: &GraphDocument,
    registry: &NodeRegistry,
    node_id: NodeId,
    template_key: PortKey,
    order: Option<OrderKey>,
) -> Result<Vec<GraphDocumentOperation>, MutationConflict> {
    let contract = user_created_port_contract(document, registry, node_id, &template_key)?;
    let (templates, count, max) = if let Some(group) = contract.group {
        let state = port_member_group_state(node_id, group, document.port_bindings.iter());
        (group.templates.as_ref(), state.complete_count(), group.max)
    } else {
        let PortInstances::UserCreated { max, .. } = contract.spec.instances else {
            unreachable!("user_created_port_contract guarantees the instance policy")
        };
        (
            std::slice::from_ref(&contract.spec.key),
            user_created_port_instance_count(node_id, &template_key, document.port_bindings.iter()),
            max,
        )
    };
    if max.is_some_and(|maximum| count >= usize::from(maximum)) {
        return Err(invalid_editor_mutation(format!(
            "port member for template '{template_key}' has reached its maximum instance count"
        )));
    }
    let instance_id = PortInstanceId::new();
    let order = order.unwrap_or_else(|| OrderKey::new(instance_id.to_string()));
    Ok(templates
        .iter()
        .map(|template| GraphDocumentOperation::InsertPortBinding {
            address: PortAddress::instance(node_id, template.clone(), instance_id),
            binding: DynamicPortBinding::UserCreated {
                order: order.clone(),
            },
        })
        .collect())
}

fn remove_port_instance_operations(
    document: &GraphDocument,
    registry: &NodeRegistry,
    address: PortAddress,
) -> Result<Vec<GraphDocumentOperation>, MutationConflict> {
    let (template, instance_id) = match &address.port {
        PortRef::Instance {
            template,
            instance_id,
        } => (template.clone(), *instance_id),
        PortRef::Declared { .. } => {
            return Err(invalid_editor_mutation(
                "only instance ports can be removed",
            ));
        }
    };
    let node_id = address.node_id;
    let resolved = crate::compatibility::resolve_editor_port(document, registry, &address)
        .map_err(MutationConflict::Editor)?;
    let binding = resolved.binding.ok_or_else(|| {
        editor_error(
            EditorMutationErrorCode::GraphPortNotFound,
            format!("port binding '{address}' does not exist"),
        )
    })?;
    match binding {
        DynamicPortBinding::Orphan { .. } => {
            return Ok(remove_port_addresses_operations(document, &[address]));
        }
        DynamicPortBinding::Resolved { .. } => {
            return Err(invalid_editor_mutation(
                "resolved derived port instances cannot be removed",
            ));
        }
        DynamicPortBinding::UserCreated { .. } => {}
    }
    let contract = user_created_port_contract(document, registry, node_id, &template)?;

    let (addresses, count, min, enforce_minimum) = if let Some(group) = contract.group {
        let state = port_member_group_state(node_id, group, document.port_bindings.iter());
        (
            group
                .templates
                .iter()
                .map(|template| PortAddress::instance(node_id, template.clone(), instance_id))
                .collect::<Vec<_>>(),
            state.complete_count(),
            group.min,
            state.is_complete(instance_id),
        )
    } else {
        let PortInstances::UserCreated { min, .. } = contract.spec.instances else {
            unreachable!("user_created_port_contract guarantees the instance policy")
        };
        (
            vec![address],
            user_created_port_instance_count(node_id, &template, document.port_bindings.iter()),
            min,
            true,
        )
    };
    if enforce_minimum && count <= usize::from(min) {
        return Err(invalid_editor_mutation(format!(
            "port member for template '{template}' requires at least {min} instances"
        )));
    }
    Ok(remove_port_addresses_operations(document, &addresses))
}

fn remove_port_addresses_operations(
    document: &GraphDocument,
    addresses: &[PortAddress],
) -> Vec<GraphDocumentOperation> {
    let address_set = addresses.iter().collect::<BTreeSet<_>>();
    let mut operations = document
        .connections
        .values()
        .filter(|connection| {
            address_set.contains(&connection.output) || address_set.contains(&connection.input)
        })
        .cloned()
        .map(|connection| GraphDocumentOperation::RemoveConnection { connection })
        .collect::<Vec<_>>();
    for address in addresses {
        if let Some(before) = document.input_states.get(address).cloned() {
            operations.push(GraphDocumentOperation::SetInputState {
                address: address.clone(),
                before: Some(before),
                after: None,
            });
        }
    }
    for address in addresses {
        if let Some(binding) = document.port_bindings.get(address).cloned() {
            operations.push(GraphDocumentOperation::RemovePortBinding {
                address: address.clone(),
                binding,
            });
        }
    }
    operations
}

struct UserCreatedPortContract<'a> {
    spec: &'a PortSpec,
    group: Option<&'a PortMemberGroupSpec>,
}

fn user_created_port_contract<'a>(
    document: &GraphDocument,
    registry: &'a NodeRegistry,
    node_id: NodeId,
    template: &PortKey,
) -> Result<UserCreatedPortContract<'a>, MutationConflict> {
    let node = document.nodes.get(&node_id).ok_or_else(|| {
        editor_error(
            EditorMutationErrorCode::GraphNodeNotFound,
            format!("node '{node_id}' does not exist"),
        )
    })?;
    let protocol = registry.protocol(&node.node_type).ok_or_else(|| {
        invalid_editor_mutation(format!("unknown node type '{}'", node.node_type))
    })?;
    let spec = protocol
        .interface
        .ports
        .iter()
        .find(|spec| &spec.key == template)
        .ok_or_else(|| {
            editor_error(
                EditorMutationErrorCode::GraphPortNotFound,
                format!("node '{node_id}' does not own port template '{template}'"),
            )
        })?;
    if !matches!(spec.instances, PortInstances::UserCreated { .. }) {
        return Err(invalid_editor_mutation(format!(
            "port template '{template}' is not user-created"
        )));
    }
    Ok(UserCreatedPortContract {
        spec,
        group: protocol.interface.member_group_for_template(template),
    })
}
