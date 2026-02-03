use std::fmt;
use serde::{Deserialize, Serialize};
use super::DataValue;

/// 基础值类型
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ValueType {
    // 基础类型
    Boolean,
    Int32,
    Int64,
    Float32,
    Float64,
    String,

    // 复合类型
    Array(Box<ValueType>),
    Object,

    // 特殊类型
    Any,
    Null,

    // 数据框架
    DataFrame,
}

impl fmt::Display for ValueType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValueType::Boolean => write!(f, "Boolean"),
            ValueType::Int32 => write!(f, "Int32"),
            ValueType::Int64 => write!(f, "Int64"),
            ValueType::Float32 => write!(f, "Float32"),
            ValueType::Float64 => write!(f, "Float64"),
            ValueType::String => write!(f, "String"),
            ValueType::Array(inner) => write!(f, "Array<{}>", inner),
            ValueType::Object => write!(f, "Object"),
            ValueType::Any => write!(f, "Any"),
            ValueType::Null => write!(f, "Null"),
            ValueType::DataFrame => write!(f, "DataFrame"),
        }
    }
}

/// todo
impl ValueType {
    pub fn default_value(&self) -> Option<DataValue> {
        match self {
            ValueType::Float64 => Some(DataValue::Float64(0.0)),
            ValueType::Int64 => Some(DataValue::Int64(0)),
            ValueType::Boolean => Some(DataValue::Boolean(false)),
            ValueType::String => Some(DataValue::String(String::new())),
            ValueType::Null => None,
            _ => None,
        }
    }

    /// 检查类型是否为数值类型
    pub fn is_numeric(&self) -> bool {
        matches!(
            self,
            ValueType::Int32 | ValueType::Int64 | ValueType::Float32 | ValueType::Float64
        )
    }

    /// 检查类型是否可比较
    pub fn is_comparable(&self) -> bool {
        matches!(
            self,
            ValueType::Boolean
                | ValueType::Int32
                | ValueType::Int64
                | ValueType::Float32
                | ValueType::Float64
                | ValueType::String
        )
    }

    /// 检查类型是否可迭代
    pub fn is_iterable(&self) -> bool {
        matches!(self, ValueType::Array(_) | ValueType::String)
    }
}