//! 数据值表示

use super::ValueType;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

/// 运行时数据值
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DataValue {
    Boolean(bool),
    Int32(i32),
    Int64(i64),
    Float32(f32),
    Float64(f64),
    String(String),
    Array(Vec<DataValue>),
    Object(serde_json::Map<String, JsonValue>),
    Null,
    DataFrame(String), // DataFrame ID
}

impl DataValue {
    /// 获取值的类型
    pub fn value_type(&self) -> ValueType {
        match self {
            DataValue::Boolean(_) => ValueType::Boolean,
            DataValue::Int32(_) => ValueType::Int32,
            DataValue::Int64(_) => ValueType::Int64,
            DataValue::Float32(_) => ValueType::Float32,
            DataValue::Float64(_) => ValueType::Float64,
            DataValue::String(_) => ValueType::String,
            DataValue::Array(arr) => {
                if let Some(first) = arr.first() {
                    ValueType::Array(Box::new(first.value_type()))
                } else {
                    ValueType::Array(Box::new(ValueType::Any))
                }
            }
            DataValue::Object(_) => ValueType::Object,
            DataValue::Null => ValueType::Null,
            DataValue::DataFrame(_) => ValueType::DataFrame,
        }
    }

    /// 转换为 JSON
    pub fn to_json(&self) -> JsonValue {
        match self {
            DataValue::Boolean(b) => JsonValue::Bool(*b),
            DataValue::Int32(i) => JsonValue::Number((*i).into()),
            DataValue::Int64(i) => JsonValue::Number((*i).into()),
            DataValue::Float32(f) => {
                serde_json::Number::from_f64(*f as f64)
                    .map(JsonValue::Number)
                    .unwrap_or(JsonValue::Null)
            }
            DataValue::Float64(f) => {
                serde_json::Number::from_f64(*f)
                    .map(JsonValue::Number)
                    .unwrap_or(JsonValue::Null)
            }
            DataValue::String(s) => JsonValue::String(s.clone()),
            DataValue::Array(arr) => {
                JsonValue::Array(arr.iter().map(|v| v.to_json()).collect())
            }
            DataValue::Object(obj) => JsonValue::Object(obj.clone()),
            DataValue::Null => JsonValue::Null,
            DataValue::DataFrame(id) => JsonValue::String(format!("DataFrame:{}", id)),
        }
    }

    /// 从 JSON 创建
    pub fn from_json(json: &JsonValue, target_type: &ValueType) -> Self {
        match (json, target_type) {
            (JsonValue::Bool(b), ValueType::Boolean) => DataValue::Boolean(*b),
            (JsonValue::Number(n), ValueType::Int32) => {
                DataValue::Int32(n.as_i64().unwrap_or(0) as i32)
            }
            (JsonValue::Number(n), ValueType::Int64) => {
                DataValue::Int64(n.as_i64().unwrap_or(0))
            }
            (JsonValue::Number(n), ValueType::Float32) => {
                DataValue::Float32(n.as_f64().unwrap_or(0.0) as f32)
            }
            (JsonValue::Number(n), ValueType::Float64) => {
                DataValue::Float64(n.as_f64().unwrap_or(0.0))
            }
            (JsonValue::String(s), ValueType::String) => DataValue::String(s.clone()),
            (JsonValue::Array(arr), ValueType::Array(inner)) => {
                DataValue::Array(arr.iter().map(|v| Self::from_json(v, inner)).collect())
            }
            (JsonValue::Object(obj), ValueType::Object) => DataValue::Object(obj.clone()),
            (JsonValue::Null, _) => DataValue::Null,
            _ => DataValue::Null,
        }
    }

    // 类型转换辅助方法
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            DataValue::Boolean(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_i32(&self) -> Option<i32> {
        match self {
            DataValue::Int32(i) => Some(*i),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            DataValue::Int64(i) => Some(*i),
            DataValue::Int32(i) => Some(*i as i64),
            _ => None,
        }
    }

    pub fn as_f32(&self) -> Option<f32> {
        match self {
            DataValue::Float32(f) => Some(*f),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            DataValue::Float64(f) => Some(*f),
            DataValue::Float32(f) => Some(*f as f64),
            DataValue::Int64(i) => Some(*i as f64),
            DataValue::Int32(i) => Some(*i as f64),
            _ => None,
        }
    }

    pub fn as_string(&self) -> Option<&str> {
        match self {
            DataValue::String(s) => Some(s),
            _ => None,
        }
    }
}

impl Default for DataValue {
    fn default() -> Self {
        DataValue::Null
    }
}
