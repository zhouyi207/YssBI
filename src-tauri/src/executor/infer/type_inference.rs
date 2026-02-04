//! 类型推断系统
use super::TypeVarId;
use crate::executor::pin::{PinId, PinTypeDesc};
use crate::executor::value::{DataType, ValueType};
use std::collections::HashMap;

/// 类型推断上下文
#[derive(Debug, Default)]
pub struct TypeInferenceContext {
    /// 类型变量到具体类型的映射
    bindings: HashMap<TypeVarId, ValueType>,
    
    /// Pin 到类型描述的映射
    pin_types: HashMap<PinId, PinTypeDesc>,
}

impl TypeInferenceContext {
    pub fn new() -> Self {
        Self::default()
    }
    
    /// 注册 Pin 的类型描述
    pub fn register_pin(&mut self, pin_id: PinId, type_desc: PinTypeDesc) {
        self.pin_types.insert(pin_id, type_desc);
    }
    
    /// 推断连接的类型（从 from_pin 到 to_pin）
    pub fn infer_connection(&mut self, from_pin: PinId, to_pin: PinId) -> Result<(), String> {
        let from_type = self.get_pin_type(from_pin)?.clone();
        let to_type = self.get_pin_type(to_pin)?.clone();
        
        self.unify(&from_type, &to_type)
    }
    
    /// 解析 Pin 的最终类型
    pub fn resolve_pin_type(&self, pin_id: PinId) -> Result<ValueType, String> {
        let pin_type = self.get_pin_type(pin_id)?;
        
        match &pin_type.data_type {
            DataType::Concrete(vt) => Ok(vt.clone()),
            DataType::TypeVar(var_id) => {
                self.bindings
                    .get(var_id)
                    .cloned()
                    .ok_or_else(|| format!("Type variable {:?} not bound", var_id))
            }
            DataType::Unknown => Err("Type still unknown".to_string()),
        }
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
            (DataType::Concrete(vt_a), DataType::Concrete(vt_b)) => {
                self.is_value_type_compatible(vt_a, vt_b)
            }
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
                if self.is_value_type_compatible(vt_a, vt_b) {
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
            // Unknown 类型可以接受任何类型，但不会改变自己
            (DataType::Unknown, _) | (_, DataType::Unknown) => Ok(()),
        }
    }
    
    /// 检查值类型兼容性
    fn is_value_type_compatible(&self, from: &ValueType, to: &ValueType) -> bool {
        // 完全相同
        if from == to {
            return true;
        }
        
        // Any 类型兼容所有类型
        if matches!(to, ValueType::Any) {
            return true;
        }
        
        // 数字类型之间可以转换
        if matches!(from, ValueType::Int64 | ValueType::Float64)
            && matches!(to, ValueType::Int64 | ValueType::Float64)
        {
            return true;
        }
        
        false
    }
    
    /// 获取 Pin 的类型描述
    fn get_pin_type(&self, pin_id: PinId) -> Result<&PinTypeDesc, String> {
        self.pin_types
            .get(&pin_id)
            .ok_or_else(|| format!("Pin {:?} not registered in type inference context", pin_id))
    }

    /// 清除所有绑定
    pub fn clear(&mut self) {
        self.bindings.clear();
        self.pin_types.clear();
    }
}
