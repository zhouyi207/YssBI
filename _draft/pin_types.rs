//! Pin 类型定义模块
//!
//! 定义所有可用的 Pin 类型及其属性，包括颜色、兼容性规则等。

use serde::{Deserialize, Serialize};

/// Pin 类型定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PinTypeDefinition {
    /// 类型标识符 (如 "exec", "float", "int", "string", "bool")
    pub name: String,
    /// 显示名称
    pub display_name: String,
    /// 是否为执行类型 (exec pin)
    pub is_exec: bool,
    /// 是否支持数组模式 (UI 渲染用)
    #[serde(default)]
    pub supports_array: bool,
    /// 可以隐式转换到的类型列表
    pub implicit_convert_to: Vec<String>,
    /// 可以显式转换到的类型列表
    pub explicit_convert_to: Vec<String>,
    /// 默认值的 JSON 表示
    pub default_value: Option<serde_json::Value>,
}

/// 类型兼容性检查结果
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TypeConversion {
    /// 相同类型，直接兼容
    Same,
    /// 可以隐式转换
    Implicit,
    /// 需要显式转换
    Explicit,
    /// 不兼容
    Incompatible,
}