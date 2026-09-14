//! Pure, serializable identities shared across Graph analysis and compilation.
//!
//! Executable semantic facts remain in `yss-graph-analysis`; this leaf crate owns only compilation
//! basis, diagnostic identity/location, and provenance contracts.

mod basis;
mod diagnostic;
mod provenance;

pub use basis::{
    CompilationBasis, CompileId, ResourceKey, ResourceObservationSet, ResourceObservedState,
    ResourceVersion, ResourceVersionSet,
};
pub use diagnostic::{
    DiagnosticArguments, DiagnosticCode, DiagnosticLocation, DiagnosticSeverity, NodeDiagnostic,
};

pub use provenance::{CompileProvenance, GraphSessionId};
