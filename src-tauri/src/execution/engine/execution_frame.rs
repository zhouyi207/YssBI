//! 执行帧（Execution Frame）
//!
//! 执行帧表示一个执行上下文，包含：
//! - 当前执行的节点
//! - 触发该节点的输入 Pin
//! - join 目标（谁 join 我这棵子树，用于嵌套控制流的汇合）
//! - 等待类型（Sequence continuation / Loop 重入）与未完成子任务计数

use crate::graph::node::NodeId;
use crate::graph::pin::{ExecRole, PinId};
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
    /// 等待子任务全部完成（Sequence continuation / Loop body）
    Waiting,
    /// 已完成
    Completed,
}

/// 等待类型
///
/// 一个 waiter 帧在 `pending_children` 归零后如何恢复。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WaitKind {
    /// Sequence：按顺序触发剩余的 Then 分支
    Continuation { remaining: Vec<ExecRole> },
    /// Loop：重跑循环节点（计数器已在节点内推进）
    LoopReentry,
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

    /// join 目标：谁负责在我这棵子树完成后被通知
    ///
    /// 唯一不变量：`frames[join_target].pending_children`
    /// == 当前存活且 `join_target` 指向它的帧数量。
    pub join_target: Option<FrameId>,

    /// 帧状态
    pub state: FrameState,

    /// 未完成的子任务数量（仅 waiter 帧有意义）
    ///
    /// spawn 下游帧时 +1，帧完成时对其 `join_target` -1；归零且处于
    /// `Waiting` 时恢复本帧。
    pub pending_children: u32,

    /// 等待类型（仅当 `state == Waiting` 时为 `Some`）
    pub wait: Option<WaitKind>,

    /// 调试信息：帧创建时的描述
    pub debug_info: String,
}

impl ExecutionFrame {
    /// 创建新的执行帧
    pub fn new(
        id: FrameId,
        node_id: NodeId,
        triggered_by: Option<PinId>,
        join_target: Option<FrameId>,
    ) -> Self {
        Self {
            id,
            node_id,
            triggered_by,
            join_target,
            state: FrameState::Ready,
            pending_children: 0,
            wait: None,
            debug_info: format!("Frame for node {:?}", node_id),
        }
    }
}

impl fmt::Debug for ExecutionFrame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExecutionFrame")
            .field("id", &self.id)
            .field("node_id", &self.node_id)
            .field("state", &self.state)
            .field("join_target", &self.join_target)
            .field("pending_children", &self.pending_children)
            .field("wait", &self.wait)
            .field("debug_info", &self.debug_info)
            .finish()
    }
}
