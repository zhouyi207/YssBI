//! 类型推断系统

use super::TypeVarId;
use crate::graph::infer::TypeVarDefinition;
use crate::graph::pin::PinDataType;
use crate::graph::pin::PinId;
use crate::graph::value::DataType;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 类型推断上下文（一次推断过程）
#[derive(Debug, Deserialize, Serialize)]
pub struct TypeInferenceContext {
    /// 类型变量定义（来自 Graph / NodeDefinition）
    type_vars: HashMap<TypeVarId, TypeVarDefinition>,

    /// Pin 到类型描述的映射
    pin_types: HashMap<PinId, PinDataType>,

    /// 推断过程中的临时绑定（root → concrete）
    bindings: HashMap<TypeVarId, DataType>,

    /// TypeVar 等价类（Union-Find parent）
    var_alias: HashMap<TypeVarId, TypeVarId>,
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
    pub fn register_type_var(&mut self, type_var: TypeVarDefinition) {
        self.type_vars.insert(type_var.id, type_var);
    }

    /// 注册 Pin 的类型描述
    pub fn register_pin_type(&mut self, pin_id: PinId, data_type: PinDataType) {
        self.pin_types.insert(pin_id, data_type);
    }

    /// 推断一条连接
    pub fn infer_connection(&mut self, from: PinId, to: PinId) -> Result<(), String> {
        let a = self.get_pin_type(from)?.clone();
        let b = self.get_pin_type(to)?.clone();
        self.unify(&a, &b)?;

        Ok(())
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
    pub fn resolve_pin_type(&self, pin_id: PinId) -> Result<DataType, String> {
        let pin = self.get_pin_type(pin_id)?;

        match &pin {
            PinDataType::Concrete(vt) => Ok(vt.clone()),
            PinDataType::TypeVar(var) => {
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
            PinDataType::Unknown => Err("Type is still unknown".into()),
        }
    }

    /// 类型统一（核心推断逻辑）
    fn unify(&mut self, a: &PinDataType, b: &PinDataType) -> Result<(), String> {
        let ta = self.resolve_data_type(a);
        let tb = self.resolve_data_type(b);

        match (ta, tb) {
            // concrete ↔ concrete
            (PinDataType::Concrete(a), PinDataType::Concrete(b)) => {
                if self.is_value_type_compatible(&a, &b) {
                    Ok(())
                } else {
                    Err(format!("Type mismatch: {:?} vs {:?}", a, b))
                }
            }

            // typevar ↔ concrete
            (PinDataType::TypeVar(var), PinDataType::Concrete(vt))
            | (PinDataType::Concrete(vt), PinDataType::TypeVar(var)) => {
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
            (PinDataType::TypeVar(a), PinDataType::TypeVar(b)) => {
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

            (PinDataType::Unknown, _) | (_, PinDataType::Unknown) => Ok(()),
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
    fn resolve_data_type(&mut self, dt: &PinDataType) -> PinDataType {
        match dt {
            PinDataType::TypeVar(var) => {
                let root = self.find_root(*var);

                if let Some(vt) = self.bindings.get(&root) {
                    PinDataType::Concrete(vt.clone())
                } else if let Some(def) = self.type_vars.get(&root) {
                    def.bound
                        .clone()
                        .map(PinDataType::Concrete)
                        .unwrap_or_else(|| PinDataType::TypeVar(root))
                } else {
                    PinDataType::TypeVar(root)
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

    fn get_pin_type(&self, pin_id: PinId) -> Result<&PinDataType, String> {
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
