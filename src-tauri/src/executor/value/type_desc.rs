//! 类型描述模块
//!
//! 提供类型推断系统的核心类型定义

use super::types::ValueType;
use super::type_var::TypeVarId;
use serde::{Deserialize, Serialize};

/// 数据类型描述
/// 
/// 用于类型推断系统，区分具体类型、类型变量和未知类型
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DataType {
    /// 具体类型（已知）
    /// 
    /// 例如：Float64, String, DataFrame
    Concrete(ValueType),
    
    /// 类型变量（待推断）
    /// 
    /// 用于泛型节点，多个 Pin 可以共享同一个类型变量
    /// 例如：Add 节点的 A, B, Result 共享 TypeVar(T1)
    TypeVar(TypeVarId),
    
    /// 未知类型（尚未连接）
    /// 
    /// 用于可以接受任意类型的 Pin
    /// 例如：Print 节点的 Value input
    Unknown,
    
    /// 联合类型（可以是多种类型之一）
    /// 
    /// 例如：Union([Int64, Float64]) 表示可以是整数或浮点数
    Union(Vec<DataType>),
}

impl DataType {
    /// 创建具体类型
    pub fn concrete(vtype: ValueType) -> Self {
        DataType::Concrete(vtype)
    }
    
    /// 创建类型变量
    pub fn type_var(var_id: TypeVarId) -> Self {
        DataType::TypeVar(var_id)
    }
    
    /// 创建未知类型
    pub fn unknown() -> Self {
        DataType::Unknown
    }
    
    /// 创建联合类型
    pub fn union(types: Vec<DataType>) -> Self {
        DataType::Union(types)
    }
    
    /// 检查是否是具体类型
    pub fn is_concrete(&self) -> bool {
        matches!(self, DataType::Concrete(_))
    }
    
    /// 检查是否是类型变量
    pub fn is_type_var(&self) -> bool {
        matches!(self, DataType::TypeVar(_))
    }
    
    /// 检查是否是未知类型
    pub fn is_unknown(&self) -> bool {
        matches!(self, DataType::Unknown)
    }
    
    /// 尝试获取具体类型
    pub fn as_concrete(&self) -> Option<&ValueType> {
        match self {
            DataType::Concrete(vtype) => Some(vtype),
            _ => None,
        }
    }
    
    /// 尝试获取类型变量 ID
    pub fn as_type_var(&self) -> Option<TypeVarId> {
        match self {
            DataType::TypeVar(var_id) => Some(*var_id),
            _ => None,
        }
    }
    
    /// 转换为字符串（用于显示）
    pub fn to_string(&self) -> String {
        match self {
            DataType::Concrete(vtype) => vtype.to_string(),
            DataType::TypeVar(var_id) => format!("T{}", var_id.0),
            DataType::Unknown => "?".to_string(),
            DataType::Union(types) => {
                let type_strs: Vec<String> = types.iter().map(|t| t.to_string()).collect();
                format!("({})", type_strs.join(" | "))
            }
        }
    }
}

impl Default for DataType {
    fn default() -> Self {
        DataType::Unknown
    }
}

impl std::fmt::Display for DataType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_data_type_creation() {
        let concrete = DataType::concrete(ValueType::Float64);
        assert!(concrete.is_concrete());
        assert_eq!(concrete.as_concrete(), Some(&ValueType::Float64));
        
        let type_var = DataType::type_var(TypeVarId::new());
        assert!(type_var.is_type_var());
        
        let unknown = DataType::unknown();
        assert!(unknown.is_unknown());
    }
    
    #[test]
    fn test_data_type_display() {
        let concrete = DataType::concrete(ValueType::Float64);
        assert_eq!(concrete.to_string(), "float64");
        
        let unknown = DataType::unknown();
        assert_eq!(unknown.to_string(), "?");
    }
}
