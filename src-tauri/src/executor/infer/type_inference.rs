//! 类型推断系统

use super::TypeVarId;
use crate::executor::infer::TypeVarDefinition;
use crate::executor::pin::{PinId, PinTypeDesc};
use crate::executor::value::{DataType, ValueType};
use std::collections::HashMap;

/// 类型推断上下文（一次推断过程）
#[derive(Debug)]
pub struct TypeInferenceContext {
    /// 类型变量定义（来自 Graph / NodeDefinition）
    type_vars: HashMap<TypeVarId, TypeVarDefinition>,

    /// 推断过程中的临时绑定
    bindings: HashMap<TypeVarId, ValueType>,

    /// Pin 到类型描述的映射
    pin_types: HashMap<PinId, PinTypeDesc>,
}

impl TypeInferenceContext {
    pub fn new() -> Self {
        Self {
            type_vars: HashMap::new(),
            bindings: HashMap::new(),
            pin_types: HashMap::new(),
        }
    }

    pub fn clear(&mut self) {
        self.type_vars.clear();
        self.bindings.clear();
        self.pin_types.clear();
    }

    /// 注册类型变量定义
    pub fn register_type_var(&mut self, type_var: TypeVarDefinition) {
        self.type_vars.insert(type_var.id, type_var);
    }

    /// 注册 Pin 的类型描述
    pub fn register_pin(&mut self, pin_id: PinId, type_desc: PinTypeDesc) {
        self.pin_types.insert(pin_id, type_desc);
    }

    /// 推断一条连接
    pub fn infer_connection(&mut self, from: PinId, to: PinId) -> Result<(), String> {
        let a = self.get_pin_type(from)?.clone();
        let b = self.get_pin_type(to)?.clone();
        self.unify(&a, &b)
    }

    /// 推断完成后提交结果（写回 TypeVarDefinition.bound）
    pub fn commit(mut self) -> Result<(), String> {
        for (var_id, value_type) in self.bindings.drain() {
            let def = self
                .type_vars
                .get_mut(&var_id)
                .ok_or_else(|| format!("TypeVar {:?} not found", var_id))?;

            // 已有绑定 → 必须一致
            if let Some(existing) = &def.bound {
                if existing != &value_type {
                    return Err(format!(
                        "TypeVar {:?} already bound to {:?}, cannot rebind to {:?}",
                        var_id, existing, value_type
                    ));
                }
            }

            def.bound = Some(value_type);
        }
        Ok(())
    }

    /// 解析 Pin 的最终类型（供 NodeProcessor 使用）
    pub fn resolve_pin_type(&self, pin_id: PinId) -> Result<ValueType, String> {
        let pin = self.get_pin_type(pin_id)?;

        match &pin.data_type {
            DataType::Concrete(vt) => Ok(vt.clone()),
            DataType::TypeVar(var) => {
                if let Some(vt) = self.bindings.get(var) {
                    Ok(vt.clone())
                } else if let Some(def) = self.type_vars.get(var) {
                    def.bound
                        .clone()
                        .ok_or_else(|| format!("TypeVar {:?} not bound", var))
                } else {
                    Err(format!("Unknown TypeVar {:?}", var))
                }
            }
            DataType::Unknown => Err("Type is still unknown".into()),
        }
    }

    /// 类型统一（核心推断逻辑）
    fn unify(&mut self, a: &PinTypeDesc, b: &PinTypeDesc) -> Result<(), String> {
        let ta = self.resolve_data_type(&a.data_type);
        let tb = self.resolve_data_type(&b.data_type);

        match (ta, tb) {
            (DataType::Concrete(a), DataType::Concrete(b)) => {
                if self.is_value_type_compatible(&a, &b) {
                    Ok(())
                } else {
                    Err(format!("Type mismatch: {:?} vs {:?}", a, b))
                }
            }

            (DataType::TypeVar(var), DataType::Concrete(vt))
            | (DataType::Concrete(vt), DataType::TypeVar(var)) => {
                let def = self
                    .type_vars
                    .get(&var)
                    .ok_or_else(|| format!("TypeVar {:?} not defined", var))?;

                if !def.satisfies_constraints(&vt) {
                    return Err(format!(
                        "Type {:?} does not satisfy constraints of {:?}",
                        vt, var
                    ));
                }

                self.bind(var, vt)
            }

            (DataType::TypeVar(a), DataType::TypeVar(b)) => {
                if a == b {
                    Ok(())
                } else {
                    // 可选：union / 延迟绑定
                    Ok(())
                }
            }

            (DataType::Unknown, _) | (_, DataType::Unknown) => Ok(()),
        }
    }

    /// 绑定类型变量（仅作用于推断上下文）
    fn bind(&mut self, var: TypeVarId, vt: ValueType) -> Result<(), String> {
        if let Some(existing) = self.bindings.get(&var) {
            if existing != &vt {
                return Err(format!(
                    "TypeVar {:?} already bound to {:?}, cannot rebind to {:?}",
                    var, existing, vt
                ));
            }
        }
        self.bindings.insert(var, vt);
        Ok(())
    }

    /// 解析 DataType（优先使用临时绑定）
    fn resolve_data_type(&self, dt: &DataType) -> DataType {
        match dt {
            DataType::TypeVar(var) => {
                if let Some(vt) = self.bindings.get(var) {
                    DataType::Concrete(vt.clone())
                } else if let Some(def) = self.type_vars.get(var) {
                    def.bound
                        .clone()
                        .map(DataType::Concrete)
                        .unwrap_or_else(|| dt.clone())
                } else {
                    dt.clone()
                }
            }
            _ => dt.clone(),
        }
    }

    /// 值类型兼容性
    fn is_value_type_compatible(&self, from: &ValueType, to: &ValueType) -> bool {
        from == to
            || matches!(to, ValueType::Any)
            || matches!(
                (from, to),
                (ValueType::Int64, ValueType::Float64) | (ValueType::Float64, ValueType::Int64)
            )
    }

    fn get_pin_type(&self, pin_id: PinId) -> Result<&PinTypeDesc, String> {
        self.pin_types
            .get(&pin_id)
            .ok_or_else(|| format!("Pin {:?} not registered", pin_id))
    }

    /// 获取类型变量的绑定类型
    /// 
    /// 返回 None 表示类型变量未绑定
    pub fn get_bound_type(&self, type_var_id: TypeVarId) -> Option<ValueType> {
        // 优先从临时绑定中获取
        if let Some(vt) = self.bindings.get(&type_var_id) {
            return Some(vt.clone());
        }
        
        // 然后从类型变量定义中获取
        if let Some(def) = self.type_vars.get(&type_var_id) {
            return def.bound.clone();
        }
        
        None
    }
}
