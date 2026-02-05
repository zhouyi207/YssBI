use super::VariableScope;
use crate::graph::value::{DataValue, DataType};
use serde::{Deserialize, Serialize};

/// 变量定义（持久化到项目文件）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableDefinition {
    // ===== 元数据 =====
    /// 变量 ID
    pub id: String,
    /// 变量名称
    pub name: String,
    /// 数据类型
    pub data_type: DataType,
    /// 描述
    #[serde(default)]
    pub description: String,

    // ===== 作用域 =====
    /// 变量作用域
    #[serde(default)]
    pub scope: VariableScope,

    // ===== 值配置 =====
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<DataValue>,

    // ===== 可选配置 =====
    /// 是否为数组
    #[serde(default)]
    pub is_array: bool,

    /// 是否为常量
    #[serde(default)]
    pub is_constant: bool,

    /// 是否暴露给外部（可被其他图引用）
    #[serde(default)]
    pub is_exposed: bool,

    /// 标签（用于分类和搜索）
    #[serde(default)]
    pub tags: Vec<String>,
}
