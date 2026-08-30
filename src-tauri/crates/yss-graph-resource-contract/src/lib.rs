//! Graph compilation resource identities, schemas, and immutable catalog snapshots.
//!
//! The built-in node catalog remains owned by `yss-graph-catalog`; this crate owns only the
//! project-resource contract consumed by graph analysis, mutation, and compilation.

#![deny(unused_must_use)]

mod catalog;
mod schema;

pub use catalog::{
    FunctionCatalogEntry, FunctionSignature, GraphResourceId, ResourceCatalogFingerprint,
    ResourceCatalogSnapshot, VariableValueContract,
};
pub use schema::{ColumnSchema, DataSchema};
