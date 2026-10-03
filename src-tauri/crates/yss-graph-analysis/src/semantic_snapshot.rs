//! Semantic facts and readiness over the single authoritative graph snapshot.
use crate::{ConcreteGraphInterface, GraphResultCategory, GraphSchemaState};
use yss_graph_analysis_contract::{
    DiagnosticArguments, DiagnosticCode, DiagnosticLocation, DiagnosticSeverity,
};
use yss_graph_diagnostics::GraphDiagnosticKind;
use yss_graph_document::{
    ConnectionId, DynamicMemberLocator, FunctionParameterId, GraphResourcePath, NodeId,
    PortAddress, PortInstanceId,
};
use yss_graph_resource_contract::GraphResourceId;
use yss_node_protocol::{
    InputCoercionKind, ParameterEditorSpec, ParameterKey, ParameterPresentation, PortDirection,
    PortKey, RelationalScalarType, ResolvedType, SchemaExpr, TypeDomain, TypeExpr, TypeState,
    TypedValue,
};

#[derive(Clone, Debug, PartialEq)]
pub struct GraphSemanticSnapshot {
    nodes: Box<[GraphNodeSemanticFact]>,
    diagnostics: Box<[GraphDiagnosticFact]>,
    outcome: GraphResolutionOutcome,
    dependencies: yss_graph_resource_contract::GraphDependencyManifest,
    functions: std::collections::BTreeMap<GraphResourcePath, GraphFunctionSemanticFact>,
}

impl GraphSemanticSnapshot {
    pub fn ready(&self) -> Option<ReadyGraphSemanticSnapshot<'_>> {
        (matches!(self.outcome, GraphResolutionOutcome::Complete)
            && !self.has_blocking_diagnostics()
            && self.nodes.iter().all(|node| {
                node.specialization.is_some()
                    && node
                        .ports
                        .iter()
                        .all(|port| !port.orphan && port.type_state.exact().is_some())
            })
            && self
                .functions
                .values()
                .all(|function| function.semantics.ready().is_some()))
        .then_some(ReadyGraphSemanticSnapshot { snapshot: self })
    }
    pub fn dependencies(&self) -> &yss_graph_resource_contract::GraphDependencyManifest {
        &self.dependencies
    }

    pub fn functions(
        &self,
    ) -> &std::collections::BTreeMap<GraphResourcePath, GraphFunctionSemanticFact> {
        &self.functions
    }

    pub fn with_dependencies(
        mut self,
        dependencies: yss_graph_resource_contract::GraphDependencyManifest,
    ) -> Self {
        self.dependencies = dependencies;
        self
    }
    pub(crate) fn with_functions(
        mut self,
        functions: std::collections::BTreeMap<GraphResourcePath, GraphFunctionSemanticFact>,
    ) -> Self {
        self.functions = functions;
        self
    }

    pub fn concrete_interface(&self) -> ConcreteGraphInterface<'_> {
        ConcreteGraphInterface { nodes: &self.nodes }
    }

    pub fn has_blocking_diagnostics(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.blocking)
    }

    pub fn new(
        nodes: impl IntoIterator<Item = GraphNodeSemanticFact>,
        diagnostics: impl IntoIterator<Item = GraphDiagnosticFact>,
        outcome: GraphResolutionOutcome,
    ) -> Self {
        Self {
            nodes: nodes.into_iter().collect(),
            diagnostics: diagnostics.into_iter().collect(),
            outcome,
            dependencies: Default::default(),
            functions: Default::default(),
        }
    }

    pub fn nodes(&self) -> &[GraphNodeSemanticFact] {
        &self.nodes
    }

    pub fn map_nodes(
        mut self,
        transform: impl FnMut(GraphNodeSemanticFact) -> GraphNodeSemanticFact,
    ) -> Self {
        self.nodes = self.nodes.into_vec().into_iter().map(transform).collect();
        self
    }

    pub fn diagnostics(&self) -> &[GraphDiagnosticFact] {
        &self.diagnostics
    }

    /// Runtime capabilities constrain readiness without introducing a second diagnostic store.
    pub fn with_execution_kernel_support(mut self, supports: &dyn Fn(&str) -> bool) -> Self {
        let mut diagnostics = self.diagnostics.into_vec();
        for node in &self.nodes {
            if node
                .specialization
                .as_ref()
                .is_some_and(|kernel| !supports(&kernel.implementation))
            {
                diagnostics.push(graph_problem(
                    GraphDiagnosticKind::NodeKernelUnavailable,
                    GraphDiagnosticLocation::Node(node.node_id),
                    [("node_type", node.node_type.as_str().into())],
                ));
            }
        }
        self.diagnostics = diagnostics.into_boxed_slice();
        if matches!(self.outcome, GraphResolutionOutcome::Complete)
            && self.has_blocking_diagnostics()
        {
            self.outcome = GraphResolutionOutcome::Incomplete;
        }
        self
    }

    pub const fn outcome(&self) -> &GraphResolutionOutcome {
        &self.outcome
    }

    pub fn node(&self, node_id: NodeId) -> Option<&GraphNodeSemanticFact> {
        self.nodes.iter().find(|node| node.node_id == node_id)
    }
}

#[derive(Clone, Copy)]
pub struct ReadyGraphSemanticSnapshot<'a> {
    snapshot: &'a GraphSemanticSnapshot,
}

impl<'a> ReadyGraphSemanticSnapshot<'a> {
    pub const fn snapshot(self) -> &'a GraphSemanticSnapshot {
        self.snapshot
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GraphResolvedParameterValue {
    Literal(serde_json::Value),
    DefaultLiteral(yss_data_contract::DataValue),
    Resource(GraphResourceId),
}

#[derive(Clone, Debug, PartialEq)]
pub struct GraphNodeSemanticFact {
    pub constant: Option<std::sync::Arc<yss_graph_document::GraphConstant>>,
    pub node_id: NodeId,
    pub node_type: yss_node_protocol::NodeTypeId,
    pub instance_title: Option<Box<str>>,
    pub title: Box<str>,
    pub icon_id: Option<Box<str>>,
    pub style_id: Option<Box<str>>,
    pub managed: bool,
    pub parameter_groups: Box<[GraphParameterGroupFact]>,
    pub parameters: Box<[GraphParameterFact]>,
    pub inputs: Box<[GraphResolvedInputBinding]>,
    pub ports: Box<[GraphPortSemanticFact]>,
    pub port_instance_additions: Box<[GraphPortInstanceAdditionFact]>,
    pub specialization: Option<GraphKernelSpecialization>,
    pub semantic_fingerprint: [u8; 32],
}

impl GraphNodeSemanticFact {
    /// Includes resolved bindings and coercions, but never localized labels or canvas layout.
    pub fn execution_fingerprint(&self) -> [u8; 32] {
        yss_canonical_hash::hash_canonical(
            "yssbi.graph-node-execution-input.v1",
            &(
                &self.semantic_fingerprint,
                &self.inputs,
                &self.specialization,
            ),
        )
        .expect("resolved execution inputs are canonically serializable")
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct GraphResolvedInputBinding {
    pub address: PortAddress,
    pub group: Option<PortInstanceId>,
    pub source: GraphResolvedInputSource,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub enum GraphResolvedInputSource {
    Output(PortAddress),
    Literal(TypedValue),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphParameterGroupFact {
    pub key: yss_node_protocol::ParameterGroupKey,
    pub title: Box<str>,
    pub description: Option<Box<str>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphParameterFact {
    pub group_key: yss_node_protocol::ParameterGroupKey,
    pub key: ParameterKey,
    pub title: Box<str>,
    pub description: Option<Box<str>>,
    pub editor: ParameterEditorSpec,
    pub presentation: ParameterPresentation,
    pub value_type: TypeExpr,
    pub effective_value: Option<GraphResolvedParameterValue>,
    pub configuration: Option<GraphParameterConfigurationFact>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GraphParameterConfigurationFact {
    SelectOptions {
        options: Box<[Box<str>]>,
    },
    ProjectColumns {
        allow_empty: bool,
        available: bool,
        unavailable_reason: Option<Box<str>>,
        options: Box<[GraphColumnFact]>,
        value: Box<[Box<str>]>,
    },
    FilterPredicate {
        available: bool,
        unavailable_reason: Option<Box<str>>,
        columns: Box<[GraphFilterColumnFact]>,
        value: Option<serde_json::Value>,
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
    pub operators: Box<[yss_node_protocol::dataframe::FilterOperator]>,
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
pub struct GraphPortSemanticFact {
    pub address: PortAddress,
    pub label: Box<str>,
    pub instance_label: Option<Box<str>>,
    pub direction: PortDirection,
    pub result_category: GraphResultCategory,
    pub backing: GraphPortBacking,
    pub orphan: bool,
    pub can_remove: bool,
    pub connections: GraphPortConnectionFacts,
    pub editor: GraphPortEditorFact,
    pub protocol_default: Option<TypedValue>,
    pub literal_allowed: bool,
    pub accepted_type: TypeExpr,
    pub accepted_domain: Option<TypeDomain>,
    pub type_state: TypeState,
    pub schema: Option<SchemaExpr>,
    pub schema_state: GraphSchemaState,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct GraphKernelSpecialization {
    pub implementation: Box<str>,
    pub input_types: Box<[GraphPortTypeBinding]>,
    pub output_types: Box<[GraphPortTypeBinding]>,
    pub coercions: Box<[GraphInputCoercion]>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct GraphPortTypeBinding {
    pub address: PortAddress,
    pub value_type: ResolvedType,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct GraphInputCoercion {
    pub address: PortAddress,
    pub kind: InputCoercionKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GraphPortBacking {
    Declared,
    DocumentInstance,
    ProjectedDerived { origin: DynamicMemberLocator },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphPortInstanceAdditionFact {
    pub template_key: PortKey,
    pub label: Box<str>,
    pub direction: PortDirection,
    pub can_add: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GraphPortConnectionFacts {
    pub current: u32,
    pub maximum: Option<u32>,
    pub ordered: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
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
    pub message_key: Box<str>,
    pub severity: DiagnosticSeverity,
    pub blocking: bool,
    pub arguments: DiagnosticArguments,
    pub primary: GraphDiagnosticLocation,
    pub related: Box<[GraphDiagnosticLocation]>,
}

pub(crate) fn graph_problem(
    kind: GraphDiagnosticKind,
    primary: GraphDiagnosticLocation,
    arguments: impl IntoIterator<Item = (&'static str, Box<str>)>,
) -> GraphDiagnosticFact {
    let definition = kind.definition();
    GraphDiagnosticFact {
        code: DiagnosticCode::new(kind.code()),
        message_key: definition.message_key.into(),
        severity: kind.default_severity(),
        blocking: definition.blocking,
        arguments: arguments
            .into_iter()
            .map(|(key, value)| (Box::<str>::from(key), value))
            .collect(),
        primary,
        related: Box::new([]),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GraphResolutionOutcome {
    Complete,
    Incomplete,
    InternalFailure {
        stage: GraphResolutionStage,
        code: Box<str>,
        node_id: Option<NodeId>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GraphResolutionStage {
    Analysis,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphFunctionParameter {
    pub id: FunctionParameterId,
    pub entry_output: PortAddress,
    pub value_type: ResolvedType,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphFunctionResult {
    pub id: FunctionParameterId,
    pub return_input: PortAddress,
    pub value_type: ResolvedType,
}

/// Parameter array order is the signature order; labels never identify ABI slots.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphFunctionAbi {
    pub parameters: Box<[GraphFunctionParameter]>,
    pub result: Option<GraphFunctionResult>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GraphFunctionSemanticFact {
    pub abi: GraphFunctionAbi,
    pub semantics: GraphSemanticSnapshot,
}
