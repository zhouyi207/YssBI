use crate::{
    graph::PinId,
    graph::{Connection, ConnectionManager},
};
use serde::{Deserialize, Serialize};

/// Connection DTO - 对应前端 Connection 类型
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionDTO {
    pub connections: Vec<ConnectionItemDTO>,
}

/// 单个连接项
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionItemDTO {
    pub from_pin: PinId,
    pub to_pin: PinId,
}

impl From<&Connection> for ConnectionItemDTO {
    fn from(value: &Connection) -> Self {
        Self {
            from_pin: value.from_pin,
            to_pin: value.to_pin,
        }
    }
}

impl From<&ConnectionManager> for ConnectionDTO {
    fn from(value: &ConnectionManager) -> Self {
        Self {
            connections: value
                .all_connections()
                .iter()
                .map(ConnectionItemDTO::from)
                .collect(),
        }
    }
}
