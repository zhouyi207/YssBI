//! Atomic edits and structural validation for persisted Graph documents.
//!
//! The persisted model remains owned by `yss-graph-document`; this crate owns
//! the invariant-preserving operations that transform that model.

#![deny(unused_must_use)]

mod constant_references;
mod error;
mod patch;
mod validation;

pub use constant_references::{constant_references_for_copy, remap_copied_constant_references};
pub use error::DocumentError;
pub use patch::{
    GraphDocumentPatchPreview, PreparedGraphDocumentPatch, apply_graph_document_patch,
    prepare_graph_document_patch, prepare_graph_document_patch_in_place,
};
pub use validation::{
    GraphDocumentRead, PortMemberGroupState, port_member_group_state,
    user_created_port_instance_count, validate_graph_document,
};
