use super::{
    AnalysisSnapshot, DiagnosticArguments, DiagnosticLocation, DiagnosticSeverity, NodeDiagnostic,
    ResolvedPortStatus, ResourceVersionSet,
};
use crate::graph::value::DataType;
use crate::node_system::document::{
    ConnectionId, EffectiveInputBinding, GraphDocument, GraphRevision, NodeId, PortAddress,
    PortAddressDto, port_member_group_state,
};
use crate::node_system::protocol::{
    ConnectionsPerPort, I18nKey, ParameterEditorSpec, ParameterPresentation, PortDirection,
    PortEditorSpec, PortInstances, PortKey, PortKind, RelationalScalarType, ResolvedSchemaFact,
    SchemaExpr, TypeExpr,
};
use crate::node_system::registry::{NodeRegistry, RegistryFingerprint};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Localization boundary owned by analysis. Catalogs and callers can implement it
/// without making analysis depend on a catalog implementation.
pub trait LocalizationLookup {
    fn text(&self, key: &I18nKey, arguments: &DiagnosticArguments) -> Box<str>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectionBasis {
    pub graph_path: Box<str>,
    pub graph_revision: u64,
    #[serde(
        serialize_with = "serialize_registry_fingerprint",
        deserialize_with = "deserialize_registry_fingerprint"
    )]
    pub registry_fingerprint: RegistryFingerprint,
    pub resource_versions: ResourceVersionSet,
}

fn serialize_registry_fingerprint<S>(
    fingerprint: &RegistryFingerprint,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&fingerprint.to_hex())
}

fn deserialize_registry_fingerprint<'de, D>(
    deserializer: D,
) -> Result<RegistryFingerprint, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Box::<str>::deserialize(deserializer)?;
    if value.len() != 64
        || !value
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(serde::de::Error::custom(
            "Registry fingerprint must be 64 lowercase hexadecimal characters",
        ));
    }
    let mut bytes = [0; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)
            .map_err(serde::de::Error::custom)?;
    }
    Ok(RegistryFingerprint::from_bytes(bytes))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EditorGraphProjectionDto {
    pub basis: ProjectionBasis,
    pub graph_path: Box<str>,
    pub source_revision: u64,
    pub nodes: Vec<EditorNodeProjectionDto>,
    pub connections: Vec<EditorConnectionProjectionDto>,
    pub diagnostics: Vec<DiagnosticDto>,
    pub outcome: CompilationOutcomeDto,
    pub has_blocking_diagnostics: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum CompilationOutcomeDto {
    Success,
    AnalysisBlocked,
    InternalFailure {
        stage: CompilationStageDto,
        code: Box<str>,
        node_id: Option<Box<str>>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CompilationStageDto {
    Analysis,
    Lowering,
}

impl From<&crate::node_system::compiler::CompilationOutcome> for CompilationOutcomeDto {
    fn from(outcome: &crate::node_system::compiler::CompilationOutcome) -> Self {
        use crate::node_system::compiler::{CompilationOutcome, CompilationStage};
        match outcome {
            CompilationOutcome::Succeeded => Self::Success,
            CompilationOutcome::AnalysisBlocked => Self::AnalysisBlocked,
            CompilationOutcome::InternalFailure(failure) => Self::InternalFailure {
                stage: match failure.stage {
                    CompilationStage::Analysis => CompilationStageDto::Analysis,
                    CompilationStage::Lowering => CompilationStageDto::Lowering,
                },
                code: failure.code.clone(),
                node_id: failure.node_id.map(|node_id| node_id.to_string().into()),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FunctionEditorPinDto {
    pub id: Box<str>,
    pub name: Box<str>,
    pub data_type: DataType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FunctionEditorProjectionDto {
    pub function_revision: crate::node_system::document::ResourceRevision,
    pub inputs: Box<[FunctionEditorPinDto]>,
    pub outputs: Box<[FunctionEditorPinDto]>,
}

pub fn build_function_editor_projection(
    function: &crate::node_system::document::FunctionDocument,
) -> Result<FunctionEditorProjectionDto, String> {
    let inputs = function
        .signature
        .parameters
        .iter()
        .map(|parameter| {
            Ok(FunctionEditorPinDto {
                id: parameter.id.0.clone(),
                name: parameter.name.clone().into_boxed_str(),
                data_type: resolve_function_data_type(&parameter.type_name)?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?
        .into_boxed_slice();
    let outputs = function
        .signature
        .return_type
        .as_deref()
        .map(|return_type| {
            Ok(FunctionEditorPinDto {
                id: "return".into(),
                name: return_type.into(),
                data_type: resolve_function_data_type(return_type)?,
            })
        })
        .into_iter()
        .collect::<Result<Vec<_>, String>>()?
        .into_boxed_slice();
    Ok(FunctionEditorProjectionDto {
        function_revision: function.revision,
        inputs,
        outputs,
    })
}

pub(crate) fn resolve_function_data_type(type_name: &str) -> Result<DataType, String> {
    let data_type = type_name.parse().or_else(|_| match type_name.trim() {
        "bool" | "boolean" | "core.bool" => Ok(DataType::Boolean),
        "int" | "integer" | "int64" | "core.int64" => Ok(DataType::Int64),
        "float" | "float64" | "number" | "core.float64" => Ok(DataType::Float64),
        "string" | "core.string" => Ok(DataType::String),
        "json" | "object" => Ok(DataType::Object),
        value => Err(format!("Unknown function data type: {value}")),
    })?;
    validate_function_data_type(&data_type)?;
    Ok(data_type)
}

fn validate_function_data_type(data_type: &DataType) -> Result<(), String> {
    match data_type {
        DataType::Struct(key) if key.trim().is_empty() => {
            Err("Function Struct type key must not be empty".into())
        }
        DataType::Array(inner) | DataType::DataSeries(inner) => validate_function_data_type(inner),
        DataType::OneOf(inner) => inner.iter().try_for_each(validate_function_data_type),
        _ => Ok(()),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorNodeProjectionDto {
    pub graph_path: Box<str>,
    pub source_revision: u64,
    pub node_id: Box<str>,
    pub node_type_id: Box<str>,
    pub position: NodePositionDto,
    pub display: NodeDisplayDto,
    pub ports: Vec<ResolvedPortDto>,
    pub parameter_editors: Vec<ParameterEditorDto>,
    pub capabilities: NodeCapabilitiesDto,
    pub diagnostics: Vec<DiagnosticDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodePositionDto {
    pub x: f64,
    pub y: f64,
}

impl PartialEq for NodePositionDto {
    fn eq(&self, other: &Self) -> bool {
        self.x.to_bits() == other.x.to_bits() && self.y.to_bits() == other.y.to_bits()
    }
}

impl Eq for NodePositionDto {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorConnectionProjectionDto {
    pub connection_id: Box<str>,
    pub output: PortAddressDto,
    pub input: PortAddressDto,
    pub order: Option<Box<str>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeDisplayDto {
    pub title: Box<str>,
    pub description: Option<Box<str>>,
    pub user_label: Option<Box<str>>,
    pub icon_id: Option<Box<str>>,
    pub style_id: Option<Box<str>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeCapabilitiesDto {
    pub managed: bool,
    pub can_copy: bool,
    pub can_delete: bool,
    pub can_edit_label: bool,
    pub can_edit_parameters: bool,
    pub has_dynamic_ports: bool,
    pub supports_inline_literals: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedPortDto {
    pub address: PortAddressDto,
    pub template_key: Box<str>,
    pub display: PortDisplayDto,
    pub direction: PortDirectionDto,
    pub kind: PortKindDto,
    pub instance_kind: PortInstanceKindDto,
    pub orphan: bool,
    pub can_remove: bool,
    pub connections: PortConnectionCapabilityDto,
    pub input: Option<EditorInputBindingDto>,
    pub resolved_type: Option<TypeSummaryDto>,
    pub resolved_schema: Option<SchemaSummaryDto>,
    pub status: ResolvedPortStatusDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortDisplayDto {
    pub label: Box<str>,
    pub instance_label: Option<Box<str>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PortDirectionDto {
    Input,
    Output,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PortKindDto {
    Data,
    Control,
    Effect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PortInstanceKindDto {
    Declared,
    UserCreated,
    Derived,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortConnectionCapabilityDto {
    pub current: u32,
    pub maximum: Option<u32>,
    pub ordered: bool,
    pub can_append: bool,
    pub can_replace: bool,
    pub can_move: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorInputBindingDto {
    pub literal_override: Option<serde_json::Value>,
    pub protocol_default: Option<serde_json::Value>,
    pub effective: EffectiveInputBindingKindDto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EffectiveInputBindingKindDto {
    Connections,
    Literal,
    ProtocolDefault,
    Unbound,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeSummaryDto {
    pub display: Box<str>,
    pub resolved: bool,
    pub data_type: Option<DataType>,
    #[serde(skip)]
    pub(crate) internal_type_expr: Option<TypeExpr>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaSummaryDto {
    pub kind: SchemaSummaryKindDto,
    pub fields: Vec<SchemaFieldDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaFieldDto {
    pub name: Box<str>,
    pub scalar_type: RelationalScalarTypeDto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RelationalScalarTypeDto {
    Boolean,
    Int64,
    Float64,
    String,
    Date,
    DateTime,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SchemaSummaryKindDto {
    Input,
    Project,
    Append,
    Rename,
    Filter,
    Derived,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ResolvedPortStatusDto {
    Resolved,
    Orphan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParameterEditorDto {
    pub key: Box<str>,
    pub display: ParameterDisplayDto,
    pub editor: ParameterEditorKindDto,
    pub presentation: ParameterPresentationDto,
    pub value_type: Option<DataType>,
    pub multiline: bool,
    pub value: Option<serde_json::Value>,
    pub configuration: Option<SchemaAwareParameterEditorDto>,
    pub inherited_value: Option<serde_json::Value>,
    pub value_source: Option<ParameterValueSourceDto>,
    pub options: Option<Vec<Box<str>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ParameterValueSourceDto {
    Project,
    Node,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ParameterPresentationDto {
    DetailPanel,
    InlineAndDetail,
}

impl From<ParameterPresentation> for ParameterPresentationDto {
    fn from(value: ParameterPresentation) -> Self {
        match value {
            ParameterPresentation::DetailPanel => Self::DetailPanel,
            ParameterPresentation::InlineAndDetail => Self::InlineAndDetail,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum SchemaAwareParameterEditorDto {
    ProjectColumns {
        available: bool,
        unavailable_reason: Option<Box<str>>,
        options: Vec<DataframeColumnOptionDto>,
        value: Vec<Box<str>>,
    },
    FilterPredicate {
        available: bool,
        unavailable_reason: Option<Box<str>>,
        columns: Vec<FilterColumnOptionDto>,
        value: Option<serde_json::Value>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataframeColumnOptionDto {
    pub name: Box<str>,
    pub data_type: RelationalScalarTypeDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilterColumnOptionDto {
    pub name: Box<str>,
    pub data_type: RelationalScalarTypeDto,
    pub operators: Vec<crate::node_system::protocol::dataframe::FilterOperator>,
    pub literal_types: Vec<FilterLiteralTypeDto>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FilterLiteralTypeDto {
    Boolean,
    Integer,
    Decimal,
    String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParameterDisplayDto {
    pub title: Box<str>,
    pub description: Option<Box<str>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ParameterEditorKindDto {
    Auto,
    Text,
    Number,
    Toggle,
    Select,
    Resource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticDto {
    pub code: Box<str>,
    pub message: Box<str>,
    pub severity: DiagnosticSeverityDto,
    pub blocking: bool,
    pub location: DiagnosticLocationDto,
    pub related: Vec<DiagnosticLocationDto>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DiagnosticSeverityDto {
    Error,
    Warning,
    Information,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum DiagnosticLocationDto {
    Graph,
    Node { node_id: Box<str> },
    Port { address: PortAddressDto },
    Connection { connection_id: Box<str> },
    Parameter { node_id: Box<str>, key: Box<str> },
    Resource { identity: Box<str> },
}

/// A revision transition containing complete node replacements. Port additions and
/// removals are intentionally not represented as independently applicable fragments.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphProjectionDelta {
    pub from_basis: ProjectionBasis,
    pub to_basis: ProjectionBasis,
    pub removed_node_ids: Vec<Box<str>>,
    pub node_replacements: Vec<EditorNodeProjectionDto>,
    pub connections: Vec<EditorConnectionProjectionDto>,
    pub diagnostics: Vec<DiagnosticDto>,
    pub outcome: CompilationOutcomeDto,
    pub has_blocking_diagnostics: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectionError {
    RevisionMismatch {
        analysis: GraphRevision,
        document: GraphRevision,
    },
    RegistryMismatch,
    StaleProjectionBasis,
    IncompatibleProjectionGraphs,
    InvalidDelta,
}

impl std::fmt::Display for ProjectionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RevisionMismatch { analysis, document } => write!(
                formatter,
                "analysis revision {} does not match document revision {}",
                analysis.get(),
                document.get()
            ),
            Self::RegistryMismatch => {
                formatter.write_str("analysis registry fingerprint does not match registry")
            }
            Self::StaleProjectionBasis => {
                formatter.write_str("projection delta does not start at the current basis")
            }
            Self::IncompatibleProjectionGraphs => {
                formatter.write_str("projection snapshots belong to different graphs")
            }
            Self::InvalidDelta => {
                formatter.write_str("projection delta is internally inconsistent")
            }
        }
    }
}

impl std::error::Error for ProjectionError {}

type EditorAnalysis = AnalysisSnapshot<
    GraphRevision,
    NodeId,
    PortAddress,
    ConnectionId,
    Box<str>,
    serde_json::Value,
    TypeExpr,
    SchemaExpr,
>;

type EditorDiagnostic = NodeDiagnostic<NodeId, PortAddress, ConnectionId, Box<str>>;

pub fn build_editor_graph_projection(
    graph_path: &str,
    document: &GraphDocument,
    analysis: &EditorAnalysis,
    outcome: &crate::node_system::compiler::CompilationOutcome,
    registry: &NodeRegistry,
    localization: &impl LocalizationLookup,
) -> Result<EditorGraphProjectionDto, ProjectionError> {
    EditorGraphProjectionDto::from_compilation_sources(
        graph_path,
        analysis,
        outcome,
        document,
        registry,
        localization,
        &crate::project::ProjectComputationSettings::default(),
    )
}

impl EditorGraphProjectionDto {
    #[cfg(test)]
    pub fn from_sources(
        graph_path: impl Into<Box<str>>,
        analysis: &EditorAnalysis,
        document: &GraphDocument,
        registry: &NodeRegistry,
        localization: &impl LocalizationLookup,
    ) -> Result<Self, ProjectionError> {
        let outcome = if analysis.has_blocking_errors() {
            crate::node_system::compiler::CompilationOutcome::AnalysisBlocked
        } else {
            crate::node_system::compiler::CompilationOutcome::Succeeded
        };
        Self::from_compilation_sources(
            graph_path,
            analysis,
            &outcome,
            document,
            registry,
            localization,
            &crate::project::ProjectComputationSettings::default(),
        )
    }

    pub fn from_compilation_sources(
        graph_path: impl Into<Box<str>>,
        analysis: &EditorAnalysis,
        outcome: &crate::node_system::compiler::CompilationOutcome,
        document: &GraphDocument,
        registry: &NodeRegistry,
        localization: &impl LocalizationLookup,
        computation_settings: &crate::project::ProjectComputationSettings,
    ) -> Result<Self, ProjectionError> {
        validate_sources(analysis, document, registry)?;

        let graph_path = graph_path.into();
        let source_revision = document.revision.get();
        let basis = ProjectionBasis {
            graph_path: graph_path.clone(),
            graph_revision: analysis.basis.graph_revision.get(),
            registry_fingerprint: analysis.basis.registry_fingerprint.clone(),
            resource_versions: analysis.basis.resource_versions.clone(),
        };
        let analyzed_nodes = analysis
            .nodes
            .iter()
            .map(|node| (node.node_id, node))
            .collect::<BTreeMap<_, _>>();
        let interfaces = analysis
            .resolved_interfaces
            .iter()
            .map(|interface| (interface.node_id, interface))
            .collect::<BTreeMap<_, _>>();
        let diagnostics = analysis
            .diagnostics
            .iter()
            .map(|diagnostic| project_diagnostic(diagnostic, localization))
            .collect::<Vec<_>>();
        let connections = document
            .connections
            .values()
            .map(|connection| EditorConnectionProjectionDto {
                connection_id: connection.id.to_string().into(),
                output: project_address(&connection.output),
                input: project_address(&connection.input),
                order: connection.order.as_ref().map(|order| order.0.clone()),
            })
            .collect();

        let nodes = document
            .nodes
            .values()
            .map(|node| {
                let protocol = registry.get(&node.node_type).map(|entry| entry.protocol());
                let normalized_node = analyzed_nodes.get(&node.id).copied();
                let normalized = normalized_node.map(|node| &node.normalized_parameters);
                let ports = interfaces
                    .get(&node.id)
                    .map(|interface| {
                        interface
                            .ports
                            .iter()
                            .filter_map(|port| {
                                let protocol = protocol?;
                                let spec = protocol
                                    .interface
                                    .ports
                                    .iter()
                                    .find(|spec| spec.key == port.template)?;
                                let orphan = port.status == ResolvedPortStatus::Orphan;
                                let instance_kind = project_instance_kind(&spec.instances);
                                let group =
                                    protocol.interface.member_group_for_template(&port.template);
                                let (minimum, instance_count, member_complete) =
                                    if let Some(group) = group {
                                        let state = port_member_group_state(
                                            node.id,
                                            group,
                                            document.port_bindings.iter(),
                                        );
                                        (
                                            group.min,
                                            state.complete_count(),
                                            state.address_is_complete(&port.address),
                                        )
                                    } else {
                                        (
                                            match &spec.instances {
                                                PortInstances::UserCreated { min, .. } => *min,
                                                _ => 0,
                                            },
                                            interface
                                                .ports
                                                .iter()
                                                .filter(|candidate| {
                                                    candidate.template == port.template
                                                })
                                                .filter(|candidate| candidate.address.is_instance())
                                                .count(),
                                            true,
                                        )
                                    };
                                let can_remove = can_remove_port(
                                    &port.address,
                                    orphan,
                                    &spec.instances,
                                    minimum,
                                    instance_count,
                                    member_complete,
                                );
                                let connections = project_connection_capability(
                                    document,
                                    &port.address,
                                    spec.connections,
                                    orphan,
                                );
                                let input = (port.direction == PortDirection::Input).then(|| {
                                    let literal_override = document
                                        .input_states
                                        .get(&port.address)
                                        .and_then(|state| state.literal_override.clone());
                                    let protocol_default = spec
                                        .input_binding
                                        .as_ref()
                                        .and_then(|binding| binding.default_value.as_ref())
                                        .map(|default| {
                                            serde_json::to_value(&default.value)
                                                .expect("protocol values must serialize")
                                        });
                                    let effective = project_effective_input_binding(
                                        document.effective_input_binding(
                                            &port.address,
                                            protocol_default.clone(),
                                        ),
                                    );
                                    EditorInputBindingDto {
                                        literal_override,
                                        protocol_default,
                                        effective,
                                    }
                                });
                                let instance_label = port.instance_label.clone();
                                let label = instance_label.clone().unwrap_or_else(|| {
                                    localization.text(&spec.label_key, &DiagnosticArguments::new())
                                });
                                Some(ResolvedPortDto {
                                    address: project_address(&port.address),
                                    template_key: port.template.as_str().into(),
                                    display: PortDisplayDto {
                                        label,
                                        instance_label,
                                    },
                                    direction: port.direction.into(),
                                    kind: port.kind.into(),
                                    instance_kind,
                                    orphan,
                                    can_remove,
                                    connections,
                                    input,
                                    resolved_type: Some(project_type_summary(
                                        analysis
                                            .partial_types
                                            .get(&port.address)
                                            .unwrap_or(&port.value_type),
                                    )),
                                    resolved_schema: analysis
                                        .partial_schemas
                                        .get(&port.address)
                                        .map(|expression| {
                                            project_schema_summary(
                                                expression,
                                                analysis.resolved_schemas.get(&port.address),
                                            )
                                        }),
                                    status: port.status.into(),
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let parameter_editors = protocol
                    .map(|protocol| {
                        protocol
                            .parameters
                            .parameters
                            .iter()
                            .filter_map(|parameter| {
                                let (editor, multiline) =
                                    project_parameter_editor(&parameter.editor)?;
                                let value = normalized
                                    .and_then(|values| values.get(&parameter.key))
                                    .cloned()
                                    .or_else(|| node.parameters.get(&parameter.key).cloned());
                                let source_schema =
                                    analysis.resolved_schemas.get(&PortAddress::declared(
                                        node.id,
                                        PortKey::new("source").expect("static source port key"),
                                    ));
                                let unavailable_reason = localization.text(
                                    &I18nKey::new("editors.dataframe.connect_source")
                                        .expect("static editor localization key"),
                                    &DiagnosticArguments::new(),
                                );
                                Some(ParameterEditorDto {
                                    key: parameter.key.as_str().into(),
                                    display: ParameterDisplayDto {
                                        title: localization.text(
                                            &parameter.title_key,
                                            &DiagnosticArguments::new(),
                                        ),
                                        description: parameter.description_key.as_ref().map(
                                            |key| {
                                                localization.text(key, &DiagnosticArguments::new())
                                            },
                                        ),
                                    },
                                    editor,
                                    presentation: parameter.presentation.into(),
                                    value_type: project_data_type(&parameter.value_type),
                                    multiline,
                                    value: value.clone(),
                                    configuration: project_schema_aware_editor(
                                        node.node_type.as_str(),
                                        value.as_ref(),
                                        source_schema,
                                        unavailable_reason,
                                    ),
                                    inherited_value: inherited_statistics_parameter_value(
                                        node.node_type.as_str(),
                                        parameter.key.as_str(),
                                        computation_settings,
                                    ),
                                    value_source: inherited_statistics_parameter_value(
                                        node.node_type.as_str(),
                                        parameter.key.as_str(),
                                        computation_settings,
                                    )
                                    .map(|_| {
                                        if value.is_some() {
                                            ParameterValueSourceDto::Node
                                        } else {
                                            ParameterValueSourceDto::Project
                                        }
                                    }),
                                    options: statistics_parameter_options(
                                        node.node_type.as_str(),
                                        parameter.key.as_str(),
                                    ),
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let node_diagnostics = analysis
                    .diagnostics
                    .iter()
                    .filter(|diagnostic| diagnostic_belongs_to_node(diagnostic, node.id, document))
                    .map(|diagnostic| project_diagnostic(diagnostic, localization))
                    .collect();
                let display = protocol.map_or_else(
                    || NodeDisplayDto {
                        title: node.node_type.as_str().into(),
                        description: None,
                        user_label: node.user_label.as_deref().map(Into::into),
                        icon_id: None,
                        style_id: None,
                    },
                    |protocol| NodeDisplayDto {
                        title: normalized_node
                            .and_then(|node| node.instance_title.clone())
                            .unwrap_or_else(|| {
                                localization
                                    .text(&protocol.catalog.title_key, &DiagnosticArguments::new())
                            }),
                        description: protocol
                            .catalog
                            .description_key
                            .as_ref()
                            .map(|key| localization.text(key, &DiagnosticArguments::new())),
                        user_label: node.user_label.as_deref().map(Into::into),
                        icon_id: Some(protocol.catalog.icon_id.as_str().into()),
                        style_id: Some(protocol.catalog.style_id.as_str().into()),
                    },
                );
                let capabilities = project_node_capabilities(protocol);
                EditorNodeProjectionDto {
                    graph_path: graph_path.clone(),
                    source_revision,
                    node_id: node.id.to_string().into(),
                    node_type_id: node.node_type.as_str().into(),
                    position: NodePositionDto {
                        x: node.position.x,
                        y: node.position.y,
                    },
                    display,
                    ports,
                    parameter_editors,
                    capabilities,
                    diagnostics: node_diagnostics,
                }
            })
            .collect();

        let outcome = CompilationOutcomeDto::from(outcome);
        let has_blocking_diagnostics = !matches!(outcome, CompilationOutcomeDto::Success)
            || diagnostics.iter().any(|diagnostic| diagnostic.blocking);
        Ok(Self {
            basis,
            graph_path,
            source_revision,
            nodes,
            connections,
            diagnostics,
            outcome,
            has_blocking_diagnostics,
        })
    }

    /// Applies a complete revision transition only when its old basis exactly
    /// matches this projection. Validation and replacement happen before commit.
    pub fn apply_delta(&mut self, delta: GraphProjectionDelta) -> Result<(), ProjectionError> {
        if self.basis != delta.from_basis {
            return Err(ProjectionError::StaleProjectionBasis);
        }
        validate_delta(&delta)?;

        let removed = delta.removed_node_ids.iter().collect::<BTreeSet<_>>();
        let replacements = delta
            .node_replacements
            .iter()
            .map(|node| (node.node_id.as_ref(), node))
            .collect::<BTreeMap<_, _>>();
        let mut next_nodes = self
            .nodes
            .iter()
            .filter(|node| !removed.contains(&node.node_id))
            .filter(|node| !replacements.contains_key(node.node_id.as_ref()))
            .cloned()
            .collect::<Vec<_>>();
        next_nodes.extend(delta.node_replacements.iter().cloned());
        next_nodes.sort_by(|left, right| left.node_id.cmp(&right.node_id));

        self.basis = delta.to_basis;
        self.graph_path = self.basis.graph_path.clone();
        self.source_revision = self.basis.graph_revision;
        self.nodes = next_nodes;
        self.connections = delta.connections;
        self.diagnostics = delta.diagnostics;
        self.outcome = delta.outcome;
        self.has_blocking_diagnostics = delta.has_blocking_diagnostics;
        Ok(())
    }
}

impl GraphProjectionDelta {
    pub fn between(
        previous: &EditorGraphProjectionDto,
        next: &EditorGraphProjectionDto,
    ) -> Result<Self, ProjectionError> {
        if previous.basis.graph_path != next.basis.graph_path {
            return Err(ProjectionError::IncompatibleProjectionGraphs);
        }
        let previous_nodes = previous
            .nodes
            .iter()
            .map(|node| (node.node_id.as_ref(), node))
            .collect::<BTreeMap<_, _>>();
        let next_nodes = next
            .nodes
            .iter()
            .map(|node| (node.node_id.as_ref(), node))
            .collect::<BTreeMap<_, _>>();
        let removed_node_ids = previous_nodes
            .keys()
            .filter(|node_id| !next_nodes.contains_key(**node_id))
            .map(|node_id| Box::<str>::from(*node_id))
            .collect();
        let node_replacements = next
            .nodes
            .iter()
            .filter(|node| previous_nodes.get(node.node_id.as_ref()).copied() != Some(*node))
            .cloned()
            .collect();

        Ok(Self {
            from_basis: previous.basis.clone(),
            to_basis: next.basis.clone(),
            removed_node_ids,
            node_replacements,
            connections: next.connections.clone(),
            diagnostics: next.diagnostics.clone(),
            outcome: next.outcome.clone(),
            has_blocking_diagnostics: next.has_blocking_diagnostics,
        })
    }
}

fn project_effective_input_binding(binding: EffectiveInputBinding) -> EffectiveInputBindingKindDto {
    match binding {
        EffectiveInputBinding::Connections(_) => EffectiveInputBindingKindDto::Connections,
        EffectiveInputBinding::Literal(_) => EffectiveInputBindingKindDto::Literal,
        EffectiveInputBinding::ProtocolDefault(_) => EffectiveInputBindingKindDto::ProtocolDefault,
        EffectiveInputBinding::Unbound => EffectiveInputBindingKindDto::Unbound,
    }
}

fn validate_sources(
    analysis: &EditorAnalysis,
    document: &GraphDocument,
    registry: &NodeRegistry,
) -> Result<(), ProjectionError> {
    if analysis.basis.graph_revision != document.revision {
        return Err(ProjectionError::RevisionMismatch {
            analysis: analysis.basis.graph_revision,
            document: document.revision,
        });
    }
    if &analysis.basis.registry_fingerprint != registry.fingerprint() {
        return Err(ProjectionError::RegistryMismatch);
    }
    Ok(())
}

fn validate_delta(delta: &GraphProjectionDelta) -> Result<(), ProjectionError> {
    if delta.from_basis.graph_path != delta.to_basis.graph_path
        || delta.to_basis.graph_revision < delta.from_basis.graph_revision
        || matches!(delta.outcome, CompilationOutcomeDto::Success) == delta.has_blocking_diagnostics
        || delta.node_replacements.iter().any(|node| {
            node.graph_path != delta.to_basis.graph_path
                || node.source_revision != delta.to_basis.graph_revision
        })
    {
        return Err(ProjectionError::InvalidDelta);
    }
    let mut identities = BTreeSet::new();
    if delta
        .node_replacements
        .iter()
        .any(|node| !identities.insert(node.node_id.as_ref()))
        || delta
            .removed_node_ids
            .iter()
            .any(|node_id| identities.contains(node_id.as_ref()))
    {
        return Err(ProjectionError::InvalidDelta);
    }
    Ok(())
}

fn project_node_capabilities(
    protocol: Option<&crate::node_system::protocol::NodeProtocol>,
) -> NodeCapabilitiesDto {
    let managed = protocol.is_some_and(|protocol| protocol.managed_role.is_some());
    NodeCapabilitiesDto {
        managed,
        can_copy: !managed,
        can_delete: !managed,
        can_edit_label: true,
        can_edit_parameters: protocol.is_some_and(|protocol| {
            protocol
                .parameters
                .parameters
                .iter()
                .any(|parameter| !matches!(parameter.editor, ParameterEditorSpec::Hidden))
        }),
        has_dynamic_ports: protocol.is_some_and(|protocol| {
            protocol
                .interface
                .ports
                .iter()
                .any(|port| !matches!(port.instances, PortInstances::Declared))
        }),
        supports_inline_literals: protocol.is_some_and(|protocol| {
            protocol
                .interface
                .ports
                .iter()
                .any(|port| matches!(port.editor, PortEditorSpec::InlineLiteral))
        }),
    }
}

fn project_instance_kind(instances: &PortInstances) -> PortInstanceKindDto {
    match instances {
        PortInstances::Declared => PortInstanceKindDto::Declared,
        PortInstances::UserCreated { .. } => PortInstanceKindDto::UserCreated,
        PortInstances::Derived { .. } => PortInstanceKindDto::Derived,
    }
}

fn can_remove_port(
    address: &PortAddress,
    orphan: bool,
    instances: &PortInstances,
    minimum: u16,
    instance_count: usize,
    member_complete: bool,
) -> bool {
    if !address.is_instance() {
        return false;
    }
    if orphan {
        return true;
    }
    matches!(instances, PortInstances::UserCreated { .. })
        && (!member_complete || instance_count > usize::from(minimum))
}

fn project_connection_capability(
    document: &GraphDocument,
    address: &PortAddress,
    capability: ConnectionsPerPort,
    orphan: bool,
) -> PortConnectionCapabilityDto {
    let current = document
        .connections
        .values()
        .filter(|connection| connection.input == *address || connection.output == *address)
        .count() as u32;
    let (maximum, ordered) = match capability {
        ConnectionsPerPort::Single => (Some(1), false),
        ConnectionsPerPort::Multiple { max, ordered } => (max.map(u32::from), ordered),
    };
    PortConnectionCapabilityDto {
        current,
        maximum,
        ordered,
        can_append: !orphan && maximum.is_none_or(|maximum| current < maximum),
        can_replace: !orphan && capability == ConnectionsPerPort::Single && current == 1,
        can_move: !orphan && current > 0,
    }
}

fn project_type_summary(value: &TypeExpr) -> TypeSummaryDto {
    TypeSummaryDto {
        display: type_display(value).into(),
        resolved: type_is_resolved(value),
        data_type: project_data_type(value),
        internal_type_expr: Some(value.clone()),
    }
}

pub(crate) fn project_data_type(value: &TypeExpr) -> Option<DataType> {
    match value {
        TypeExpr::Concrete(id) => Some(match id.as_str() {
            "core.bool" => DataType::Boolean,
            "core.int64" => DataType::Int64,
            "core.float64" => DataType::Float64,
            "core.string" => DataType::String,
            "core.date" => DataType::Date,
            "core.datetime" => DataType::Datetime,
            "core.time" => DataType::Time,
            "core.categorical" => DataType::Categorical,
            "core.object" => DataType::Object,
            "tabular.dataframe" => DataType::DataFrame,
            semantic_id => DataType::Struct(semantic_id.to_owned()),
        }),
        TypeExpr::Applied {
            constructor,
            arguments,
        } if constructor.as_str() == "core.data_series" && arguments.len() == 1 => {
            project_data_type(&arguments[0]).map(|element| DataType::DataSeries(Box::new(element)))
        }
        TypeExpr::Applied {
            constructor,
            arguments,
        } if constructor.as_str() == "core.array" && arguments.len() == 1 => {
            project_data_type(&arguments[0]).map(|element| DataType::Array(Box::new(element)))
        }
        TypeExpr::Applied { .. } => None,
        TypeExpr::Union(values) if !values.is_empty() => values
            .iter()
            .map(project_data_type)
            .collect::<Option<Vec<_>>>()
            .map(DataType::one_of),
        TypeExpr::Unknown => None,
        TypeExpr::Union(_) | TypeExpr::Generic(_) => None,
    }
}

fn type_display(value: &TypeExpr) -> String {
    match value {
        TypeExpr::Concrete(id) => id.as_str().to_owned(),
        TypeExpr::Generic(id) => id.as_str().to_owned(),
        TypeExpr::Applied {
            constructor,
            arguments,
        } => format!(
            "{}<{}>",
            constructor.as_str(),
            arguments
                .iter()
                .map(type_display)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        TypeExpr::Union(values) => values
            .iter()
            .map(type_display)
            .collect::<Vec<_>>()
            .join(" | "),
        TypeExpr::Unknown => "unknown".to_owned(),
    }
}

fn type_is_resolved(value: &TypeExpr) -> bool {
    match value {
        TypeExpr::Concrete(_) => true,
        TypeExpr::Applied { arguments, .. } | TypeExpr::Union(arguments) => {
            arguments.iter().all(type_is_resolved)
        }
        TypeExpr::Generic(_) | TypeExpr::Unknown => false,
    }
}

fn project_schema_summary(
    value: &SchemaExpr,
    resolved: Option<&ResolvedSchemaFact>,
) -> SchemaSummaryDto {
    let kind = match value {
        SchemaExpr::Input(_) => SchemaSummaryKindDto::Input,
        SchemaExpr::Project { .. } => SchemaSummaryKindDto::Project,
        SchemaExpr::Append { .. } => SchemaSummaryKindDto::Append,
        SchemaExpr::Rename { .. } => SchemaSummaryKindDto::Rename,
        SchemaExpr::Filter { .. } => SchemaSummaryKindDto::Filter,
        SchemaExpr::Derived { .. } => SchemaSummaryKindDto::Derived,
    };
    let fields = resolved
        .into_iter()
        .flat_map(|fact| fact.fields.iter())
        .map(|field| SchemaFieldDto {
            name: field.name.0.clone(),
            scalar_type: relational_scalar_type_dto(field.scalar_type),
        })
        .collect();
    SchemaSummaryDto { kind, fields }
}

fn inherited_statistics_parameter_value(
    node_type_id: &str,
    key: &str,
    settings: &crate::project::ProjectComputationSettings,
) -> Option<serde_json::Value> {
    if !node_type_id.starts_with("yssbi.statistics.") {
        return None;
    }
    match key {
        "convergence_tolerance" => {
            serde_json::Number::from_f64(settings.numeric.tolerance.absolute)
                .map(serde_json::Value::Number)
        }
        "missing_value_policy" => Some(serde_json::Value::String(
            match settings.missing_values.statistics {
                crate::project::StatisticalMissingValuePolicy::Listwise => "Listwise",
                crate::project::StatisticalMissingValuePolicy::Reject => "Reject",
            }
            .to_owned(),
        )),
        _ => None,
    }
}

fn statistics_parameter_options(node_type_id: &str, key: &str) -> Option<Vec<Box<str>>> {
    (node_type_id.starts_with("yssbi.statistics.") && key == "missing_value_policy")
        .then(|| vec!["Listwise".into(), "Reject".into()])
}

fn project_schema_aware_editor(
    node_type_id: &str,
    value: Option<&serde_json::Value>,
    source_schema: Option<&ResolvedSchemaFact>,
    unavailable_reason: Box<str>,
) -> Option<SchemaAwareParameterEditorDto> {
    use crate::node_system::protocol::dataframe::{FilterPredicate, ProjectColumns};

    let available = source_schema.is_some();
    let unavailable_reason = (!available).then_some(unavailable_reason);
    match node_type_id {
        "yssbi.dataframe.project" => Some(SchemaAwareParameterEditorDto::ProjectColumns {
            available,
            unavailable_reason,
            options: source_schema
                .into_iter()
                .flat_map(|fact| fact.fields.iter())
                .map(project_dataframe_column_option)
                .collect(),
            value: value
                .and_then(|value| serde_json::from_value::<ProjectColumns>(value.clone()).ok())
                .map(|columns| columns.as_slice().to_vec())
                .unwrap_or_default(),
        }),
        "yssbi.dataframe.filter.rows" => Some(SchemaAwareParameterEditorDto::FilterPredicate {
            available,
            unavailable_reason,
            columns: source_schema
                .into_iter()
                .flat_map(|fact| fact.fields.iter())
                .map(|field| FilterColumnOptionDto {
                    name: field.name.0.clone(),
                    data_type: relational_scalar_type_dto(field.scalar_type),
                    operators: filter_operators(field.scalar_type),
                    literal_types: filter_literal_types(field.scalar_type),
                })
                .collect(),
            value: value
                .and_then(|value| serde_json::from_value::<FilterPredicate>(value.clone()).ok())
                .and_then(|predicate| serde_json::to_value(predicate).ok()),
        }),
        _ => return None,
    }
}

fn project_dataframe_column_option(
    field: &crate::node_system::protocol::SchemaField,
) -> DataframeColumnOptionDto {
    DataframeColumnOptionDto {
        name: field.name.0.clone(),
        data_type: relational_scalar_type_dto(field.scalar_type),
    }
}

fn relational_scalar_type_dto(value: RelationalScalarType) -> RelationalScalarTypeDto {
    match value {
        RelationalScalarType::Boolean => RelationalScalarTypeDto::Boolean,
        RelationalScalarType::Int64 => RelationalScalarTypeDto::Int64,
        RelationalScalarType::Float64 => RelationalScalarTypeDto::Float64,
        RelationalScalarType::String => RelationalScalarTypeDto::String,
        RelationalScalarType::Date => RelationalScalarTypeDto::Date,
        RelationalScalarType::DateTime => RelationalScalarTypeDto::DateTime,
        RelationalScalarType::Unknown => RelationalScalarTypeDto::Unknown,
    }
}

fn filter_literal_types(scalar_type: RelationalScalarType) -> Vec<FilterLiteralTypeDto> {
    match scalar_type {
        RelationalScalarType::Boolean => vec![FilterLiteralTypeDto::Boolean],
        RelationalScalarType::Int64 => vec![FilterLiteralTypeDto::Integer],
        RelationalScalarType::Float64 => {
            vec![FilterLiteralTypeDto::Integer, FilterLiteralTypeDto::Decimal]
        }
        RelationalScalarType::String => vec![FilterLiteralTypeDto::String],
        RelationalScalarType::Date
        | RelationalScalarType::DateTime
        | RelationalScalarType::Unknown => vec![],
    }
}

fn filter_operators(
    scalar_type: RelationalScalarType,
) -> Vec<crate::node_system::protocol::dataframe::FilterOperator> {
    use crate::node_system::protocol::dataframe::FilterOperator::*;
    match scalar_type {
        RelationalScalarType::Boolean => vec![Equal, NotEqual, IsNull, IsNotNull],
        RelationalScalarType::Int64
        | RelationalScalarType::Float64
        | RelationalScalarType::String => vec![
            Equal,
            NotEqual,
            LessThan,
            LessThanOrEqual,
            GreaterThan,
            GreaterThanOrEqual,
            IsNull,
            IsNotNull,
        ],
        RelationalScalarType::Date | RelationalScalarType::DateTime => {
            vec![IsNull, IsNotNull]
        }
        RelationalScalarType::Unknown => vec![],
    }
}

fn project_parameter_editor(
    editor: &ParameterEditorSpec,
) -> Option<(ParameterEditorKindDto, bool)> {
    Some(match editor {
        ParameterEditorSpec::Auto => (ParameterEditorKindDto::Auto, false),
        ParameterEditorSpec::Hidden => return None,
        ParameterEditorSpec::Text { multiline } => (ParameterEditorKindDto::Text, *multiline),
        ParameterEditorSpec::Number => (ParameterEditorKindDto::Number, false),
        ParameterEditorSpec::Toggle => (ParameterEditorKindDto::Toggle, false),
        ParameterEditorSpec::Select => (ParameterEditorKindDto::Select, false),
        ParameterEditorSpec::Resource { .. } => (ParameterEditorKindDto::Resource, false),
    })
}

fn project_address(address: &PortAddress) -> PortAddressDto {
    address.into()
}

fn project_diagnostic(
    diagnostic: &EditorDiagnostic,
    localization: &impl LocalizationLookup,
) -> DiagnosticDto {
    DiagnosticDto {
        code: diagnostic.code.as_str().into(),
        message: localization.text(&diagnostic.message_key, &diagnostic.arguments),
        severity: diagnostic.severity.into(),
        blocking: diagnostic.severity.is_blocking(),
        location: project_location(&diagnostic.primary),
        related: diagnostic.related.iter().map(project_location).collect(),
    }
}

fn project_location(
    location: &DiagnosticLocation<NodeId, PortAddress, ConnectionId, Box<str>>,
) -> DiagnosticLocationDto {
    match location {
        DiagnosticLocation::Graph => DiagnosticLocationDto::Graph,
        DiagnosticLocation::Node(node_id) => DiagnosticLocationDto::Node {
            node_id: node_id.to_string().into(),
        },
        DiagnosticLocation::Port(address) => DiagnosticLocationDto::Port {
            address: project_address(address),
        },
        DiagnosticLocation::Connection(connection_id) => DiagnosticLocationDto::Connection {
            connection_id: connection_id.to_string().into(),
        },
        DiagnosticLocation::Parameter { node_id, key } => DiagnosticLocationDto::Parameter {
            node_id: node_id.to_string().into(),
            key: key.as_str().into(),
        },
        DiagnosticLocation::Resource(identity) => DiagnosticLocationDto::Resource {
            identity: identity.clone(),
        },
    }
}

fn diagnostic_belongs_to_node(
    diagnostic: &EditorDiagnostic,
    node_id: NodeId,
    document: &GraphDocument,
) -> bool {
    match &diagnostic.primary {
        DiagnosticLocation::Node(id) | DiagnosticLocation::Parameter { node_id: id, .. } => {
            *id == node_id
        }
        DiagnosticLocation::Port(address) => address.node_id == node_id,
        DiagnosticLocation::Connection(connection_id) => document
            .connections
            .get(connection_id)
            .is_some_and(|connection| {
                connection.input.node_id == node_id || connection.output.node_id == node_id
            }),
        DiagnosticLocation::Graph | DiagnosticLocation::Resource(_) => false,
    }
}

impl From<PortDirection> for PortDirectionDto {
    fn from(value: PortDirection) -> Self {
        match value {
            PortDirection::Input => Self::Input,
            PortDirection::Output => Self::Output,
        }
    }
}

impl From<PortKind> for PortKindDto {
    fn from(value: PortKind) -> Self {
        match value {
            PortKind::Data => Self::Data,
            PortKind::Control => Self::Control,
            PortKind::Effect => Self::Effect,
        }
    }
}

impl From<ResolvedPortStatus> for ResolvedPortStatusDto {
    fn from(value: ResolvedPortStatus) -> Self {
        match value {
            ResolvedPortStatus::Resolved => Self::Resolved,
            ResolvedPortStatus::Orphan => Self::Orphan,
        }
    }
}

impl From<DiagnosticSeverity> for DiagnosticSeverityDto {
    fn from(value: DiagnosticSeverity) -> Self {
        match value {
            DiagnosticSeverity::Error => Self::Error,
            DiagnosticSeverity::Warning => Self::Warning,
            DiagnosticSeverity::Information => Self::Information,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node_system::analysis::SemanticDependency;
    use crate::node_system::catalog::{
        CONTROL_REROUTE_NODE_TYPE, DATA_REROUTE_NODE_TYPE, EFFECT_REROUTE_NODE_TYPE,
        REROUTE_INPUT_PORT, REROUTE_OUTPUT_PORT, build_builtin_node_system,
    };
    use crate::node_system::compiler::{CompilationOutcome, GraphCompiler, ResourceSnapshot};
    use crate::node_system::document::{
        DocumentConnection, DocumentNode, DynamicMemberLocator, DynamicPortBinding,
        FunctionDocument, FunctionParameterId, FunctionSignature, GraphResourcePath, InputState,
        LastKnownPortMetadata, NodePosition, OrderKey, PortInstanceId,
    };
    use crate::node_system::protocol::{
        NodeTypeId, ParameterKey, PortKey, TypeConstructorId, TypeExpr, TypeId, TypedValue, Value,
        numeric_data_series_type,
    };
    use crate::node_system::registry::{RegistryFingerprint, TransparentNodeRole};
    use serde_json::json;
    use uuid::Uuid;

    #[test]
    fn projection_projects_unknown_without_any() {
        let summary = project_type_summary(&TypeExpr::Unknown);

        assert!(!summary.resolved);
        assert_eq!(summary.data_type, None);
    }

    #[test]
    fn projection_projects_numeric_series_union_without_legacy_ids() {
        let summary = project_type_summary(&numeric_data_series_type());

        assert!(summary.resolved);
        assert_eq!(
            summary.display.as_ref(),
            "core.data_series<core.float64> | core.data_series<core.int64>"
        );
        assert_eq!(
            summary.data_type,
            Some(DataType::one_of(vec![
                DataType::DataSeries(Box::new(DataType::Float64)),
                DataType::DataSeries(Box::new(DataType::Int64)),
            ]))
        );
    }

    #[test]
    fn type_summary_serializes_structured_data_type_without_display_parsing() {
        let summary = project_type_summary(&TypeExpr::Applied {
            constructor: TypeConstructorId::new("core.data_series").unwrap(),
            arguments: vec![TypeExpr::Concrete(TypeId::new("core.float64").unwrap())],
        });

        assert_eq!(
            serde_json::to_value(summary).unwrap(),
            json!({
                "display": "core.data_series<core.float64>",
                "resolved": true,
                "dataType": {
                    "kind": "DataSeries",
                    "inner": { "kind": "Float64" }
                }
            })
        );
        assert_eq!(
            project_data_type(&TypeExpr::Concrete(
                TypeId::new("statistics.model").unwrap()
            )),
            Some(DataType::Struct("statistics.model".into()))
        );
        assert_eq!(
            project_data_type(&TypeExpr::Generic(
                crate::node_system::protocol::TypeParameterId::new("value").unwrap()
            )),
            None
        );
    }

    fn connection_capability(
        capability: ConnectionsPerPort,
        orphan: bool,
        current: u128,
    ) -> PortConnectionCapabilityDto {
        let address = PortAddress::declared(
            NodeId::from_uuid(Uuid::from_u128(10)),
            PortKey::new("port").unwrap(),
        );
        let mut document = GraphDocument::default();
        for index in 0..current {
            let connection_id = ConnectionId::from_uuid(Uuid::from_u128(100 + index));
            document.connections.insert(
                connection_id,
                DocumentConnection {
                    id: connection_id,
                    output: address.clone(),
                    input: PortAddress::declared(
                        NodeId::from_uuid(Uuid::from_u128(1_000 + index)),
                        PortKey::new("input").unwrap(),
                    ),
                    order: None,
                },
            );
        }
        project_connection_capability(&document, &address, capability, orphan)
    }

    #[test]
    fn projects_connection_capabilities_single_append_replace_and_move_truth_table() {
        assert_eq!(
            connection_capability(ConnectionsPerPort::Single, false, 0),
            PortConnectionCapabilityDto {
                current: 0,
                maximum: Some(1),
                ordered: false,
                can_append: true,
                can_replace: false,
                can_move: false,
            }
        );
        let occupied = connection_capability(ConnectionsPerPort::Single, false, 1);
        assert_eq!(
            occupied,
            PortConnectionCapabilityDto {
                current: 1,
                maximum: Some(1),
                ordered: false,
                can_append: false,
                can_replace: true,
                can_move: true,
            }
        );
        assert_eq!(
            serde_json::to_value(occupied).unwrap(),
            json!({
                "current": 1,
                "maximum": 1,
                "ordered": false,
                "canAppend": false,
                "canReplace": true,
                "canMove": true
            })
        );
    }

    #[test]
    fn projects_connection_capabilities_loaded_single_overflow_is_never_replaceable() {
        assert_eq!(
            connection_capability(ConnectionsPerPort::Single, false, 2),
            PortConnectionCapabilityDto {
                current: 2,
                maximum: Some(1),
                ordered: false,
                can_append: false,
                can_replace: false,
                can_move: true,
            }
        );
    }

    #[test]
    fn projects_connection_capabilities_multiple_capacity_and_order_truth_table() {
        let bounded = ConnectionsPerPort::Multiple {
            max: Some(2),
            ordered: false,
        };
        assert_eq!(
            connection_capability(bounded, false, 1),
            PortConnectionCapabilityDto {
                current: 1,
                maximum: Some(2),
                ordered: false,
                can_append: true,
                can_replace: false,
                can_move: true,
            }
        );
        assert_eq!(
            connection_capability(bounded, false, 2),
            PortConnectionCapabilityDto {
                current: 2,
                maximum: Some(2),
                ordered: false,
                can_append: false,
                can_replace: false,
                can_move: true,
            }
        );
        assert_eq!(
            connection_capability(
                ConnectionsPerPort::Multiple {
                    max: None,
                    ordered: true,
                },
                false,
                2,
            ),
            PortConnectionCapabilityDto {
                current: 2,
                maximum: None,
                ordered: true,
                can_append: true,
                can_replace: false,
                can_move: true,
            }
        );
    }

    #[test]
    fn projects_connection_capabilities_disable_orphans_and_track_connected_control_effect_ports() {
        assert_eq!(
            connection_capability(ConnectionsPerPort::Single, true, 1),
            PortConnectionCapabilityDto {
                current: 1,
                maximum: Some(1),
                ordered: false,
                can_append: false,
                can_replace: false,
                can_move: false,
            }
        );
        let connected_control = connection_capability(ConnectionsPerPort::Single, false, 1);
        let connected_effect = connection_capability(
            ConnectionsPerPort::Multiple {
                max: None,
                ordered: true,
            },
            false,
            1,
        );
        assert!(connected_control.can_replace && connected_control.can_move);
        assert!(connected_effect.can_append && connected_effect.can_move);
    }

    struct EmptyResources;

    impl ResourceSnapshot for EmptyResources {
        fn versions(&self) -> ResourceVersionSet {
            BTreeMap::new()
        }
    }

    fn insert_projection_node(
        document: &mut GraphDocument,
        id: u128,
        node_type: &str,
        position: NodePosition,
    ) -> NodeId {
        let id = NodeId::from_uuid(Uuid::from_u128(id));
        document.nodes.insert(
            id,
            DocumentNode {
                id,
                node_type: NodeTypeId::new(node_type).unwrap(),
                position,
                parameters: BTreeMap::new(),
                user_label: None,
            },
        );
        id
    }

    fn insert_projection_connection(
        document: &mut GraphDocument,
        id: u128,
        output: (NodeId, &str),
        input: (NodeId, &str),
    ) {
        let id = ConnectionId::from_uuid(Uuid::from_u128(id));
        document.connections.insert(
            id,
            DocumentConnection {
                id,
                output: PortAddress::declared(output.0, PortKey::new(output.1).unwrap()),
                input: PortAddress::declared(input.0, PortKey::new(input.1).unwrap()),
                order: None,
            },
        );
    }

    fn reroute_projection_document(include_data_endpoints: bool) -> GraphDocument {
        let mut document = GraphDocument::default();
        for (index, (reroute_type, endpoint_type, output_port, input_port)) in [
            (DATA_REROUTE_NODE_TYPE, "yssbi.logic.not", "result", "input"),
            (
                CONTROL_REROUTE_NODE_TYPE,
                "yssbi.control.do",
                "then",
                "enter",
            ),
            (
                EFFECT_REROUTE_NODE_TYPE,
                "yssbi.control.do",
                "effect_out",
                "effect_in",
            ),
        ]
        .into_iter()
        .enumerate()
        {
            let y = 80.0 + index as f64 * 60.0;
            let source = (index != 0 || include_data_endpoints).then(|| {
                insert_projection_node(
                    &mut document,
                    10 + index as u128 * 2,
                    endpoint_type,
                    NodePosition { x: 0.0, y },
                )
            });

            if index == 0 && include_data_endpoints {
                document.input_states.insert(
                    PortAddress::declared(source.unwrap(), PortKey::new("input").unwrap()),
                    InputState {
                        literal_override: Some(
                            serde_json::to_value(TypedValue {
                                value_type: TypeExpr::Concrete(TypeId::new("core.bool").unwrap()),
                                value: Value::Bool(true),
                            })
                            .unwrap(),
                        ),
                    },
                );
            }
            let first = insert_projection_node(
                &mut document,
                100 + index as u128 * 2,
                reroute_type,
                NodePosition { x: 40.0, y },
            );
            let second = insert_projection_node(
                &mut document,
                101 + index as u128 * 2,
                reroute_type,
                NodePosition { x: 140.0, y },
            );
            let target = (index != 0 || include_data_endpoints).then(|| {
                insert_projection_node(
                    &mut document,
                    11 + index as u128 * 2,
                    endpoint_type,
                    NodePosition { x: 180.0, y },
                )
            });
            if index != 0 || include_data_endpoints {
                insert_projection_connection(
                    &mut document,
                    300 + index as u128 * 3,
                    (source.unwrap(), output_port),
                    (first, REROUTE_INPUT_PORT),
                );
            }
            insert_projection_connection(
                &mut document,
                301 + index as u128 * 3,
                (first, REROUTE_OUTPUT_PORT),
                (second, REROUTE_INPUT_PORT),
            );
            if index != 0 || include_data_endpoints {
                insert_projection_connection(
                    &mut document,
                    302 + index as u128 * 3,
                    (second, REROUTE_OUTPUT_PORT),
                    (target.unwrap(), input_port),
                );
            }
        }
        document
    }

    fn reroute_locale_independent_contract(
        projection: &EditorGraphProjectionDto,
    ) -> serde_json::Value {
        json!({
            "nodes": projection.nodes.iter().map(|node| json!({
                "nodeId": node.node_id,
                "nodeTypeId": node.node_type_id,
                "position": node.position,
                "styleId": node.display.style_id,
                "ports": node.ports.iter().map(|port| json!({
                    "address": port.address,
                    "templateKey": port.template_key,
                    "direction": port.direction,
                    "kind": port.kind,
                    "resolvedType": port.resolved_type,
                })).collect::<Vec<_>>(),
                "parameterEditors": node.parameter_editors,
                "capabilities": node.capabilities,
            })).collect::<Vec<_>>(),
            "connections": projection.connections,
        })
    }

    #[test]
    fn phase2_reroute_projection_preserves_all_kinds_after_semantic_collapse_across_locales() {
        let builtin = build_builtin_node_system().unwrap();
        let document = reroute_projection_document(true);
        let result =
            GraphCompiler::new(builtin.registry.as_ref(), &EmptyResources).compile(&document);

        assert_eq!(
            result.outcome,
            CompilationOutcome::Succeeded,
            "{:?}",
            result.analysis.diagnostics
        );
        let semantic = result.semantic.as_ref().unwrap();
        assert_eq!(
            semantic
                .nodes
                .iter()
                .filter(|node| {
                    builtin
                        .registry
                        .get(&node.node_type_id)
                        .is_some_and(|registered| {
                            registered.transparent_role() == Some(TransparentNodeRole::Reroute)
                        })
                })
                .count(),
            0
        );
        assert_eq!(semantic.dependencies.len(), 3);
        for (index, expected_kind) in [PortKindDto::Data, PortKindDto::Control, PortKindDto::Effect]
            .into_iter()
            .enumerate()
        {
            let source = NodeId::from_uuid(Uuid::from_u128(10 + index as u128 * 2));
            let target = NodeId::from_uuid(Uuid::from_u128(11 + index as u128 * 2));
            assert!(
                semantic
                    .dependencies
                    .iter()
                    .any(|dependency| match dependency {
                        SemanticDependency::Value(edge) => {
                            expected_kind == PortKindDto::Data
                                && edge.source.node_id == source
                                && edge.target.node_id == target
                        }
                        SemanticDependency::Control(edge) => {
                            expected_kind == PortKindDto::Control
                                && edge.source_node == source
                                && edge.target_node == target
                        }
                        SemanticDependency::Effect(edge) => {
                            expected_kind == PortKindDto::Effect
                                && edge.predecessor == source
                                && edge.successor == target
                        }
                    })
            );
        }

        let en = build_editor_graph_projection(
            "functions/reroute-projection",
            &document,
            &result.analysis,
            &result.outcome,
            builtin.registry.as_ref(),
            &builtin.catalog.localization("en-US"),
        )
        .unwrap();
        let zh = build_editor_graph_projection(
            "functions/reroute-projection",
            &document,
            &result.analysis,
            &result.outcome,
            builtin.registry.as_ref(),
            &builtin.catalog.localization("zh-CN"),
        )
        .unwrap();

        assert_eq!(en.nodes.len(), 12);
        assert_eq!(en.connections.len(), 9);
        assert_eq!(
            en.nodes
                .iter()
                .filter(|node| node.display.style_id.as_deref() == Some("builtin.reroute"))
                .count(),
            6
        );
        let first_reroute = en
            .nodes
            .iter()
            .position(|node| node.display.style_id.as_deref() == Some("builtin.reroute"))
            .unwrap();
        assert_ne!(
            en.nodes[first_reroute].display.title,
            zh.nodes[first_reroute].display.title
        );
        assert_eq!(
            reroute_locale_independent_contract(&en),
            reroute_locale_independent_contract(&zh)
        );

        for (index, kind) in [PortKindDto::Data, PortKindDto::Control, PortKindDto::Effect]
            .into_iter()
            .enumerate()
        {
            let source_id = NodeId::from_uuid(Uuid::from_u128(100 + index as u128 * 2));
            let target_id = NodeId::from_uuid(Uuid::from_u128(101 + index as u128 * 2));
            for (node_id, x) in [(source_id, 40.0), (target_id, 140.0)] {
                let node = en
                    .nodes
                    .iter()
                    .find(|node| node.node_id.as_ref() == node_id.to_string())
                    .unwrap();
                assert_eq!(
                    node.position,
                    NodePositionDto {
                        x,
                        y: 80.0 + index as f64 * 60.0,
                    }
                );
                assert_eq!(node.display.style_id.as_deref(), Some("builtin.reroute"));
                assert!(node.parameter_editors.is_empty());
                assert!(!node.capabilities.managed);
                assert!(node.capabilities.can_delete);
                assert_eq!(node.ports.len(), 2);
                assert_eq!(node.ports[0].direction, PortDirectionDto::Input);
                assert_eq!(node.ports[1].direction, PortDirectionDto::Output);
                assert!(node.ports.iter().all(|port| port.kind == kind));
                if kind == PortKindDto::Data {
                    assert_eq!(node.ports[0].resolved_type, node.ports[1].resolved_type);
                }
            }
            let connection_id = ConnectionId::from_uuid(Uuid::from_u128(301 + index as u128 * 3));
            let connection = en
                .connections
                .iter()
                .find(|connection| connection.connection_id.as_ref() == connection_id.to_string())
                .unwrap();
            assert_eq!(connection.order, None);
            assert_eq!(
                connection.output,
                PortAddressDto::Declared {
                    node_id: source_id.to_string().into(),
                    port_key: REROUTE_OUTPUT_PORT.into(),
                }
            );
            assert_eq!(
                connection.input,
                PortAddressDto::Declared {
                    node_id: target_id.to_string().into(),
                    port_key: REROUTE_INPUT_PORT.into(),
                }
            );
        }
    }

    struct NamedResources {
        function_path: GraphResourcePath,
        function: FunctionDocument,
        function_graph: GraphDocument,
        function_name: Box<str>,
        variable: crate::variable::VariableInstance,
        database_name: Box<str>,
        database_columns: Vec<crate::schema::ColumnInfoDTO>,
    }

    impl ResourceSnapshot for NamedResources {
        fn versions(&self) -> ResourceVersionSet {
            BTreeMap::from([
                (
                    crate::node_system::analysis::ResourceKey::new(self.function_path.0.clone()),
                    crate::node_system::analysis::ResourceVersion::new("function-v1"),
                ),
                (
                    crate::node_system::analysis::ResourceKey::new(format!(
                        "variables/{}",
                        self.variable.id
                    )),
                    crate::node_system::analysis::ResourceVersion::new("variable-v1"),
                ),
                (
                    crate::node_system::analysis::ResourceKey::new("databases/sales"),
                    crate::node_system::analysis::ResourceVersion::new("database-v1"),
                ),
            ])
        }

        fn function_name(&self, path: &GraphResourcePath) -> Option<&str> {
            (path == &self.function_path).then_some(self.function_name.as_ref())
        }

        fn function_document(&self, path: &GraphResourcePath) -> Option<&FunctionDocument> {
            (path == &self.function_path).then_some(&self.function)
        }

        fn function_graph_document(&self, path: &GraphResourcePath) -> Option<&GraphDocument> {
            (path == &self.function_path).then_some(&self.function_graph)
        }

        fn variable(
            &self,
            id: &crate::variable::VariableId,
        ) -> Option<&crate::variable::VariableInstance> {
            (id == &self.variable.id).then_some(&self.variable)
        }

        fn database_name(&self, id: &str) -> Option<&str> {
            (id == "sales").then_some(self.database_name.as_ref())
        }

        fn database_schema(&self, id: &str) -> Option<&[crate::schema::ColumnInfoDTO]> {
            (id == "sales").then_some(self.database_columns.as_slice())
        }
    }

    #[test]
    fn resource_bound_editor_titles_use_authoritative_names_and_preserve_labels() {
        let builtin = build_builtin_node_system().unwrap();
        let registry = builtin.registry;
        let catalog = builtin.catalog;
        let variable_id = crate::variable::VariableId::from(Uuid::from_u128(100));
        let function_path = GraphResourcePath("functions/calculate-sales".into());
        let resources = NamedResources {
            function_path: function_path.clone(),
            function: FunctionDocument::new(FunctionSignature::default()),
            function_graph: GraphDocument::default(),
            function_name: "Calculate Sales".into(),
            variable: crate::variable::VariableInstance {
                id: variable_id,
                name: "Revenue".into(),
                data_type: DataType::Int64,
                data_value: crate::graph::value::DataValue::Int64(1),
                tabular: None,
                description: String::new(),
                scope: crate::variable::VariableScope::Global,
                tags: Vec::new(),
            },
            database_name: "Sales Database".into(),
            database_columns: Vec::new(),
        };
        let variable_path = format!("variables/{variable_id}");
        let mut document = GraphDocument::default();
        for (index, node_type, parameter, resource, user_label) in [
            (
                1,
                "yssbi.project.function.call",
                "target",
                function_path.0.as_ref(),
                None,
            ),
            (
                2,
                "yssbi.project.variable.get",
                "variable",
                variable_path.as_str(),
                Some("Previous period"),
            ),
            (
                3,
                "yssbi.dataframe.source.get",
                "dataframe",
                "databases/sales",
                None,
            ),
            (
                4,
                "yssbi.dataframe.source.get",
                "dataframe",
                "databases/missing",
                None,
            ),
        ] {
            let node_id = NodeId::from_uuid(Uuid::from_u128(index));
            document.nodes.insert(
                node_id,
                DocumentNode {
                    id: node_id,
                    node_type: NodeTypeId::new(node_type).unwrap(),
                    position: NodePosition { x: 0.0, y: 0.0 },
                    parameters: BTreeMap::from([(
                        ParameterKey::new(parameter).unwrap(),
                        json!(resource),
                    )]),
                    user_label: user_label.map(str::to_owned),
                },
            );
        }

        let analysis = GraphCompiler::new(registry.as_ref(), &resources)
            .compile(&document)
            .analysis;
        let projection = EditorGraphProjectionDto::from_sources(
            "events/resource-titles",
            &analysis,
            &document,
            registry.as_ref(),
            &catalog.localization("en-US"),
        )
        .unwrap();
        let titles = projection
            .nodes
            .iter()
            .filter(|node| node.node_id.as_ref() != Uuid::from_u128(4).to_string())
            .map(|node| (node.node_type_id.as_ref(), node.display.title.as_ref()))
            .collect::<BTreeMap<_, _>>();

        assert_eq!(titles["yssbi.project.function.call"], "Calculate Sales");
        assert_eq!(titles["yssbi.project.variable.get"], "Revenue");
        assert_eq!(titles["yssbi.dataframe.source.get"], "Sales Database");
        let missing = projection
            .nodes
            .iter()
            .find(|node| node.node_id.as_ref() == Uuid::from_u128(4).to_string())
            .unwrap();
        assert_eq!(missing.display.title.as_ref(), "Get DataFrame");
        assert!(missing.diagnostics.iter().any(|diagnostic| {
            diagnostic.code.as_ref() == "compiler.resource.resolution_failed"
        }));
        assert_eq!(
            projection
                .nodes
                .iter()
                .find(|node| node.node_type_id.as_ref() == "yssbi.project.variable.get")
                .unwrap()
                .display
                .user_label
                .as_deref(),
            Some("Previous period"),
        );
    }

    fn basis(revision: u64) -> ProjectionBasis {
        ProjectionBasis {
            graph_path: "functions/main".into(),
            graph_revision: revision,
            registry_fingerprint: RegistryFingerprint::from_bytes([7; 32]),
            resource_versions: BTreeMap::new(),
        }
    }

    fn capabilities() -> NodeCapabilitiesDto {
        NodeCapabilitiesDto {
            managed: false,
            can_copy: true,
            can_delete: true,
            can_edit_label: true,
            can_edit_parameters: false,
            has_dynamic_ports: true,
            supports_inline_literals: false,
        }
    }

    fn port(key: &str) -> ResolvedPortDto {
        ResolvedPortDto {
            address: PortAddressDto::Declared {
                node_id: "node-1".into(),
                port_key: key.into(),
            },
            template_key: key.into(),
            display: PortDisplayDto {
                label: key.into(),
                instance_label: None,
            },
            direction: PortDirectionDto::Input,
            kind: PortKindDto::Data,
            instance_kind: PortInstanceKindDto::Declared,
            orphan: false,
            can_remove: false,
            connections: PortConnectionCapabilityDto {
                current: 0,
                maximum: Some(1),
                ordered: false,
                can_append: true,
                can_replace: false,
                can_move: false,
            },
            input: Some(EditorInputBindingDto {
                literal_override: None,
                protocol_default: None,
                effective: EffectiveInputBindingKindDto::Unbound,
            }),
            resolved_type: Some(TypeSummaryDto {
                display: "core.string".into(),
                resolved: true,
                data_type: Some(DataType::String),
                internal_type_expr: Some(TypeExpr::Concrete(TypeId::new("core.string").unwrap())),
            }),
            resolved_schema: None,
            status: ResolvedPortStatusDto::Resolved,
        }
    }

    fn node(revision: u64, ports: Vec<ResolvedPortDto>) -> EditorNodeProjectionDto {
        EditorNodeProjectionDto {
            graph_path: "functions/main".into(),
            source_revision: revision,
            node_id: "node-1".into(),
            node_type_id: "test.node".into(),
            position: NodePositionDto { x: 0.0, y: 0.0 },
            display: NodeDisplayDto {
                title: "Test".into(),
                description: None,
                user_label: None,
                icon_id: None,
                style_id: None,
            },
            ports,
            parameter_editors: Vec::new(),
            capabilities: capabilities(),
            diagnostics: Vec::new(),
        }
    }

    fn projection(revision: u64, ports: Vec<ResolvedPortDto>) -> EditorGraphProjectionDto {
        EditorGraphProjectionDto {
            basis: basis(revision),
            graph_path: "functions/main".into(),
            source_revision: revision,
            nodes: vec![node(revision, ports)],
            connections: Vec::new(),
            diagnostics: Vec::new(),
            outcome: CompilationOutcomeDto::Success,
            has_blocking_diagnostics: false,
        }
    }

    #[test]
    fn projection_basis_serializes_registry_fingerprint_as_lowercase_sha256_hex() {
        let value = serde_json::to_value(basis(7)).unwrap();
        assert_eq!(
            value["registryFingerprint"],
            "0707070707070707070707070707070707070707070707070707070707070707"
        );
        assert_eq!(
            serde_json::from_value::<ProjectionBasis>(value).unwrap(),
            basis(7)
        );
    }

    #[test]
    fn projection_basis_rejects_malformed_registry_fingerprint_wire_values() {
        let valid = serde_json::to_value(basis(7)).unwrap();
        for malformed in [
            serde_json::json!("070707070707070707070707070707070707070707070707070707070707070A"),
            serde_json::json!("070707070707070707070707070707070707070707070707070707070707070"),
            serde_json::json!("07070707070707070707070707070707070707070707070707070707070707070"),
            serde_json::json!("070707070707070707070707070707070707070707070707070707070707070g"),
        ] {
            let mut value = valid.clone();
            value["registryFingerprint"] = malformed;
            assert!(serde_json::from_value::<ProjectionBasis>(value).is_err());
        }
    }

    #[test]
    fn schema_aware_editors_are_unavailable_without_source_schema() {
        for (node_type, value, expected_kind) in [
            (
                "yssbi.dataframe.project",
                Some(json!(["amount"])),
                "projectColumns",
            ),
            (
                "yssbi.dataframe.filter.rows",
                Some(json!({
                    "column": "amount",
                    "operator": "greaterThan",
                    "value": { "type": "decimal", "value": "10.5" }
                })),
                "filterPredicate",
            ),
        ] {
            let editor = project_schema_aware_editor(
                node_type,
                value.as_ref(),
                None,
                "Connect DataFrame input".into(),
            )
            .expect("schema-aware editor");
            let serialized = serde_json::to_value(editor).unwrap();
            assert_eq!(serialized["kind"], expected_kind);
            assert_eq!(serialized["available"], false);
            assert_eq!(serialized["unavailableReason"], "Connect DataFrame input");
            let options = serialized
                .get("options")
                .or_else(|| serialized.get("columns"))
                .unwrap();
            assert_eq!(options, &json!([]));
        }
    }

    #[test]
    fn schema_aware_editors_project_typed_options_and_operator_matrix() {
        use crate::node_system::protocol::{
            RelationalScalarType, ResolvedSchemaFact, SchemaColumnRef, SchemaField,
        };

        let fact = ResolvedSchemaFact::new(
            SchemaExpr::Input(PortKey::new("source").unwrap()),
            [
                SchemaField {
                    name: SchemaColumnRef("active".into()),
                    scalar_type: RelationalScalarType::Boolean,
                    lineage: None,
                },
                SchemaField {
                    name: SchemaColumnRef("count".into()),
                    scalar_type: RelationalScalarType::Int64,
                    lineage: None,
                },
                SchemaField {
                    name: SchemaColumnRef("amount".into()),
                    scalar_type: RelationalScalarType::Float64,
                    lineage: None,
                },
                SchemaField {
                    name: SchemaColumnRef("status".into()),
                    scalar_type: RelationalScalarType::String,
                    lineage: None,
                },
                SchemaField {
                    name: SchemaColumnRef("day".into()),
                    scalar_type: RelationalScalarType::Date,
                    lineage: None,
                },
                SchemaField {
                    name: SchemaColumnRef("created".into()),
                    scalar_type: RelationalScalarType::DateTime,
                    lineage: None,
                },
                SchemaField {
                    name: SchemaColumnRef("opaque".into()),
                    scalar_type: RelationalScalarType::Unknown,
                    lineage: None,
                },
            ],
        );
        let project = project_schema_aware_editor(
            "yssbi.dataframe.project",
            Some(&json!(["status", "count"])),
            Some(&fact),
            "unused".into(),
        )
        .unwrap();
        let project = serde_json::to_value(project).unwrap();
        assert_eq!(project["value"], json!(["status", "count"]));
        assert_eq!(
            project["options"][0],
            json!({ "name": "active", "dataType": "boolean" })
        );
        assert_eq!(
            project["options"][6],
            json!({ "name": "opaque", "dataType": "unknown" })
        );

        let predicate = json!({
            "column": "count",
            "operator": "greaterThan",
            "value": { "type": "integer", "value": "9007199254740993" }
        });
        let filter = project_schema_aware_editor(
            "yssbi.dataframe.filter.rows",
            Some(&predicate),
            Some(&fact),
            "unused".into(),
        )
        .unwrap();
        let filter = serde_json::to_value(filter).unwrap();
        assert_eq!(filter["value"], predicate);
        assert_eq!(
            filter["columns"][0]["operators"],
            json!(["equal", "notEqual", "isNull", "isNotNull"])
        );
        assert_eq!(
            filter["columns"][1]["operators"],
            json!([
                "equal",
                "notEqual",
                "lessThan",
                "lessThanOrEqual",
                "greaterThan",
                "greaterThanOrEqual",
                "isNull",
                "isNotNull"
            ])
        );
        assert_eq!(
            filter["columns"][2]["operators"],
            filter["columns"][1]["operators"]
        );
        assert_eq!(
            filter["columns"][3]["operators"],
            filter["columns"][1]["operators"]
        );
        assert_eq!(
            filter["columns"][4]["operators"],
            json!(["isNull", "isNotNull"])
        );
        assert_eq!(
            filter["columns"][5]["operators"],
            json!(["isNull", "isNotNull"])
        );
        assert_eq!(filter["columns"][6]["operators"], json!([]));
        assert_eq!(filter["columns"][0]["literalTypes"], json!(["boolean"]));
        assert_eq!(filter["columns"][1]["literalTypes"], json!(["integer"]));
        assert_eq!(
            filter["columns"][2]["literalTypes"],
            json!(["integer", "decimal"])
        );
        assert_eq!(filter["columns"][3]["literalTypes"], json!(["string"]));
        assert_eq!(filter["columns"][4]["literalTypes"], json!([]));
        assert_eq!(filter["columns"][6]["literalTypes"], json!([]));
    }

    #[test]
    fn diagnostic_locations_serialize_struct_fields_as_camel_case() {
        let locations = vec![
            DiagnosticLocationDto::Node {
                node_id: "node-1".into(),
            },
            DiagnosticLocationDto::Port {
                address: PortAddressDto::Declared {
                    node_id: "node-1".into(),
                    port_key: "input".into(),
                },
            },
            DiagnosticLocationDto::Connection {
                connection_id: "connection-1".into(),
            },
            DiagnosticLocationDto::Parameter {
                node_id: "node-1".into(),
                key: "formula".into(),
            },
        ];

        assert_eq!(
            serde_json::to_value(locations).unwrap(),
            json!([
                { "kind": "node", "nodeId": "node-1" },
                {
                    "kind": "port",
                    "address": {
                        "kind": "declared",
                        "nodeId": "node-1",
                        "portKey": "input"
                    }
                },
                { "kind": "connection", "connectionId": "connection-1" },
                { "kind": "parameter", "nodeId": "node-1", "key": "formula" }
            ])
        );
    }

    #[test]
    fn editor_projection_includes_positions_connections_and_input_bindings() {
        let builtin = build_builtin_node_system().unwrap();
        let registry = builtin.registry;
        let catalog = builtin.catalog;
        let branch_id = NodeId::from_uuid(Uuid::from_u128(1));
        let sleep_id = NodeId::from_uuid(Uuid::from_u128(2));
        let connection_id = ConnectionId::from_uuid(Uuid::from_u128(3));
        let branch_enter = PortAddress::declared(branch_id, PortKey::new("enter").unwrap());
        let branch_condition = PortAddress::declared(branch_id, PortKey::new("condition").unwrap());
        let branch_true = PortAddress::declared(branch_id, PortKey::new("true").unwrap());
        let sleep_enter = PortAddress::declared(sleep_id, PortKey::new("enter").unwrap());
        let mut document = GraphDocument::default();
        document.nodes.insert(
            branch_id,
            DocumentNode {
                id: branch_id,
                node_type: NodeTypeId::new("yssbi.control.branch").unwrap(),
                position: NodePosition { x: 12.5, y: -4.0 },
                parameters: BTreeMap::new(),
                user_label: None,
            },
        );
        document.nodes.insert(
            sleep_id,
            DocumentNode {
                id: sleep_id,
                node_type: NodeTypeId::new("yssbi.control.sleep").unwrap(),
                position: NodePosition { x: 48.0, y: 8.0 },
                parameters: BTreeMap::new(),
                user_label: None,
            },
        );
        document.connections.insert(
            connection_id,
            DocumentConnection {
                id: connection_id,
                output: branch_true,
                input: sleep_enter,
                order: Some(OrderKey("rank-1".into())),
            },
        );
        document.input_states.insert(
            branch_condition,
            InputState {
                literal_override: Some(json!(true)),
            },
        );
        let analysis = GraphCompiler::new(registry.as_ref(), &EmptyResources)
            .compile(&document)
            .analysis;
        let localization = catalog.localization("en-US");

        let projection = EditorGraphProjectionDto::from_sources(
            "functions/main",
            &analysis,
            &document,
            &registry,
            &localization,
        )
        .unwrap();

        assert_eq!(
            projection.nodes[0].position,
            NodePositionDto { x: 12.5, y: -4.0 }
        );
        assert_eq!(
            projection.connections[0].connection_id.as_ref(),
            connection_id.to_string()
        );
        assert!(matches!(
            projection.connections[0].output,
            PortAddressDto::Declared { .. }
        ));
        assert_eq!(projection.connections[0].order.as_deref(), Some("rank-1"));
        let branch = &projection.nodes[0];
        let sleep = &projection.nodes[1];
        assert!(
            branch
                .ports
                .iter()
                .find(|port| port.template_key.as_ref() == "true")
                .unwrap()
                .input
                .is_none()
        );
        fn binding<'a>(node: &'a EditorNodeProjectionDto, key: &str) -> &'a EditorInputBindingDto {
            node.ports
                .iter()
                .find(|port| port.template_key.as_ref() == key)
                .unwrap()
                .input
                .as_ref()
                .unwrap()
        }
        let effective = |node: &EditorNodeProjectionDto, key: &str| binding(node, key).effective;
        assert_eq!(
            effective(branch, "condition"),
            EffectiveInputBindingKindDto::Literal
        );
        assert_eq!(
            effective(sleep, "enter"),
            EffectiveInputBindingKindDto::Connections
        );
        assert_eq!(
            effective(sleep, "duration"),
            EffectiveInputBindingKindDto::ProtocolDefault
        );
        assert_eq!(
            effective(branch, "enter"),
            EffectiveInputBindingKindDto::Unbound
        );
        assert_eq!(
            binding(branch, "condition").literal_override,
            Some(json!(true))
        );
        assert_eq!(
            binding(sleep, "duration").protocol_default,
            Some(json!({ "Decimal": "1" }))
        );

        let zh_projection = EditorGraphProjectionDto::from_sources(
            "functions/main",
            &analysis,
            &document,
            &registry,
            &catalog.localization("zh-CN"),
        )
        .unwrap();
        assert_eq!(zh_projection.basis, projection.basis);
        assert_eq!(zh_projection.graph_path, projection.graph_path);
        assert_eq!(zh_projection.source_revision, projection.source_revision);
        assert_eq!(zh_projection.connections, projection.connections);
        assert_eq!(zh_projection.nodes.len(), projection.nodes.len());
        for (localized, original) in zh_projection.nodes.iter().zip(&projection.nodes) {
            assert_eq!(localized.node_id, original.node_id);
            assert_eq!(localized.node_type_id, original.node_type_id);
            assert_eq!(localized.position, original.position);
            assert_eq!(
                localized
                    .ports
                    .iter()
                    .map(|port| &port.address)
                    .collect::<Vec<_>>(),
                original
                    .ports
                    .iter()
                    .map(|port| &port.address)
                    .collect::<Vec<_>>()
            );
        }

        let old_input = projection.connections[0].input.clone();
        let mut current = projection.clone();
        let mut next = projection;
        next.basis.graph_revision += 1;
        next.source_revision += 1;
        for node in &mut next.nodes {
            node.source_revision += 1;
        }
        next.connections[0].input = project_address(&branch_enter);
        let delta = GraphProjectionDelta::between(&current, &next).unwrap();
        current.apply_delta(delta).unwrap();

        assert_ne!(current.connections[0].input, old_input);
        assert_eq!(current.connections, next.connections);
    }

    #[test]
    fn grouped_port_removal_capability_distinguishes_complete_and_partial_members() {
        let builtin = build_builtin_node_system().unwrap();
        let registry = builtin.registry;
        let catalog = builtin.catalog;
        let loop_id = NodeId::from_uuid(Uuid::from_u128(20));
        let complete_id = PortInstanceId::from_uuid(Uuid::from_u128(21));
        let partial_id = PortInstanceId::from_uuid(Uuid::from_u128(22));
        let mut parameters = BTreeMap::new();
        parameters.insert(ParameterKey::new("max_iterations").unwrap(), json!(100));
        let mut document = GraphDocument::default();
        document
            .create_node(DocumentNode {
                id: loop_id,
                node_type: NodeTypeId::new("yssbi.control.loop").unwrap(),
                position: NodePosition { x: 0.0, y: 0.0 },
                parameters,
                user_label: None,
            })
            .unwrap();
        for (template, instance_id) in [
            ("initial_source", complete_id),
            ("body_input", complete_id),
            ("next_source", complete_id),
            ("result", complete_id),
            ("initial_source", partial_id),
        ] {
            document
                .bind_port(
                    PortAddress::instance(loop_id, PortKey::new(template).unwrap(), instance_id),
                    DynamicPortBinding::UserCreated {
                        order: OrderKey(instance_id.to_string().into()),
                    },
                )
                .unwrap();
        }
        let analysis = GraphCompiler::new(registry.as_ref(), &EmptyResources)
            .compile(&document)
            .analysis;
        let projection = EditorGraphProjectionDto::from_sources(
            "events/grouped-capability",
            &analysis,
            &document,
            &registry,
            &catalog.localization("en-US"),
        )
        .unwrap();
        let loop_node = projection
            .nodes
            .iter()
            .find(|node| node.node_id.as_ref() == loop_id.to_string())
            .unwrap();

        for port in &loop_node.ports {
            let PortAddressDto::Instance { instance_id, .. } = &port.address else {
                continue;
            };
            if instance_id.as_ref() == complete_id.to_string() {
                assert!(!port.can_remove, "complete member must preserve Loop min=1");
            } else if instance_id.as_ref() == partial_id.to_string() {
                assert!(port.can_remove, "partial endpoints must remain removable");
            }
        }
    }

    #[test]
    fn grouped_port_capability_ignores_non_user_created_siblings() {
        let builtin = build_builtin_node_system().unwrap();
        let registry = builtin.registry;
        let catalog = builtin.catalog;
        let loop_id = NodeId::from_uuid(Uuid::from_u128(30));
        let complete_id = PortInstanceId::from_uuid(Uuid::from_u128(31));
        let mixed_id = PortInstanceId::from_uuid(Uuid::from_u128(32));
        let mut parameters = BTreeMap::new();
        parameters.insert(ParameterKey::new("max_iterations").unwrap(), json!(100));
        let mut document = GraphDocument::default();
        document
            .create_node(DocumentNode {
                id: loop_id,
                node_type: NodeTypeId::new("yssbi.control.loop").unwrap(),
                position: NodePosition { x: 0.0, y: 0.0 },
                parameters,
                user_label: None,
            })
            .unwrap();
        for template in ["initial_source", "body_input", "next_source", "result"] {
            document
                .bind_port(
                    PortAddress::instance(loop_id, PortKey::new(template).unwrap(), complete_id),
                    DynamicPortBinding::UserCreated {
                        order: OrderKey("complete".into()),
                    },
                )
                .unwrap();
        }
        let locator = || DynamicMemberLocator::FunctionParameter {
            function: GraphResourcePath("functions/mixed".into()),
            parameter: FunctionParameterId("value".into()),
        };
        for (template, binding) in [
            (
                "initial_source",
                DynamicPortBinding::UserCreated {
                    order: OrderKey("partial".into()),
                },
            ),
            (
                "body_input",
                DynamicPortBinding::Resolved {
                    origin: locator(),
                    order: OrderKey("resolved-body".into()),
                    last_known: LastKnownPortMetadata::default(),
                },
            ),
            (
                "next_source",
                DynamicPortBinding::Orphan {
                    origin: locator(),
                    order: OrderKey("orphan-next".into()),
                    last_known: LastKnownPortMetadata {
                        label: "Next".into(),
                        value_type: None,
                    },
                },
            ),
            (
                "result",
                DynamicPortBinding::Resolved {
                    origin: locator(),
                    order: OrderKey("resolved-result".into()),
                    last_known: LastKnownPortMetadata::default(),
                },
            ),
        ] {
            document
                .bind_port(
                    PortAddress::instance(loop_id, PortKey::new(template).unwrap(), mixed_id),
                    binding,
                )
                .unwrap();
        }

        let analysis = GraphCompiler::new(registry.as_ref(), &EmptyResources)
            .compile(&document)
            .analysis;
        let projection = EditorGraphProjectionDto::from_sources(
            "events/mixed-binding-capability",
            &analysis,
            &document,
            &registry,
            &catalog.localization("en-US"),
        )
        .unwrap();
        let loop_node = projection
            .nodes
            .iter()
            .find(|node| node.node_id.as_ref() == loop_id.to_string())
            .unwrap();
        let mut saw_partial_user_created = false;
        let mut saw_orphan = false;

        for port in &loop_node.ports {
            let PortAddressDto::Instance {
                template_key,
                instance_id,
                ..
            } = &port.address
            else {
                continue;
            };
            if instance_id.as_ref() == complete_id.to_string() {
                assert!(
                    !port.can_remove,
                    "non-user siblings must not inflate complete_count"
                );
            } else if instance_id.as_ref() == mixed_id.to_string() {
                if template_key.as_ref() == "initial_source" {
                    saw_partial_user_created = true;
                    assert!(!port.orphan);
                    assert!(
                        port.can_remove,
                        "partial UserCreated endpoint must be removable"
                    );
                } else {
                    saw_orphan = true;
                    assert!(port.orphan);
                    assert!(port.can_remove, "orphan endpoints keep the UI removal rule");
                }
            }
        }
        assert!(saw_partial_user_created);
        assert!(saw_orphan);
    }

    #[test]
    fn projection_basis_is_consistent_with_envelope() {
        let projection = projection(4, vec![port("value")]);

        assert_eq!(projection.basis.graph_path, projection.graph_path);
        assert_eq!(projection.basis.graph_revision, projection.source_revision);
    }

    #[test]
    fn stale_basis_is_rejected_without_mutation() {
        let mut current = projection(2, vec![port("old")]);
        let original = current.clone();
        let stale = projection(1, vec![port("stale")]);
        let next = projection(3, vec![port("new")]);
        let delta = GraphProjectionDelta::between(&stale, &next).unwrap();

        assert_eq!(
            current.apply_delta(delta).unwrap_err(),
            ProjectionError::StaleProjectionBasis
        );
        assert_eq!(current, original);
    }

    #[test]
    fn dynamic_interface_is_replaced_atomically_as_a_whole_node() {
        let mut current = projection(5, vec![port("a"), port("b")]);
        let next = projection(6, vec![port("c")]);
        let delta = GraphProjectionDelta::between(&current, &next).unwrap();

        assert_eq!(delta.node_replacements, next.nodes);
        assert_eq!(delta.node_replacements[0].ports, vec![port("c")]);
        let serialized = serde_json::to_value(&delta).unwrap();
        assert!(serialized.get("addedPins").is_none());

        current.apply_delta(delta).unwrap();
        assert_eq!(current, next);
    }

    #[test]
    fn editor_schema_summary_projects_transformed_typed_fields() {
        let fact = crate::node_system::protocol::ResolvedSchemaFact::new(
            SchemaExpr::Rename {
                input: Box::new(SchemaExpr::Input(PortKey::new("source").unwrap())),
                mapping: crate::node_system::protocol::RenameExpr::Explicit(vec![]),
            },
            [crate::node_system::protocol::SchemaField {
                name: crate::node_system::protocol::SchemaColumnRef("total".into()),
                scalar_type: crate::node_system::protocol::RelationalScalarType::Float64,
                lineage: None,
            }],
        );

        let summary = project_schema_summary(&fact.expression, Some(&fact));

        assert_eq!(
            summary.fields,
            vec![SchemaFieldDto {
                name: "total".into(),
                scalar_type: RelationalScalarTypeDto::Float64,
            }]
        );
    }

    #[test]
    fn editor_dto_does_not_serialize_protocol_ast() {
        let json = serde_json::to_string(&projection(1, vec![port("value")])).unwrap();

        for forbidden in [
            "execution",
            "interface",
            "typeConstraints",
            "valueType",
            "managedRole",
            "protocolFingerprint",
        ] {
            assert!(
                !json.contains(forbidden),
                "unexpected protocol field: {forbidden}"
            );
        }
    }
}
