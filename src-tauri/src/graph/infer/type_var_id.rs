//! TypeVar 标识符定义
//! 防止在编译器把 TypeVarID 和别的 ID 混用

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// TypeVar 唯一标识符
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TypeVarId(Uuid);

impl TypeVarId {
    /// 统一 id 生成入口
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// 占位 ID
    pub fn nil() -> Self {
        Self(Uuid::nil())
    }
}

/// 定义 TypeVarID 的默认状态
impl Default for TypeVarId {
    fn default() -> Self {
        Self::new()
    }
}

/// 可以正常打印显示
impl fmt::Display for TypeVarId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// 从 UUID 显示转换为 TypeVarID
impl From<Uuid> for TypeVarId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

/// 从 TypeVarId 显式转换为 UUID
impl From<TypeVarId> for Uuid {
    fn from(id: TypeVarId) -> Self {
        id.0
    }
}
