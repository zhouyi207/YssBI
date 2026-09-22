use crate::graph::PortAddressDto;
use serde::{Deserialize, Serialize};
use yss_data_contract::ValueType;
use yss_graph_analysis_contract::ResourceVersionSet;
use yss_node_protocol::ParameterPresentation;
use yss_node_registry::RegistryFingerprint;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectionBasis {
    pub graph_path: Box<str>,
    pub semantic_input_hash: Box<str>,
    #[serde(
        serialize_with = "serialize_registry_fingerprint",
        deserialize_with = "deserialize_registry_fingerprint"
    )]
    pub registry_fingerprint: RegistryFingerprint,
    pub resource_versions: ResourceVersionSet,
    pub resource_observations: yss_graph_analysis_contract::ResourceObservationSet,
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
    pub nodes: Vec<EditorNodeProjectionDto>,
    pub connections: Vec<EditorConnectionProjectionDto>,
    pub diagnostics: Vec<DiagnosticDto>,
    pub outcome: ResolutionOutcomeDto,
    pub has_blocking_diagnostics: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ResolutionOutcomeDto {
    Success,
    AnalysisBlocked,
    InternalFailure {
        stage: ResolutionStageDto,
        code: Box<str>,
        node_id: Option<Box<str>>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ResolutionStageDto {
    Analysis,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorNodeProjectionDto {
    pub graph_path: Box<str>,
    pub node_id: Box<str>,
    pub node_type_id: Box<str>,
    pub position: NodePositionDto,
    pub display: NodeDisplayDto,
    pub ports: Vec<EditorPortDto>,
    pub port_instance_additions: Vec<PortInstanceAdditionDto>,
    pub parameter_groups: Vec<ParameterGroupDto>,
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
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NodeCapabilitiesDto {
    pub managed: bool,
}

#[cfg(test)]
mod node_capabilities_tests {
    use super::NodeCapabilitiesDto;

    #[test]
    fn node_ownership_wire_is_exact_and_requires_managed() {
        let value = serde_json::json!({ "managed": false });
        let parsed: NodeCapabilitiesDto = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(serde_json::to_value(parsed).unwrap(), value);
        for invalid in [
            serde_json::json!({}),
            serde_json::json!({ "managed": "false" }),
            serde_json::json!({ "managed": false, "extra": true }),
        ] {
            assert!(serde_json::from_value::<NodeCapabilitiesDto>(invalid).is_err());
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorPortDto {
    pub address: PortAddressDto,
    pub display: PortDisplayDto,
    pub direction: PortDirectionDto,
    pub orphan: bool,
    pub can_remove: bool,
    pub connections: PortConnectionCapabilityDto,
    pub input: Option<EditorInputBindingDto>,
    pub accepted_type: AcceptedTypeDto,
    pub type_state: PortTypeStateDto,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortInstanceAdditionDto {
    pub template_key: Box<str>,
    pub label: Box<str>,
    pub direction: PortDirectionDto,
    pub can_add: bool,
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
pub struct AcceptedTypeDto {
    pub display: Box<str>,
    pub domain: Option<Vec<ValueType>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "status",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum PortTypeStateDto {
    Exact {
        display: Box<str>,
        data_type: Option<ValueType>,
    },
    Constrained {
        display: Box<str>,
        domain: Vec<ValueType>,
    },
    Unknown {
        reason_code: Box<str>,
    },
    Conflict {
        diagnostic_code: Box<str>,
    },
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

pub type RelationalScalarTypeDto = Option<yss_data_contract::SemanticType>;

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
pub struct ParameterGroupDto {
    pub key: Box<str>,
    pub display: ParameterDisplayDto,
    pub parameters: Vec<ParameterEditorDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParameterEditorDto {
    pub key: Box<str>,
    pub display: ParameterDisplayDto,
    pub editor: ParameterEditorKindDto,
    pub presentation: ParameterPresentationDto,
    pub value_type: Option<ValueType>,
    pub multiline: bool,
    pub value: Option<serde_json::Value>,
    pub configuration: Option<SchemaAwareParameterEditorDto>,
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
    SelectOptions {
        options: Vec<Box<str>>,
    },
    ProjectColumns {
        allow_empty: bool,
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
    pub operators: Vec<yss_node_protocol::dataframe::FilterOperator>,
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
    SemanticDomain,
    GraphConstant,
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
    pub message_key: Box<str>,
    pub arguments: std::collections::BTreeMap<Box<str>, Box<str>>,
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
