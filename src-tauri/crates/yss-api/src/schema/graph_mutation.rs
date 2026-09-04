use serde::{Deserialize, Serialize};

use yss_graph_document::{
    ConnectionId, JsonValue, NodeId, NodePosition, OrderKey, ParameterValues, PortAddress,
    PortInstanceId,
};
use yss_graph_protocol::PortKey;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum PortAddressDto {
    Declared {
        node_id: Box<str>,
        port_key: Box<str>,
    },
    Instance {
        node_id: Box<str>,
        template_key: Box<str>,
        instance_id: Box<str>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PortAddressMappingError {
    #[error("port node identity is invalid")]
    InvalidNodeId,
    #[error("port key is invalid")]
    InvalidPortKey,
    #[error("port instance identity is invalid")]
    InvalidInstanceId,
}

impl From<&PortAddress> for PortAddressDto {
    fn from(address: &PortAddress) -> Self {
        match &address.port {
            yss_graph_document::PortRef::Declared { key } => Self::Declared {
                node_id: address.node_id.to_string().into_boxed_str(),
                port_key: key.as_str().into(),
            },
            yss_graph_document::PortRef::Instance {
                template,
                instance_id,
            } => Self::Instance {
                node_id: address.node_id.to_string().into_boxed_str(),
                template_key: template.as_str().into(),
                instance_id: instance_id.to_string().into_boxed_str(),
            },
        }
    }
}

impl From<PortAddress> for PortAddressDto {
    fn from(address: PortAddress) -> Self {
        Self::from(&address)
    }
}

impl TryFrom<PortAddressDto> for PortAddress {
    type Error = PortAddressMappingError;

    fn try_from(address: PortAddressDto) -> Result<Self, Self::Error> {
        match address {
            PortAddressDto::Declared { node_id, port_key } => Ok(Self::declared(
                parse_node_id(&node_id)?,
                PortKey::new(port_key).map_err(|_| PortAddressMappingError::InvalidPortKey)?,
            )),
            PortAddressDto::Instance {
                node_id,
                template_key,
                instance_id,
            } => Ok(Self::instance(
                parse_node_id(&node_id)?,
                PortKey::new(template_key).map_err(|_| PortAddressMappingError::InvalidPortKey)?,
                PortInstanceId::from_uuid(
                    uuid::Uuid::parse_str(&instance_id)
                        .map_err(|_| PortAddressMappingError::InvalidInstanceId)?,
                ),
            )),
        }
    }
}

fn parse_node_id(value: &str) -> Result<NodeId, PortAddressMappingError> {
    uuid::Uuid::parse_str(value)
        .map(NodeId::from_uuid)
        .map_err(|_| PortAddressMappingError::InvalidNodeId)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    content = "payload",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum EditorGraphMutationDto {
    CreateNode {
        descriptor: crate::schema::catalog::NodeCreationDescriptorDto,
        position: NodePosition,
        user_label: Option<String>,
        #[serde(default)]
        connect_from: Option<PortAddressDto>,
    },
    DeleteNodes {
        node_ids: Vec<NodeId>,
    },
    SetParameters {
        node_id: NodeId,
        parameters: ParameterValues,
    },
    MoveNodes {
        positions: Vec<NodePositionMutationDto>,
    },
    Connect {
        output: PortAddressDto,
        input: PortAddressDto,
        order: Option<OrderKey>,
    },
    MoveConnections {
        source: PortAddressDto,
        target: PortAddressDto,
    },
    DisconnectConnections {
        connection_ids: Vec<ConnectionId>,
    },
    InsertReroute {
        connection_id: ConnectionId,
        position: NodePosition,
    },
    DisconnectPort {
        address: PortAddressDto,
    },
    DisconnectNode {
        node_id: NodeId,
    },
    SetLiteral {
        address: PortAddressDto,
        literal: Option<JsonValue>,
    },
    AddPortInstance {
        node_id: NodeId,
        template_key: PortKey,
        order: Option<OrderKey>,
    },
    RemovePortInstance {
        address: PortAddressDto,
    },
    DuplicateSubgraph {
        node_ids: Vec<NodeId>,
        offset: NodePosition,
    },
    InsertSubgraph {
        snapshot_json: String,
        anchor: NodePosition,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodePositionMutationDto {
    pub node_id: NodeId,
    pub position: NodePosition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum EditorMutationMappingError {
    #[error("editor mutation contains an invalid port address")]
    InvalidPortAddress,
    #[error("editor mutation contains an invalid node creation descriptor")]
    InvalidNodeCreation,
    #[error("editor mutation contains an invalid clipboard subgraph")]
    InvalidClipboardSubgraph,
}

impl TryFrom<EditorGraphMutationDto> for yss_graph_editor::EditorGraphMutation {
    type Error = EditorMutationMappingError;

    fn try_from(value: EditorGraphMutationDto) -> Result<Self, Self::Error> {
        let address = |value: PortAddressDto| {
            PortAddress::try_from(value).map_err(|_| EditorMutationMappingError::InvalidPortAddress)
        };
        Ok(match value {
            EditorGraphMutationDto::CreateNode {
                descriptor,
                position,
                user_label,
                connect_from,
            } => yss_graph_editor::EditorGraphMutation::CreateNode {
                descriptor: descriptor
                    .try_into()
                    .map_err(|_| EditorMutationMappingError::InvalidNodeCreation)?,
                position,
                user_label,
                connect_from: connect_from.map(address).transpose()?,
            },
            EditorGraphMutationDto::DeleteNodes { node_ids } => {
                yss_graph_editor::EditorGraphMutation::DeleteNodes { node_ids }
            }
            EditorGraphMutationDto::SetParameters {
                node_id,
                parameters,
            } => yss_graph_editor::EditorGraphMutation::SetParameters {
                node_id,
                parameters,
            },
            EditorGraphMutationDto::MoveNodes { positions } => {
                yss_graph_editor::EditorGraphMutation::MoveNodes {
                    positions: positions
                        .into_iter()
                        .map(|position| yss_graph_editor::NodePositionMutation {
                            node_id: position.node_id,
                            position: position.position,
                        })
                        .collect(),
                }
            }
            EditorGraphMutationDto::Connect {
                output,
                input,
                order,
            } => yss_graph_editor::EditorGraphMutation::Connect {
                output: address(output)?,
                input: address(input)?,
                order,
            },
            EditorGraphMutationDto::MoveConnections { source, target } => {
                yss_graph_editor::EditorGraphMutation::MoveConnections {
                    source: address(source)?,
                    target: address(target)?,
                }
            }
            EditorGraphMutationDto::DisconnectConnections { connection_ids } => {
                yss_graph_editor::EditorGraphMutation::DisconnectConnections { connection_ids }
            }
            EditorGraphMutationDto::InsertReroute {
                connection_id,
                position,
            } => yss_graph_editor::EditorGraphMutation::InsertReroute {
                connection_id,
                position,
            },
            EditorGraphMutationDto::DisconnectPort { address: value } => {
                yss_graph_editor::EditorGraphMutation::DisconnectPort {
                    address: address(value)?,
                }
            }
            EditorGraphMutationDto::DisconnectNode { node_id } => {
                yss_graph_editor::EditorGraphMutation::DisconnectNode { node_id }
            }
            EditorGraphMutationDto::SetLiteral {
                address: value,
                literal,
            } => yss_graph_editor::EditorGraphMutation::SetLiteral {
                address: address(value)?,
                literal,
            },
            EditorGraphMutationDto::AddPortInstance {
                node_id,
                template_key,
                order,
            } => yss_graph_editor::EditorGraphMutation::AddPortInstance {
                node_id,
                template_key,
                order,
            },
            EditorGraphMutationDto::RemovePortInstance { address: value } => {
                yss_graph_editor::EditorGraphMutation::RemovePortInstance {
                    address: address(value)?,
                }
            }
            EditorGraphMutationDto::DuplicateSubgraph { node_ids, offset } => {
                yss_graph_editor::EditorGraphMutation::DuplicateSubgraph { node_ids, offset }
            }
            EditorGraphMutationDto::InsertSubgraph {
                snapshot_json,
                anchor,
            } => yss_graph_editor::EditorGraphMutation::InsertSubgraph {
                snapshot: crate::schema::graph_clipboard::parse_clipboard_snapshot(&snapshot_json)
                    .map_err(|_| EditorMutationMappingError::InvalidClipboardSubgraph)?,
                anchor,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn port_address_dto_preserves_tagged_camel_case_wire_and_typed_errors() {
        let node = NodeId::from_uuid(uuid::Uuid::from_u128(1));
        let declared = PortAddressDto::Declared {
            node_id: node.to_string().into(),
            port_key: "value".into(),
        };
        assert_eq!(
            serde_json::to_value(&declared).unwrap(),
            json!({ "kind": "declared", "nodeId": node.to_string(), "portKey": "value" })
        );
        assert!(matches!(
            PortAddress::try_from(PortAddressDto::Declared {
                node_id: "bad".into(),
                port_key: "value".into(),
            }),
            Err(PortAddressMappingError::InvalidNodeId)
        ));
    }
}
