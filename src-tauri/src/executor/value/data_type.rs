//! 数据类型定义
use super::ValueType;
use crate::executor::infer::TypeVarId;
use serde::{Deserialize, Serialize};

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
