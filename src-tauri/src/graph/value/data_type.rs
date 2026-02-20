use super::DataValue;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// 基础值类型
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "inner")]
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

    // 数据框架
    DataFrame,
    DataSeries(Box<DataType>),

    // 特殊类型
    Any,
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
            DataType::DataFrame => write!(f, "DataFrame"),
            DataType::DataSeries(inner) => write!(f, "DataSeries<{}>", inner),
            DataType::Any => write!(f, "Any"),
        }
    }
}

impl FromStr for DataType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Boolean" => Ok(DataType::Boolean),
            "Int32" => Ok(DataType::Int32),
            "Int64" => Ok(DataType::Int64),
            "Float32" => Ok(DataType::Float32),
            "Float64" => Ok(DataType::Float64),
            "String" => Ok(DataType::String),
            "Object" => Ok(DataType::Object),
            "DataFrame" => Ok(DataType::DataFrame),
            "DataSeries" => Ok(DataType::DataSeries(Box::new(DataType::Any))),
            "Any" => Ok(DataType::Any),
            _ => {
                if let Some(inner) = s.strip_prefix("Array<").and_then(|s| s.strip_suffix('>')) {
                    let inner = inner.parse()?;
                    return Ok(DataType::Array(Box::new(inner)));
                }
                if let Some(inner) = s.strip_prefix("DataSeries<").and_then(|s| s.strip_suffix('>')) {
                    let inner = inner.parse()?;
                    return Ok(DataType::DataSeries(Box::new(inner)));
                }
                Err(format!("Unknown DataType: {}", s))
            }
        }
    }
}

impl DataType {
    /// 返回该类型的默认值（用于 Pin 占位、变量初始化等）
    pub fn default_value(&self) -> DataValue {
        match self {
            DataType::Boolean => DataValue::Boolean(false),
            DataType::Int32 => DataValue::Int32(0),
            DataType::Int64 => DataValue::Int64(0),
            DataType::Float32 => DataValue::Float32(0.0),
            DataType::Float64 => DataValue::Float64(0.0),
            DataType::String => DataValue::String(String::new()),
            DataType::Array(_) => DataValue::Array(Vec::new()),
            DataType::Object => DataValue::Object(std::collections::HashMap::new()),
            DataType::Any | DataType::DataFrame | DataType::DataSeries(_) => DataValue::Null,
        }
    }

    /// 是否为标量/基础类型（非复合、非 Any）
    pub fn is_primitive(&self) -> bool {
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

    /// 是否为数值类型
    pub fn is_numeric(&self) -> bool {
        matches!(
            self,
            DataType::Int32 | DataType::Int64 | DataType::Float32 | DataType::Float64
        )
    }

    /// 是否支持比较运算（==, !=, <, > 等）
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

    /// 是否可迭代（for-in、map 等）
    pub fn is_iterable(&self) -> bool {
        matches!(self, DataType::Array(_) | DataType::String)
    }

    /// Array 的元素类型，非 Array 返回 None
    pub fn array_inner(&self) -> Option<&DataType> {
        match self {
            DataType::Array(inner) => Some(inner),
            _ => None,
        }
    }

    /// DataSeries 的元素类型，非 DataSeries 返回 None
    pub fn series_inner(&self) -> Option<&DataType> {
        match self {
            DataType::DataSeries(inner) => Some(inner),
            _ => None,
        }
    }

    /// 检查 from 类型的值是否可以赋给本类型
    pub fn can_accept(&self, from: &DataType) -> bool {
        if from == self {
            return true;
        }
        if matches!(self, DataType::Any) || matches!(from, DataType::Any) {
            return true;
        }
        match (from, self) {
            // 数值隐式提升
            (DataType::Int32, DataType::Int64)
            | (DataType::Int32, DataType::Float64)
            | (DataType::Int64, DataType::Float64)
            | (DataType::Float32, DataType::Float64) => true,
            // 容器类型：内层 Any 接受任意具体类型
            (DataType::Array(from_inner), DataType::Array(to_inner)) => {
                to_inner.can_accept(from_inner)
            }
            (DataType::DataSeries(from_inner), DataType::DataSeries(to_inner)) => {
                to_inner.can_accept(from_inner)
            }
            _ => false,
        }
    }
}
