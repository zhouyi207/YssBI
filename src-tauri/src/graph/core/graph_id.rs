//! Graph 标识符定义
//! 防止在编译器把 GraphId 和别的 ID 混用

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Pin 唯一标识符
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GraphId(Uuid);

impl GraphId {
    /// 统一 id 生成入口
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// 占位 ID
    pub fn nil() -> Self {
        Self(Uuid::nil())
    }
}

/// 定义 GraphId 的默认状态
impl Default for GraphId {
    fn default() -> Self {
        Self::new()
    }
}

/// 可以正常打印显示
impl fmt::Display for GraphId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// 从 UUID 显示转换为 GraphId
impl From<Uuid> for GraphId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

/// 从 GraphId 显式转换为 UUID
impl From<GraphId> for Uuid {
    fn from(id: GraphId) -> Self {
        id.0
    }
}
