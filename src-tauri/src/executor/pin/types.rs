//! Pin 相关类型定义

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::executor::types::DataValue;

/// Pin 唯一标识符
pub type PinId = Uuid;

/// 数据 Pin 状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataPinState {
    /// 未初始化（默认状态）
    Uninitialized,
    /// 数据正在生成
    Pending,
    /// 数据就绪可读取
    Ready,
    /// 数据生成失败
    Error,
}

impl Default for DataPinState {
    fn default() -> Self {
        DataPinState::Uninitialized
    }
}

/// 执行 Pin 状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecPinState {
    /// 空闲，可触发
    Idle,
    /// 正在执行
    Running,
    /// 执行完成
    Completed,
    /// 执行失败
    Failed,
    /// 被依赖 Pin 阻塞
    Blocked,
}

impl Default for ExecPinState {
    fn default() -> Self {
        ExecPinState::Idle
    }
}

/// 数据 Pin 事件（用于事件驱动的执行流）
#[derive(Debug, Clone)]
pub enum DataPinEvent {
    /// Pin 状态变化
    StateChanged {
        pin_id: PinId,
        old_state: DataPinState,
        new_state: DataPinState,
    },
    /// Pin 数据更新
    DataUpdated {
        pin_id: PinId,
        value: DataValue,
    },
    /// Pin 连接建立
    Connected {
        from_pin: PinId,
        to_pin: PinId,
    },
    /// Pin 连接断开
    Disconnected {
        from_pin: PinId,
        to_pin: PinId,
    },
}

/// Pin 类型枚举（用于节点内查找/过滤 Pin）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinType {
    /// 输入数据 Pin
    InData,
    /// 输出数据 Pin
    OutData,
    /// 执行 Pin
    Exec,
}
