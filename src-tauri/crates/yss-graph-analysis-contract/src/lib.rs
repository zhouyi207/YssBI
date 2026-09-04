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

/// Graph localization consumes a caller-provided lookup and does not own a
/// locale, transport DTO, or catalog delivery policy.
pub trait LocalizationLookup {
    fn text(&self, key: &yss_graph_protocol::I18nKey, arguments: &DiagnosticArguments) -> Box<str>;
}
pub use provenance::{CompileProvenance, GraphSessionId};
