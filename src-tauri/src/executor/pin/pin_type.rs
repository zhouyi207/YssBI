//! Pin 类型描述

use super::TypeConstraint;
use super::TypeVarId;
use crate::executor::value::{DataType, ValueType};
use serde::{Deserialize, Serialize};

/// Pin 类型描述
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PinTypeDesc {
    /// 数据类型
    pub data_type: DataType,
    
    /// 类型约束
    pub constraints: Vec<TypeConstraint>,
    
    /// 是否可选
    pub is_optional: bool,
    
    /// 是否数组
    pub is_array: bool,
}

impl PinTypeDesc {
    /// 创建具体类型的 Pin
    pub fn concrete(vt: ValueType) -> Self {
        Self {
            data_type: DataType::Concrete(vt),
            constraints: vec![],
            is_optional: false,
            is_array: false,
        }
    }

    /// 创建类型变量 Pin
    pub fn type_var(id: TypeVarId) -> Self {
        Self {
            data_type: DataType::TypeVar(id),
            constraints: vec![],
            is_optional: false,
            is_array: false,
        }
    }

    /// 创建带约束的类型变量 Pin
    pub fn type_var_with_constraints(id: TypeVarId, constraints: Vec<TypeConstraint>) -> Self {
        Self {
            data_type: DataType::TypeVar(id),
            constraints,
            is_optional: false,
            is_array: false,
        }
    }

    /// 创建未知类型 Pin
    pub fn unknown() -> Self {
        Self {
            data_type: DataType::Unknown,
            constraints: vec![],
            is_optional: false,
            is_array: false,
        }
    }

    /// 设置为可选
    pub fn optional(mut self) -> Self {
        self.is_optional = true;
        self
    }

    /// 设置为数组
    pub fn array(mut self) -> Self {
        self.is_array = true;
        self
    }

    /// 检查类型是否满足所有约束
    pub fn satisfies_constraints(&self, vt: &ValueType) -> bool {
        self.constraints.iter().all(|c| c.satisfies(vt))
    }
}
