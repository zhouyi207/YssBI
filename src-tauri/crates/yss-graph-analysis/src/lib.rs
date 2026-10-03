//! Graph document analysis and authoritative semantic snapshots.
//!
//! Serializable semantic-analysis contracts remain owned by
//! `yss-graph-analysis-contract`; this crate owns the executable analysis behavior.

#![deny(unused_must_use)]

mod analysis;
mod concrete_interface;
mod derived_ports;
mod document_index;
mod function_validation;
mod node_projection;
mod parameter_projection;
mod port_projection;
mod resolution;
mod result_category;
mod schema_resolution;
mod schema_state;
mod semantic_snapshot;
mod semantic_validation;
mod type_resolution;

pub use analysis::{GraphAnalysis, analyze};
pub use concrete_interface::ConcreteGraphInterface;
pub use function_validation::direct_function_dependencies;
pub use parameter_projection::referenced_constant;
pub use resolution::{resolve_graph_semantics, resolve_graph_semantics_with_cache};
pub use result_category::{GraphPlotDataKind, GraphResultCategory, GraphStatisticalReportKind};
pub use schema_state::{GraphSchemaIssue, GraphSchemaState};
pub use semantic_snapshot::{
    GraphColumnFact, GraphDiagnosticFact, GraphDiagnosticLocation, GraphFilterColumnFact,
    GraphFilterLiteralType, GraphFunctionAbi, GraphFunctionParameter, GraphFunctionResult,
    GraphFunctionSemanticFact, GraphInputCoercion, GraphKernelSpecialization,
    GraphNodeSemanticFact, GraphParameterConfigurationFact, GraphParameterFact,
    GraphParameterGroupFact, GraphPortBacking, GraphPortConnectionFacts, GraphPortEditorFact,
    GraphPortInstanceAdditionFact, GraphPortSemanticFact, GraphPortTypeBinding,
    GraphResolutionOutcome, GraphResolutionStage, GraphResolvedInputBinding,
    GraphResolvedInputSource, GraphResolvedParameterValue, GraphSemanticSnapshot,
    ReadyGraphSemanticSnapshot,
};
pub use type_resolution::{GraphSemanticCache, type_patterns_can_connect};

use semantic_snapshot::graph_problem;

#[cfg(test)]
mod tests;
