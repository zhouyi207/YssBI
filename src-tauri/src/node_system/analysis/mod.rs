//! Pure, serializable products of node-system semantic analysis.
//!
//! Document-owned identities and inferred type/schema values are generic so this layer can consume
//! their authoritative representations without redefining them or depending on execution/runtime.

mod basis;
mod diagnostic;
mod observability;
mod projection;
mod semantic;
mod snapshot;
mod trace_store;

pub use crate::node_system::document::PortAddressDto;
pub use basis::{
    CompilationBasis, CompileId, CompileProjection, ResourceKey, ResourceVersion,
    ResourceVersionSet,
};
pub use diagnostic::{
    DiagnosticArguments, DiagnosticCode, DiagnosticLocation, DiagnosticSeverity, Location,
    NodeDiagnostic, Severity,
};
pub use observability::{
    CompileProvenance, CorrelationContext, NOOP_TRACE_SINK, NoopTraceSink, ParentCallId,
    ProjectSessionId, RedactionPolicy, RunId, SensitiveFieldAction, SpanEvent, SpanKind,
    SpanStatus, TraceFieldSensitivity, TraceSink, TraceValue,
};
pub use projection::{
    DiagnosticDto, DiagnosticLocationDto, DiagnosticSeverityDto, EditorConnectionProjectionDto,
    EditorGraphProjectionDto, EditorInputBindingDto, EditorNodeProjectionDto,
    EffectiveInputBindingKindDto, GraphProjectionDelta, LocalizationBundle, LocalizationLookup,
    NodeCapabilitiesDto, NodeDisplayDto, NodePositionDto, ParameterDisplayDto, ParameterEditorDto,
    ParameterEditorKindDto, PortConnectionCapabilityDto, PortDirectionDto, PortDisplayDto,
    PortInstanceKindDto, PortKindDto, ProjectionBasis, ProjectionError, ResolvedPortDto,
    ResolvedPortStatusDto, SchemaSummaryDto, SchemaSummaryKindDto, TypeSummaryDto,
    build_editor_graph_projection,
};
pub use semantic::{
    ControlEdge, EffectDependency, SemanticDependency, ValidatedSemanticGraph,
    ValidatedSemanticNode, ValidatedSemanticPort, ValueEdge,
};
pub use snapshot::{
    AnalysisSnapshot, AnalyzedNode, ResolvedInterface, ResolvedPort, ResolvedPortStatus,
    SchemaFacts, TypeFacts, ValidationError,
};
pub use trace_store::{BoundedTraceSink, DEFAULT_PROJECT_TRACE_CAPACITY, TraceRecord};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node_system::protocol::I18nKey;
    use crate::node_system::registry::RegistryFingerprint;
    use std::collections::BTreeMap;

    type Snapshot = AnalysisSnapshot<u64, u64, String, u64, String, String, String, String>;
    type SemanticGraph = ValidatedSemanticGraph<u64, u64, String, u64, String, String, String>;

    #[test]
    fn editor_projection_dtos_are_reexported() {
        fn assert_public<T>() {}
        assert_public::<EditorConnectionProjectionDto>();
        assert_public::<EditorInputBindingDto>();
        assert_public::<EffectiveInputBindingKindDto>();
        assert_public::<NodePositionDto>();
    }

    fn basis(revision: u64) -> CompilationBasis<u64> {
        CompilationBasis {
            graph_revision: revision,
            registry_fingerprint: RegistryFingerprint::from_bytes([1; 32]),
            resource_versions: BTreeMap::from([
                (ResourceKey::new("z.resource"), ResourceVersion::new("2")),
                (ResourceKey::new("a.resource"), ResourceVersion::new("1")),
            ]),
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

    #[test]
    fn deterministic_maps_serialize_in_key_order() {
        let json = serde_json::to_string(&basis(1)).unwrap();
        let first = json.find("a.resource").unwrap();
        let second = json.find("z.resource").unwrap();

        assert!(first < second);
    }
}
