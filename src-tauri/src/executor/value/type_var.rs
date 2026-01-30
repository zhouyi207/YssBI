//! 类型变量模块
//!
//! 提供类型推断系统的类型变量支持

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU32, Ordering};

/// 类型变量 ID
/// 
/// 用于表示待推断的类型，多个 Pin 可以共享同一个类型变量
/// 
/// # 示例
/// 
/// ```rust
/// // Add 节点的类型变量
/// let t1 = TypeVarId::new();
/// 
/// // A, B, Result 共享同一个类型变量
/// let a_type = DataType::TypeVar(t1);
/// let b_type = DataType::TypeVar(t1);
/// let result_type = DataType::TypeVar(t1);
/// 
/// // 一旦 A 的类型确定为 Float64
/// // B 和 Result 也自动变成 Float64
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TypeVarId(pub u32);

impl TypeVarId {
    /// 创建新的类型变量 ID
    /// 
    /// 使用原子计数器确保 ID 唯一性
    pub fn new() -> Self {
        static COUNTER: AtomicU32 = AtomicU32::new(1);
        TypeVarId(COUNTER.fetch_add(1, Ordering::SeqCst))
    }
    
    /// 从数字创建类型变量 ID（用于测试）
    pub fn from_u32(id: u32) -> Self {
        TypeVarId(id)
    }
    
    /// 获取 ID 数字
    pub fn as_u32(&self) -> u32 {
        self.0
    }
}

impl Default for TypeVarId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TypeVarId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "T{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_type_var_id_unique() {
        let id1 = TypeVarId::new();
        let id2 = TypeVarId::new();
        let id3 = TypeVarId::new();
        
        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
        assert_ne!(id1, id3);
    }
    
    #[test]
    fn test_type_var_id_display() {
        let id = TypeVarId::from_u32(42);
        assert_eq!(format!("{}", id), "T42");
    }
    
    #[test]
    fn test_type_var_id_from_u32() {
        let id = TypeVarId::from_u32(100);
        assert_eq!(id.as_u32(), 100);
    }
}
