//! 类型推断系统
use super::{DataType, PinTypeDesc, TypeVarId, ValueType};
use std::collections::HashMap;

/// 类型推断上下文
#[derive(Debug, Default)]
pub struct TypeInferenceContext {
    /// 类型变量到具体类型的映射
    bindings: HashMap<TypeVarId, ValueType>,
}

impl TypeInferenceContext {
    pub fn new() -> Self {
        Self::default()
    }

    /// 绑定类型变量到具体类型
    pub fn bind(&mut self, var: TypeVarId, vt: ValueType) -> Result<(), String> {
        if let Some(existing) = self.bindings.get(&var) {
            if existing != &vt {
                return Err(format!(
                    "Type variable {:?} already bound to {:?}, cannot rebind to {:?}",
                    var, existing, vt
                ));
            }
        }
        self.bindings.insert(var, vt);
        Ok(())
    }

    /// 解析类型（将类型变量替换为具体类型）
    pub fn resolve(&self, dt: &DataType) -> DataType {
        match dt {
            DataType::TypeVar(var) => {
                if let Some(vt) = self.bindings.get(var) {
                    DataType::Concrete(vt.clone())
                } else {
                    dt.clone()
                }
            }
            _ => dt.clone(),
        }
    }

    /// 检查两个类型是否兼容
    pub fn are_compatible(&self, a: &PinTypeDesc, b: &PinTypeDesc) -> bool {
        let resolved_a = self.resolve(&a.data_type);
        let resolved_b = self.resolve(&b.data_type);

        match (&resolved_a, &resolved_b) {
            (DataType::Concrete(vt_a), DataType::Concrete(vt_b)) => vt_a == vt_b,
            (DataType::Unknown, _) | (_, DataType::Unknown) => true,
            (DataType::TypeVar(_), _) | (_, DataType::TypeVar(_)) => true,
        }
    }

    /// 统一两个类型（尝试推断类型变量）
    pub fn unify(&mut self, a: &PinTypeDesc, b: &PinTypeDesc) -> Result<(), String> {
        let resolved_a = self.resolve(&a.data_type);
        let resolved_b = self.resolve(&b.data_type);

        match (&resolved_a, &resolved_b) {
            (DataType::Concrete(vt_a), DataType::Concrete(vt_b)) => {
                if vt_a == vt_b {
                    Ok(())
                } else {
                    Err(format!("Type mismatch: {:?} vs {:?}", vt_a, vt_b))
                }
            }
            (DataType::TypeVar(var), DataType::Concrete(vt))
            | (DataType::Concrete(vt), DataType::TypeVar(var)) => {
                // 检查约束
                let constraints = if matches!(a.data_type, DataType::TypeVar(_)) {
                    &a.constraints
                } else {
                    &b.constraints
                };

                if constraints.iter().all(|c| c.satisfies(vt)) {
                    self.bind(*var, vt.clone())
                } else {
                    Err(format!("Type {:?} does not satisfy constraints", vt))
                }
            }
            (DataType::TypeVar(var_a), DataType::TypeVar(var_b)) => {
                // 两个类型变量，暂时不绑定
                if var_a == var_b {
                    Ok(())
                } else {
                    // 可以选择将一个绑定到另一个，这里暂时允许
                    Ok(())
                }
            }
            (DataType::Unknown, _) | (_, DataType::Unknown) => Ok(()),
        }
    }

    /// 清除所有绑定
    pub fn clear(&mut self) {
        self.bindings.clear();
    }
}
