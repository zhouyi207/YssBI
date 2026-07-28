//! Variable 标识符定义
//! 防止在编译器把 VariableID 和别的 ID 混用

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Variable 唯一标识符
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct VariableId(Uuid);

impl VariableId {
    /// 统一 id 生成入口
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// 占位 ID
    pub fn nil() -> Self {
        Self(Uuid::nil())
    }
}

/// 定义 VariableID 的默认状态
impl Default for VariableId {
    fn default() -> Self {
        Self::new()
    }
}

/// 可以正常打印显示
impl fmt::Display for VariableId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// 从 UUID 显示转换为 VariableID
impl From<Uuid> for VariableId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

/// 从 VariableId 显式转换为 UUID
impl From<VariableId> for Uuid {
    fn from(id: VariableId) -> Self {
        id.0
    }
}
