use crate::execution::plan::PlanCompilationBasis;
use crate::graph::resource_catalog::ResourceCatalogSnapshot;
use crate::graph::settings::GraphCompileSettings;
use crate::graph_document::{ConnectionId, GraphDocument, NodeId, PortAddress, TypedValue};
use crate::node_system::analysis::{
    DiagnosticArguments, DiagnosticCode, DiagnosticLocation, DiagnosticSeverity, ResourceKey,
    ResourceVersion, ResourceVersionSet,
};
use crate::node_system::protocol::{
    ParameterEditorSpec, ParameterKey, ParameterPresentation, PortDirection, PortKey, PortKind,
    RelationalScalarType, ResolvedSchemaFact, SchemaExpr, TypeExpr,
};

#[cfg(test)]
pub(crate) mod result_category;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphNodeFact {
    pub node_id: NodeId,
    pub node_type: crate::node_system::protocol::NodeTypeId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphProjectionFacts {
    nodes: Box<[GraphNodeProjectionFacts]>,
    diagnostics: Box<[GraphDiagnosticFact]>,
    outcome: GraphCompilationOutcome,
}

impl GraphProjectionFacts {
    pub fn new(
        nodes: impl IntoIterator<Item = GraphNodeProjectionFacts>,
        diagnostics: impl IntoIterator<Item = GraphDiagnosticFact>,
        outcome: GraphCompilationOutcome,
    ) -> Self {
        Self {
            nodes: nodes.into_iter().collect(),
            diagnostics: diagnostics.into_iter().collect(),
            outcome,
        }
    }

    pub fn nodes(&self) -> &[GraphNodeProjectionFacts] {
        &self.nodes
    }

    pub fn diagnostics(&self) -> &[GraphDiagnosticFact] {
        &self.diagnostics
    }

    pub const fn outcome(&self) -> &GraphCompilationOutcome {
        &self.outcome
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphNodeProjectionFacts {
    pub node_id: NodeId,
    pub node_type: crate::node_system::protocol::NodeTypeId,
    pub instance_title: Option<Box<str>>,
    pub title: Box<str>,
    pub icon_id: Option<Box<str>>,
    pub style_id: Option<Box<str>>,
    pub managed: bool,
    pub parameters: Box<[GraphParameterFact]>,
    pub ports: Box<[GraphPortFact]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphParameterFact {
    pub key: ParameterKey,
    pub title: Box<str>,
    pub description: Option<Box<str>>,
    pub editor: ParameterEditorSpec,
    pub presentation: ParameterPresentation,
    pub value_type: TypeExpr,
    pub inherited_value: Option<TypedValue>,
    pub value_source: Option<GraphParameterValueSource>,
    pub options: Box<[Box<str>]>,
    pub configuration: Option<GraphParameterConfigurationFact>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GraphParameterValueSource {
    Project,
    Node,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GraphParameterConfigurationFact {
    ProjectColumns {
        available: bool,
        unavailable_reason: Option<Box<str>>,
        options: Box<[GraphColumnFact]>,
        value: Box<[Box<str>]>,
    },
    FilterPredicate {
        available: bool,
        unavailable_reason: Option<Box<str>>,
        columns: Box<[GraphFilterColumnFact]>,
        value: Option<TypedValue>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphColumnFact {
    pub name: Box<str>,
    pub data_type: RelationalScalarType,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphFilterColumnFact {
    pub name: Box<str>,
    pub data_type: RelationalScalarType,
    pub operators: Box<[crate::node_system::protocol::dataframe::FilterOperator]>,
    pub literal_types: Box<[GraphFilterLiteralType]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GraphFilterLiteralType {
    Boolean,
    Integer,
    Decimal,
    String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphPortFact {
    pub address: PortAddress,
    pub template_key: PortKey,
    pub label: Box<str>,
    pub instance_label: Option<Box<str>>,
    pub direction: PortDirection,
    pub kind: PortKind,
    pub instance_kind: GraphPortInstanceKind,
    pub orphan: bool,
    pub connections: GraphPortConnectionFacts,
    pub member_minimum: u16,
    pub member_instance_count: usize,
    pub member_complete: bool,
    pub editor: GraphPortEditorFact,
    pub protocol_default: Option<TypedValue>,
    pub value_type: TypeExpr,
    pub schema: Option<SchemaExpr>,
    pub resolved_schema: Option<ResolvedSchemaFact>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GraphPortInstanceKind {
    Declared,
    UserCreated,
    Derived,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GraphPortConnectionFacts {
    pub current: u32,
    pub maximum: Option<u32>,
    pub ordered: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GraphPortEditorFact {
    Default,
    Hidden,
    InlineLiteral,
    SchemaColumns { allow_multiple: bool },
}

pub type GraphDiagnosticLocation = DiagnosticLocation<NodeId, PortAddress, ConnectionId, Box<str>>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphDiagnosticFact {
    pub code: DiagnosticCode,
    pub severity: DiagnosticSeverity,
    pub arguments: DiagnosticArguments,
    pub primary: GraphDiagnosticLocation,
    pub related: Box<[GraphDiagnosticLocation]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GraphCompilationOutcome {
    Complete,
    Incomplete,
    InternalFailure {
        stage: GraphCompilationStage,
        code: Box<str>,
        node_id: Option<NodeId>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GraphCompilationStage {
    Analysis,
    Lowering,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphAnalysis {
    nodes: Box<[GraphNodeFact]>,
    registry_fingerprint: [u8; 32],
    graph_revision: u64,
    resource_versions: ResourceVersionSet,
    projection_facts: Option<GraphProjectionFacts>,
}

impl GraphAnalysis {
    pub fn nodes(&self) -> &[GraphNodeFact] {
        &self.nodes
    }

    pub fn registry_fingerprint(&self) -> &[u8; 32] {
        &self.registry_fingerprint
    }

    pub const fn graph_revision(&self) -> u64 {
        self.graph_revision
    }

    pub fn resource_versions(&self) -> &ResourceVersionSet {
        &self.resource_versions
    }

    pub fn projection_facts(&self) -> Option<&GraphProjectionFacts> {
        self.projection_facts.as_ref()
    }

    pub fn with_projection_facts(mut self, facts: GraphProjectionFacts) -> Self {
        self.projection_facts = Some(facts);
        self
    }
}

pub struct GraphAnalysisInput<'a> {
    pub document: &'a GraphDocument,
    pub catalog: &'a ResourceCatalogSnapshot,
    pub settings: &'a GraphCompileSettings,
    pub basis: &'a PlanCompilationBasis,
}

pub fn analyze(input: GraphAnalysisInput<'_>) -> GraphAnalysis {
    let _ = (
        input.catalog.fingerprint(),
        input.settings.absolute_tolerance,
    );
    let nodes = input
        .document
        .nodes
        .values()
        .map(|node| GraphNodeFact {
            node_id: node.id,
            node_type: node.node_type.clone(),
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();
    GraphAnalysis {
        nodes,
        registry_fingerprint: input.basis.registry_fingerprint().as_bytes(),
        graph_revision: input.basis.graph_revision().get(),
        resource_versions: input
            .basis
            .resource_versions()
            .iter()
            .map(|(key, version)| {
                (
                    ResourceKey::new(key.as_str()),
                    ResourceVersion::new(version.as_str()),
                )
            })
            .collect(),
        projection_facts: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::plan::{
        PlanGraphRevision, PlanProjectSessionId, PlanRegistryFingerprint,
    };
    use std::collections::BTreeMap;

    #[test]
    fn analysis_accepts_neutral_document_catalog_settings_and_basis() {
        let document = GraphDocument::default();
        let catalog = ResourceCatalogSnapshot::new(
            BTreeMap::new(),
            BTreeMap::new(),
            BTreeMap::new(),
            crate::graph::resource_catalog::ResourceCatalogFingerprint::from_bytes([3; 32]),
        );
        let settings = GraphCompileSettings {
            absolute_tolerance: 1e-12,
            relative_tolerance: 1e-9,
        };
        let basis = PlanCompilationBasis::new(
            PlanProjectSessionId::from_existing("project".into()),
            PlanGraphRevision::from_existing(1),
            PlanRegistryFingerprint::from_bytes([4; 32]),
            BTreeMap::new(),
            BTreeMap::new(),
        );
        let analysis = analyze(GraphAnalysisInput {
            document: &document,
            catalog: &catalog,
            settings: &settings,
            basis: &basis,
        });
        assert!(analysis.nodes().is_empty());
        assert_eq!(analysis.registry_fingerprint(), &[4; 32]);
    }
}
