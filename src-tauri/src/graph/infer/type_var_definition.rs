use super::{TypeConstraint, TypeVarId};
use crate::graph::value::DataType;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TypeVarDefinition {
    pub id: TypeVarId,
    pub constraints: Vec<TypeConstraint>,
    pub bound: Option<DataType>,
}

impl TypeVarDefinition {
    /// 检查类型是否满足所有约束
    pub fn satisfies_constraints(&self, vt: &DataType) -> bool {
        self.constraints.iter().all(|c| c.satisfies(vt))
    }
}
