//! 数据类型定义
use crate::graph::value::DataType;
use crate::graph::{TypeVarId, TypeVarKey};
use serde::{Deserialize, Serialize};

/// 数据类型（支持类型变量）
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PinDataTypeDefinition {
    /// 具体类型
    Concrete(DataType),

    /// 类型变量（用于泛型）
    TypeVar(TypeVarKey),

    /// 未知类型
    Unknown,
}

impl PinDataTypeDefinition {
    pub fn concrete(vt: DataType) -> Self {
        PinDataTypeDefinition::Concrete(vt)
    }

    pub fn type_var(id: TypeVarKey) -> Self {
        PinDataTypeDefinition::TypeVar(id)
    }

    pub fn unknown() -> Self {
        PinDataTypeDefinition::Unknown
    }

    pub fn to_inference(&self, type_var_id: TypeVarId) -> PinDataTypeInference {
        match self {
            PinDataTypeDefinition::Concrete(vt) => PinDataTypeInference::concrete(vt.clone()),
            PinDataTypeDefinition::TypeVar(_key) => PinDataTypeInference::type_var(type_var_id),
            PinDataTypeDefinition::Unknown => PinDataTypeInference::unknown(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PinDataTypeInference {
    /// 具体类型
    Concrete(DataType),

    /// 类型变量（用于泛型）
    TypeVar(TypeVarId),

    /// 未知类型
    Unknown,
}

impl PinDataTypeInference {
    pub fn concrete(vt: DataType) -> Self {
        PinDataTypeInference::Concrete(vt)
    }

    pub fn type_var(id: TypeVarId) -> Self {
        PinDataTypeInference::TypeVar(id)
    }

    pub fn unknown() -> Self {
        PinDataTypeInference::Unknown
    }
}
