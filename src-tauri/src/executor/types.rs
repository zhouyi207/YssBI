//! 执行器类型定义
//!
//! 定义执行引擎内部使用的数据类型和状态枚举。

use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// 执行引擎内部传递的数据容器
///
/// 针对 BI 系统优化，支持常见数据类型 and 大型数据结构的零拷贝传递
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum DataValue {
    /// 空值
    None,
    /// 数字类型
    Number(f64),
    /// 字符串类型
    String(String),
    /// 布尔值
    Boolean(bool),
    /// 列表（支持嵌套的 DataValue）
    List(Vec<DataValue>),
    /// JSON 对象（用于复杂结构）
    Object(serde_json::Value),
    /// DataFrame（使用 Arc 实现零拷贝传递）
    /// 注意：这里使用 serde_json::Value 作为占位，实际可替换为 polars::DataFrame
    #[serde(skip)]
    DataFrame(Arc<serde_json::Value>),
}

impl DataValue {
    /// 获取数据类型名称
    pub fn type_name(&self) -> &'static str {
        match self {
            DataValue::None => "none",
            DataValue::Number(_) => "number",
            DataValue::String(_) => "string",
            DataValue::Boolean(_) => "boolean",
            DataValue::List(_) => "list",
            DataValue::Object(_) => "object",
            DataValue::DataFrame(_) => "dataframe",
        }
    }

    /// 检查类型是否兼容（用于连线验证）
    pub fn is_compatible_with(&self, other: &DataValue) -> bool {
        match (self, other) {
            // 相同类型总是兼容
            (DataValue::None, DataValue::None) => true,
            (DataValue::Number(_), DataValue::Number(_)) => true,
            (DataValue::String(_), DataValue::String(_)) => true,
            (DataValue::Boolean(_), DataValue::Boolean(_)) => true,
            (DataValue::List(_), DataValue::List(_)) => true,
            (DataValue::Object(_), DataValue::Object(_)) => true,
            (DataValue::DataFrame(_), DataValue::DataFrame(_)) => true,
            // None 可以兼容任何类型
            (DataValue::None, _) | (_, DataValue::None) => true,
            // 其他情况不兼容
            _ => false,
        }
    }

    /// 检查类型是否可以转换为目标类型
    pub fn can_convert_to(&self, target_type: &str) -> bool {
        match (self.type_name(), target_type) {
            // 相同类型可以转换
            (a, b) if a == b => true,
            // None 可以转换为任何类型
            ("none", _) | (_, "none") => true,
            // Number 可以转换为 String
            ("number", "string") => true,
            // Boolean 可以转换为 String 或 Number
            ("boolean", "string") | ("boolean", "number") => true,
            // String 可以尝试转换为 Number 或 Boolean
            ("string", "number") | ("string", "boolean") => true,
            _ => false,
        }
    }
}

impl Default for DataValue {
    fn default() -> Self {
        DataValue::None
    }
}

/// 节点执行模型
/// 
/// 定义节点如何参与执行流程
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionModel {
    /// 纯控制流节点（如 Sequence, Branch）
    /// - 只有 ExecPin
    /// - 决定执行顺序
    /// - 不产生数据
    ControlFlow,
    
    /// 纯数据流节点（如 Constant, Add, Math）
    /// - 只有 DataPin
    /// - 纯函数式计算
    /// - 可以安全缓存结果
    /// - 不参与 ExecFlow
    DataFlow,
    
    /// 混合节点（如 IfElse, Print, SetVariable）
    /// - 同时有 ExecPin 和 DataPin
    /// - 需要数据来决定控制流
    /// - 或者在控制流中产生副作用
    Hybrid,
    
    /// 事件节点（如 OnRun, OnClick）
    /// - 执行的起点
    /// - 只有输出 ExecPin
    Event,
}

impl ExecutionModel {
    /// 判断节点是否可以缓存数据结果
    pub fn is_cacheable(&self) -> bool {
        matches!(self, ExecutionModel::DataFlow)
    }
    
    /// 判断节点是否参与控制流
    pub fn participates_in_control_flow(&self) -> bool {
        !matches!(self, ExecutionModel::DataFlow)
    }
}
