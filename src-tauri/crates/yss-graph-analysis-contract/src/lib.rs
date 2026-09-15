//! Pure, serializable identities shared by graph editing and execution preparation.
//!
//! Executable semantic facts remain in `yss-graph-analysis`; this leaf crate owns only analysis
//! basis, resource versions, and diagnostic identity/location contracts.

mod basis;
mod diagnostic;

pub use basis::{
    GraphAnalysisBasis, ResourceKey, ResourceObservationSet, ResourceObservedState,
    ResourceVersion, ResourceVersionSet,
};
pub use diagnostic::{
    DiagnosticArguments, DiagnosticCode, DiagnosticLocation, DiagnosticSeverity, NodeDiagnostic,
};
