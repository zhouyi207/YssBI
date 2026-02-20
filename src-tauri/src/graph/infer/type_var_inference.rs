use super::{TypeConstraint, TypeVarId};
use crate::graph::value::DataType;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct TypeVarInference {
    pub id: TypeVarId,
    pub constraints: Vec<TypeConstraint>,
    pub bound: Option<DataType>,
}

impl TypeVarInference {
    /// 检查类型是否满足所有约束
    pub fn satisfies_constraints(&self, vt: &DataType) -> bool {
        self.constraints.iter().all(|c| c.satisfies(vt))
    }

    /// 检查类型是否满足约束；当 TypeVar 期望标量时，DataSeries(inner) 可解包为 inner 检查。
    /// 与类型推断中的 unify 逻辑一致，供连接校验与推断共用。
    pub fn satisfies_constraints_with_unwrap(&self, vt: &DataType) -> bool {
        if self.satisfies_constraints(vt) {
            return true;
        }
        if let DataType::DataSeries(inner) = vt {
            return self.satisfies_constraints(inner);
        }
        false
    }
}
