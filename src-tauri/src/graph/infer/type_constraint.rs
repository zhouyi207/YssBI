//! pin type constraints

use crate::graph::value::DataType;
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
    OneOf(Vec<DataType>),

    /// 所有子约束均需满足
    And(Vec<TypeConstraint>),

    /// 任一子约束满足即可
    Or(Vec<TypeConstraint>),
}

impl TypeConstraint {
    /// 检查类型是否满足约束
    pub fn satisfies(&self, vt: &DataType) -> bool {
        match self {
            TypeConstraint::Numeric => vt.is_numeric(),
            TypeConstraint::Comparable => vt.is_comparable(),
            TypeConstraint::Iterable => vt.is_iterable(),
            TypeConstraint::Serializable => true, // 所有类型都可序列化
            TypeConstraint::OneOf(types) => {
                types.contains(vt)
                    || vt.series_inner()
                        .map_or(false, |inner| types.contains(inner))
            }
            TypeConstraint::And(constraints) => constraints.iter().all(|c| c.satisfies(vt)),
            TypeConstraint::Or(constraints) => constraints.iter().any(|c| c.satisfies(vt)),
        }
    }
}
