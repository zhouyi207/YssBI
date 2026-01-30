//! 基于 Polars/Arrow 的值类型定义
//!
//! 使用 Polars 的类型系统来表示数据，提供更好的 BI 系统集成

use polars::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// 执行引擎中传递的值类型
/// 
/// 基于 Polars AnyValue 设计，支持标量和 DataFrame
#[derive(Debug, Clone)]
pub enum Value {
    /// 空值
    Null,
    
    /// 布尔值
    Boolean(bool),
    
    /// 整数（统一使用 i64）
    Int64(i64),
    
    /// 浮点数（统一使用 f64）
    Float64(f64),
    
    /// 字符串
    String(String),
    
    /// 日期（天数，从 Unix epoch 开始）
    Date(i32),
    
    /// 时间戳（微秒，从 Unix epoch 开始）
    Datetime(i64),
    
    /// 时间间隔（纳秒）
    Duration(i64),
    
    /// 列表（支持嵌套）
    List(Vec<Value>),
    
    /// 结构体（类似 JSON 对象）
    Struct(Vec<(String, Value)>),
    
    /// DataFrame（使用 Arc 实现零拷贝传递）
    DataFrame(Arc<DataFrame>),
    
    /// Series（单列数据）
    Series(Arc<Series>),
}

/// 值类型枚举（对应 Polars DataType）
/// 
/// 用于 Pin 的 data_type 字段，描述期望的数据类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum ValueType {
    /// 空类型
    Null,
    
    /// 布尔类型
    Boolean,
    
    /// 整数类型
    Int64,
    
    /// 浮点数类型
    Float64,
    
    /// 字符串类型
    String,
    
    /// 日期类型
    Date,
    
    /// 时间戳类型
    Datetime,
    
    /// 时间间隔类型
    Duration,
    
    /// 列表类型（包含元素类型）
    List(Box<ValueType>),
    
    /// 结构体类型（包含字段定义）
    Struct(Vec<(String, ValueType)>),
    
    /// DataFrame 类型
    DataFrame,
    
    /// Series 类型
    Series,
    
    /// 任意类型（用于泛型节点）
    Any,
}

impl Value {
    /// 获取值的类型
    pub fn value_type(&self) -> ValueType {
        match self {
            Value::Null => ValueType::Null,
            Value::Boolean(_) => ValueType::Boolean,
            Value::Int64(_) => ValueType::Int64,
            Value::Float64(_) => ValueType::Float64,
            Value::String(_) => ValueType::String,
            Value::Date(_) => ValueType::Date,
            Value::Datetime(_) => ValueType::Datetime,
            Value::Duration(_) => ValueType::Duration,
            Value::List(items) => {
                if items.is_empty() {
                    ValueType::List(Box::new(ValueType::Any))
                } else {
                    ValueType::List(Box::new(items[0].value_type()))
                }
            }
            Value::Struct(fields) => {
                let field_types = fields
                    .iter()
                    .map(|(name, value)| (name.clone(), value.value_type()))
                    .collect();
                ValueType::Struct(field_types)
            }
            Value::DataFrame(_) => ValueType::DataFrame,
            Value::Series(_) => ValueType::Series,
        }
    }

    /// 检查值是否与类型兼容
    pub fn is_compatible_with(&self, target_type: &ValueType) -> bool {
        match (self.value_type(), target_type) {
            // Any 类型兼容所有值
            (_, ValueType::Any) => true,
            // Null 可以兼容任何类型
            (ValueType::Null, _) => true,
            // 相同类型兼容
            (a, b) if a == *b => true,
            // 数字类型之间可以转换
            (ValueType::Int64, ValueType::Float64) => true,
            (ValueType::Float64, ValueType::Int64) => true,
            // 其他情况不兼容
            _ => false,
        }
    }

    /// 尝试转换为布尔值
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Boolean(b) => Some(*b),
            Value::Int64(i) => Some(*i != 0),
            Value::Float64(f) => Some(*f != 0.0),
            _ => None,
        }
    }

    /// 尝试转换为整数
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Int64(i) => Some(*i),
            Value::Float64(f) => Some(*f as i64),
            Value::Boolean(b) => Some(if *b { 1 } else { 0 }),
            _ => None,
        }
    }

    /// 尝试转换为浮点数
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Float64(f) => Some(*f),
            Value::Int64(i) => Some(*i as f64),
            Value::Boolean(b) => Some(if *b { 1.0 } else { 0.0 }),
            _ => None,
        }
    }

    /// 尝试转换为字符串
    pub fn as_string(&self) -> Option<String> {
        match self {
            Value::String(s) => Some(s.clone()),
            Value::Int64(i) => Some(i.to_string()),
            Value::Float64(f) => Some(f.to_string()),
            Value::Boolean(b) => Some(b.to_string()),
            _ => None,
        }
    }

    /// 尝试获取 DataFrame 引用
    pub fn as_dataframe(&self) -> Option<&DataFrame> {
        match self {
            Value::DataFrame(df) => Some(df.as_ref()),
            _ => None,
        }
    }

    /// 尝试获取 Series 引用
    pub fn as_series(&self) -> Option<&Series> {
        match self {
            Value::Series(s) => Some(s.as_ref()),
            _ => None,
        }
    }

    /// 尝试获取列表
    pub fn as_list(&self) -> Option<&Vec<Value>> {
        match self {
            Value::List(list) => Some(list),
            _ => None,
        }
    }
}

impl ValueType {
    /// 从字符串解析类型（向后兼容旧的字符串类型）
    pub fn from_string(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "null" | "none" => ValueType::Null,
            "boolean" | "bool" => ValueType::Boolean,
            "int" | "integer" | "int64" => ValueType::Int64,
            "float" | "number" | "float64" => ValueType::Float64,
            "string" | "str" | "text" => ValueType::String,
            "date" => ValueType::Date,
            "datetime" | "timestamp" => ValueType::Datetime,
            "duration" | "interval" => ValueType::Duration,
            "list" | "array" => ValueType::List(Box::new(ValueType::Any)),
            "struct" | "object" => ValueType::Struct(vec![]),
            "dataframe" | "df" => ValueType::DataFrame,
            "series" => ValueType::Series,
            "any" | "*" => ValueType::Any,
            _ => ValueType::Any,
        }
    }

    /// 转换为字符串（用于显示）
    pub fn to_string(&self) -> String {
        match self {
            ValueType::Null => "null".to_string(),
            ValueType::Boolean => "boolean".to_string(),
            ValueType::Int64 => "int64".to_string(),
            ValueType::Float64 => "float64".to_string(),
            ValueType::String => "string".to_string(),
            ValueType::Date => "date".to_string(),
            ValueType::Datetime => "datetime".to_string(),
            ValueType::Duration => "duration".to_string(),
            ValueType::List(inner) => format!("list<{}>", inner.to_string()),
            ValueType::Struct(_) => "struct".to_string(),
            ValueType::DataFrame => "dataframe".to_string(),
            ValueType::Series => "series".to_string(),
            ValueType::Any => "any".to_string(),
        }
    }

    /// 转换为 Polars DataType
    pub fn to_polars_dtype(&self) -> DataType {
        match self {
            ValueType::Null => DataType::Null,
            ValueType::Boolean => DataType::Boolean,
            ValueType::Int64 => DataType::Int64,
            ValueType::Float64 => DataType::Float64,
            ValueType::String => DataType::String,
            ValueType::Date => DataType::Date,
            ValueType::Datetime => DataType::Datetime(TimeUnit::Microseconds, None),
            ValueType::Duration => DataType::Duration(TimeUnit::Nanoseconds),
            ValueType::List(inner) => DataType::List(Box::new(inner.to_polars_dtype())),
            ValueType::Struct(fields) => {
                let polars_fields: Vec<Field> = fields
                    .iter()
                    .map(|(name, vtype)| Field::new(name.into(), vtype.to_polars_dtype()))
                    .collect();
                DataType::Struct(polars_fields)
            }
            ValueType::DataFrame | ValueType::Series | ValueType::Any => DataType::Unknown(Default::default()),
        }
    }

    /// 从 Polars DataType 创建
    pub fn from_polars_dtype(dtype: &DataType) -> Self {
        match dtype {
            DataType::Null => ValueType::Null,
            DataType::Boolean => ValueType::Boolean,
            DataType::Int8 | DataType::Int16 | DataType::Int32 | DataType::Int64 => ValueType::Int64,
            DataType::UInt8 | DataType::UInt16 | DataType::UInt32 | DataType::UInt64 => ValueType::Int64,
            DataType::Float32 | DataType::Float64 => ValueType::Float64,
            DataType::String => ValueType::String,
            DataType::Date => ValueType::Date,
            DataType::Datetime(_, _) => ValueType::Datetime,
            DataType::Duration(_) => ValueType::Duration,
            DataType::List(inner) => ValueType::List(Box::new(ValueType::from_polars_dtype(inner))),
            DataType::Struct(fields) => {
                let value_fields = fields
                    .iter()
                    .map(|f| (f.name().to_string(), ValueType::from_polars_dtype(f.dtype())))
                    .collect();
                ValueType::Struct(value_fields)
            }
            _ => ValueType::Any,
        }
    }
}

impl Default for Value {
    fn default() -> Self {
        Value::Null
    }
}

impl Default for ValueType {
    fn default() -> Self {
        ValueType::Any
    }
}

// 实现 Display trait 用于调试
impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Null => write!(f, "null"),
            Value::Boolean(b) => write!(f, "{}", b),
            Value::Int64(i) => write!(f, "{}", i),
            Value::Float64(fl) => write!(f, "{}", fl),
            Value::String(s) => write!(f, "\"{}\"", s),
            Value::Date(d) => write!(f, "Date({})", d),
            Value::Datetime(dt) => write!(f, "Datetime({})", dt),
            Value::Duration(dur) => write!(f, "Duration({})", dur),
            Value::List(items) => {
                write!(f, "[")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", item)?;
                }
                write!(f, "]")
            }
            Value::Struct(fields) => {
                write!(f, "{{")?;
                for (i, (name, value)) in fields.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}: {}", name, value)?;
                }
                write!(f, "}}")
            }
            Value::DataFrame(df) => write!(f, "DataFrame({} rows × {} cols)", df.height(), df.width()),
            Value::Series(s) => write!(f, "Series({} items)", s.len()),
        }
    }
}
