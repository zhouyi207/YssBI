//! 数据传输对象（DTO）
//!
//! 定义用于序列化、前端交互和持久化存储的数据结构。
//! 这些结构是"贫血模型"，只包含数据，不包含业务逻辑。

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ==================== Pin DTO ====================

/// Pin 数据传输对象
///
/// 用于序列化和前端交互的 Pin 数据结构
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PinDto {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub pin_type: String,
    pub links: Vec<String>,
    
    /// 默认值（来自节点定义）
    #[serde(rename = "defaultValue", skip_serializing_if = "Option::is_none")]
    pub default_value: Option<Value>,
    
    /// 用户设置的值（覆盖默认值）
    #[serde(rename = "userValue", skip_serializing_if = "Option::is_none")]
    pub user_value: Option<Value>,
    
    #[serde(rename = "isArray", default)]
    pub is_array: bool,
    
    /// 是否显示输入控件
    #[serde(rename = "showWidget", default = "default_true")]
    pub show_widget: bool,
    
    /// 控件类型提示（slider, color, textarea 等）
    #[serde(rename = "widgetType", skip_serializing_if = "Option::is_none")]
    pub widget_type: Option<String>,
}

fn default_true() -> bool {
    true
}

/// Pin 定义（元数据）
///
/// 用于定义函数/宏的输入输出参数
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PinDefDto {
    pub name: String,
    #[serde(rename = "type")]
    pub pin_type: String,
    #[serde(rename = "defaultValue")]
    pub default_value: Option<Value>,
    #[serde(rename = "isArray", default)]
    pub is_array: bool,
}

impl Default for PinDefDto {
    fn default() -> Self {
        Self {
            name: "".into(),
            pin_type: "object".into(),
            default_value: None,
            is_array: false,
        }
    }
}

// ==================== Node DTO ====================

/// 节点数据传输对象
///
/// 用于序列化和前端交互的节点数据结构
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NodeDto {
    pub id: String,
    #[serde(rename = "type")]
    pub node_type: String,
    pub title: String,
    pub inputs: Vec<PinDto>,
    pub outputs: Vec<PinDto>,
    #[serde(rename = "variableId")]
    pub variable_id: Option<String>,
    #[serde(rename = "subGraphId")]
    pub sub_graph_id: Option<String>,
}

// ==================== Variable DTO ====================

/// 变量数据传输对象
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VariableDto {
    pub name: String,
    #[serde(rename = "type")]
    pub var_type: String,
    pub value: Value,
}

// ==================== Graph DTO ====================

/// 图数据传输对象
///
/// 用于执行时的简化图表示
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GraphDto {
    pub version: String,
    pub nodes: Vec<NodeDto>,
    pub variables: Option<std::collections::HashMap<String, VariableDto>>,
}
