//! 执行栈（Execution Stack）
//!
//! 管理执行帧：
//! - `frames`：所有存活帧（可执行 + 等待中的 waiter）
//! - `ready`：待执行帧的 LIFO 队列
//!
//! waiter 帧在等待子任务时 `park` 出队，但仍存活于 `frames` 中被子帧的
//! `join_target` 引用；子任务全部完成后由执行器恢复。

use super::execution_frame::{ExecutionFrame, FrameId};
use std::collections::HashMap;

/// 执行栈
pub struct ExecutionStack {
    /// 所有存活帧（按 FrameId 索引）
    frames: HashMap<FrameId, ExecutionFrame>,

    /// 待执行帧（栈顶是下一个执行的帧）
    ready: Vec<FrameId>,

    /// 下一个帧 ID
    next_frame_id: u64,
}

impl ExecutionStack {
    /// 创建新的执行栈
    pub fn new() -> Self {
        Self {
            frames: HashMap::new(),
            ready: Vec::new(),
            next_frame_id: 0,
        }
    }

    /// 生成新的帧 ID
    fn next_id(&mut self) -> FrameId {
        let id = FrameId::new(self.next_frame_id);
        self.next_frame_id += 1;
        id
    }

    /// 创建并入队新帧，返回其帧 ID
    pub fn push_ready(
        &mut self,
        node_id: crate::graph::node::NodeId,
        triggered_by: Option<crate::graph::pin::PinId>,
        join_target: Option<FrameId>,
    ) -> FrameId {
        let frame_id = self.next_id();
        let frame = ExecutionFrame::new(frame_id, node_id, triggered_by, join_target);
        self.frames.insert(frame_id, frame);
        self.ready.push(frame_id);
        frame_id
    }

    /// 弹出栈顶待执行帧（从 ready 移除，但保留在 frames 中）
    pub fn pop_ready(&mut self) -> Option<ExecutionFrame> {
        let frame_id = self.ready.pop()?;
        self.frames.get(&frame_id).cloned()
    }

    /// 将帧登记为等待中的 waiter（存入 frames，但不入 ready 队列）
    pub fn park(&mut self, frame: ExecutionFrame) {
        self.frames.insert(frame.id, frame);
    }

    /// 获取指定帧
    pub fn get(&self, frame_id: FrameId) -> Option<&ExecutionFrame> {
        self.frames.get(&frame_id)
    }

    /// 获取可变的指定帧
    pub fn get_mut(&mut self, frame_id: FrameId) -> Option<&mut ExecutionFrame> {
        self.frames.get_mut(&frame_id)
    }

    /// 从 frames 中彻底移除指定帧
    pub fn remove(&mut self, frame_id: FrameId) -> Option<ExecutionFrame> {
        self.frames.remove(&frame_id)
    }

    /// 是否还有待执行帧
    pub fn is_empty(&self) -> bool {
        self.ready.is_empty()
    }

    /// 获取栈的调试信息
    pub fn debug_info(&self) -> String {
        let mut info = format!("ExecutionStack (ready: {})\n", self.ready.len());
        for (i, &frame_id) in self.ready.iter().enumerate() {
            if let Some(frame) = self.frames.get(&frame_id) {
                info.push_str(&format!(
                    "  [{}] {:?} - {:?} - {}\n",
                    i, frame.id, frame.state, frame.debug_info
                ));
            }
        }
        info
    }
}

impl Default for ExecutionStack {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::node::NodeId;

    #[test]
    fn test_push_and_pop_ready() {
        let mut stack = ExecutionStack::new();
        assert!(stack.is_empty());

        let node_id = NodeId::new();
        let frame_id = stack.push_ready(node_id, None, None);

        assert!(!stack.is_empty());
        assert_eq!(stack.get(frame_id).unwrap().node_id, node_id);

        let popped = stack.pop_ready().unwrap();
        assert_eq!(popped.id, frame_id);
        // pop_ready 仅出 ready 队列，帧仍存活于 frames
        assert!(stack.is_empty());
        assert!(stack.get(frame_id).is_some());
    }

    #[test]
    fn test_park_and_pending_children() {
        let mut stack = ExecutionStack::new();

        let parent_node = NodeId::new();
        let child_node = NodeId::new();

        let parent_id = stack.push_ready(parent_node, None, None);
        let mut parent = stack.pop_ready().unwrap();
        parent.pending_children = 1;
        stack.park(parent);

        let child_id = stack.push_ready(child_node, None, Some(parent_id));
        assert_eq!(stack.get(child_id).unwrap().join_target, Some(parent_id));
        assert_eq!(stack.get(parent_id).unwrap().pending_children, 1);

        // parent 已 park，不在 ready 队列中
        let next = stack.pop_ready().unwrap();
        assert_eq!(next.id, child_id);
        assert!(stack.is_empty());
    }
}
