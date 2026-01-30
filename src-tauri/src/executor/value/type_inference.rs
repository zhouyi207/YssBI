//! 类型推断引擎
//!
//! 提供类型推断的核心逻辑，支持类型变量绑定、类型统一和约束检查

use super::pin_type::PinTypeDesc;
use super::type_constraint::TypeConstraint;
use super::type_desc::DataType;
use super::type_var::TypeVarId;
use super::types::ValueType;
use crate::executor::pin::PinId;
use std::collections::HashMap;

/// 类型推断上下文
/// 
/// 管理类型变量的绑定和 Pin 的类型信息
/// 
/// # 示例
/// 
/// ```rust
/// let mut ctx = TypeInferenceContext::new();
/// 
/// // 注册 Pin
/// ctx.register_pin(print_value_pin, PinTypeDesc::unknown());
/// ctx.register_pin(divide_result_pin, PinTypeDesc::concrete(ValueType::Float64));
/// 
/// // 推断连接
/// ctx.infer_connection(divide_result_pin, print_value_pin)?;
/// 
/// // 解析类型
/// let resolved_type = ctx.resolve_pin_type(print_value_pin)?;
/// assert_eq!(resolved_type, ValueType::Float64);
/// ```
#[derive(Debug)]
pub struct TypeInferenceContext {
    /// 类型变量的绑定结果
    /// 
    /// Key: TypeVarId, Value: 绑定的具体类型
    type_var_bindings: HashMap<TypeVarId, ValueType>,
    
    /// Pin 的类型描述
    /// 
    /// Key: PinId, Value: Pin 的类型信息
    pin_types: HashMap<PinId, PinTypeDesc>,
    
    /// 类型变量的等价类（用于统一）
    /// 
    /// Key: TypeVarId, Value: 代表元素的 TypeVarId
    /// 使用 Union-Find 算法实现类型变量的统一
    type_var_union: HashMap<TypeVarId, TypeVarId>,
}

impl TypeInferenceContext {
    /// 创建新的类型推断上下文
    pub fn new() -> Self {
        Self {
            type_var_bindings: HashMap::new(),
            pin_types: HashMap::new(),
            type_var_union: HashMap::new(),
        }
    }
    
    /// 注册 Pin 的类型描述
    /// 
    /// # 参数
    /// 
    /// * `pin_id` - Pin 的唯一标识
    /// * `type_desc` - Pin 的类型描述
    pub fn register_pin(&mut self, pin_id: PinId, type_desc: PinTypeDesc) {
        self.pin_types.insert(pin_id, type_desc);
    }
    
    /// 推断连接的类型
    /// 
    /// 根据连接的两端 Pin 的类型，进行类型推断和统一
    /// 
    /// # 参数
    /// 
    /// * `from_pin` - 源 Pin（输出）
    /// * `to_pin` - 目标 Pin（输入）
    /// 
    /// # 返回
    /// 
    /// * `Ok(())` - 推断成功
    /// * `Err(String)` - 推断失败，返回错误信息
    /// 
    /// # 示例
    /// 
    /// ```rust
    /// // Divide.Result (Float64) -> Print.Value (Unknown)
    /// ctx.infer_connection(divide_result_pin, print_value_pin)?;
    /// // Print.Value 推断为 Float64
    /// ```
    pub fn infer_connection(&mut self, from_pin: PinId, to_pin: PinId) -> Result<(), String> {
        let from_type = self.get_pin_type(from_pin)?.clone();
        let to_type = self.get_pin_type(to_pin)?.clone();
        
        match (&from_type.data_type, &to_type.data_type) {
            // 具体类型 -> 未知类型：推断为具体类型
            (DataType::Concrete(vtype), DataType::Unknown) => {
                self.set_pin_type(to_pin, DataType::Concrete(vtype.clone()))?;
            }
            
            // 具体类型 -> 类型变量：绑定类型变量
            (DataType::Concrete(vtype), DataType::TypeVar(var_id)) => {
                self.bind_type_var(*var_id, vtype.clone(), &to_type.constraints)?;
            }
            
            // 类型变量 -> 未知类型：传递类型变量
            (DataType::TypeVar(var_id), DataType::Unknown) => {
                self.set_pin_type(to_pin, DataType::TypeVar(*var_id))?;
            }
            
            // 类型变量 -> 类型变量：统一类型变量
            (DataType::TypeVar(var1), DataType::TypeVar(var2)) => {
                self.unify_type_vars(*var1, *var2, &from_type.constraints, &to_type.constraints)?;
            }
            
            // 类型变量 -> 具体类型：绑定类型变量
            (DataType::TypeVar(var_id), DataType::Concrete(vtype)) => {
                self.bind_type_var(*var_id, vtype.clone(), &from_type.constraints)?;
            }
            
            // 具体类型 -> 具体类型：检查兼容性
            (DataType::Concrete(from_vtype), DataType::Concrete(to_vtype)) => {
                if !self.is_compatible(from_vtype, to_vtype) {
                    return Err(format!(
                        "Type mismatch: cannot connect {} to {}",
                        from_vtype.to_string(),
                        to_vtype.to_string()
                    ));
                }
            }
            
            // 未知类型 -> 任意类型：等待推断
            (DataType::Unknown, _) => {
                // 源类型未知，暂时无法推断
            }
            
            // 联合类型：暂不支持
            (DataType::Union(_), _) | (_, DataType::Union(_)) => {
                return Err("Union types not yet supported".to_string());
            }
        }
        
        Ok(())
    }
    
    /// 绑定类型变量
    /// 
    /// 将类型变量绑定到具体类型，并检查约束
    /// 
    /// # 参数
    /// 
    /// * `var_id` - 类型变量 ID
    /// * `vtype` - 要绑定的具体类型
    /// * `constraints` - 类型约束
    fn bind_type_var(
        &mut self,
        var_id: TypeVarId,
        vtype: ValueType,
        constraints: &[TypeConstraint],
    ) -> Result<(), String> {
        // 查找类型变量的代表元素（Union-Find）
        let representative = self.find_type_var(var_id);
        
        // 检查是否已经绑定
        if let Some(existing_type) = self.type_var_bindings.get(&representative) {
            // 已经绑定，检查是否兼容
            if !self.is_compatible(&vtype, existing_type) {
                return Err(format!(
                    "Type variable {} already bound to {}, cannot bind to {}",
                    representative,
                    existing_type.to_string(),
                    vtype.to_string()
                ));
            }
            return Ok(());
        }
        
        // 检查约束
        for constraint in constraints {
            if !constraint.is_satisfied_by(&vtype) {
                return Err(format!(
                    "Type {} does not satisfy constraint: {}",
                    vtype.to_string(),
                    constraint.description()
                ));
            }
        }
        
        // 绑定类型变量
        self.type_var_bindings.insert(representative, vtype.clone());
        
        // 更新所有使用该类型变量的 Pin
        self.propagate_type_binding(representative, vtype)?;
        
        Ok(())
    }
    
    /// 统一两个类型变量
    /// 
    /// 使用 Union-Find 算法将两个类型变量合并为一个等价类
    /// 
    /// # 参数
    /// 
    /// * `var1` - 第一个类型变量
    /// * `var2` - 第二个类型变量
    /// * `constraints1` - 第一个类型变量的约束
    /// * `constraints2` - 第二个类型变量的约束
    fn unify_type_vars(
        &mut self,
        var1: TypeVarId,
        var2: TypeVarId,
        constraints1: &[TypeConstraint],
        constraints2: &[TypeConstraint],
    ) -> Result<(), String> {
        let rep1 = self.find_type_var(var1);
        let rep2 = self.find_type_var(var2);
        
        // 如果已经在同一个等价类，无需操作
        if rep1 == rep2 {
            return Ok(());
        }
        
        // 合并约束
        let mut merged_constraints = constraints1.to_vec();
        merged_constraints.extend_from_slice(constraints2);
        
        // 如果其中一个已经绑定，将另一个也绑定到相同类型
        if let Some(vtype) = self.type_var_bindings.get(&rep1).cloned() {
            self.bind_type_var(rep2, vtype, &merged_constraints)?;
        } else if let Some(vtype) = self.type_var_bindings.get(&rep2).cloned() {
            self.bind_type_var(rep1, vtype, &merged_constraints)?;
        } else {
            // 都未绑定，合并等价类
            self.type_var_union.insert(rep2, rep1);
        }
        
        Ok(())
    }
    
    /// 查找类型变量的代表元素（Union-Find 的 Find 操作）
    fn find_type_var(&mut self, var_id: TypeVarId) -> TypeVarId {
        if let Some(&parent) = self.type_var_union.get(&var_id) {
            if parent != var_id {
                // 路径压缩
                let root = self.find_type_var(parent);
                self.type_var_union.insert(var_id, root);
                return root;
            }
        }
        var_id
    }
    
    /// 传播类型绑定
    /// 
    /// 当类型变量绑定后，更新所有使用该类型变量的 Pin
    fn propagate_type_binding(&mut self, var_id: TypeVarId, vtype: ValueType) -> Result<(), String> {
        // 先收集需要更新的 Pin ID（避免借用冲突）
        let representative = var_id;
        let pins_to_update: Vec<PinId> = self
            .pin_types
            .iter()
            .filter_map(|(pin_id, desc)| {
                if let DataType::TypeVar(v) = &desc.data_type {
                    // 使用临时变量避免在闭包中调用 self.find_type_var
                    if self.type_var_union.get(v).copied().unwrap_or(*v) == representative {
                        return Some(*pin_id);
                    }
                }
                None
            })
            .collect();
        
        // 更新所有相关 Pin
        for pin_id in pins_to_update {
            self.set_pin_type(pin_id, DataType::Concrete(vtype.clone()))?;
        }
        
        Ok(())
    }
    
    /// 解析 Pin 的最终类型
    /// 
    /// # 参数
    /// 
    /// * `pin_id` - Pin 的唯一标识
    /// 
    /// # 返回
    /// 
    /// * `Ok(ValueType)` - 解析成功，返回具体类型
    /// * `Err(String)` - 解析失败，返回错误信息
    pub fn resolve_pin_type(&mut self, pin_id: PinId) -> Result<ValueType, String> {
        let pin_type = self.get_pin_type(pin_id)?;
        
        match &pin_type.data_type {
            DataType::Concrete(vtype) => Ok(vtype.clone()),
            
            DataType::TypeVar(var_id) => {
                let representative = self.find_type_var(*var_id);
                self.type_var_bindings
                    .get(&representative)
                    .cloned()
                    .ok_or_else(|| format!("Type variable {} not bound", representative))
            }
            
            DataType::Unknown => Err("Type still unknown".to_string()),
            
            DataType::Union(_) => Err("Union types not yet supported".to_string()),
        }
    }
    
    /// 检查类型兼容性
    /// 
    /// # 参数
    /// 
    /// * `from_type` - 源类型
    /// * `to_type` - 目标类型
    /// 
    /// # 返回
    /// 
    /// * `true` - 类型兼容
    /// * `false` - 类型不兼容
    fn is_compatible(&self, from_type: &ValueType, to_type: &ValueType) -> bool {
        // Any 类型兼容所有类型
        if matches!(to_type, ValueType::Any) || matches!(from_type, ValueType::Any) {
            return true;
        }
        
        // 相同类型兼容
        if from_type == to_type {
            return true;
        }
        
        // 数字类型之间可以转换
        if matches!(from_type, ValueType::Int64 | ValueType::Float64)
            && matches!(to_type, ValueType::Int64 | ValueType::Float64)
        {
            return true;
        }
        
        // Null 可以兼容任何类型
        if matches!(from_type, ValueType::Null) || matches!(to_type, ValueType::Null) {
            return true;
        }
        
        false
    }
    
    /// 获取所有类型变量的绑定
    pub fn get_bindings(&self) -> &HashMap<TypeVarId, ValueType> {
        &self.type_var_bindings
    }
    
    /// 清空所有类型推断信息
    pub fn clear(&mut self) {
        self.type_var_bindings.clear();
        self.pin_types.clear();
        self.type_var_union.clear();
    }
    
    // ==================== 辅助方法 ====================
    
    /// 获取 Pin 的类型描述
    fn get_pin_type(&self, pin_id: PinId) -> Result<&PinTypeDesc, String> {
        self.pin_types
            .get(&pin_id)
            .ok_or_else(|| format!("Pin {:?} not registered", pin_id))
    }
    
    /// 设置 Pin 的类型
    fn set_pin_type(&mut self, pin_id: PinId, data_type: DataType) -> Result<(), String> {
        if let Some(pin_type) = self.pin_types.get_mut(&pin_id) {
            pin_type.data_type = data_type;
            Ok(())
        } else {
            Err(format!("Pin {:?} not registered", pin_id))
        }
    }
}

impl Default for TypeInferenceContext {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;
    
    #[test]
    fn test_concrete_to_unknown() {
        let mut ctx = TypeInferenceContext::new();
        
        let from_pin = Uuid::new_v4();
        let to_pin = Uuid::new_v4();
        
        ctx.register_pin(from_pin, PinTypeDesc::concrete(ValueType::Float64));
        ctx.register_pin(to_pin, PinTypeDesc::unknown());
        
        ctx.infer_connection(from_pin, to_pin).unwrap();
        
        let resolved = ctx.resolve_pin_type(to_pin).unwrap();
        assert_eq!(resolved, ValueType::Float64);
    }
    
    #[test]
    fn test_concrete_to_type_var() {
        let mut ctx = TypeInferenceContext::new();
        
        let from_pin = Uuid::new_v4();
        let to_pin = Uuid::new_v4();
        let type_var = TypeVarId::new();
        
        ctx.register_pin(from_pin, PinTypeDesc::concrete(ValueType::Int64));
        ctx.register_pin(to_pin, PinTypeDesc::type_var(type_var));
        
        ctx.infer_connection(from_pin, to_pin).unwrap();
        
        let resolved = ctx.resolve_pin_type(to_pin).unwrap();
        assert_eq!(resolved, ValueType::Int64);
    }
    
    #[test]
    fn test_type_var_unification() {
        let mut ctx = TypeInferenceContext::new();
        
        let pin_a = Uuid::new_v4();
        let pin_b = Uuid::new_v4();
        let pin_result = Uuid::new_v4();
        let pin_constant = Uuid::new_v4();
        
        let type_var = TypeVarId::new();
        
        // Add 节点：A, B, Result 共享类型变量
        ctx.register_pin(pin_a, PinTypeDesc::type_var(type_var));
        ctx.register_pin(pin_b, PinTypeDesc::type_var(type_var));
        ctx.register_pin(pin_result, PinTypeDesc::type_var(type_var));
        
        // Constant 节点
        ctx.register_pin(pin_constant, PinTypeDesc::concrete(ValueType::Float64));
        
        // 连接：Constant -> Add.A
        ctx.infer_connection(pin_constant, pin_a).unwrap();
        
        // 验证：A, B, Result 都变成 Float64
        assert_eq!(ctx.resolve_pin_type(pin_a).unwrap(), ValueType::Float64);
        assert_eq!(ctx.resolve_pin_type(pin_b).unwrap(), ValueType::Float64);
        assert_eq!(ctx.resolve_pin_type(pin_result).unwrap(), ValueType::Float64);
    }
    
    #[test]
    fn test_constraint_violation() {
        let mut ctx = TypeInferenceContext::new();
        
        let from_pin = Uuid::new_v4();
        let to_pin = Uuid::new_v4();
        let type_var = TypeVarId::new();
        
        ctx.register_pin(from_pin, PinTypeDesc::concrete(ValueType::String));
        ctx.register_pin(
            to_pin,
            PinTypeDesc::type_var_with_constraints(type_var, vec![TypeConstraint::Numeric]),
        );
        
        let result = ctx.infer_connection(from_pin, to_pin);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("does not satisfy constraint"));
    }
    
    #[test]
    fn test_type_mismatch() {
        let mut ctx = TypeInferenceContext::new();
        
        let from_pin = Uuid::new_v4();
        let to_pin = Uuid::new_v4();
        
        ctx.register_pin(from_pin, PinTypeDesc::concrete(ValueType::String));
        ctx.register_pin(to_pin, PinTypeDesc::concrete(ValueType::Boolean));
        
        let result = ctx.infer_connection(from_pin, to_pin);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Type mismatch"));
    }
    
    #[test]
    fn test_numeric_compatibility() {
        let mut ctx = TypeInferenceContext::new();
        
        let from_pin = Uuid::new_v4();
        let to_pin = Uuid::new_v4();
        
        ctx.register_pin(from_pin, PinTypeDesc::concrete(ValueType::Int64));
        ctx.register_pin(to_pin, PinTypeDesc::concrete(ValueType::Float64));
        
        // 数字类型之间应该兼容
        let result = ctx.infer_connection(from_pin, to_pin);
        assert!(result.is_ok());
    }
}
