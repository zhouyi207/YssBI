use serde::{Deserialize, Serialize};

use crate::graph_document::{NodeId, PortAddress, PortInstanceId};
use crate::node_system::protocol::PortKey;

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
            crate::graph_document::PortRef::Declared { key } => Self::Declared {
                node_id: address.node_id.to_string().into_boxed_str(),
                port_key: key.as_str().into(),
            },
            crate::graph_document::PortRef::Instance {
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
                PortKey::new(template_key)
                    .map_err(|_| PortAddressMappingError::InvalidPortKey)?,
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
