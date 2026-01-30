//! 类型约束模块
//!
//! 提供类型推断系统的约束支持

use super::types::ValueType;
use serde::{Deserialize, Serialize};

/// 类型约束
/// 
/// 用于限制类型变量的可能取值，提供更好的类型检查
/// 
/// # 示例
/// 
/// ```rust
/// // Add 节点：输入必须是数字
/// let constraint = TypeConstraint::Numeric;
/// 
/// // 检查类型是否满足约束
/// assert!(constraint.is_satisfied_by(&ValueType::Float64));
/// assert!(constraint.is_satisfied_by(&ValueType::Int64));
/// assert!(!constraint.is_satisfied_by(&ValueType::String));
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TypeConstraint {
    /// 必须是数字类型（Int64 或 Float64）
    /// 
    /// 用于：Add, Subtract, Multiply, Divide 等数学运算节点
    Numeric,
    
    /// 必须是可比较类型（支持 <, >, ==）
    /// 
    /// 用于：Compare, Greater, Less 等比较节点
    Comparable,
    
    /// 必须是可迭代类型（List, DataFrame, Series）
    /// 
    /// 用于：ForEach, Map, Filter 等迭代节点
    Iterable,
    
    /// 必须是可序列化类型（可以转换为 JSON）
    /// 
    /// 用于：Print, Log, Export 等输出节点
    Serializable,
    
    /// 必须是特定类型之一
    /// 
    /// 用于：需要限制类型范围的节点
    OneOf(Vec<ValueType>),
    
    /// 自定义约束（用于扩展）
    /// 
    /// 用于：特殊的业务逻辑约束
    Custom(String),
}

impl TypeConstraint {
    /// 检查类型是否满足约束
    /// 
    /// # 参数
    /// 
    /// * `vtype` - 要检查的类型
    /// 
    /// # 返回
    /// 
    /// * `true` - 类型满足约束
    /// * `false` - 类型不满足约束
    pub fn is_satisfied_by(&self, vtype: &ValueType) -> bool {
        match self {
            TypeConstraint::Numeric => {
                matches!(vtype, ValueType::Int64 | ValueType::Float64)
            }
            
            TypeConstraint::Comparable => {
                matches!(
                    vtype,
                    ValueType::Int64
                        | ValueType::Float64
                        | ValueType::String
                        | ValueType::Date
                        | ValueType::Datetime
                        | ValueType::Boolean
                )
            }
            
            TypeConstraint::Iterable => {
                matches!(
                    vtype,
                    ValueType::List(_) | ValueType::DataFrame | ValueType::Series
                )
            }
            
            TypeConstraint::Serializable => {
                // DataFrame 和 Series 不能直接序列化为 JSON
                !matches!(vtype, ValueType::DataFrame | ValueType::Series)
            }
            
            TypeConstraint::OneOf(types) => types.contains(vtype),
            
            TypeConstraint::Custom(_) => {
                // 自定义约束需要额外的验证逻辑
                // 这里默认返回 true，实际使用时需要扩展
                true
            }
        }
    }
    
    /// 获取约束的描述
    pub fn description(&self) -> String {
        match self {
            TypeConstraint::Numeric => "Must be a numeric type (Int64 or Float64)".to_string(),
            TypeConstraint::Comparable => "Must be a comparable type".to_string(),
            TypeConstraint::Iterable => "Must be an iterable type (List, DataFrame, or Series)".to_string(),
            TypeConstraint::Serializable => "Must be serializable to JSON".to_string(),
            TypeConstraint::OneOf(types) => {
                let type_names: Vec<String> = types.iter().map(|t| t.to_string()).collect();
                format!("Must be one of: {}", type_names.join(", "))
            }
            TypeConstraint::Custom(name) => format!("Custom constraint: {}", name),
        }
    }
    
    /// 获取满足约束的所有可能类型
    pub fn possible_types(&self) -> Vec<ValueType> {
        match self {
            TypeConstraint::Numeric => vec![ValueType::Int64, ValueType::Float64],
            
            TypeConstraint::Comparable => vec![
                ValueType::Int64,
                ValueType::Float64,
                ValueType::String,
                ValueType::Date,
                ValueType::Datetime,
                ValueType::Boolean,
            ],
            
            TypeConstraint::Iterable => vec![
                ValueType::List(Box::new(ValueType::Any)),
                ValueType::DataFrame,
                ValueType::Series,
            ],
            
            TypeConstraint::Serializable => vec![
                ValueType::Null,
                ValueType::Boolean,
                ValueType::Int64,
                ValueType::Float64,
                ValueType::String,
                ValueType::Date,
                ValueType::Datetime,
                ValueType::Duration,
                ValueType::List(Box::new(ValueType::Any)),
                ValueType::Struct(vec![]),
            ],
            
            TypeConstraint::OneOf(types) => types.clone(),
            
            TypeConstraint::Custom(_) => vec![ValueType::Any],
        }
    }
}

impl std::fmt::Display for TypeConstraint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.description())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_numeric_constraint() {
        let constraint = TypeConstraint::Numeric;
        
        assert!(constraint.is_satisfied_by(&ValueType::Int64));
        assert!(constraint.is_satisfied_by(&ValueType::Float64));
        assert!(!constraint.is_satisfied_by(&ValueType::String));
        assert!(!constraint.is_satisfied_by(&ValueType::Boolean));
    }
    
    #[test]
    fn test_comparable_constraint() {
        let constraint = TypeConstraint::Comparable;
        
        assert!(constraint.is_satisfied_by(&ValueType::Int64));
        assert!(constraint.is_satisfied_by(&ValueType::Float64));
        assert!(constraint.is_satisfied_by(&ValueType::String));
        assert!(constraint.is_satisfied_by(&ValueType::Date));
        assert!(constraint.is_satisfied_by(&ValueType::Boolean));
        assert!(!constraint.is_satisfied_by(&ValueType::DataFrame));
    }
    
    #[test]
    fn test_iterable_constraint() {
        let constraint = TypeConstraint::Iterable;
        
        assert!(constraint.is_satisfied_by(&ValueType::List(Box::new(ValueType::Int64))));
        assert!(constraint.is_satisfied_by(&ValueType::DataFrame));
        assert!(constraint.is_satisfied_by(&ValueType::Series));
        assert!(!constraint.is_satisfied_by(&ValueType::Int64));
        assert!(!constraint.is_satisfied_by(&ValueType::String));
    }
    
    #[test]
    fn test_serializable_constraint() {
        let constraint = TypeConstraint::Serializable;
        
        assert!(constraint.is_satisfied_by(&ValueType::Int64));
        assert!(constraint.is_satisfied_by(&ValueType::String));
        assert!(constraint.is_satisfied_by(&ValueType::Boolean));
        assert!(!constraint.is_satisfied_by(&ValueType::DataFrame));
        assert!(!constraint.is_satisfied_by(&ValueType::Series));
    }
    
    #[test]
    fn test_one_of_constraint() {
        let constraint = TypeConstraint::OneOf(vec![
            ValueType::Int64,
            ValueType::Float64,
            ValueType::String,
        ]);
        
        assert!(constraint.is_satisfied_by(&ValueType::Int64));
        assert!(constraint.is_satisfied_by(&ValueType::Float64));
        assert!(constraint.is_satisfied_by(&ValueType::String));
        assert!(!constraint.is_satisfied_by(&ValueType::Boolean));
        assert!(!constraint.is_satisfied_by(&ValueType::DataFrame));
    }
    
    #[test]
    fn test_constraint_description() {
        let numeric = TypeConstraint::Numeric;
        assert!(numeric.description().contains("numeric"));
        
        let comparable = TypeConstraint::Comparable;
        assert!(comparable.description().contains("comparable"));
    }
    
    #[test]
    fn test_possible_types() {
        let numeric = TypeConstraint::Numeric;
        let types = numeric.possible_types();
        assert_eq!(types.len(), 2);
        assert!(types.contains(&ValueType::Int64));
        assert!(types.contains(&ValueType::Float64));
    }
}
