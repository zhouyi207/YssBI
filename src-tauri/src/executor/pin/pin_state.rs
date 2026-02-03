//! Pin 状态

use serde::{Deserialize, Serialize};

/// 数据 Pin 状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataPinState {
    /// 未初始化
    Uninitialized,
    /// 已就绪（有值）
    Ready,
    /// 计算中
    Computing,
    /// 错误
    Error,
}

impl Default for DataPinState {
    fn default() -> Self {
        Self::Uninitialized
    }
}

/// 执行 Pin 状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecPinState {
    /// 空闲
    Idle,
    /// 已触发
    Triggered,
    /// 执行中
    Executing,
    /// 已完成
    Completed,
    /// 错误
    Error,
}

impl Default for ExecPinState {
    fn default() -> Self {
        Self::Idle
    }
}
