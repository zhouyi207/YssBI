//! Pin 类型描述

use crate::executor::infer::TypeVarId;
use crate::executor::value::{DataType, DataValue, ValueType};
use serde::{Deserialize, Serialize};

/// Pin 类型描述
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PinTypeDesc {
    /// 数据类型
    pub data_type: DataType,

    /// 是否可选
    pub is_optional: bool,

    /// 是否数组
    pub is_array: bool,
}

impl PinTypeDesc {
    pub fn default_value(&self) -> Option<DataValue> {
        if self.is_optional {
            return None;
        }

        match &self.data_type {
            DataType::Concrete(vt) => vt.default_value(),
            DataType::TypeVar(_) => None,
            DataType::Unknown => None,
        }
    }

    /// 创建具体类型的 Pin
    pub fn concrete(vt: ValueType) -> Self {
        Self {
            data_type: DataType::Concrete(vt),
            is_optional: false,
            is_array: false,
        }
    }

    /// 创建类型变量 Pin
    pub fn type_var(id: TypeVarId) -> Self {
        Self {
            data_type: DataType::TypeVar(id),
            is_optional: false,
            is_array: false,
        }
    }

    /// 创建未知类型 Pin
    pub fn unknown() -> Self {
        Self {
            data_type: DataType::Unknown,
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
}
