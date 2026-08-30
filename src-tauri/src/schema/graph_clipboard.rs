//! Transport mapping for graph clipboard snapshots.
//!
//! Clipboard values are graph-domain facts while their camelCase wire shape is
//! an IPC concern.  Keeping the mapping here prevents commands from exposing
//! Graph's internal clipboard representation directly.

use serde::{Deserialize, Serialize};

use crate::graph::document::{
    ClipboardConnection, ClipboardDynamicMemberOrigin, ClipboardDynamicPortBinding,
    ClipboardInputState, ClipboardLastKnownPortMetadata, ClipboardNode, ClipboardNodeCreation,
    ClipboardPortAddress, ClipboardPortBinding, ClipboardPortRef, ClipboardSubgraph,
    MAX_CLIPBOARD_SERIALIZED_BYTES,
};
use yss_graph_document::{NodePosition, TypedValue};
use yss_graph_protocol::{ParameterValues, TypeExpr};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ClipboardNodeCreationDto {
    Static {
        #[serde(rename = "nodeTypeId")]
        node_type_id: Box<str>,
    },
    ResourceBound {
        #[serde(rename = "nodeTypeId")]
        node_type_id: Box<str>,
        #[serde(rename = "resourcePath")]
        resource_path: Box<str>,
        #[serde(rename = "createArgs")]
        create_args: ClipboardResourceBoundCreateArgsDto,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ClipboardResourceBoundCreateArgsDto {
    Function,
    Variable,
    Database,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ClipboardPortRefDto {
    Declared {
        key: Box<str>,
    },
    Instance {
        template: Box<str>,
        local_instance_id: Box<str>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClipboardPortAddressDto {
    pub node_id: Box<str>,
    pub port: ClipboardPortRefDto,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClipboardNodeDto {
    pub local_id: Box<str>,
    pub creation: ClipboardNodeCreationDto,
    pub parameters: ParameterValues,
    pub user_label: Option<String>,
    pub relative_position: NodePosition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ClipboardDynamicMemberOriginDto {
    FunctionParameter {
        function: Box<str>,
        parameter: Box<str>,
    },
    SchemaField {
        source: Box<str>,
        field: Box<str>,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClipboardLastKnownPortMetadataDto {
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_type: Option<TypeExpr>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ClipboardDynamicPortBindingDto {
    UserCreated {
        order: Box<str>,
    },
    Resolved {
        origin: ClipboardDynamicMemberOriginDto,
        order: Box<str>,
        last_known: ClipboardLastKnownPortMetadataDto,
    },
    Orphan {
        origin: ClipboardDynamicMemberOriginDto,
        order: Box<str>,
        last_known: ClipboardLastKnownPortMetadataDto,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClipboardPortBindingDto {
    pub address: ClipboardPortAddressDto,
    pub binding: ClipboardDynamicPortBindingDto,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClipboardInputStateDto {
    pub address: ClipboardPortAddressDto,
    pub state: ClipboardInputStatePayloadDto,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClipboardInputStatePayloadDto {
    pub literal_override: Option<TypedValue>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClipboardConnectionDto {
    pub output: ClipboardPortAddressDto,
    pub input: ClipboardPortAddressDto,
    pub order: Option<Box<str>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClipboardSubgraphDto {
    pub schema_version: u32,
    pub nodes: Vec<ClipboardNodeDto>,
    pub port_bindings: Vec<ClipboardPortBindingDto>,
    pub input_states: Vec<ClipboardInputStateDto>,
    pub connections: Vec<ClipboardConnectionDto>,
}

impl From<ClipboardSubgraph> for ClipboardSubgraphDto {
    fn from(value: ClipboardSubgraph) -> Self {
        Self {
            schema_version: value.schema_version,
            nodes: value
                .nodes
                .into_iter()
                .map(ClipboardNodeDto::from)
                .collect(),
            port_bindings: value
                .port_bindings
                .into_iter()
                .map(ClipboardPortBindingDto::from)
                .collect(),
            input_states: value
                .input_states
                .into_iter()
                .map(ClipboardInputStateDto::from)
                .collect(),
            connections: value
                .connections
                .into_iter()
                .map(ClipboardConnectionDto::from)
                .collect(),
        }
    }
}

impl From<ClipboardNode> for ClipboardNodeDto {
    fn from(value: ClipboardNode) -> Self {
        Self {
            local_id: value.local_id.0,
            creation: value.creation.into(),
            parameters: value.parameters,
            user_label: value.user_label,
            relative_position: value.relative_position,
        }
    }
}

impl From<ClipboardNodeCreation> for ClipboardNodeCreationDto {
    fn from(value: ClipboardNodeCreation) -> Self {
        match value {
            ClipboardNodeCreation::Static { node_type_id } => Self::Static {
                node_type_id: node_type_id.as_str().into(),
            },
            ClipboardNodeCreation::ResourceBound {
                node_type_id,
                resource_path,
                create_args,
            } => Self::ResourceBound {
                node_type_id: node_type_id.as_str().into(),
                resource_path: resource_path.as_str().into(),
                create_args: create_args.into(),
            },
        }
    }
}

impl From<yss_graph_catalog::ResourceBoundCreateArgs> for ClipboardResourceBoundCreateArgsDto {
    fn from(value: yss_graph_catalog::ResourceBoundCreateArgs) -> Self {
        match value {
            yss_graph_catalog::ResourceBoundCreateArgs::Function => Self::Function,
            yss_graph_catalog::ResourceBoundCreateArgs::Variable => Self::Variable,
            yss_graph_catalog::ResourceBoundCreateArgs::Database => Self::Database,
        }
    }
}

impl From<ClipboardPortAddress> for ClipboardPortAddressDto {
    fn from(value: ClipboardPortAddress) -> Self {
        Self {
            node_id: value.node_id.0,
            port: value.port.into(),
        }
    }
}

impl From<ClipboardPortRef> for ClipboardPortRefDto {
    fn from(value: ClipboardPortRef) -> Self {
        match value {
            ClipboardPortRef::Declared { key } => Self::Declared {
                key: key.as_str().into(),
            },
            ClipboardPortRef::Instance {
                template,
                local_instance_id,
            } => Self::Instance {
                template: template.as_str().into(),
                local_instance_id: local_instance_id.0,
            },
        }
    }
}

impl From<ClipboardDynamicMemberOrigin> for ClipboardDynamicMemberOriginDto {
    fn from(value: ClipboardDynamicMemberOrigin) -> Self {
        match value {
            ClipboardDynamicMemberOrigin::FunctionParameter {
                function,
                parameter,
            } => Self::FunctionParameter {
                function: function.as_str().into(),
                parameter: parameter.as_str().into(),
            },
            ClipboardDynamicMemberOrigin::SchemaField { source, field } => Self::SchemaField {
                source: source.as_str().into(),
                field: field.as_str().into(),
            },
        }
    }
}

impl From<ClipboardLastKnownPortMetadata> for ClipboardLastKnownPortMetadataDto {
    fn from(value: ClipboardLastKnownPortMetadata) -> Self {
        Self {
            label: value.label,
            value_type: value.value_type,
        }
    }
}

impl From<ClipboardDynamicPortBinding> for ClipboardDynamicPortBindingDto {
    fn from(value: ClipboardDynamicPortBinding) -> Self {
        match value {
            ClipboardDynamicPortBinding::UserCreated { order } => Self::UserCreated {
                order: order.as_str().into(),
            },
            ClipboardDynamicPortBinding::Resolved {
                origin,
                order,
                last_known,
            } => Self::Resolved {
                origin: origin.into(),
                order: order.as_str().into(),
                last_known: last_known.into(),
            },
            ClipboardDynamicPortBinding::Orphan {
                origin,
                order,
                last_known,
            } => Self::Orphan {
                origin: origin.into(),
                order: order.as_str().into(),
                last_known: last_known.into(),
            },
        }
    }
}

impl From<ClipboardPortBinding> for ClipboardPortBindingDto {
    fn from(value: ClipboardPortBinding) -> Self {
        Self {
            address: value.address.into(),
            binding: value.binding.into(),
        }
    }
}

impl From<ClipboardInputState> for ClipboardInputStateDto {
    fn from(value: ClipboardInputState) -> Self {
        Self {
            address: value.address.into(),
            state: ClipboardInputStatePayloadDto {
                literal_override: value.state.literal_override,
            },
        }
    }
}

impl From<ClipboardConnection> for ClipboardConnectionDto {
    fn from(value: ClipboardConnection) -> Self {
        Self {
            output: value.output.into(),
            input: value.input.into(),
            order: value.order.map(|order| order.as_str().into()),
        }
    }
}

// Keep the domain conversion helpers local to this boundary.  They are used
// by the mutation mapper for the raw snapshotJson payload.
pub(crate) fn parse_clipboard_snapshot(
    snapshot_json: &str,
) -> Result<ClipboardSubgraph, crate::graph::document::MutationConflict> {
    if snapshot_json.len() > MAX_CLIPBOARD_SERIALIZED_BYTES {
        return Err(
            crate::graph::document::MutationConflict::ClipboardSubgraphInvalid(
                "clipboard payload byte limit exceeded".into(),
            ),
        );
    }
    let dto = serde_json::from_str::<ClipboardSubgraphDto>(snapshot_json).map_err(|error| {
        crate::graph::document::MutationConflict::ClipboardSubgraphInvalid(error.to_string().into())
    })?;
    let encoded = serde_json::to_vec(&dto).map_err(|error| {
        crate::graph::document::MutationConflict::ClipboardSubgraphInvalid(error.to_string().into())
    })?;
    crate::graph::document::deserialize_clipboard_subgraph(&encoded).map(|validated| validated.0)
}
