//! 数据类型定义
use crate::graph::infer::TypeVarId;
use crate::graph::value::DataType;
use serde::{Deserialize, Serialize};

/// 数据类型（支持类型变量）
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PinDataType {
    /// 具体类型
    Concrete(DataType),

    /// 类型变量（用于泛型）
    TypeVar(TypeVarId),

    /// 未知类型
    Unknown,
}

impl PinDataType {
    pub fn concrete(vt: DataType) -> Self {
        PinDataType::Concrete(vt)
    }

    pub fn type_var(id: TypeVarId) -> Self {
        PinDataType::TypeVar(id)
    }

    pub fn unknown() -> Self {
        PinDataType::Unknown
    }
}
