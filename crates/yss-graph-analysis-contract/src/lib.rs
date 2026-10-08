//! Pure, serializable identities shared by graph editing and execution preparation.
//!
//! Executable semantic facts remain in `yss-graph-analysis`; this leaf crate owns only analysis
//! environment basis and diagnostic identity/location contracts. Resource dependencies belong to
//! the semantic snapshot and `yss-graph-resource-contract`.

mod basis;
mod diagnostic;

pub use basis::GraphAnalysisBasis;
pub use diagnostic::{DiagnosticArguments, DiagnosticCode, DiagnosticLocation, DiagnosticSeverity};
