//! 节点类型定义模块
//!
//! 定义节点相关的数据结构。

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Pin 数据（运行时）
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PinData {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub pin_type: String,
    pub links: Vec<String>,
    #[serde(rename = "defaultValue")]
    pub default_value: Option<Value>,
    #[serde(rename = "isArray", default)]
    pub is_array: bool,
}

/// Pin 定义（元数据）
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PinDefinition {
    pub name: String,
    #[serde(rename = "type")]
    pub pin_type: String,
    #[serde(rename = "defaultValue")]
    pub default_value: Option<Value>,
    #[serde(rename = "isArray", default)]
    pub is_array: bool,
}

impl Default for PinDefinition {
    fn default() -> Self {
        Self {
            name: "".into(),
            pin_type: "object".into(),
            default_value: None,
            is_array: false,
        }
    }
}

/// 节点数据（运行时）
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NodeData {
    pub id: String,
    #[serde(rename = "type")]
    pub node_type: String,
    pub title: String,
    pub inputs: Vec<PinData>,
    pub outputs: Vec<PinData>,
    #[serde(rename = "variableId")]
    pub variable_id: Option<String>,
    #[serde(rename = "subGraphId")]
    pub sub_graph_id: Option<String>,
}

/// 变量数据
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VariableData {
    pub name: String,
    #[serde(rename = "type")]
    pub var_type: String,
    pub value: Value,
}

/// 图数据（完整项目）
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GraphData {
    pub version: String,
    pub nodes: Vec<NodeData>,
    pub variables: Option<std::collections::HashMap<String, VariableData>>,
}
