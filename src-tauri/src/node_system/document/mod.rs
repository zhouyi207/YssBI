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
    FunctionDocument, FunctionDocumentPatch, FunctionParameter, FunctionResourceKey,
    FunctionSignature, GraphResourceLifecycleKind, GraphResourceLifecyclePatch,
    GraphResourceLifecycleState, GraphResourceMoveHistoryPatch, HistoryError, HistoryMutation,
    HistoryPersistencePolicy, HistoryStatusDto, ProjectDocumentState, ProjectHistory,
    ProjectHistoryTransaction, ResourceDeltaEvent, ResourceDocumentPatch, ResourceKey,
    ResourceKind, ResourcePatch, ResourcePathMovePatch, VariableDocument, VariableDocumentPatch,
    VariableEffectHistorySnapshots, VariableResourceKey, WorksheetResourceKey,
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
    EditorGraphMutationDto, GraphDeltaEvent, GraphMutation, MutationConflict, MutationRequest,
    NodePositionMutationDto, PortAddressDto, RevisionGap, RevisionedGraphStore, apply_mutation,
    detect_revision_gap,
};
pub use patch::{GraphDocumentOperation, GraphDocumentPatch};

#[cfg(test)]
mod tests;
