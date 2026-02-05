use super::DataValue;
use serde::{Deserialize, Serialize};
use std::fmt;

/// 基础值类型
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DataType {
    // 基础类型
    Boolean,
    Int32,
    Int64,
    Float32,
    Float64,
    String,

    // 复合类型
    Array(Box<DataType>),
    Object,

    // 特殊类型
    Any,
    Null,

    // 数据框架
    DataFrame,
}

impl fmt::Display for DataType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DataType::Boolean => write!(f, "Boolean"),
            DataType::Int32 => write!(f, "Int32"),
            DataType::Int64 => write!(f, "Int64"),
            DataType::Float32 => write!(f, "Float32"),
            DataType::Float64 => write!(f, "Float64"),
            DataType::String => write!(f, "String"),
            DataType::Array(inner) => write!(f, "Array<{}>", inner),
            DataType::Object => write!(f, "Object"),
            DataType::Any => write!(f, "Any"),
            DataType::Null => write!(f, "Null"),
            DataType::DataFrame => write!(f, "DataFrame"),
        }
    }
}

/// todo
impl DataType {
    pub fn default_value(&self) -> Option<DataValue> {
        match self {
            DataType::Float64 => Some(DataValue::Float64(0.0)),
            DataType::Int64 => Some(DataValue::Int64(0)),
            DataType::Boolean => Some(DataValue::Boolean(false)),
            DataType::String => Some(DataValue::String(String::new())),
            DataType::Null => None,
            _ => None,
        }
    }

    /// 检查类型是否为数值类型
    pub fn is_numeric(&self) -> bool {
        matches!(
            self,
            DataType::Int32 | DataType::Int64 | DataType::Float32 | DataType::Float64
        )
    }

    /// 检查类型是否可比较
    pub fn is_comparable(&self) -> bool {
        matches!(
            self,
            DataType::Boolean
                | DataType::Int32
                | DataType::Int64
                | DataType::Float32
                | DataType::Float64
                | DataType::String
        )
    }

    /// 检查类型是否可迭代
    pub fn is_iterable(&self) -> bool {
        matches!(self, DataType::Array(_) | DataType::String)
    }
}
