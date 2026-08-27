//! Pure, normalized graph document data and invariant-preserving transactions.

mod error;
mod history;
pub mod materialization;
mod mutation;
mod patch;
mod subgraph;
mod transaction;

pub use error::DocumentError;
pub use history::{
    DatabaseDocumentPatch, DatabaseResourceKey, FunctionDocument, FunctionDocumentPatch,
    FunctionParameter, FunctionResourceKey, FunctionSignature, HistoryError, HistoryMutation,
    HistoryPersistencePolicy, HistoryStatusDto, ProjectDocumentState, ProjectHistory,
    ProjectHistoryTransaction, ResourceDeltaEvent, ResourceDocumentPatch, ResourceKey,
    ResourceKind, ResourceLifecycleHistoryPatch, ResourceLifecycleHistoryPayload,
    ResourceLifecycleKind, ResourceLifecyclePatch, ResourceLifecycleState,
    ResourceMoveHistoryPatch, ResourceMoveHistoryPayload, ResourcePatch, ResourcePathMovePatch,
    VariableDocument, VariableDocumentPatch, VariableEffectHistorySnapshots, VariableResourceKey,
    WorksheetDocumentPatch, WorksheetDocumentState, WorksheetResourceKey,
};
pub use materialization::{
    CompilationBasisToken, CompilationRegistryFingerprint, CompilationResourceKey,
    CompilationResourceVersion, CompilationResourceVersions, MaterializationAuthorization,
};
pub(crate) use mutation::ProjectedConnectPlan;
pub use mutation::{
    EditorGraphMutationDto, EditorMutationError, EditorMutationErrorCode, GraphDeltaEvent,
    MutationConflict, MutationRequest, NodePositionMutationDto, PortAddressDto, RevisionGap,
    detect_revision_gap,
};
#[cfg(test)]
pub(crate) use mutation::{GraphMutation, RevisionedGraphStore};
pub use patch::{GraphDocumentOperation, GraphDocumentPatch};
#[cfg(test)]
pub(crate) use subgraph::instantiate_subgraph_for_test;
pub use subgraph::{
    CLIPBOARD_SUBGRAPH_SCHEMA_VERSION, ClipboardConnectionDto, ClipboardDynamicMemberOriginDto,
    ClipboardDynamicPortBindingDto, ClipboardInputStateDto, ClipboardLastKnownPortMetadataDto,
    ClipboardNodeCreationDto, ClipboardNodeDto, ClipboardNodeId, ClipboardPortAddressDto,
    ClipboardPortBindingDto, ClipboardPortInstanceId, ClipboardPortRefDto, ClipboardSubgraphDto,
    duplicate_subgraph, export_subgraph,
};
pub(crate) use subgraph::{deserialize_clipboard_subgraph, instantiate_subgraph};
pub use transaction::EffectiveInputBinding;
pub(crate) use transaction::port_member_group_state;

use crate::graph_document::{
    ConnectionId, DocumentConnection, DocumentNode, DynamicMemberLocator, DynamicPortBinding,
    FunctionParameterId, GraphDocument, GraphResourcePath, GraphRevision, InputState,
    LastKnownPortMetadata, NodeId, NodePosition, OrderKey, ParameterValues, PortAddress,
    PortInstanceId, PortRef, SchemaFieldIdentity, SchemaSourceIdentity, TypedValue,
};
use crate::project::{HistoryEntryId, OperationId, ProjectRevision, ResourceRevision};

#[cfg(test)]
mod tests;
