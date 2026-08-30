//! Atomic edits and structural validation for persisted Graph documents.
//!
//! The persisted model remains owned by `yss-graph-document`; this crate owns
//! the invariant-preserving operations that transform that model.

#![deny(unused_must_use)]

mod error;
mod patch;
mod validation;

pub use error::DocumentError;
pub use patch::{
    GraphDocumentOperation, GraphDocumentPatch, apply_graph_document_patch,
    apply_graph_document_patch_to_candidate,
};
pub use validation::{PortMemberGroupState, port_member_group_state, validate_graph_document};
