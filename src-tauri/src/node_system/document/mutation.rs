use super::materialization::ProjectedMemberRef;
use super::{
    ConnectionId, DocumentConnection, DocumentError, DocumentNode, GraphDocument,
    GraphDocumentOperation, GraphDocumentPatch, GraphResourcePath, MaterializationAuthorization,
    NodeId, OperationId, OrderKey, PortAddress, PortInstanceId, ResourceKey, ResourceRevision,
    TypedValue,
};

use serde::{Deserialize, Serialize};
use std::fmt;

/// A client mutation paired with the resource revision it was based on.
///
/// `operation_id` is correlation metadata only. The store echoes it in the
/// committed event and does not use it for identity or deduplication.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq)]
pub enum MutationConflict {
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
    History(Box<str>),
    Document(DocumentError),
}

impl fmt::Display for MutationConflict {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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
    DeleteNode {
        node_id: NodeId,
    },
    Connect {
        output: PortAddress,
        input: PortAddress,
        order: Option<OrderKey>,
    },
    Disconnect {
        connection_id: ConnectionId,
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
            Self::DeleteNode { node_id } => delete_node_operations(document, node_id)?,
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
            Self::Disconnect { connection_id } => {
                let connection = document
                    .connections
                    .get(&connection_id)
                    .cloned()
                    .ok_or(DocumentError::ConnectionNotFound(connection_id))?;
                vec![GraphDocumentOperation::RemoveConnection { connection }]
            }
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

    let input = PortAddress::instance(
        member.node_id(),
        member.template().clone(),
        PortInstanceId::new(),
    );
    Ok(vec![
        GraphDocumentOperation::InsertPortBinding {
            address: input.clone(),
            binding: authorization.into_binding(),
        },
        GraphDocumentOperation::InsertConnection {
            connection: DocumentConnection {
                id: ConnectionId::new(),
                output,
                input,
                order,
            },
        },
    ])
}

fn delete_node_operations(
    document: &GraphDocument,
    node_id: NodeId,
) -> Result<Vec<GraphDocumentOperation>, DocumentError> {
    let node = document
        .nodes
        .get(&node_id)
        .cloned()
        .ok_or(DocumentError::NodeNotFound(node_id))?;
    let mut operations = Vec::new();
    operations.extend(
        document
            .connections
            .values()
            .filter(|connection| {
                connection.output.node_id == node_id || connection.input.node_id == node_id
            })
            .cloned()
            .map(|connection| GraphDocumentOperation::RemoveConnection { connection }),
    );
    operations.extend(
        document
            .input_states
            .iter()
            .filter(|(address, _)| address.node_id == node_id)
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
            .filter(|(address, _)| address.node_id == node_id)
            .map(
                |(address, binding)| GraphDocumentOperation::RemovePortBinding {
                    address: address.clone(),
                    binding: binding.clone(),
                },
            ),
    );
    operations.push(GraphDocumentOperation::RemoveNode { node });
    Ok(operations)
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

pub fn apply_mutation(
    store: &mut RevisionedGraphStore,
    request: MutationRequest<GraphMutation>,
) -> Result<GraphDeltaEvent<GraphDocumentPatch>, MutationConflict> {
    store.apply_mutation(request)
}
