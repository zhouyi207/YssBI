use super::materialization::ProjectedMemberRef;
use super::{
    ConnectionId, DocumentConnection, DocumentError, DocumentNode, DynamicMemberLocator,
    DynamicPortBinding, GraphDocument, GraphDocumentOperation, GraphDocumentPatch,
    GraphResourcePath, MaterializationAuthorization, NodeId, NodePosition, OperationId, OrderKey,
    ParameterValues, PortAddress, PortInstanceId, PortRef, ResourceKey, ResourceRevision,
    TypedValue, port_member_group_state,
};
use crate::node_system::catalog::{
    CatalogResourcePath, NodeCreationDescriptor, ResourceBoundCreateArgsDto,
    reroute_node_type_for_kind,
};
use crate::node_system::compatibility::EditorMutationValidationSnapshot;
use crate::node_system::protocol::{
    ConnectionsPerPort, LiteralPolicy, NodeProtocol, NodeScope, NodeTypeId, PortDirection,
    PortInstances, PortKey, PortKind, PortMemberGroupSpec, PortSpec,
};
use crate::node_system::registry::NodeRegistry;

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// A client mutation paired with the resource revision it was based on.
///
/// `operation_id` is correlation metadata only. The store echoes it in the
/// committed event and does not use it for identity or deduplication.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MutationRequest<T> {
    pub resource: ResourceKey,
    pub base_revision: ResourceRevision,
    pub operation_id: OperationId,
    pub payload: T,
}

impl<T> MutationRequest<T> {
    pub const fn new(
        resource: ResourceKey,
        base_revision: ResourceRevision,
        operation_id: OperationId,
        payload: T,
    ) -> Self {
        Self {
            resource,
            base_revision,
            operation_id,
            payload,
        }
    }
}

/// A delta produced only after the corresponding mutation has committed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphDeltaEvent<T> {
    pub graph_path: GraphResourcePath,
    pub from_revision: ResourceRevision,
    pub to_revision: ResourceRevision,
    pub caused_by: Option<OperationId>,
    pub payload: T,
}

impl<T> GraphDeltaEvent<T> {
    pub fn revision_gap_after(&self, applied_revision: ResourceRevision) -> Option<RevisionGap> {
        detect_revision_gap(applied_revision, self)
    }

    pub fn has_monotonic_revision(&self) -> bool {
        self.to_revision == self.from_revision.next()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevisionGap {
    pub expected_before_revision: ResourceRevision,
    pub actual_before_revision: ResourceRevision,
}

pub fn detect_revision_gap<T>(
    applied_revision: ResourceRevision,
    event: &GraphDeltaEvent<T>,
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
        requested: ResourceKey,
        store: ResourceKey,
    },
    StaleRevision {
        base_revision: ResourceRevision,
        current_revision: ResourceRevision,
    },
    CompilationBasisGraphMismatch {
        basis_graph_path: GraphResourcePath,
        store_graph_path: GraphResourcePath,
    },
    CompilationBasisStale {
        basis_revision: ResourceRevision,
        current_revision: ResourceRevision,
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
                base_revision.get(),
                current_revision.get()
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
                basis_revision.get(),
                current_revision.get()
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum PortAddressDto {
    Declared {
        node_id: Box<str>,
        port_key: Box<str>,
    },
    Instance {
        node_id: Box<str>,
        template_key: Box<str>,
        instance_id: Box<str>,
    },
}

impl From<&PortAddress> for PortAddressDto {
    fn from(address: &PortAddress) -> Self {
        match &address.port {
            PortRef::Declared { key } => Self::Declared {
                node_id: address.node_id.to_string().into(),
                port_key: key.as_str().into(),
            },
            PortRef::Instance {
                template,
                instance_id,
            } => Self::Instance {
                node_id: address.node_id.to_string().into(),
                template_key: template.as_str().into(),
                instance_id: instance_id.to_string().into(),
            },
        }
    }
}

impl From<PortAddress> for PortAddressDto {
    fn from(address: PortAddress) -> Self {
        Self::from(&address)
    }
}

impl TryFrom<PortAddressDto> for PortAddress {
    type Error = String;

    fn try_from(address: PortAddressDto) -> Result<Self, Self::Error> {
        match address {
            PortAddressDto::Declared { node_id, port_key } => Ok(Self::declared(
                parse_node_id(&node_id)?,
                PortKey::new(port_key).map_err(|error| error.to_string())?,
            )),
            PortAddressDto::Instance {
                node_id,
                template_key,
                instance_id,
            } => Ok(Self::instance(
                parse_node_id(&node_id)?,
                PortKey::new(template_key).map_err(|error| error.to_string())?,
                PortInstanceId::from_uuid(
                    uuid::Uuid::parse_str(&instance_id).map_err(|error| error.to_string())?,
                ),
            )),
        }
    }
}

fn parse_node_id(value: &str) -> Result<NodeId, String> {
    uuid::Uuid::parse_str(value)
        .map(NodeId::from_uuid)
        .map_err(|error| error.to_string())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    content = "payload",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum EditorGraphMutationDto {
    CreateNode {
        descriptor: NodeCreationDescriptor,
        position: NodePosition,
        user_label: Option<String>,
        #[serde(default)]
        connect_from: Option<PortAddressDto>,
    },
    DeleteNodes {
        node_ids: Vec<NodeId>,
    },
    SetParameters {
        node_id: NodeId,
        parameters: ParameterValues,
    },
    MoveNodes {
        positions: Vec<NodePositionMutationDto>,
    },
    Connect {
        output: PortAddressDto,
        input: PortAddressDto,
        order: Option<OrderKey>,
    },
    MoveConnections {
        source: PortAddressDto,
        target: PortAddressDto,
    },
    DisconnectConnections {
        connection_ids: Vec<ConnectionId>,
    },
    InsertReroute {
        connection_id: ConnectionId,
        position: NodePosition,
    },
    DisconnectPort {
        address: PortAddressDto,
    },
    DisconnectNode {
        node_id: NodeId,
    },
    SetLiteral {
        address: PortAddressDto,
        literal: Option<TypedValue>,
    },
    AddPortInstance {
        node_id: NodeId,
        template: PortKey,
        order: Option<OrderKey>,
    },
    RemovePortInstance {
        address: PortAddressDto,
    },
    DuplicateSubgraph {
        node_ids: Vec<NodeId>,
        offset: NodePosition,
    },
    InsertSubgraph {
        snapshot_json: String,
        anchor: NodePosition,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodePositionMutationDto {
    pub node_id: NodeId,
    pub position: NodePosition,
}

pub(crate) struct ProjectedConnectPlan {
    pub projection_address: PortAddress,
    pub direction: PortDirection,
    pub kind: PortKind,
    pub connections: ConnectionsPerPort,
    pub member: ProjectedMemberRef,
    pub authorization: MaterializationAuthorization,
}

impl EditorGraphMutationDto {
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
        catalog_validation: Option<&crate::project::CatalogMutationValidationSnapshot>,
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
        catalog_validation: Option<&crate::project::CatalogMutationValidationSnapshot>,
        compatibility_source: Option<&crate::node_system::compatibility::SourcePort>,
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

    pub(crate) fn into_patch_with_editor_validation(
        self,
        graph_path: &GraphResourcePath,
        document: &GraphDocument,
        registry: &NodeRegistry,
        catalog_validation: Option<&crate::project::CatalogMutationValidationSnapshot>,
        compatibility_source: Option<&crate::node_system::compatibility::SourcePort>,
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
        catalog_validation: Option<&crate::project::CatalogMutationValidationSnapshot>,
        compatibility_source: Option<&crate::node_system::compatibility::SourcePort>,
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
        catalog_validation: Option<&crate::project::CatalogMutationValidationSnapshot>,
        compatibility_source: Option<&crate::node_system::compatibility::SourcePort>,
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
        catalog_validation: Option<&crate::project::CatalogMutationValidationSnapshot>,
        compatibility_source: Option<&crate::node_system::compatibility::SourcePort>,
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
        catalog_validation: Option<&crate::project::CatalogMutationValidationSnapshot>,
        compatibility_source: Option<&crate::node_system::compatibility::SourcePort>,
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
                        descriptor @ NodeCreationDescriptor::Static { .. }
                        | descriptor @ NodeCreationDescriptor::ParameterizedStatic { .. } => {
                            let node_type_id = match &descriptor {
                                NodeCreationDescriptor::Static { node_type_id }
                                | NodeCreationDescriptor::ParameterizedStatic {
                                    node_type_id,
                                    ..
                                } => node_type_id.clone(),
                                NodeCreationDescriptor::ResourceBound { .. } => unreachable!(),
                            };
                            let protocol = registry.protocol(&node_type_id).ok_or_else(|| {
                                catalog_descriptor_invalid(format!(
                                    "unknown node type '{node_type_id}'"
                                ))
                            })?;
                            let authoritative =
                                crate::node_system::catalog::authoritative_static_descriptor(
                                    registry, protocol,
                                );
                            if authoritative.as_ref() != Some(&descriptor) {
                                return Err(catalog_descriptor_invalid(
                                    "catalog creation descriptor does not match registry authority",
                                ));
                            }
                            let allow_missing = matches!(
                                descriptor,
                                NodeCreationDescriptor::ParameterizedStatic { .. }
                            );
                            (node_type_id, ParameterValues::new(), false, allow_missing)
                        }
                        NodeCreationDescriptor::ResourceBound {
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
                    let source_address: PortAddress =
                        connect_from.try_into().map_err(invalid_editor_mutation)?;
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
                    let candidates = crate::node_system::compatibility::connection_candidates(
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
                let output = output.try_into().map_err(|detail| {
                    editor_error(EditorMutationErrorCode::GraphPortNotFound, detail)
                })?;
                let input = input.try_into().map_err(|detail| {
                    editor_error(EditorMutationErrorCode::GraphPortNotFound, detail)
                })?;
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
                let source = source.try_into().map_err(|detail| {
                    editor_error(EditorMutationErrorCode::GraphPortNotFound, detail)
                })?;
                let target = target.try_into().map_err(|detail| {
                    editor_error(EditorMutationErrorCode::GraphPortNotFound, detail)
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
                let address = address.try_into().map_err(|detail| {
                    editor_error(EditorMutationErrorCode::GraphPortNotFound, detail)
                })?;
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
                let address = address.try_into().map_err(|detail| {
                    editor_error(EditorMutationErrorCode::GraphPortNotFound, detail)
                })?;
                validate_literal_target(document, registry, &address, literal.as_ref())?;
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
            Self::RemovePortInstance { address } => remove_port_instance_operations(
                document,
                registry,
                address.try_into().map_err(|detail| {
                    editor_error(EditorMutationErrorCode::GraphPortNotFound, detail)
                })?,
            )?,
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
            Self::InsertSubgraph {
                snapshot_json,
                anchor,
            } => {
                let snapshot = super::deserialize_clipboard_subgraph(snapshot_json.as_bytes())?;
                return super::instantiate_subgraph(
                    graph_path,
                    document,
                    registry,
                    catalog_validation.ok_or_else(|| {
                        catalog_resource_stale("subgraph catalog snapshot is unavailable")
                    })?,
                    snapshot,
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
    resource_revision: ResourceRevision,
    create_args: ResourceBoundCreateArgsDto,
    snapshot: &crate::project::CatalogMutationValidationSnapshot,
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
            ResourceBoundCreateArgsDto::Function,
            crate::project::CatalogMutationResource::Function {
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
            ResourceBoundCreateArgsDto::Variable,
            crate::project::CatalogMutationResource::Variable {
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
            ResourceBoundCreateArgsDto::Database,
            crate::project::CatalogMutationResource::Database {
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
        ResourceBoundCreateArgsDto::Function => "target",
        ResourceBoundCreateArgsDto::Variable => "variable",
        ResourceBoundCreateArgsDto::Database => "dataframe",
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
        crate::node_system::protocol::ParameterKey::new(expected_binding)
            .expect("catalog bindings are static valid keys"),
        serde_json::Value::String(resource_path.as_str().to_owned()),
    )]))
}

fn validate_resource_path(
    resource_path: &CatalogResourcePath,
    create_args: ResourceBoundCreateArgsDto,
) -> Result<(), MutationConflict> {
    let path = resource_path.as_str();
    let valid = match create_args {
        ResourceBoundCreateArgsDto::Function => crate::project::GraphResourcePath::new(path)
            .is_ok_and(|canonical| {
                canonical.as_str() == path && canonical.as_str().starts_with("functions/")
            }),
        ResourceBoundCreateArgsDto::Variable => path
            .strip_prefix("variables/")
            .and_then(|id| uuid::Uuid::parse_str(id).ok())
            .is_some_and(|id| format!("variables/{id}") == path),
        ResourceBoundCreateArgsDto::Database => path
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
    scope: &crate::variable::VariableScope,
) -> Result<(), MutationConflict> {
    let in_scope = match scope {
        crate::variable::VariableScope::Global => true,
        crate::variable::VariableScope::Event { event_path } => {
            event_path.as_str() == graph_path.0.as_ref()
        }
        crate::variable::VariableScope::Function { function_path } => {
            function_path.as_str() == graph_path.0.as_ref()
        }
    };
    if in_scope {
        Ok(())
    } else {
        Err(catalog_descriptor_invalid(format!(
            "variable resource is out of scope for graph '{}'",
            graph_path.0
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
    let contract = crate::node_system::catalog::validate_reroute_protocol_contract(
        registered,
        output.spec.kind,
    )
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
    let graph_scope = if graph_path.0.starts_with("events/") {
        NodeScope::Event
    } else if graph_path.0.starts_with("functions/") {
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
            protocol.scope, graph_path.0
        )))
    } else {
        Ok(())
    }
}

fn append_atomic_connection(
    document: &GraphDocument,
    registry: &NodeRegistry,
    operations: &mut Vec<GraphDocumentOperation>,
    source: &crate::node_system::compatibility::SourcePort,
    candidate: crate::node_system::compatibility::CandidatePort,
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
        ConnectionsPerPort::Multiple { ordered: true, .. } => Some(OrderKey(
            format!("{:05}", document.connections.len()).into(),
        )),
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
            let order = OrderKey(format!("{index:05}").into());
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
                    order: OrderKey(format!("{index:05}").into()),
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
    let nominal = |type_id: &crate::node_system::protocol::TypeId, value: &serde_json::Value| {
        registry.validate_nominal_parameter(type_id, value)
    };
    validate_shared_parameters(protocol, parameters, &nominal)
}

pub(super) fn validate_parameters(
    protocol: &NodeProtocol,
    parameters: &ParameterValues,
) -> Result<(), MutationConflict> {
    let nominal = |_: &crate::node_system::protocol::TypeId, _: &serde_json::Value| None;
    validate_shared_parameters(protocol, parameters, &nominal)
}

fn validate_shared_parameters(
    protocol: &NodeProtocol,
    parameters: &ParameterValues,
    nominal: &impl crate::node_system::protocol::validation::NominalParameterValidator,
) -> Result<(), MutationConflict> {
    let Some(issue) =
        crate::node_system::protocol::validate_parameter_values(protocol, parameters, nominal)
            .into_iter()
            .next()
    else {
        return Ok(());
    };
    use crate::node_system::protocol::ParameterIssueKind;
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
    positions: Vec<NodePositionMutationDto>,
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

struct MutationPort<'a> {
    spec: &'a PortSpec,
    binding: Option<&'a DynamicPortBinding>,
}

fn resolve_mutation_port<'a>(
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

pub(super) fn move_connection_operations(
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

pub(super) fn move_connection_operations_with_id_allocator(
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

fn projected_connect_operations(
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

pub(super) fn validate_resolved_dynamic_binding_authority(
    protocol: &NodeProtocol,
    spec: &PortSpec,
    parameters: &ParameterValues,
    origin: &DynamicMemberLocator,
    catalog: &crate::project::CatalogMutationValidationSnapshot,
) -> Result<crate::node_system::protocol::TypeExpr, MutationConflict> {
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
                    &crate::node_system::protocol::ParameterKey::new("target")
                        .expect("function target is a valid parameter key"),
                )
                .and_then(serde_json::Value::as_str);
            if target != Some(function.0.as_ref()) {
                return Err(invalid_editor_mutation(
                    "resolved function member does not match the node target",
                ));
            }
            let path = CatalogResourcePath::new(function.0.clone());
            let Some(crate::project::CatalogMutationResource::Function { signature, .. }) =
                catalog.resources.get(&path)
            else {
                return Err(MutationConflict::ReferencedResourceUnavailable(
                    format!("function resource '{}' is unavailable", function.0).into(),
                ));
            };
            let resolver_id = resolver.as_str();
            let type_name = if resolver_id
                == crate::node_system::compiler::FUNCTION_CALL_ARGUMENTS_RESOLVER
                && spec.direction == PortDirection::Input
            {
                signature
                    .parameters
                    .iter()
                    .find(|candidate| candidate.id == *parameter)
                    .map(|parameter| parameter.type_name.as_str())
            } else if resolver_id == crate::node_system::compiler::FUNCTION_CALL_RESULTS_RESOLVER
                && spec.direction == PortDirection::Output
                && parameter.0.as_ref() == "return"
            {
                signature.return_type.as_deref()
            } else {
                None
            };
            let type_name = type_name.ok_or_else(|| {
                invalid_editor_mutation(format!(
                    "function member '{}:{}' is not authoritative for template '{}' on '{}'",
                    function.0, parameter.0, spec.key, protocol.type_id
                ))
            })?;
            crate::node_system::compatibility::function_type_expr(type_name).map_err(|error| {
                invalid_editor_mutation(format!(
                    "function member '{}:{}' has invalid authoritative type '{}': {error}",
                    function.0, parameter.0, type_name
                ))
            })
        }
        DynamicMemberLocator::SchemaField { source, field } => {
            let path = CatalogResourcePath::new(source.0.clone());
            if !matches!(
                catalog.resources.get(&path),
                Some(crate::project::CatalogMutationResource::Database { .. })
            ) {
                return Err(MutationConflict::ReferencedResourceUnavailable(
                    format!("database resource '{}' is unavailable", source.0).into(),
                ));
            }
            if resolver.as_str() != crate::node_system::compiler::DATAFRAME_COLUMNS_RESOLVER {
                return Err(invalid_editor_mutation(format!(
                    "schema member '{}:{}' is invalid for template '{}'",
                    source.0, field.0, spec.key
                )));
            }
            Err(MutationConflict::ReferencedResourceUnavailable(
                format!(
                    "current database field authority for '{}:{}' is unavailable",
                    source.0, field.0
                )
                .into(),
            ))
        }
    }
}

pub(super) fn validate_subgraph_port(
    document: &GraphDocument,
    registry: &NodeRegistry,
    address: &PortAddress,
) -> Result<(), MutationConflict> {
    resolve_mutation_port(document, registry, address).map(|_| ())
}

pub(super) fn validate_subgraph_connection(
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

fn connect_operations(
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

pub(super) fn connect_operations_with_id_allocator(
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

fn connect_operations_prevalidated_type(
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
) -> &'a crate::node_system::protocol::TypeExpr {
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
    output_type: &crate::node_system::protocol::TypeExpr,
    input_type: &crate::node_system::protocol::TypeExpr,
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
    if crate::node_system::compiler::type_exprs_compatibility(
        output_type,
        input_type,
        output_type_parameters,
        input_type_parameters,
    ) == crate::node_system::compiler::TypeCompatibility::Incompatible
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

pub(super) fn validate_literal_target(
    document: &GraphDocument,
    registry: &NodeRegistry,
    address: &PortAddress,
    literal: Option<&TypedValue>,
) -> Result<(), MutationConflict> {
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
    if let Some(literal) = literal {
        crate::node_system::protocol::validate_typed_literal(
            literal,
            &port.spec.value_type,
            registry,
        )
        .map_err(|_| invalid_editor_mutation("literal does not match the input value type"))?;
    }
    Ok(())
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
    let order = order.unwrap_or_else(|| OrderKey(instance_id.to_string().into()));
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
#[derive(Debug)]
pub enum GraphMutation {
    CreateNode {
        node: DocumentNode,
    },
    Connect {
        output: PortAddress,
        input: PortAddress,
        order: Option<OrderKey>,
    },
    SetLiteral {
        address: PortAddress,
        literal: Option<TypedValue>,
    },
    MaterializeProjectedMemberAndConnect {
        member: ProjectedMemberRef,
        authorization: MaterializationAuthorization,
        output: PortAddress,
        order: Option<OrderKey>,
    },
}

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
            Self::Connect {
                output,
                input,
                order,
            } => vec![GraphDocumentOperation::InsertConnection {
                connection: DocumentConnection {
                    id: ConnectionId::new(),
                    output,
                    input,
                    order,
                },
            }],
            Self::SetLiteral { address, literal } => {
                let before = document.input_states.get(&address).cloned();
                vec![GraphDocumentOperation::SetInputState {
                    address,
                    before,
                    after: literal.map(|value| super::InputState {
                        literal_override: Some(value),
                    }),
                }]
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
            basis_revision: member.basis().graph_revision(),
            current_revision: document.revision,
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
#[derive(Debug, Clone)]
pub struct RevisionedGraphStore {
    graph_path: GraphResourcePath,
    document: GraphDocument,
}

impl RevisionedGraphStore {
    pub fn new(graph_path: GraphResourcePath, document: GraphDocument) -> Self {
        Self {
            graph_path,
            document,
        }
    }

    pub const fn graph_path(&self) -> &GraphResourcePath {
        &self.graph_path
    }

    pub const fn document(&self) -> &GraphDocument {
        &self.document
    }

    pub const fn revision(&self) -> ResourceRevision {
        self.document.revision
    }

    pub fn into_document(self) -> GraphDocument {
        self.document
    }

    pub fn apply_mutation(
        &mut self,
        request: MutationRequest<GraphMutation>,
    ) -> Result<GraphDeltaEvent<GraphDocumentPatch>, MutationConflict> {
        let store_resource = ResourceKey::Graph(self.graph_path.clone());
        if request.resource != store_resource {
            return Err(MutationConflict::ResourceMismatch {
                requested: request.resource,
                store: store_resource,
            });
        }
        if request.base_revision != self.document.revision {
            return Err(MutationConflict::StaleRevision {
                base_revision: request.base_revision,
                current_revision: self.document.revision,
            });
        }

        let from_revision = self.document.revision;
        let patch = request
            .payload
            .into_patch(&self.graph_path, &self.document)?;
        self.document.apply_patch(&patch)?;

        Ok(GraphDeltaEvent {
            graph_path: self.graph_path.clone(),
            from_revision,
            to_revision: self.document.revision,
            caused_by: Some(request.operation_id),
            payload: patch,
        })
    }
}
