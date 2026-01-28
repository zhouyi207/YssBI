//! Node 相关类型定义

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Node 唯一标识符
pub type NodeId = Uuid;

/// 节点整体状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeState {
    /// 空闲，Pin 未就绪/可执行
    Idle,
    /// 有执行 Pin 正在运行
    Running,
    /// 所有执行 Pin 执行完成
    Completed,
    /// 任一执行 Pin 执行失败
    Failed,
    /// 依赖的外部 Pin 未就绪，被阻塞
    Blocked,
    /// 已销毁，不可再操作
    Disposed,
}

impl Default for NodeState {
    fn default() -> Self {
        NodeState::Idle
    }
}
