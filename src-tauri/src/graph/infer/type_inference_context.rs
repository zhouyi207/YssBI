//! 类型推断系统

use crate::graph::infer::TypeVarInference;
use crate::graph::pin::PinDataTypeInference;
use crate::graph::pin::PinId;
use crate::graph::value::DataType;
use crate::graph::PinInstance;
use crate::graph::TypeVarId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 类型推断上下文（一次推断过程）
#[derive(Debug, Deserialize, Serialize)]
pub struct TypeInferenceContext {
    /// 类型变量定义（来自 Graph / NodeDefinition）
    pub type_vars: HashMap<TypeVarId, TypeVarInference>,

    /// Pin 到类型描述的映射
    pub pin_types: HashMap<PinId, PinDataTypeInference>,

    /// 推断过程中的临时绑定（root → concrete）
    pub bindings: HashMap<TypeVarId, DataType>,

    /// TypeVar 等价类（Union-Find parent）
    pub var_alias: HashMap<TypeVarId, TypeVarId>,
}

impl TypeInferenceContext {
    pub fn new() -> Self {
        Self {
            type_vars: HashMap::new(),
            pin_types: HashMap::new(),
            bindings: HashMap::new(),
            var_alias: HashMap::new(),
        }
    }

    pub fn clear(&mut self) {
        self.type_vars.clear();
        self.bindings.clear();
        self.pin_types.clear();
        self.var_alias.clear();
    }

    fn find_root(&mut self, var: TypeVarId) -> TypeVarId {
        match self.var_alias.get(&var).copied() {
            None => var, // 没有 parent，自己就是 root
            Some(parent) => {
                let root = self.find_root(parent);
                if root != parent {
                    self.var_alias.insert(var, root); // 路径压缩
                }
                root
            }
        }
    }

    fn union(&mut self, a: TypeVarId, b: TypeVarId) {
        let ra = self.find_root(a);
        let rb = self.find_root(b);

        if ra != rb {
            // 简单策略：ra 指向 rb
            self.var_alias.insert(ra, rb);
        }
    }

    /// 注册类型变量定义
    pub fn register_type_var(&mut self, type_var: TypeVarInference) {
        self.type_vars.insert(type_var.id, type_var);
    }

    /// 注册 Pin 的类型描述
    pub fn register_pin_type(&mut self, pin_instance: PinInstance) {
        if let Some(data_type) = pin_instance.definition.data_type {
            if let Some(type_var_id) = pin_instance.type_var_id {
                // 使用类型变量的 pin
                let data_type_inference = data_type.to_inference(type_var_id);
                self.pin_types.insert(pin_instance.id, data_type_inference);
            } else {
                // 具体类型的 pin，直接使用具体类型（不需要类型变量）
                // 这种情况下，pin 的类型已经确定，不需要推断
                match data_type {
                    crate::graph::pin::PinDataTypeDefinition::Concrete(concrete_type) => {
                        // 为具体类型创建一个临时的 TypeVarId 和 TypeVarInference
                        let temp_type_var_id = TypeVarId::new();
                        let type_var_inference = TypeVarInference {
                            id: temp_type_var_id,
                            constraints: vec![],
                            bound: Some(concrete_type.clone()),
                        };
                        self.type_vars.insert(temp_type_var_id, type_var_inference);
                        
                        let data_type_inference = PinDataTypeInference::Concrete(concrete_type);
                        self.pin_types.insert(pin_instance.id, data_type_inference);
                    }
                    crate::graph::pin::PinDataTypeDefinition::TypeVar(_) => {
                        // 这种情况不应该发生：TypeVar 类型但没有 type_var_id
                        // 跳过这个 pin
                    }
                    crate::graph::pin::PinDataTypeDefinition::Unknown => {
                        // Unknown 类型，跳过
                    }
                }
            }
        }
    }

    /// 推断一条连接
    pub fn infer_connection(&mut self, from: PinId, to: PinId) -> Result<(), String> {
        let a = self.get_pin_type(from)?.clone();
        let b = self.get_pin_type(to)?.clone();
        self.unify(&a, &b)?;

        Ok(())
    }

    /// 推断完成后提交结果（写回 TypeVarInference.bound）
    pub fn commit(&mut self) -> Result<(), String> {
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
    pub fn resolve_pin_type(&self, pin_id: PinId) -> Result<DataType, String> {
        let pin = self.get_pin_type(pin_id)?;

        match &pin {
            PinDataTypeInference::Concrete(vt) => Ok(vt.clone()),
            PinDataTypeInference::TypeVar(var) => {
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
            PinDataTypeInference::Unknown => Err("Type is still unknown".into()),
        }
    }

    /// 类型统一（核心推断逻辑）
    fn unify(&mut self, a: &PinDataTypeInference, b: &PinDataTypeInference) -> Result<(), String> {
        let ta = self.resolve_data_type(a);
        let tb = self.resolve_data_type(b);

        match (ta, tb) {
            // concrete ↔ concrete
            (PinDataTypeInference::Concrete(a), PinDataTypeInference::Concrete(b)) => {
                if self.is_value_type_compatible(&a, &b) {
                    Ok(())
                } else {
                    Err(format!("Type mismatch: {:?} vs {:?}", a, b))
                }
            }

            // typevar ↔ concrete
            (PinDataTypeInference::TypeVar(var), PinDataTypeInference::Concrete(vt))
            | (PinDataTypeInference::Concrete(vt), PinDataTypeInference::TypeVar(var)) => {
                let root = self.find_root(var);

                let def = self
                    .type_vars
                    .get(&root)
                    .ok_or_else(|| format!("TypeVar {:?} not defined", root))?;

                if !def.satisfies_constraints(&vt) {
                    return Err(format!(
                        "Type {:?} does not satisfy constraints of {:?}",
                        vt, root
                    ));
                }

                self.bind(root, vt)
            }

            // TypeVar ↔ TypeVar
            (PinDataTypeInference::TypeVar(a), PinDataTypeInference::TypeVar(b)) => {
                let ra = self.find_root(a);
                let rb = self.find_root(b);

                if ra == rb {
                    return Ok(());
                }

                // 如果其中一个已经绑定了 concrete
                match (
                    self.bindings.get(&ra).cloned(),
                    self.bindings.get(&rb).cloned(),
                ) {
                    (Some(ta), Some(tb)) => {
                        // 两边都有 concrete → 必须能 unify
                        if self.is_value_type_compatible(&ta, &tb) {
                            self.union(ra, rb);
                            let root = self.find_root(ra);
                            self.bind(root, ta)?;
                            Ok(())
                        } else {
                            Err(format!("Type mismatch: {:?} vs {:?}", ta, tb))
                        }
                    }
                    (Some(t), None) => {
                        self.union(ra, rb);
                        let root = self.find_root(ra);
                        self.bind(root, t)
                    }
                    (None, Some(t)) => {
                        self.union(rb, ra);
                        let root = self.find_root(ra);
                        self.bind(root, t)
                    }
                    (None, None) => {
                        self.union(ra, rb);
                        Ok(())
                    }
                }
            }

            (PinDataTypeInference::Unknown, _) | (_, PinDataTypeInference::Unknown) => Ok(()),
        }
    }

    /// 绑定类型变量（仅作用于推断上下文）
    fn bind(&mut self, var: TypeVarId, vt: DataType) -> Result<(), String> {
        let root = self.find_root(var);

        if let Some(existing) = self.bindings.get(&root) {
            if existing != &vt {
                return Err(format!(
                    "TypeVar {:?} already bound to {:?}, cannot rebind to {:?}",
                    root, existing, vt
                ));
            }
        }

        self.bindings.insert(root, vt);
        Ok(())
    }
    /// 解析 DataType（优先使用临时绑定）
    fn resolve_data_type(&mut self, dt: &PinDataTypeInference) -> PinDataTypeInference {
        match dt {
            PinDataTypeInference::TypeVar(var) => {
                let root = self.find_root(*var);

                if let Some(vt) = self.bindings.get(&root) {
                    PinDataTypeInference::Concrete(vt.clone())
                } else if let Some(def) = self.type_vars.get(&root) {
                    def.bound
                        .clone()
                        .map(PinDataTypeInference::Concrete)
                        .unwrap_or_else(|| dt.clone())
                } else {
                    dt.clone()
                }
            }
            _ => dt.clone(),
        }
    }

    /// 值类型兼容性
    fn is_value_type_compatible(&self, from: &DataType, to: &DataType) -> bool {
        from == to
            || matches!(to, DataType::Any)
            || matches!(
                (from, to),
                (DataType::Int64, DataType::Float64) | (DataType::Float64, DataType::Int64)
            )
    }

    fn get_pin_type(&self, pin_id: PinId) -> Result<&PinDataTypeInference, String> {
        self.pin_types
            .get(&pin_id)
            .ok_or_else(|| format!("Pin {:?} not registered", pin_id))
    }

    /// 获取类型变量的绑定类型
    ///
    /// 返回 None 表示类型变量未绑定
    pub fn get_bound_type(&self, type_var_id: TypeVarId) -> Option<DataType> {
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
