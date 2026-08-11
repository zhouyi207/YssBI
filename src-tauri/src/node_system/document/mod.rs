//! Pure, normalized graph document data and invariant-preserving transactions.

mod error;
mod history;
mod ids;
pub mod materialization;
mod model;
mod mutation;
mod patch;
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
pub use ids::{
    ConnectionId, GraphRevision, HistoryEntryId, NodeId, OperationId, PortInstanceId,
    ProjectRevision, ProjectTransactionRevision, ResourceRevision,
};
pub use materialization::{
    CompilationBasisToken, CompilationRegistryFingerprint, CompilationResourceKey,
    CompilationResourceVersion, CompilationResourceVersions, MaterializationAuthorization,
};
pub(crate) use model::port_member_group_state;
pub use model::{
    DocumentConnection, DocumentNode, DynamicMemberLocator, DynamicPortBinding,
    EffectiveInputBinding, FunctionParameterId, GraphDocument, GraphResourcePath, InputState,
    LastKnownPortMetadata, NodePosition, OrderKey, ParameterValues, PortAddress, PortRef,
    SchemaFieldIdentity, SchemaSourceIdentity, TypedValue,
};
pub use mutation::{
    EditorGraphMutationDto, GraphDeltaEvent, MutationConflict, MutationRequest,
    NodePositionMutationDto, PortAddressDto, RevisionGap, detect_revision_gap,
};
#[cfg(test)]
pub(crate) use mutation::{GraphMutation, RevisionedGraphStore};
pub use patch::{GraphDocumentOperation, GraphDocumentPatch};

#[cfg(test)]
mod tests;
