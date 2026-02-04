//! pin type constraints

use crate::executor::value::ValueType;
use serde::{Deserialize, Serialize};

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
