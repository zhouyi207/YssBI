//! Graph editor orchestration over canonical persisted documents and edit primitives.

pub mod materialization;
mod mutation;
mod subgraph;

pub use materialization::{
    CompilationBasisToken, CompilationRegistryFingerprint, CompilationResourceKey,
    CompilationResourceVersion, CompilationResourceVersions, MaterializationAuthorization,
};
pub(crate) use mutation::ProjectedConnectPlan;
pub use mutation::{
    EditorGraphMutation, EditorMutationError, EditorMutationErrorCode, MutationConflict,
    NodePositionMutation,
};
#[cfg(test)]
pub(crate) use mutation::{GraphMutation, RevisionedGraphStore};
#[cfg(test)]
pub(crate) use subgraph::instantiate_subgraph_for_test;
pub use subgraph::{
    CLIPBOARD_SUBGRAPH_SCHEMA_VERSION, ClipboardConnection, ClipboardDynamicMemberOrigin,
    ClipboardDynamicPortBinding, ClipboardInputState, ClipboardLastKnownPortMetadata,
    ClipboardNode, ClipboardNodeCreation, ClipboardNodeId, ClipboardPortAddress,
    ClipboardPortBinding, ClipboardPortInstanceId, ClipboardPortRef, ClipboardSubgraph,
    MAX_CLIPBOARD_SERIALIZED_BYTES, duplicate_subgraph, export_subgraph,
};
pub(crate) use subgraph::{deserialize_clipboard_subgraph, instantiate_subgraph};

use yss_graph_document::{
    ConnectionId, DocumentConnection, DocumentNode, DynamicMemberLocator, DynamicPortBinding,
    FunctionParameterId, GraphDocument, GraphResourcePath, GraphRevision, InputState,
    LastKnownPortMetadata, NodeId, NodePosition, OrderKey, ParameterValues, PortAddress,
    PortInstanceId, PortRef, SchemaFieldIdentity, SchemaSourceIdentity, TypedValue,
};
