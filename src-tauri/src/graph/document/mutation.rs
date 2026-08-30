use super::materialization::ProjectedMemberRef;
use super::{
    ConnectionId, DocumentConnection, DocumentError, DocumentNode, DynamicMemberLocator,
    DynamicPortBinding, GraphDocument, GraphDocumentOperation, GraphDocumentPatch,
    GraphResourcePath, MaterializationAuthorization, NodeId, NodePosition, OrderKey,
    ParameterValues, PortAddress, PortInstanceId, PortRef, TypedValue, port_member_group_state,
};
use crate::graph::catalog::reroute_node_type_for_kind;
use crate::graph::catalog::{CatalogResourcePath, NodeCreation, ResourceBoundCreateArgs};
use crate::graph::compatibility::EditorMutationValidationSnapshot;
use crate::graph::protocol::{
    ConnectionsPerPort, LiteralPolicy, NodeProtocol, NodeScope, NodeTypeId, PortDirection,
    PortInstances, PortKey, PortKind, PortMemberGroupSpec, PortSpec,
};
use crate::graph::registry::NodeRegistry;
#[cfg(test)]
use crate::project::{MutationRequest, OperationId, ResourceKey, ResourceRevision};

#[cfg(test)]
use serde::Deserialize;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

#[path = "mutation/connection.rs"]
mod connection;
use connection::{
    connect_operations, connect_operations_prevalidated_type, normalize_editor_literal_target,
    projected_connect_operations, resolve_mutation_port,
};
#[allow(unused_imports)]
pub(super) use connection::{
    connect_operations_with_id_allocator, move_connection_operations,
    move_connection_operations_with_id_allocator, validate_literal_target,
    validate_resolved_dynamic_binding_authority, validate_subgraph_connection,
    validate_subgraph_port,
};

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevisionGap {
    pub expected_before_revision: ResourceRevision,
    pub actual_before_revision: ResourceRevision,
}

#[cfg(test)]
pub fn detect_revision_gap<T>(
    applied_revision: ResourceRevision,
    event: &crate::application::events::GraphDeltaEvent<T>,
) -> Option<RevisionGap> {
    (event.from_revision != applied_revision).then_some(RevisionGap {
        expected_before_revision: applied_revision,
        actual_before_revision: event.from_revision,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum EditorMutationErrorCode {
    GraphPortNotFound,
    GraphNodeNotFound,
    GraphConnectionNotFound,
    GraphPortOrphan,
    GraphConnectionDirectionMismatch,
    GraphConnectionKindMismatch,
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
            Self::GraphConnectionKindMismatch => "graph_connection_kind_mismatch",
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
    RecoveryRequired(Box<str>),
    ResourceMismatch {
        requested: Box<str>,
        store: Box<str>,
    },
    StaleRevision {
        base_revision: u64,
        current_revision: u64,
    },
    CompilationBasisGraphMismatch {
        basis_graph_path: GraphResourcePath,
        store_graph_path: GraphResourcePath,
    },
    CompilationBasisStale {
        basis_revision: u64,
        current_revision: u64,
    },
    MaterializationUnauthorized,
    StaleProjectLifecycle(Box<str>),
    CatalogResourceStale(Box<str>),
    CatalogDescriptorInvalid(Box<str>),
    ClipboardSubgraphInvalid(Box<str>),
    ReferencedResourceUnavailable(Box<str>),
    Editor(EditorMutationError),
    InvalidEditorMutation(Box<str>),
    Projection(Box<str>),
    History(Box<str>),
    Document(DocumentError),
}

impl MutationConflict {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::RecoveryRequired(_) => "project_recovery_required",
            Self::StaleProjectLifecycle(_) => "stale_project_lifecycle",
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
            Self::RecoveryRequired(message) => formatter.write_str(message),
            Self::ResourceMismatch { requested, store } => write!(
                formatter,
                "mutation targets {requested:?}, but this store owns {store:?}"
            ),
            Self::StaleRevision {
                base_revision,
                current_revision,
            } => write!(
                formatter,
                "mutation base revision {} is stale; current revision is {}",
                base_revision, current_revision
            ),
            Self::CompilationBasisGraphMismatch {
                basis_graph_path,
                store_graph_path,
            } => write!(
                formatter,
                "compilation basis graph {basis_graph_path:?} does not match store graph {store_graph_path:?}"
            ),
            Self::CompilationBasisStale {
                basis_revision,
                current_revision,
            } => write!(
                formatter,
                "compilation basis revision {} is stale; current revision is {}",
                basis_revision, current_revision
            ),
            Self::MaterializationUnauthorized => {
                formatter.write_str("materialization authorization does not match projected member")
            }
            Self::StaleProjectLifecycle(message)
            | Self::CatalogResourceStale(message)
            | Self::CatalogDescriptorInvalid(message)
            | Self::ClipboardSubgraphInvalid(message)
            | Self::ReferencedResourceUnavailable(message)
            | Self::InvalidEditorMutation(message) => formatter.write_str(message),
            Self::Editor(error) => formatter.write_str(&error.detail),
            Self::Projection(message) => {
                write!(formatter, "committed graph projection failed: {message}")
            }
            Self::History(message) => {
                write!(formatter, "project history transaction failed: {message}")
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
        template: PortKey,
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
        snapshot: super::ClipboardSubgraph,
        anchor: NodePosition,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct NodePositionMutation {
    pub node_id: NodeId,
    pub position: NodePosition,
}

#[cfg(all(test, any()))]
impl From<&EditorGraphMutation> for crate::schema::graph_mutation::EditorGraphMutationDto {
    fn from(value: &EditorGraphMutation) -> Self {
        use crate::schema::graph_mutation::EditorGraphMutationDto;
        let address =
            |value: &PortAddress| crate::schema::graph_mutation::PortAddressDto::from(value);
        match value {
            EditorGraphMutation::CreateNode {
                descriptor,
                position,
                user_label,
                connect_from,
            } => EditorGraphMutationDto::CreateNode {
                descriptor: descriptor.clone(),
                position: *position,
                user_label: user_label.clone(),
                connect_from: connect_from.as_ref().map(address),
            },
            EditorGraphMutation::DeleteNodes { node_ids } => EditorGraphMutationDto::DeleteNodes {
                node_ids: node_ids.clone(),
            },
            EditorGraphMutation::SetParameters {
                node_id,
                parameters,
            } => EditorGraphMutationDto::SetParameters {
                node_id: *node_id,
                parameters: parameters.clone(),
            },
            EditorGraphMutation::MoveNodes { positions } => EditorGraphMutationDto::MoveNodes {
                positions: positions
                    .iter()
                    .map(
                        |position| crate::schema::graph_mutation::NodePositionMutationDto {
                            node_id: position.node_id,
                            position: position.position,
                        },
                    )
                    .collect(),
            },
            EditorGraphMutation::Connect {
                output,
                input,
                order,
            } => EditorGraphMutationDto::Connect {
                output: address(output),
                input: address(input),
                order: order.clone(),
            },
            EditorGraphMutation::MoveConnections { source, target } => {
                EditorGraphMutationDto::MoveConnections {
                    source: address(source),
                    target: address(target),
                }
            }
            EditorGraphMutation::DisconnectConnections { connection_ids } => {
                EditorGraphMutationDto::DisconnectConnections {
                    connection_ids: connection_ids.clone(),
                }
            }
            EditorGraphMutation::InsertReroute {
                connection_id,
                position,
            } => EditorGraphMutationDto::InsertReroute {
                connection_id: *connection_id,
                position: *position,
            },
            EditorGraphMutation::DisconnectPort { address: value } => {
                EditorGraphMutationDto::DisconnectPort {
                    address: address(value),
                }
            }
            EditorGraphMutation::DisconnectNode { node_id } => {
                EditorGraphMutationDto::DisconnectNode { node_id: *node_id }
            }
            EditorGraphMutation::SetLiteral {
                address: value,
                literal,
            } => EditorGraphMutationDto::SetLiteral {
                address: address(value),
                literal: literal.clone(),
            },
            EditorGraphMutation::AddPortInstance {
                node_id,
                template,
                order,
            } => EditorGraphMutationDto::AddPortInstance {
                node_id: *node_id,
                template: template.clone(),
                order: order.clone(),
            },
            EditorGraphMutation::RemovePortInstance { address: value } => {
                EditorGraphMutationDto::RemovePortInstance {
                    address: address(value),
                }
            }
            EditorGraphMutation::DuplicateSubgraph { node_ids, offset } => {
                EditorGraphMutationDto::DuplicateSubgraph {
                    node_ids: node_ids.clone(),
                    offset: *offset,
                }
            }
            EditorGraphMutation::InsertSubgraph { snapshot, anchor } => {
                EditorGraphMutationDto::InsertSubgraph {
                    snapshot_json: serde_json::to_string(snapshot)
                        .expect("domain clipboard snapshots must serialize"),
                    anchor: *anchor,
                }
            }
        }
    }
}

#[cfg(all(test, any()))]
impl Serialize for EditorGraphMutation {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let dto = crate::schema::graph_mutation::EditorGraphMutationDto::from(self);
        dto.serialize(serializer)
    }
}

#[cfg(all(test, any()))]
impl<'de> Deserialize<'de> for EditorGraphMutation {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let dto = crate::schema::graph_mutation::EditorGraphMutationDto::deserialize(deserializer)?;
        dto.try_into().map_err(
            |error: crate::schema::graph_mutation::EditorMutationMappingError| {
                serde::de::Error::custom(error.to_string())
            },
        )
    }
}

pub(crate) struct ProjectedConnectPlan {
    pub projection_address: PortAddress,
    pub direction: PortDirection,
    pub kind: PortKind,
    pub connections: ConnectionsPerPort,
    pub member: ProjectedMemberRef,
    pub authorization: MaterializationAuthorization,
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

    pub(crate) fn into_patch_with_catalog_snapshot(
        self,
        graph_path: &GraphResourcePath,
        document: &GraphDocument,
        registry: &NodeRegistry,
        catalog_validation: Option<&crate::graph::compatibility::CatalogMutationValidationSnapshot>,
    ) -> Result<GraphDocumentPatch, MutationConflict> {
        self.into_patch_with_compatibility(
            graph_path,
            document,
            registry,
            catalog_validation,
            None,
            None,
        )
    }

    pub(crate) fn into_patch_with_compatibility(
        self,
        graph_path: &GraphResourcePath,
        document: &GraphDocument,
        registry: &NodeRegistry,
        catalog_validation: Option<&crate::graph::compatibility::CatalogMutationValidationSnapshot>,
        compatibility_source: Option<&crate::graph::compatibility::SourcePort>,
        projected_connect: Option<ProjectedConnectPlan>,
    ) -> Result<GraphDocumentPatch, MutationConflict> {
        self.into_patch_with_editor_validation_impl(
            graph_path,
            document,
            registry,
            catalog_validation,
            compatibility_source,
            None,
            projected_connect,
            #[cfg(test)]
            None,
            #[cfg(test)]
            None,
        )
    }

    #[cfg(test)]
    pub(crate) fn into_patch_with_editor_validation(
        self,
        graph_path: &GraphResourcePath,
        document: &GraphDocument,
        registry: &NodeRegistry,
        catalog_validation: Option<&crate::graph::compatibility::CatalogMutationValidationSnapshot>,
        compatibility_source: Option<&crate::graph::compatibility::SourcePort>,
        mutation_validation: Option<&EditorMutationValidationSnapshot>,
    ) -> Result<GraphDocumentPatch, MutationConflict> {
        self.into_patch_with_editor_validation_impl(
            graph_path,
            document,
            registry,
            catalog_validation,
            compatibility_source,
            mutation_validation,
            None,
            #[cfg(test)]
            None,
            #[cfg(test)]
            None,
        )
    }

    pub(crate) fn into_patch_with_editor_validation_and_projected_connect(
        self,
        graph_path: &GraphResourcePath,
        document: &GraphDocument,
        registry: &NodeRegistry,
        catalog_validation: Option<&crate::graph::compatibility::CatalogMutationValidationSnapshot>,
        compatibility_source: Option<&crate::graph::compatibility::SourcePort>,
        mutation_validation: Option<&EditorMutationValidationSnapshot>,
        projected_connect: Option<ProjectedConnectPlan>,
    ) -> Result<GraphDocumentPatch, MutationConflict> {
        self.into_patch_with_editor_validation_impl(
            graph_path,
            document,
            registry,
            catalog_validation,
            compatibility_source,
            mutation_validation,
            projected_connect,
            #[cfg(test)]
            None,
            #[cfg(test)]
            None,
        )
    }

    #[cfg(test)]
    pub(crate) fn into_patch_with_editor_validation_and_allocator(
        self,
        graph_path: &GraphResourcePath,
        document: &GraphDocument,
        registry: &NodeRegistry,
        catalog_validation: Option<&crate::graph::compatibility::CatalogMutationValidationSnapshot>,
        compatibility_source: Option<&crate::graph::compatibility::SourcePort>,
        mutation_validation: Option<&EditorMutationValidationSnapshot>,
        projected_connect: Option<ProjectedConnectPlan>,
        allocate_connection_id: &dyn Fn() -> ConnectionId,
    ) -> Result<GraphDocumentPatch, MutationConflict> {
        self.into_patch_with_editor_validation_impl(
            graph_path,
            document,
            registry,
            catalog_validation,
            compatibility_source,
            mutation_validation,
            projected_connect,
            None,
            Some(allocate_connection_id),
        )
    }

    #[cfg(test)]
    pub(super) fn into_patch_with_editor_validation_and_allocators(
        self,
        graph_path: &GraphResourcePath,
        document: &GraphDocument,
        registry: &NodeRegistry,
        catalog_validation: Option<&crate::graph::compatibility::CatalogMutationValidationSnapshot>,
        compatibility_source: Option<&crate::graph::compatibility::SourcePort>,
        mutation_validation: Option<&EditorMutationValidationSnapshot>,
        allocate_node_id: &dyn Fn() -> NodeId,
        allocate_connection_id: &dyn Fn() -> ConnectionId,
    ) -> Result<GraphDocumentPatch, MutationConflict> {
        self.into_patch_with_editor_validation_impl(
            graph_path,
            document,
            registry,
            catalog_validation,
            compatibility_source,
            mutation_validation,
            None,
            Some(allocate_node_id),
            Some(allocate_connection_id),
        )
    }

    fn into_patch_with_editor_validation_impl(
        self,
        graph_path: &GraphResourcePath,
        document: &GraphDocument,
        registry: &NodeRegistry,
        catalog_validation: Option<&crate::graph::compatibility::CatalogMutationValidationSnapshot>,
        compatibility_source: Option<&crate::graph::compatibility::SourcePort>,
        mutation_validation: Option<&EditorMutationValidationSnapshot>,
        projected_connect: Option<ProjectedConnectPlan>,
        #[cfg(test)] allocate_node_id: Option<&dyn Fn() -> NodeId>,
        #[cfg(test)] allocate_connection_id: Option<&dyn Fn() -> ConnectionId>,
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
                let (node_type_id, parameters, resource_bound, allow_missing_parameters) =
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
                            let authoritative =
                                crate::graph::catalog::authoritative_static_descriptor(
                                    registry, protocol,
                                );
                            if authoritative.as_ref() != Some(&descriptor) {
                                return Err(catalog_descriptor_invalid(
                                    "catalog creation descriptor does not match registry authority",
                                ));
                            }
                            let allow_missing =
                                matches!(descriptor, NodeCreation::ParameterizedStatic { .. });
                            (node_type_id, ParameterValues::new(), false, allow_missing)
                        }
                        NodeCreation::ResourceBound {
                            node_type_id,
                            resource_path,
                            resource_revision,
                            create_args,
                        } => (
                            node_type_id.clone(),
                            materialize_resource_descriptor(
                                graph_path,
                                &node_type_id,
                                &resource_path,
                                resource_revision,
                                create_args,
                                catalog_validation.ok_or_else(|| {
                                    catalog_resource_stale(
                                        "resource validation snapshot is unavailable",
                                    )
                                })?,
                            )?,
                            true,
                            false,
                        ),
                    };
                let protocol = registry.protocol(&node_type_id).ok_or_else(|| {
                    if resource_bound {
                        catalog_descriptor_invalid(format!("unknown node type '{node_type_id}'"))
                    } else {
                        invalid_editor_mutation(format!("unknown node type '{node_type_id}'"))
                    }
                })?;
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
                    let source = compatibility_source.ok_or_else(|| {
                        invalid_editor_mutation(
                            "an analyzed compatibility source is required for create-and-connect",
                        )
                    })?;
                    if source.address != source_address {
                        return Err(invalid_editor_mutation(
                            "connectFrom does not match the analyzed source port",
                        ));
                    }
                    let source_port = resolve_mutation_port(document, registry, &source_address)?;
                    if source_port.spec.direction != source.direction
                        || source_port.spec.kind != source.kind
                        || matches!(source_port.binding, Some(DynamicPortBinding::Orphan { .. }))
                    {
                        return Err(invalid_editor_mutation(
                            "connectFrom no longer resolves to the analyzed source port",
                        ));
                    }
                    let resources = catalog_validation.ok_or_else(|| {
                        invalid_editor_mutation("catalog compatibility snapshot is unavailable")
                    })?;
                    let validation = mutation_validation.ok_or_else(|| {
                        MutationConflict::Projection(
                            "create-and-connect validation snapshot is unavailable".into(),
                        )
                    })?;
                    let candidates = crate::graph::compatibility::connection_candidates(
                        graph_path,
                        &connection_descriptor,
                        registry,
                        resources,
                        source,
                    )
                    .map_err(invalid_editor_mutation)?;
                    let mut first_type_error = None;
                    let candidate = candidates
                        .into_iter()
                        .find(|candidate| {
                            match validation.validate_create_connection(&source_address, candidate)
                            {
                                Ok(()) => true,
                                Err(error) => {
                                    first_type_error.get_or_insert(error);
                                    false
                                }
                            }
                        })
                        .ok_or_else(|| {
                            MutationConflict::Editor(
                                first_type_error.expect(
                                    "same-kind connection candidates produce a type result",
                                ),
                            )
                        })?;
                    append_atomic_connection(
                        document,
                        registry,
                        &mut operations,
                        source,
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
            } => {
                if let Some(plan) = projected_connect {
                    projected_connect_operations(
                        graph_path, document, registry, output, input, order, plan,
                    )?
                } else if let Some(validation) = mutation_validation {
                    #[cfg(test)]
                    if let Some(allocate) = allocate_connection_id {
                        return connect_operations_with_id_allocator(
                            document, registry, validation, output, input, order, allocate,
                        )
                        .map(GraphDocumentPatch::new);
                    }
                    connect_operations(document, registry, validation, output, input, order)?
                } else {
                    connect_operations_prevalidated_type(document, registry, output, input, order)?
                }
            }
            Self::MoveConnections { source, target } => {
                let validation = mutation_validation.ok_or_else(|| {
                    MutationConflict::Projection(
                        "editor mutation validation snapshot is unavailable".into(),
                    )
                })?;
                #[cfg(test)]
                if let Some(allocate) = allocate_connection_id {
                    return move_connection_operations_with_id_allocator(
                        document, registry, validation, source, target, allocate,
                    )
                    .map(GraphDocumentPatch::new);
                }
                move_connection_operations(document, registry, validation, source, target)?
            }
            Self::DisconnectConnections { connection_ids } => {
                validate_direct_targets(&connection_ids)?;
                disconnect_connection_operations(document, connection_ids)?
            }
            Self::InsertReroute {
                connection_id,
                position,
            } => {
                #[cfg(test)]
                if let (Some(allocate_node_id), Some(allocate_connection_id)) =
                    (allocate_node_id, allocate_connection_id)
                {
                    return insert_reroute_operations_with_allocators(
                        document,
                        registry,
                        connection_id,
                        position,
                        allocate_node_id,
                        allocate_connection_id,
                    )
                    .map(GraphDocumentPatch::new);
                }
                insert_reroute_operations(document, registry, connection_id, position)?
            }
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
                    after: literal.map(|value| super::InputState {
                        literal_override: Some(value),
                    }),
                }]
            }
            Self::AddPortInstance {
                node_id,
                template,
                order,
            } => add_port_instance_operations(document, registry, node_id, template, order)?,
            Self::RemovePortInstance { address } => {
                remove_port_instance_operations(document, registry, address)?
            }
            Self::DuplicateSubgraph { node_ids, offset } => {
                return super::duplicate_subgraph(
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
                return super::instantiate_subgraph(
                    graph_path,
                    document,
                    registry,
                    catalog_validation.ok_or_else(|| {
                        catalog_resource_stale("subgraph catalog snapshot is unavailable")
                    })?,
                    super::subgraph::ValidatedClipboardSubgraph(snapshot),
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
    node_type_id: &NodeTypeId,
    resource_path: &CatalogResourcePath,
    resource_revision: u64,
    create_args: ResourceBoundCreateArgs,
    snapshot: &crate::graph::compatibility::CatalogMutationValidationSnapshot,
) -> Result<ParameterValues, MutationConflict> {
    validate_resource_path(resource_path, create_args)?;
    let resource = snapshot.resources.get(resource_path).ok_or_else(|| {
        catalog_resource_stale(format!(
            "catalog resource '{}' is unavailable",
            resource_path.as_str()
        ))
    })?;
    let (current_revision, allowed_node_type, binding, scope) = match (create_args, resource) {
        (
            ResourceBoundCreateArgs::Function,
            crate::graph::compatibility::CatalogMutationResource::Function {
                revision,
                allowed_node_type_id,
                parameter_binding,
                ..
            },
        ) => (
            *revision,
            allowed_node_type_id,
            parameter_binding.as_ref(),
            None,
        ),
        (
            ResourceBoundCreateArgs::Variable,
            crate::graph::compatibility::CatalogMutationResource::Variable {
                revision,
                scope,
                allowed_node_type_ids,
                parameter_binding,
                ..
            },
        ) => {
            if !allowed_node_type_ids
                .iter()
                .any(|allowed| allowed == node_type_id)
            {
                return Err(catalog_descriptor_invalid(
                    "resource descriptor node type is not allowed",
                ));
            }
            (
                *revision,
                node_type_id,
                parameter_binding.as_ref(),
                Some(scope),
            )
        }
        (
            ResourceBoundCreateArgs::Database,
            crate::graph::compatibility::CatalogMutationResource::Database {
                authority_revision,
                allowed_node_type_id,
                parameter_binding,
            },
        ) => (
            *authority_revision,
            allowed_node_type_id,
            parameter_binding.as_ref(),
            None,
        ),
        _ => {
            return Err(catalog_descriptor_invalid(
                "resource descriptor create arguments do not match the resource kind",
            ));
        }
    };
    if allowed_node_type != node_type_id {
        return Err(catalog_descriptor_invalid(
            "resource descriptor node type is not allowed",
        ));
    }
    if current_revision != resource_revision {
        return Err(catalog_resource_stale(format!(
            "catalog resource '{}' revision is stale",
            resource_path.as_str()
        )));
    }
    let expected_binding = match create_args {
        ResourceBoundCreateArgs::Function => "target",
        ResourceBoundCreateArgs::Variable => "variable",
        ResourceBoundCreateArgs::Database => "dataframe",
    };
    if binding != expected_binding {
        return Err(catalog_descriptor_invalid(
            "catalog resource parameter binding is invalid",
        ));
    }
    if let Some(scope) = scope {
        validate_variable_scope(graph_path, scope)?;
    }
    Ok(BTreeMap::from([(
        crate::graph::protocol::ParameterKey::new(expected_binding)
            .expect("catalog bindings are static valid keys"),
        serde_json::Value::String(resource_path.as_str().to_owned()),
    )]))
}

fn validate_resource_path(
    resource_path: &CatalogResourcePath,
    create_args: ResourceBoundCreateArgs,
) -> Result<(), MutationConflict> {
    let path = resource_path.as_str();
    let valid = match create_args {
        ResourceBoundCreateArgs::Function => crate::graph_document::GraphResourcePath::new(path)
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
        Err(catalog_descriptor_invalid(format!(
            "catalog resource path '{path}' is malformed for its create arguments"
        )))
    }
}

fn validate_variable_scope(
    graph_path: &GraphResourcePath,
    scope: &yss_variable_contract::VariableScope,
) -> Result<(), MutationConflict> {
    let in_scope = match scope {
        yss_variable_contract::VariableScope::Global => true,
        yss_variable_contract::VariableScope::Event { event_path } => {
            event_path.as_str() == graph_path.as_str()
        }
        yss_variable_contract::VariableScope::Function { function_path } => {
            function_path.as_str() == graph_path.as_str()
        }
    };
    if in_scope {
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
    operations.extend(document.input_states.iter().filter_map(|(address, state)| {
        selected
            .contains(&address.node_id)
            .then(|| GraphDocumentOperation::SetInputState {
                address: address.clone(),
                before: Some(state.clone()),
                after: None,
            })
    }));
    operations.extend(
        document
            .port_bindings
            .iter()
            .filter_map(|(address, binding)| {
                selected.contains(&address.node_id).then(|| {
                    GraphDocumentOperation::RemovePortBinding {
                        address: address.clone(),
                        binding: binding.clone(),
                    }
                })
            }),
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
    if output.spec.kind != input.spec.kind {
        return Err(invalid_editor_mutation(
            "reroute connection endpoints have different port kinds",
        ));
    }

    let reroute_node_type = reroute_node_type_for_kind(output.spec.kind);
    let registered = registry.get(&reroute_node_type).ok_or_else(|| {
        invalid_editor_mutation(format!("unknown reroute node type '{reroute_node_type}'"))
    })?;
    let contract =
        crate::graph::catalog::validate_reroute_protocol_contract(registered, output.spec.kind)
            .map_err(|detail| MutationConflict::Projection(detail.into()))?;

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
                node_type: reroute_node_type,
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
    staged.apply_patch(&GraphDocumentPatch::new(operations.clone()))?;
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
    let graph_scope = if graph_path.as_str().starts_with("events/") {
        NodeScope::Event
    } else if graph_path.as_str().starts_with("functions/") {
        NodeScope::Function
    } else {
        NodeScope::Any
    };
    if protocol.scope != NodeScope::Any
        && graph_scope != NodeScope::Any
        && protocol.scope != graph_scope
    {
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
    operations: &mut Vec<GraphDocumentOperation>,
    source: &crate::graph::compatibility::SourcePort,
    candidate: crate::graph::compatibility::CandidatePort,
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
    staged.apply_patch(&GraphDocumentPatch::new(operations.clone()))?;
    operations.extend(connect_operations_prevalidated_type(
        &staged, registry, output, input, order,
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
    let nominal = |type_id: &crate::graph::protocol::TypeId, value: &serde_json::Value| {
        registry.validate_nominal_parameter(type_id, value)
    };
    validate_shared_parameters(protocol, parameters, &nominal)
}

#[cfg(test)]
pub(super) fn validate_parameters(
    protocol: &NodeProtocol,
    parameters: &ParameterValues,
) -> Result<(), MutationConflict> {
    let nominal = |_: &crate::graph::protocol::TypeId, _: &serde_json::Value| None;
    validate_shared_parameters(protocol, parameters, &nominal)
}

fn validate_shared_parameters(
    protocol: &NodeProtocol,
    parameters: &ParameterValues,
    nominal: &impl crate::graph::protocol::validation::NominalParameterValidator,
) -> Result<(), MutationConflict> {
    let Some(issue) =
        crate::graph::protocol::validate_parameter_values(protocol, parameters, nominal)
            .into_iter()
            .next()
    else {
        return Ok(());
    };
    use crate::graph::protocol::ParameterIssueKind;
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
    template: PortKey,
    order: Option<OrderKey>,
) -> Result<Vec<GraphDocumentOperation>, MutationConflict> {
    let contract = user_created_port_contract(document, registry, node_id, &template)?;
    let (templates, count, max) = if let Some(group) = contract.group {
        let state = port_member_group_state(node_id, group, document.port_bindings.iter());
        (group.templates.as_ref(), state.complete_count(), group.max)
    } else {
        let PortInstances::UserCreated { max, .. } = contract.spec.instances else {
            unreachable!("user_created_port_contract guarantees the instance policy")
        };
        (
            std::slice::from_ref(&contract.spec.key),
            user_created_instance_count(document, node_id, &template),
            max,
        )
    };
    if max.is_some_and(|maximum| count >= usize::from(maximum)) {
        return Err(invalid_editor_mutation(format!(
            "port member for template '{template}' has reached its maximum instance count"
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
    let contract = user_created_port_contract(document, registry, node_id, &template)?;
    let binding = document
        .port_bindings
        .get(&address)
        .cloned()
        .ok_or_else(|| {
            editor_error(
                EditorMutationErrorCode::GraphPortNotFound,
                format!("port binding '{address}' does not exist"),
            )
        })?;
    if !matches!(binding, DynamicPortBinding::UserCreated { .. }) {
        return Err(invalid_editor_mutation(
            "only user-created port instances can be removed",
        ));
    }

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
            user_created_instance_count(document, node_id, &template),
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

fn user_created_instance_count(
    document: &GraphDocument,
    node_id: NodeId,
    template: &PortKey,
) -> usize {
    document
        .port_bindings
        .iter()
        .filter(|(address, binding)| {
            address.node_id == node_id
                && matches!(&address.port, PortRef::Instance { template: current, .. } if current == template)
                && matches!(binding, DynamicPortBinding::UserCreated { .. })
        })
        .count()
}

/// Mutations accepted by the authoritative graph store.
///
/// The projected-member variant intentionally has no `PortInstanceId` field:
/// its durable instance identity is allocated while the store builds the
/// atomic materialize-and-connect patch.
#[cfg(test)]
#[derive(Debug)]
pub enum GraphMutation {
    CreateNode {
        node: DocumentNode,
    },
    MaterializeProjectedMemberAndConnect {
        member: ProjectedMemberRef,
        authorization: MaterializationAuthorization,
        output: PortAddress,
        order: Option<OrderKey>,
    },
}

#[cfg(test)]
impl GraphMutation {
    fn into_patch(
        self,
        graph_path: &GraphResourcePath,
        document: &GraphDocument,
    ) -> Result<GraphDocumentPatch, MutationConflict> {
        let operations = match self {
            Self::CreateNode { node } => {
                vec![GraphDocumentOperation::InsertNode { node }]
            }

            Self::MaterializeProjectedMemberAndConnect {
                member,
                authorization,
                output,
                order,
            } => materialize_projected_member_operations(
                graph_path,
                document,
                member,
                authorization,
                output,
                order,
            )?,
        };
        Ok(GraphDocumentPatch::new(operations))
    }
}

fn materialize_projected_member_operations(
    graph_path: &GraphResourcePath,
    document: &GraphDocument,
    member: ProjectedMemberRef,
    authorization: MaterializationAuthorization,
    output: PortAddress,
    order: Option<OrderKey>,
) -> Result<Vec<GraphDocumentOperation>, MutationConflict> {
    if authorization.member() != &member {
        return Err(MutationConflict::MaterializationUnauthorized);
    }
    if member.basis().graph_path() != graph_path {
        return Err(MutationConflict::CompilationBasisGraphMismatch {
            basis_graph_path: member.basis().graph_path().clone(),
            store_graph_path: graph_path.clone(),
        });
    }
    if member.basis().graph_revision() != document.revision {
        return Err(MutationConflict::CompilationBasisStale {
            basis_revision: member.basis().graph_revision().get(),
            current_revision: document.revision.get(),
        });
    }

    let materialized = PortAddress::instance(
        member.node_id(),
        member.template().clone(),
        PortInstanceId::new(),
    );
    let (connection_output, connection_input) = match member.direction() {
        PortDirection::Input => (output, materialized.clone()),
        PortDirection::Output => (materialized.clone(), output),
    };
    Ok(vec![
        GraphDocumentOperation::InsertPortBinding {
            address: materialized,
            binding: authorization.into_binding(),
        },
        GraphDocumentOperation::InsertConnection {
            connection: DocumentConnection {
                id: ConnectionId::new(),
                output: connection_output,
                input: connection_input,
                order,
            },
        },
    ])
}

/// Authoritative in-memory wrapper for one graph document.
///
/// Mutation planning and patch application are pure in-memory operations, so
/// callers never need to hold this authority across filesystem or network I/O.
#[cfg(test)]
#[derive(Debug, Clone)]
pub struct RevisionedGraphStore {
    graph_path: GraphResourcePath,
    document: GraphDocument,
}

#[cfg(test)]
impl RevisionedGraphStore {
    pub fn new(graph_path: GraphResourcePath, document: GraphDocument) -> Self {
        Self {
            graph_path,
            document,
        }
    }

    pub const fn document(&self) -> &GraphDocument {
        &self.document
    }

    pub const fn revision(&self) -> ResourceRevision {
        ResourceRevision::from_graph_revision(self.document.revision)
    }

    pub fn apply_mutation(
        &mut self,
        request: MutationRequest<GraphMutation>,
    ) -> Result<crate::application::events::GraphDeltaEvent<GraphDocumentPatch>, MutationConflict>
    {
        let store_resource = ResourceKey::Graph(self.graph_path.clone());
        if request.resource != store_resource {
            return Err(MutationConflict::ResourceMismatch {
                requested: match request.resource {
                    ResourceKey::Graph(path) => path.as_str().into(),
                    _ => self.graph_path.as_str().into(),
                },
                store: self.graph_path.as_str().into(),
            });
        }
        if request.base_revision != ResourceRevision::from_graph_revision(self.document.revision) {
            return Err(MutationConflict::StaleRevision {
                base_revision: request.base_revision.get(),
                current_revision: self.document.revision.get(),
            });
        }

        let from_revision = ResourceRevision::from_graph_revision(self.document.revision);
        let patch = request
            .payload
            .into_patch(&self.graph_path, &self.document)?;
        self.document.apply_patch(&patch)?;

        Ok(crate::application::events::GraphDeltaEvent {
            graph_path: self.graph_path.clone(),
            from_revision,
            to_revision: ResourceRevision::from_graph_revision(self.document.revision),
            caused_by: Some(request.operation_id),
            payload: patch,
        })
    }
}
