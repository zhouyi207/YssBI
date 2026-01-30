//! Pin 类型描述模块
//!
//! 提供 Pin 的完整类型信息，包括类型、约束和属性

use super::type_constraint::TypeConstraint;
use super::type_desc::DataType;
use super::type_var::TypeVarId;
use super::types::ValueType;
use serde::{Deserialize, Serialize};

/// Pin 类型描述
/// 
/// 包含 Pin 的完整类型信息：类型、约束、是否可选、是否数组
/// 
/// # 示例
/// 
/// ```rust
/// // Print 节点的 Value input：接受任意类型
/// let print_value = PinTypeDesc::unknown();
/// 
/// // Add 节点的 A input：类型变量 + 数字约束
/// let type_var = TypeVarId::new();
/// let add_a = PinTypeDesc::type_var_with_constraints(
///     type_var,
///     vec![TypeConstraint::Numeric]
/// );
/// 
/// // Constant 节点的 output：具体类型
/// let constant_output = PinTypeDesc::concrete(ValueType::Float64);
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PinTypeDesc {
    /// 类型描述
    pub data_type: DataType,
    
    /// 类型约束
    pub constraints: Vec<TypeConstraint>,
    
    /// 是否可选（允许 Null）
    pub optional: bool,
    
    /// 是否是数组
    pub is_array: bool,
}

impl PinTypeDesc {
    /// 创建具体类型的 Pin
    /// 
    /// # 示例
    /// 
    /// ```rust
    /// let pin = PinTypeDesc::concrete(ValueType::Float64);
    /// ```
    pub fn concrete(vtype: ValueType) -> Self {
        Self {
            data_type: DataType::Concrete(vtype),
            constraints: vec![],
            optional: false,
            is_array: false,
        }
    }
    
    /// 创建类型变量的 Pin
    /// 
    /// # 示例
    /// 
    /// ```rust
    /// let type_var = TypeVarId::new();
    /// let pin = PinTypeDesc::type_var(type_var);
    /// ```
    pub fn type_var(var_id: TypeVarId) -> Self {
        Self {
            data_type: DataType::TypeVar(var_id),
            constraints: vec![],
            optional: false,
            is_array: false,
        }
    }
    
    /// 创建带约束的类型变量 Pin
    /// 
    /// # 示例
    /// 
    /// ```rust
    /// let type_var = TypeVarId::new();
    /// let pin = PinTypeDesc::type_var_with_constraints(
    ///     type_var,
    ///     vec![TypeConstraint::Numeric]
    /// );
    /// ```
    pub fn type_var_with_constraints(var_id: TypeVarId, constraints: Vec<TypeConstraint>) -> Self {
        Self {
            data_type: DataType::TypeVar(var_id),
            constraints,
            optional: false,
            is_array: false,
        }
    }
    
    /// 创建未知类型的 Pin（如 Print 的 input）
    /// 
    /// # 示例
    /// 
    /// ```rust
    /// let pin = PinTypeDesc::unknown();
    /// ```
    pub fn unknown() -> Self {
        Self {
            data_type: DataType::Unknown,
            constraints: vec![],
            optional: false,
            is_array: false,
        }
    }
    
    /// 创建 Any 类型的 Pin（接受任意类型）
    /// 
    /// # 示例
    /// 
    /// ```rust
    /// let pin = PinTypeDesc::any();
    /// ```
    pub fn any() -> Self {
        Self {
            data_type: DataType::Concrete(ValueType::Any),
            constraints: vec![],
            optional: false,
            is_array: false,
        }
    }
    
    /// 设置为可选（允许 Null）
    pub fn optional(mut self) -> Self {
        self.optional = true;
        self
    }
    
    /// 设置为数组
    pub fn array(mut self) -> Self {
        self.is_array = true;
        self
    }
    
    /// 添加约束
    pub fn with_constraint(mut self, constraint: TypeConstraint) -> Self {
        self.constraints.push(constraint);
        self
    }
    
    /// 添加多个约束
    pub fn with_constraints(mut self, constraints: Vec<TypeConstraint>) -> Self {
        self.constraints.extend(constraints);
        self
    }
    
    /// 检查类型是否满足所有约束
    pub fn satisfies_constraints(&self, vtype: &ValueType) -> bool {
        self.constraints.iter().all(|c| c.is_satisfied_by(vtype))
    }
    
    /// 从旧的 ValueType 创建（向后兼容）
    pub fn from_value_type(vtype: ValueType) -> Self {
        Self::concrete(vtype)
    }
    
    /// 从字符串类型名称创建 PinTypeDesc
    /// 
    /// 用于从前端传来的类型字符串创建类型描述
    /// 
    /// # 示例
    /// 
    /// ```rust
    /// let pin = PinTypeDesc::from_string("float64");  // Concrete(Float64)
    /// let pin = PinTypeDesc::from_string("any");      // Unknown
    /// let pin = PinTypeDesc::from_string("object");   // Unknown
    /// ```
    pub fn from_string(type_str: &str) -> Self {
        match type_str {
            // Unknown 类型
            "any" | "object" | "unknown" => Self::unknown(),
            
            // 具体类型
            "float64" | "float" | "float32" => Self::concrete(ValueType::Float64),
            "int64" | "int" | "int32" | "int16" | "int8" => Self::concrete(ValueType::Int64),
            "uint64" | "uint32" | "uint16" | "uint8" => Self::concrete(ValueType::Int64),
            "string" => Self::concrete(ValueType::String),
            "bool" | "boolean" => Self::concrete(ValueType::Boolean),
            "date" => Self::concrete(ValueType::Date),
            "datetime" => Self::concrete(ValueType::Datetime),
            "duration" => Self::concrete(ValueType::Duration),
            "dataframe" => Self::concrete(ValueType::DataFrame),
            "series" => Self::concrete(ValueType::Series),
            "array" => Self::concrete(ValueType::List(Box::new(ValueType::Any))),
            "exec" => Self::concrete(ValueType::Any), // exec pins 不参与类型推断
            
            // 未知类型，默认为 Unknown
            _ => Self::unknown(),
        }
    }
    
    /// 转换为旧的 ValueType（向后兼容）
    pub fn to_value_type(&self) -> ValueType {
        match &self.data_type {
            DataType::Concrete(vtype) => vtype.clone(),
            DataType::Unknown => ValueType::Any,
            DataType::TypeVar(_) => ValueType::Any,
            DataType::Union(_) => ValueType::Any,
        }
    }
    
    /// 获取类型描述字符串
    pub fn type_string(&self) -> String {
        let base_type = self.data_type.to_string();
        
        let mut parts = vec![base_type];
        
        if self.is_array {
            parts.push("[]".to_string());
        }
        
        if self.optional {
            parts.push("?".to_string());
        }
        
        if !self.constraints.is_empty() {
            let constraint_strs: Vec<String> = self
                .constraints
                .iter()
                .map(|c| format!("{:?}", c))
                .collect();
            parts.push(format!("({})", constraint_strs.join(", ")));
        }
        
        parts.join(" ")
    }
}

impl Default for PinTypeDesc {
    fn default() -> Self {
        Self::unknown()
    }
}

impl std::fmt::Display for PinTypeDesc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.type_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_concrete_pin() {
        let pin = PinTypeDesc::concrete(ValueType::Float64);
        assert!(pin.data_type.is_concrete());
        assert_eq!(pin.to_value_type(), ValueType::Float64);
    }
    
    #[test]
    fn test_type_var_pin() {
        let type_var = TypeVarId::new();
        let pin = PinTypeDesc::type_var(type_var);
        assert!(pin.data_type.is_type_var());
        assert_eq!(pin.to_value_type(), ValueType::Any);
    }
    
    #[test]
    fn test_unknown_pin() {
        let pin = PinTypeDesc::unknown();
        assert!(pin.data_type.is_unknown());
        assert_eq!(pin.to_value_type(), ValueType::Any);
    }
    
    #[test]
    fn test_pin_with_constraints() {
        let type_var = TypeVarId::new();
        let pin = PinTypeDesc::type_var_with_constraints(
            type_var,
            vec![TypeConstraint::Numeric]
        );
        
        assert!(pin.satisfies_constraints(&ValueType::Float64));
        assert!(pin.satisfies_constraints(&ValueType::Int64));
        assert!(!pin.satisfies_constraints(&ValueType::String));
    }
    
    #[test]
    fn test_optional_and_array() {
        let pin = PinTypeDesc::concrete(ValueType::Int64)
            .optional()
            .array();
        
        assert!(pin.optional);
        assert!(pin.is_array);
    }
    
    #[test]
    fn test_type_string() {
        let pin = PinTypeDesc::concrete(ValueType::Float64);
        assert!(pin.type_string().contains("float64"));
        
        let array_pin = PinTypeDesc::concrete(ValueType::Int64).array();
        assert!(array_pin.type_string().contains("[]"));
        
        let optional_pin = PinTypeDesc::concrete(ValueType::String).optional();
        assert!(optional_pin.type_string().contains("?"));
    }
    
    #[test]
    fn test_from_value_type() {
        let pin = PinTypeDesc::from_value_type(ValueType::Boolean);
        assert_eq!(pin.to_value_type(), ValueType::Boolean);
    }
}
