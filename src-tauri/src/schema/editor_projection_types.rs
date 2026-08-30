use crate::graph::analysis::contracts::ResourceVersionSet;
use crate::graph::protocol::{ParameterPresentation, TypeExpr};
use crate::graph::registry::RegistryFingerprint;
use crate::schema::graph_mutation::PortAddressDto;
use serde::{Deserialize, Serialize};
use yss_data_contract::DataType;

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
    pub operators: Vec<crate::graph::protocol::dataframe::FilterOperator>,
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
    pub function_revision: u64,
    pub inputs: Box<[FunctionEditorPinDto]>,
    pub outputs: Box<[FunctionEditorPinDto]>,
}

impl From<&crate::project::FunctionEditorProjection> for FunctionEditorProjectionDto {
    fn from(value: &crate::project::FunctionEditorProjection) -> Self {
        Self {
            function_revision: value.function_revision,
            inputs: value
                .inputs
                .iter()
                .map(|pin| FunctionEditorPinDto {
                    id: pin.id.clone(),
                    name: pin.name.clone(),
                    data_type: pin.data_type.clone(),
                })
                .collect(),
            outputs: value
                .outputs
                .iter()
                .map(|pin| FunctionEditorPinDto {
                    id: pin.id.clone(),
                    name: pin.name.clone(),
                    data_type: pin.data_type.clone(),
                })
                .collect(),
        }
    }
}
