//! Node 标识符定义
//! 防止在编译器把 NodeId 和别的 ID 混用

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Pin 唯一标识符
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(Uuid);

impl NodeId {
    /// 统一 id 生成入口
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// 占位 ID
    pub fn nil() -> Self {
        Self(Uuid::nil())
    }
}

/// 定义 NodeId 的默认状态
impl Default for NodeId {
    fn default() -> Self {
        Self::new()
    }
}

/// 可以正常打印显示
impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// 从 UUID 显示转换为 NodeId
impl From<Uuid> for NodeId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

/// 从 NodeId 显式转换为 UUID
impl From<NodeId> for Uuid {
    fn from(id: NodeId) -> Self {
        id.0
    }
}
