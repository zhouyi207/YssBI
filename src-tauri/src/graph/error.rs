use thiserror::Error;
use yss_graph_resource_contract::GraphResourceId;

#[derive(Debug, Error)]
pub enum GraphCatalogError {
    #[error("catalog resource is missing")]
    ResourceMissing { resource: GraphResourceId },
    #[error("catalog resource schema is invalid")]
    InvalidSchema { resource: GraphResourceId },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphMutationErrorCode {
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
    CatalogDescriptorInvalid,
    ClipboardSubgraphInvalid,
    ReferencedResourceUnavailable,
}

#[derive(Debug, Error)]
#[error("graph mutation failed")]
pub struct GraphMutationSource {
    #[source]
    source: GraphMutationSourceKind,
}

#[derive(Debug, Error)]
enum GraphMutationSourceKind {
    #[error("graph mutation invariant failed")]
    Invariant,
}

impl GraphMutationSource {
    pub(crate) fn invariant() -> Self {
        Self {
            source: GraphMutationSourceKind::Invariant,
        }
    }
}

#[derive(Debug, Error)]
pub enum GraphMutationError {
    #[error(transparent)]
    Catalog(#[from] GraphCatalogError),
    #[error("graph mutation is invalid")]
    InvalidMutation { code: GraphMutationErrorCode },
    #[error(transparent)]
    Internal(#[from] GraphMutationSource),
}
