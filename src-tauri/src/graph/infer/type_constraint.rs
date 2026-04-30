//! pin type constraints

use super::TypeVarKey;
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

    /// 本 TypeVar 的类型可以通过 Convert 转换为另一 TypeVar 的类型
    /// 即：can_convert(self_type, other_bound_type)
    ConvertibleTo(TypeVarKey),

    /// 本 TypeVar 的类型可以作为另一 TypeVar 类型的转换结果
    /// 即：can_convert(other_bound_type, self_type)
    ConvertibleFrom(TypeVarKey),
}

impl TypeConstraint {
    /// 检查类型是否满足约束（无上下文，ConvertibleTo/From 一律通过）
    pub fn satisfies(&self, vt: &DataType) -> bool {
        match self {
            TypeConstraint::Numeric => vt.is_numeric(),
            TypeConstraint::Comparable => vt.is_comparable(),
            TypeConstraint::Iterable => vt.is_iterable(),
            TypeConstraint::Serializable => true,
            TypeConstraint::OneOf(types) => {
                types.contains(vt)
                    || vt
                        .series_inner()
                        .map_or(false, |inner| types.contains(inner))
            }
            TypeConstraint::And(constraints) => constraints.iter().all(|c| c.satisfies(vt)),
            TypeConstraint::Or(constraints) => constraints.iter().any(|c| c.satisfies(vt)),
            TypeConstraint::ConvertibleTo(_) | TypeConstraint::ConvertibleFrom(_) => true,
        }
    }

    /// 带上下文的约束检查：resolver 根据 TypeVarKey 返回关联 TypeVar 的当前绑定类型
    pub fn satisfies_with_resolver<F>(&self, vt: &DataType, resolver: &F) -> bool
    where
        F: Fn(&TypeVarKey) -> Option<DataType>,
    {
        match self {
            TypeConstraint::ConvertibleTo(key) => match resolver(key) {
                Some(target) => DataType::can_convert(vt, &target),
                None => true,
            },
            TypeConstraint::ConvertibleFrom(key) => match resolver(key) {
                Some(source) => DataType::can_convert(&source, vt),
                None => true,
            },
            TypeConstraint::And(cs) => cs.iter().all(|c| c.satisfies_with_resolver(vt, resolver)),
            TypeConstraint::Or(cs) => cs.iter().any(|c| c.satisfies_with_resolver(vt, resolver)),
            other => other.satisfies(vt),
        }
    }
}
