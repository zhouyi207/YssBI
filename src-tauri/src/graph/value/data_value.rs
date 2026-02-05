//! 数据值表示

use super::DataType;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::ops::{Add, Sub, Mul, Div};

/// 运行时数据值
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DataValue {
    Boolean(bool),
    Int32(i32),
    Int64(i64),
    Float32(f32),
    Float64(f64),
    String(String),
    Array(Vec<DataValue>),
    Object(serde_json::Map<String, JsonValue>),
    DataFrame(String), // DataFrame ID
    Null,
}

impl DataValue {
    /// 获取值的类型
    pub fn value_type(&self) -> DataType {
        match self {
            DataValue::Boolean(_) => DataType::Boolean,
            DataValue::Int32(_) => DataType::Int32,
            DataValue::Int64(_) => DataType::Int64,
            DataValue::Float32(_) => DataType::Float32,
            DataValue::Float64(_) => DataType::Float64,
            DataValue::String(_) => DataType::String,
            DataValue::Array(arr) => {
                if let Some(first) = arr.first() {
                    DataType::Array(Box::new(first.value_type()))
                } else {
                    DataType::Array(Box::new(DataType::Any))
                }
            }
            DataValue::Object(_) => DataType::Object,
            DataValue::Null => DataType::Null,
            DataValue::DataFrame(_) => DataType::DataFrame,
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
    pub fn from_json(json: &JsonValue, target_type: &DataType) -> Self {
        match (json, target_type) {
            (JsonValue::Bool(b), DataType::Boolean) => DataValue::Boolean(*b),
            (JsonValue::Number(n), DataType::Int32) => {
                DataValue::Int32(n.as_i64().unwrap_or(0) as i32)
            }
            (JsonValue::Number(n), DataType::Int64) => {
                DataValue::Int64(n.as_i64().unwrap_or(0))
            }
            (JsonValue::Number(n), DataType::Float32) => {
                DataValue::Float32(n.as_f64().unwrap_or(0.0) as f32)
            }
            (JsonValue::Number(n), DataType::Float64) => {
                DataValue::Float64(n.as_f64().unwrap_or(0.0))
            }
            (JsonValue::String(s), DataType::String) => DataValue::String(s.clone()),
            (JsonValue::Array(arr), DataType::Array(inner)) => {
                DataValue::Array(arr.iter().map(|v| Self::from_json(v, inner)).collect())
            }
            (JsonValue::Object(obj), DataType::Object) => DataValue::Object(obj.clone()),
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

// ============================================================================
// 运算符重载实现
// ============================================================================

/// 加法运算符实现
impl Add for DataValue {
    type Output = Result<DataValue, String>;

    fn add(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            // 同类型运算
            (DataValue::Int32(a), DataValue::Int32(b)) => Ok(DataValue::Int32(a + b)),
            (DataValue::Int64(a), DataValue::Int64(b)) => Ok(DataValue::Int64(a + b)),
            (DataValue::Float32(a), DataValue::Float32(b)) => Ok(DataValue::Float32(a + b)),
            (DataValue::Float64(a), DataValue::Float64(b)) => Ok(DataValue::Float64(a + b)),
            
            // 类型提升：Int32 + Int64 -> Int64
            (DataValue::Int32(a), DataValue::Int64(b)) => Ok(DataValue::Int64(a as i64 + b)),
            (DataValue::Int64(a), DataValue::Int32(b)) => Ok(DataValue::Int64(a + b as i64)),
            
            // 类型提升：Int32 + Float32 -> Float32
            (DataValue::Int32(a), DataValue::Float32(b)) => Ok(DataValue::Float32(a as f32 + b)),
            (DataValue::Float32(a), DataValue::Int32(b)) => Ok(DataValue::Float32(a + b as f32)),
            
            // 类型提升：Int -> Float64
            (DataValue::Int32(a), DataValue::Float64(b)) => Ok(DataValue::Float64(a as f64 + b)),
            (DataValue::Float64(a), DataValue::Int32(b)) => Ok(DataValue::Float64(a + b as f64)),
            (DataValue::Int64(a), DataValue::Float64(b)) => Ok(DataValue::Float64(a as f64 + b)),
            (DataValue::Float64(a), DataValue::Int64(b)) => Ok(DataValue::Float64(a + b as f64)),
            
            // 类型提升：Float32 + Float64 -> Float64
            (DataValue::Float32(a), DataValue::Float64(b)) => Ok(DataValue::Float64(a as f64 + b)),
            (DataValue::Float64(a), DataValue::Float32(b)) => Ok(DataValue::Float64(a + b as f64)),
            
            // 字符串拼接
            (DataValue::String(a), DataValue::String(b)) => Ok(DataValue::String(format!("{}{}", a, b))),
            
            (a, b) => Err(format!(
                "Cannot add {:?} and {:?}: incompatible types",
                a.value_type(),
                b.value_type()
            )),
        }
    }
}

/// 减法运算符实现
impl Sub for DataValue {
    type Output = Result<DataValue, String>;

    fn sub(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            // 同类型运算
            (DataValue::Int32(a), DataValue::Int32(b)) => Ok(DataValue::Int32(a - b)),
            (DataValue::Int64(a), DataValue::Int64(b)) => Ok(DataValue::Int64(a - b)),
            (DataValue::Float32(a), DataValue::Float32(b)) => Ok(DataValue::Float32(a - b)),
            (DataValue::Float64(a), DataValue::Float64(b)) => Ok(DataValue::Float64(a - b)),
            
            // 类型提升：Int32 - Int64 -> Int64
            (DataValue::Int32(a), DataValue::Int64(b)) => Ok(DataValue::Int64(a as i64 - b)),
            (DataValue::Int64(a), DataValue::Int32(b)) => Ok(DataValue::Int64(a - b as i64)),
            
            // 类型提升：Int32 - Float32 -> Float32
            (DataValue::Int32(a), DataValue::Float32(b)) => Ok(DataValue::Float32(a as f32 - b)),
            (DataValue::Float32(a), DataValue::Int32(b)) => Ok(DataValue::Float32(a - b as f32)),
            
            // 类型提升：Int -> Float64
            (DataValue::Int32(a), DataValue::Float64(b)) => Ok(DataValue::Float64(a as f64 - b)),
            (DataValue::Float64(a), DataValue::Int32(b)) => Ok(DataValue::Float64(a - b as f64)),
            (DataValue::Int64(a), DataValue::Float64(b)) => Ok(DataValue::Float64(a as f64 - b)),
            (DataValue::Float64(a), DataValue::Int64(b)) => Ok(DataValue::Float64(a - b as f64)),
            
            // 类型提升：Float32 - Float64 -> Float64
            (DataValue::Float32(a), DataValue::Float64(b)) => Ok(DataValue::Float64(a as f64 - b)),
            (DataValue::Float64(a), DataValue::Float32(b)) => Ok(DataValue::Float64(a - b as f64)),
            
            (a, b) => Err(format!(
                "Cannot subtract {:?} from {:?}: incompatible types",
                b.value_type(),
                a.value_type()
            )),
        }
    }
}

/// 乘法运算符实现
impl Mul for DataValue {
    type Output = Result<DataValue, String>;

    fn mul(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            // 同类型运算
            (DataValue::Int32(a), DataValue::Int32(b)) => Ok(DataValue::Int32(a * b)),
            (DataValue::Int64(a), DataValue::Int64(b)) => Ok(DataValue::Int64(a * b)),
            (DataValue::Float32(a), DataValue::Float32(b)) => Ok(DataValue::Float32(a * b)),
            (DataValue::Float64(a), DataValue::Float64(b)) => Ok(DataValue::Float64(a * b)),
            
            // 类型提升：Int32 * Int64 -> Int64
            (DataValue::Int32(a), DataValue::Int64(b)) => Ok(DataValue::Int64(a as i64 * b)),
            (DataValue::Int64(a), DataValue::Int32(b)) => Ok(DataValue::Int64(a * b as i64)),
            
            // 类型提升：Int32 * Float32 -> Float32
            (DataValue::Int32(a), DataValue::Float32(b)) => Ok(DataValue::Float32(a as f32 * b)),
            (DataValue::Float32(a), DataValue::Int32(b)) => Ok(DataValue::Float32(a * b as f32)),
            
            // 类型提升：Int -> Float64
            (DataValue::Int32(a), DataValue::Float64(b)) => Ok(DataValue::Float64(a as f64 * b)),
            (DataValue::Float64(a), DataValue::Int32(b)) => Ok(DataValue::Float64(a * b as f64)),
            (DataValue::Int64(a), DataValue::Float64(b)) => Ok(DataValue::Float64(a as f64 * b)),
            (DataValue::Float64(a), DataValue::Int64(b)) => Ok(DataValue::Float64(a * b as f64)),
            
            // 类型提升：Float32 * Float64 -> Float64
            (DataValue::Float32(a), DataValue::Float64(b)) => Ok(DataValue::Float64(a as f64 * b)),
            (DataValue::Float64(a), DataValue::Float32(b)) => Ok(DataValue::Float64(a * b as f64)),
            
            (a, b) => Err(format!(
                "Cannot multiply {:?} and {:?}: incompatible types",
                a.value_type(),
                b.value_type()
            )),
        }
    }
}

/// 除法运算符实现
impl Div for DataValue {
    type Output = Result<DataValue, String>;

    fn div(self, rhs: Self) -> Self::Output {
        // 检查除零
        let is_zero = match &rhs {
            DataValue::Int32(v) => *v == 0,
            DataValue::Int64(v) => *v == 0,
            DataValue::Float32(v) => *v == 0.0,
            DataValue::Float64(v) => *v == 0.0,
            _ => false,
        };
        
        if is_zero {
            return Err("Division by zero".to_string());
        }
        
        match (self, rhs) {
            // 同类型运算
            (DataValue::Int32(a), DataValue::Int32(b)) => Ok(DataValue::Int32(a / b)),
            (DataValue::Int64(a), DataValue::Int64(b)) => Ok(DataValue::Int64(a / b)),
            (DataValue::Float32(a), DataValue::Float32(b)) => Ok(DataValue::Float32(a / b)),
            (DataValue::Float64(a), DataValue::Float64(b)) => Ok(DataValue::Float64(a / b)),
            
            // 类型提升：Int32 / Int64 -> Int64
            (DataValue::Int32(a), DataValue::Int64(b)) => Ok(DataValue::Int64(a as i64 / b)),
            (DataValue::Int64(a), DataValue::Int32(b)) => Ok(DataValue::Int64(a / b as i64)),
            
            // 类型提升：Int32 / Float32 -> Float32
            (DataValue::Int32(a), DataValue::Float32(b)) => Ok(DataValue::Float32(a as f32 / b)),
            (DataValue::Float32(a), DataValue::Int32(b)) => Ok(DataValue::Float32(a / b as f32)),
            
            // 类型提升：Int -> Float64
            (DataValue::Int32(a), DataValue::Float64(b)) => Ok(DataValue::Float64(a as f64 / b)),
            (DataValue::Float64(a), DataValue::Int32(b)) => Ok(DataValue::Float64(a / b as f64)),
            (DataValue::Int64(a), DataValue::Float64(b)) => Ok(DataValue::Float64(a as f64 / b)),
            (DataValue::Float64(a), DataValue::Int64(b)) => Ok(DataValue::Float64(a / b as f64)),
            
            // 类型提升：Float32 / Float64 -> Float64
            (DataValue::Float32(a), DataValue::Float64(b)) => Ok(DataValue::Float64(a as f64 / b)),
            (DataValue::Float64(a), DataValue::Float32(b)) => Ok(DataValue::Float64(a / b as f64)),
            
            (a, b) => Err(format!(
                "Cannot divide {:?} by {:?}: incompatible types",
                a.value_type(),
                b.value_type()
            )),
        }
    }
}

impl DataValue {
    /// 辅助方法：执行加法运算
    pub fn add(&self, other: &DataValue) -> Result<DataValue, String> {
        self.clone() + other.clone()
    }
    
    /// 辅助方法：执行减法运算
    pub fn sub(&self, other: &DataValue) -> Result<DataValue, String> {
        self.clone() - other.clone()
    }
    
    /// 辅助方法：执行乘法运算
    pub fn mul(&self, other: &DataValue) -> Result<DataValue, String> {
        self.clone() * other.clone()
    }
    
    /// 辅助方法：执行除法运算
    pub fn div(&self, other: &DataValue) -> Result<DataValue, String> {
        self.clone() / other.clone()
    }
}
