//! 执行帧（Execution Frame）
//!
//! 执行帧表示一个执行上下文，包含：
//! - 当前执行的节点
//! - 触发该节点的输入 Pin
//! - 父帧引用（用于嵌套控制流）
//! - 剩余的 continuation（用于 Sequence）

use crate::executor::node::NodeId;
use crate::executor::pin::{ExecRole, PinId};
use std::fmt;

/// 帧 ID（用于追踪和调试）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FrameId(pub u64);

impl FrameId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }
}

/// 执行帧状态
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameState {
    /// 准备执行
    Ready,
    /// 正在执行
    Running,
    /// 等待子流程完成
    WaitingForChild,
    /// 已完成
    Completed,
    /// 已暂停（等待外部事件）
    Suspended,
}

/// 执行帧
///
/// 表示一个节点的执行上下文
#[derive(Clone)]
pub struct ExecutionFrame {
    /// 帧 ID
    pub id: FrameId,

    /// 当前执行的节点
    pub node_id: NodeId,

    /// 触发该节点的输入 Pin（可选）
    pub triggered_by: Option<PinId>,

    /// 父帧 ID（用于嵌套控制流）
    pub parent_frame: Option<FrameId>,

    /// 帧状态
    pub state: FrameState,

    /// 剩余的 continuation（用于 Sequence）
    /// 
    /// 当节点返回 TriggerAndContinue 时，remaining 会被保存到这里
    /// 当子流程完成后，执行器会继续执行这些 continuation
    pub remaining_continuations: Vec<ExecRole>,

    /// 调试信息：帧创建时的描述
    pub debug_info: String,
}

impl ExecutionFrame {
    /// 创建新的执行帧
    pub fn new(
        id: FrameId,
        node_id: NodeId,
        triggered_by: Option<PinId>,
        parent_frame: Option<FrameId>,
    ) -> Self {
        Self {
            id,
            node_id,
            triggered_by,
            parent_frame,
            state: FrameState::Ready,
            remaining_continuations: Vec::new(),
            debug_info: format!("Frame for node {:?}", node_id),
        }
    }

    /// 创建根帧（没有父帧）
    pub fn root(id: FrameId, node_id: NodeId) -> Self {
        Self::new(id, node_id, None, None)
    }

    /// 创建子帧
    pub fn child(
        id: FrameId,
        node_id: NodeId,
        triggered_by: Option<PinId>,
        parent_id: FrameId,
    ) -> Self {
        Self::new(id, node_id, triggered_by, Some(parent_id))
    }

    /// 设置剩余的 continuation
    pub fn with_continuations(mut self, continuations: Vec<ExecRole>) -> Self {
        self.remaining_continuations = continuations;
        self
    }

    /// 设置调试信息
    pub fn with_debug_info(mut self, info: impl Into<String>) -> Self {
        self.debug_info = info.into();
        self
    }

    /// 是否有剩余的 continuation
    pub fn has_continuations(&self) -> bool {
        !self.remaining_continuations.is_empty()
    }

    /// 弹出下一个 continuation
    pub fn pop_continuation(&mut self) -> Option<ExecRole> {
        if self.remaining_continuations.is_empty() {
            None
        } else {
            Some(self.remaining_continuations.remove(0))
        }
    }

    /// 是否是根帧
    pub fn is_root(&self) -> bool {
        self.parent_frame.is_none()
    }
}

impl fmt::Debug for ExecutionFrame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExecutionFrame")
            .field("id", &self.id)
            .field("node_id", &self.node_id)
            .field("state", &self.state)
            .field("parent_frame", &self.parent_frame)
            .field("remaining_continuations", &self.remaining_continuations.len())
            .field("debug_info", &self.debug_info)
            .finish()
    }
}
