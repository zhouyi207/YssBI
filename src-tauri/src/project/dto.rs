//! 数据传输对象（DTO）

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 节点 DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeDto {
    pub id: String,
    pub node_type: String,
    pub title: String,
    pub inputs: Vec<PinDto>,
    pub outputs: Vec<PinDto>,
    pub variable_id: Option<String>,
    pub sub_graph_id: Option<String>,
}

/// Pin DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PinDto {
    pub id: String,
    pub name: String,
    pub pin_type: String,
    pub default_value: Option<Value>,
    pub user_value: Option<Value>,
    pub is_array: bool,
    pub show_widget: bool,
    pub widget_type: Option<String>,
}

/// 连接 DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionDto {
    pub from_pin: String,
    pub to_pin: String,
}

/// 项目 DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectDto {
    pub name: String,
    pub version: String,
    pub nodes: Vec<NodeDto>,
    pub connections: Vec<ConnectionDto>,
}
