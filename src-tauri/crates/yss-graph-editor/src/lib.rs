//! Graph editor mutations, validation, and portable subgraph operations.

#![forbid(unsafe_code)]
#![deny(unused_must_use)]

mod compatibility;
mod mutation;
mod subgraph;

#[cfg(test)]
mod tests;

pub use compatibility::{
    CatalogCompatibilityError, CatalogFunctionParameter, CatalogFunctionSignature,
    CatalogMutationResource, CatalogMutationValidationSnapshot, filter_compatible_catalog,
};
pub use mutation::{
    EditorGraphMutation, EditorMutationError, EditorMutationErrorCode, MutationConflict,
    NodePositionMutation,
};
pub use subgraph::{
    ClipboardConnection, ClipboardDynamicMemberOrigin, ClipboardDynamicPortBinding,
    ClipboardInputState, ClipboardLastKnownPortMetadata, ClipboardNode, ClipboardNodeCreation,
    ClipboardNodeId, ClipboardPortAddress, ClipboardPortBinding, ClipboardPortInstanceId,
    ClipboardPortRef, ClipboardSubgraph, deserialize_clipboard_subgraph, export_subgraph,
};
