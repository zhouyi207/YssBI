//! Pin 标识符定义
//! 防止在编译器把 PinID 和别的 ID 混用

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Pin 唯一标识符
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PinId(Uuid);

impl PinId {
    /// 统一 id 生成入口
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// 占位 ID
    pub fn nil() -> Self {
        Self(Uuid::nil())
    }
}

/// 定义 PinID 的默认状态
impl Default for PinId {
    fn default() -> Self {
        Self::new()
    }
}

/// 可以正常打印显示
impl fmt::Display for PinId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// 从 UUID 显示转换为 PinID
impl From<Uuid> for PinId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

/// 从 PinId 显式转换为 UUID
impl From<PinId> for Uuid {
    fn from(id: PinId) -> Self {
        id.0
    }
}
