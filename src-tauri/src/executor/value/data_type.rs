//! 数据类型定义
use super::DataValue;

use serde::{Deserialize, Serialize};
use std::fmt;

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

/// 数据类型（支持类型变量）
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DataType {
    /// 具体类型
    Concrete(ValueType),

    /// 类型变量（用于泛型）
    TypeVar(TypeVarId),

    /// 未知类型
    Unknown,
}

impl DataType {
    pub fn concrete(vt: ValueType) -> Self {
        DataType::Concrete(vt)
    }

    pub fn type_var(id: TypeVarId) -> Self {
        DataType::TypeVar(id)
    }

    pub fn unknown() -> Self {
        DataType::Unknown
    }
}

/// 类型变量 ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TypeVarId(pub u32);

impl TypeVarId {
    pub fn new() -> Self {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        TypeVarId(COUNTER.fetch_add(1, Ordering::SeqCst))
    }
}

impl Default for TypeVarId {
    fn default() -> Self {
        Self::new()
    }
}

/// 类型约束
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TypeConstraint {
    /// 数值类型约束
    Numeric,

    /// 可比较约束
    Comparable,

    /// 可迭代约束
    Iterable,

    /// 可序列化约束
    Serializable,

    /// 指定类型集合
    OneOf(Vec<ValueType>),
}

impl TypeConstraint {
    /// 检查类型是否满足约束
    pub fn satisfies(&self, vt: &ValueType) -> bool {
        match self {
            TypeConstraint::Numeric => vt.is_numeric(),
            TypeConstraint::Comparable => vt.is_comparable(),
            TypeConstraint::Iterable => vt.is_iterable(),
            TypeConstraint::Serializable => true, // 所有类型都可序列化
            TypeConstraint::OneOf(types) => types.contains(vt),
        }
    }
}
