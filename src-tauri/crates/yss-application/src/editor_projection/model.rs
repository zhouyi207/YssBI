use yss_data_contract::DataType;
use yss_graph_analysis::GraphDiagnosticLocation;
use yss_graph_analysis_contract::{DiagnosticArguments, ResourceVersionSet};
use yss_graph_document::{
    ConnectionId, GraphResourcePath, GraphRevision, NodeId, NodePosition, PortAddress, TypedValue,
};
use yss_graph_protocol::{
    ParameterKey, ParameterPresentation, PortDirection, PortKey, PortKind, RelationalScalarType,
    TypeExpr,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditorProjectionBasis {
    pub graph_path: GraphResourcePath,
    pub registry_fingerprint: [u8; 32],
    pub resource_versions: ResourceVersionSet,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EditorNodeModel {
    pub node_id: NodeId,
    pub node_type: yss_graph_protocol::NodeTypeId,
    pub position: NodePosition,
    pub display: EditorNodeDisplay,
    pub ports: Box<[EditorPortModel]>,
    pub port_instance_additions: Box<[EditorPortInstanceAdditionModel]>,
    pub parameters: Box<[EditorParameterModel]>,
    pub capabilities: EditorNodeCapabilities,
    pub diagnostics: Box<[EditorDiagnosticModel]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditorNodeDisplay {
    pub title: Box<str>,
    pub user_label: Option<Box<str>>,
    pub icon_id: Option<Box<str>>,
    pub style_id: Option<Box<str>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditorNodeCapabilities {
    pub managed: bool,
    pub can_copy: bool,
    pub can_delete: bool,
    pub can_edit_label: bool,
    pub can_edit_parameters: bool,
    pub supports_inline_literals: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EditorPortModel {
    pub address: PortAddress,
    pub display: EditorPortDisplay,
    pub direction: PortDirection,
    pub kind: PortKind,
    pub orphan: bool,
    pub can_remove: bool,
    pub connections: EditorPortConnectionCapabilities,
    pub input: Option<EditorInputBinding>,
    pub resolved_type: Option<EditorTypeSummary>,
    pub resolved_schema: Option<EditorSchemaSummary>,
    pub status: EditorPortStatus,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditorPortDisplay {
    pub label: Box<str>,
    pub instance_label: Option<Box<str>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditorPortInstanceAdditionModel {
    pub template_key: PortKey,
    pub label: Box<str>,
    pub direction: PortDirection,
    pub can_add: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditorPortConnectionCapabilities {
    pub current: u32,
    pub maximum: Option<u32>,
    pub ordered: bool,
    pub can_append: bool,
    pub can_replace: bool,
    pub can_move: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EditorInputBinding {
    pub literal_override: Option<TypedValue>,
    pub protocol_default: Option<TypedValue>,
    pub effective: EditorEffectiveInputBinding,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EditorEffectiveInputBinding {
    Connections,
    Literal,
    ProtocolDefault,
    Unbound,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EditorTypeSummary {
    pub display: Box<str>,
    pub resolved: bool,
    pub data_type: Option<DataType>,
    pub internal_type_expr: TypeExpr,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditorSchemaSummary {
    pub kind: EditorSchemaSummaryKind,
    pub fields: Box<[EditorSchemaField]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditorSchemaField {
    pub name: Box<str>,
    pub scalar_type: RelationalScalarType,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EditorSchemaSummaryKind {
    Input,
    Project,
    Append,
    Rename,
    Filter,
    Derived,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EditorPortStatus {
    Resolved,
    Orphan,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EditorParameterModel {
    pub key: ParameterKey,
    pub display: EditorParameterDisplay,
    pub editor: ParameterEditorKind,
    pub presentation: ParameterPresentation,
    pub value_type: Option<DataType>,
    pub multiline: bool,
    pub value: Option<TypedValue>,
    pub configuration: Option<EditorParameterConfiguration>,
    pub inherited_value: Option<TypedValue>,
    pub value_source: Option<EditorParameterValueSource>,
    pub options: Option<Box<[Box<str>]>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditorParameterDisplay {
    pub title: Box<str>,
    pub description: Option<Box<str>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParameterEditorKind {
    Auto,
    Text,
    Number,
    Toggle,
    Select,
    Resource,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EditorParameterValueSource {
    Project,
    Node,
}

#[derive(Clone, Debug, PartialEq)]
pub enum EditorParameterConfiguration {
    ProjectColumns {
        available: bool,
        unavailable_reason: Option<Box<str>>,
        options: Box<[EditorColumnOption]>,
        value: Box<[Box<str>]>,
    },
    FilterPredicate {
        available: bool,
        unavailable_reason: Option<Box<str>>,
        columns: Box<[EditorFilterColumnOption]>,
        value: Option<TypedValue>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditorColumnOption {
    pub name: Box<str>,
    pub data_type: RelationalScalarType,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditorFilterColumnOption {
    pub name: Box<str>,
    pub data_type: RelationalScalarType,
    pub operators: Box<[yss_graph_protocol::dataframe::FilterOperator]>,
    pub literal_types: Box<[EditorFilterLiteralType]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EditorFilterLiteralType {
    Boolean,
    Integer,
    Decimal,
    String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EditorDiagnosticModel {
    pub code: Box<str>,
    pub severity: EditorDiagnosticSeverity,
    pub arguments: DiagnosticArguments,
    pub location: GraphDiagnosticLocation,
    pub related: Box<[GraphDiagnosticLocation]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EditorDiagnosticSeverity {
    Error,
    Warning,
    Information,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditorConnectionModel {
    pub connection_id: ConnectionId,
    pub output: PortAddress,
    pub input: PortAddress,
    pub order: Option<Box<str>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EditorProjectionModel {
    pub basis: EditorProjectionBasis,
    pub graph_path: GraphResourcePath,
    pub source_revision: GraphRevision,
    pub nodes: Box<[EditorNodeModel]>,
    pub connections: Box<[EditorConnectionModel]>,
    pub diagnostics: Box<[EditorDiagnosticModel]>,
    pub outcome: EditorCompilationOutcome,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EditorCompilationOutcome {
    Complete,
    Incomplete,
    InternalFailure {
        stage: EditorCompilationStage,
        code: Box<str>,
        node_id: Option<NodeId>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EditorCompilationStage {
    Analysis,
    Lowering,
}

#[derive(Debug, thiserror::Error)]
pub enum EditorProjectionError {
    #[error("analysis and document revisions do not match")]
    RevisionMismatch {
        analysis: GraphRevision,
        document: GraphRevision,
    },
    #[error("analysis and catalog registry fingerprints do not match")]
    RegistryMismatch,
    #[error("projection facts are unavailable")]
    MissingProjectionFacts,
    #[error("projection facts do not match the graph document")]
    ProjectionFactsMismatch,
    #[error("projection basis is stale")]
    StaleProjectionBasis,
    #[error("projection graphs are incompatible")]
    IncompatibleProjectionGraphs,
    #[error("projection delta is invalid")]
    InvalidDelta,
}

pub struct EditorProjectionInput<'a> {
    pub graph_path: &'a GraphResourcePath,
    pub document: &'a yss_graph_document::GraphDocument,
    pub analysis: &'a yss_graph_analysis::GraphAnalysis,
    pub registry_fingerprint: [u8; 32],
}

impl From<yss_graph_analysis_contract::DiagnosticSeverity> for EditorDiagnosticSeverity {
    fn from(value: yss_graph_analysis_contract::DiagnosticSeverity) -> Self {
        match value {
            yss_graph_analysis_contract::DiagnosticSeverity::Error => Self::Error,
            yss_graph_analysis_contract::DiagnosticSeverity::Warning => Self::Warning,
            yss_graph_analysis_contract::DiagnosticSeverity::Information => Self::Information,
        }
    }
}
