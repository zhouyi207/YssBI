//! Pure, serializable products of node-system semantic analysis.
//!
//! Document-owned identities and inferred type/schema values are generic so this layer can consume
//! their authoritative representations without redefining them or depending on execution/runtime.

mod basis;
mod diagnostic;
mod provenance;
mod semantic;
mod snapshot;

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
pub use semantic::{
    ControlEdge, EffectDependency, SemanticDependency, ValidatedSemanticGraph,
    ValidatedSemanticNode, ValidatedSemanticNodeSet, ValidatedSemanticPort, ValueEdge,
};
pub use snapshot::{
    AnalysisSnapshot, AnalyzedNode, ResolvedInterface, ResolvedPort, ResolvedPortStatus,
    SchemaFacts, SemanticValidationResult, TypeFacts, ValidationError,
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use yss_graph_protocol::I18nKey;
    use yss_graph_registry::RegistryFingerprint;

    type Snapshot = AnalysisSnapshot<u64, u64, String, u64, String, String, String, String>;
    type SemanticGraph = ValidatedSemanticGraph<u64, u64, String, u64, String, String, String>;

    fn basis(revision: u64) -> CompilationBasis<u64> {
        CompilationBasis {
            graph_revision: revision,
            registry_fingerprint: RegistryFingerprint::from_bytes([1; 32]),
            resource_versions: BTreeMap::from([
                (ResourceKey::new("z.resource"), ResourceVersion::new("2")),
                (ResourceKey::new("a.resource"), ResourceVersion::new("1")),
            ]),
            resource_observations: BTreeMap::new(),
        }
    }

    fn diagnostic(severity: DiagnosticSeverity) -> NodeDiagnostic<u64, String, u64, String> {
        NodeDiagnostic {
            code: DiagnosticCode::new("node.input.not_connected"),
            message_key: I18nKey::new("diagnostics.node.input_not_connected").unwrap(),
            arguments: BTreeMap::new(),
            severity,
            primary: DiagnosticLocation::Node(7),
            related: Box::new([]),
        }
    }

    fn snapshot(diagnostics: Vec<NodeDiagnostic<u64, String, u64, String>>) -> Snapshot {
        AnalysisSnapshot {
            basis: basis(1),
            nodes: Box::new([]),
            resolved_interfaces: Box::new([]),
            partial_types: BTreeMap::new(),
            partial_schemas: BTreeMap::new(),
            resolved_schemas: BTreeMap::new(),
            diagnostics: diagnostics.into_boxed_slice(),
        }
    }

    fn semantic_graph(revision: u64) -> SemanticGraph {
        ValidatedSemanticGraph {
            basis: basis(revision),
            nodes: Box::new([]),
            dependencies: Box::new([]),
            resolved_schemas: BTreeMap::new(),
        }
    }

    #[test]
    fn validated_rejects_blocking_diagnostics() {
        let result =
            snapshot(vec![diagnostic(DiagnosticSeverity::Error)]).validated(semantic_graph(1));

        assert_eq!(
            result.unwrap_err(),
            ValidationError::BlockingDiagnostics { count: 1 }
        );
    }

    #[test]
    fn validated_accepts_non_blocking_diagnostics() {
        let graph = semantic_graph(1);
        let result =
            snapshot(vec![diagnostic(DiagnosticSeverity::Warning)]).validated(graph.clone());

        assert_eq!(result.unwrap(), graph);
    }

    #[test]
    fn validated_rejects_a_graph_from_another_basis() {
        let result = snapshot(Vec::new()).validated(semantic_graph(2));

        assert_eq!(result.unwrap_err(), ValidationError::BasisMismatch);
    }
}
