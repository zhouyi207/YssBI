use super::{VariableId, VariableScope};
use crate::graph::value::{DataValue, DataType};
use serde::{Deserialize, Serialize};

/// 变量定义（持久化到项目文件）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VariableInstance {
    /// 变量 ID
    pub id: VariableId,
    /// 变量名称
    pub name: String,
    /// 数据类型
    pub data_type: DataType,
    /// 值
    pub data_value: DataValue,
    /// 描述
    pub description: String,
    /// 变量作用域
    pub scope: VariableScope,
    /// 标签（用于分类和搜索）
    pub tags: Vec<String>,
}
