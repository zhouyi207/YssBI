//! 执行栈（Execution Stack）
//!
//! 管理执行帧的栈结构，支持：
//! - 压栈/出栈
//! - 父子帧关系追踪
//! - 子流程完成检测

use super::execution_frame::{ExecutionFrame, FrameId};
use std::collections::HashMap;

/// 执行栈
///
/// 管理所有执行帧，支持嵌套控制流
pub struct ExecutionStack {
    /// 所有帧（按 FrameId 索引）
    frames: HashMap<FrameId, ExecutionFrame>,

    /// 执行栈（栈顶是当前执行的帧）
    stack: Vec<FrameId>,

    /// 下一个帧 ID
    next_frame_id: u64,
}

impl ExecutionStack {
    /// 创建新的执行栈
    pub fn new() -> Self {
        Self {
            frames: HashMap::new(),
            stack: Vec::new(),
            next_frame_id: 0,
        }
    }

    /// 生成新的帧 ID
    fn next_id(&mut self) -> FrameId {
        let id = FrameId::new(self.next_frame_id);
        self.next_frame_id += 1;
        id
    }

    /// 压入新帧
    pub fn push(&mut self, frame: ExecutionFrame) {
        let frame_id = frame.id;
        self.frames.insert(frame_id, frame);
        self.stack.push(frame_id);
    }

    /// 创建并压入新帧
    pub fn push_new(
        &mut self,
        node_id: crate::executor::node::NodeId,
        triggered_by: Option<crate::executor::pin::PinId>,
        parent_frame: Option<FrameId>,
    ) -> FrameId {
        let frame_id = self.next_id();
        let frame = ExecutionFrame::new(frame_id, node_id, triggered_by, parent_frame);
        self.push(frame);
        frame_id
    }

    /// 弹出栈顶帧
    pub fn pop(&mut self) -> Option<ExecutionFrame> {
        let frame_id = self.stack.pop()?;
        self.frames.remove(&frame_id)
    }

    /// 查看栈顶帧（不弹出）
    pub fn peek(&self) -> Option<&ExecutionFrame> {
        let frame_id = self.stack.last()?;
        self.frames.get(frame_id)
    }

    /// 获取可变的栈顶帧
    pub fn peek_mut(&mut self) -> Option<&mut ExecutionFrame> {
        let frame_id = *self.stack.last()?;
        self.frames.get_mut(&frame_id)
    }

    /// 获取指定帧
    pub fn get_frame(&self, frame_id: FrameId) -> Option<&ExecutionFrame> {
        self.frames.get(&frame_id)
    }

    /// 获取可变的指定帧
    pub fn get_frame_mut(&mut self, frame_id: FrameId) -> Option<&mut ExecutionFrame> {
        self.frames.get_mut(&frame_id)
    }

    /// 栈是否为空
    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }

    /// 栈大小
    pub fn len(&self) -> usize {
        self.stack.len()
    }

    /// 检查是否有子流程正在执行
    ///
    /// 当一个帧处于 WaitingForChild 状态时，说明它有子流程正在执行
    pub fn has_active_children(&self, parent_id: FrameId) -> bool {
        self.stack.iter().any(|&frame_id| {
            if let Some(frame) = self.frames.get(&frame_id) {
                frame.parent_frame == Some(parent_id)
            } else {
                false
            }
        })
    }

    /// 查找父帧
    pub fn find_parent(&self, frame_id: FrameId) -> Option<&ExecutionFrame> {
        let frame = self.frames.get(&frame_id)?;
        let parent_id = frame.parent_frame?;
        self.frames.get(&parent_id)
    }

    /// 查找可变的父帧
    pub fn find_parent_mut(&mut self, frame_id: FrameId) -> Option<&mut ExecutionFrame> {
        let parent_id = self.frames.get(&frame_id)?.parent_frame?;
        self.frames.get_mut(&parent_id)
    }

    /// 清空栈
    pub fn clear(&mut self) {
        self.frames.clear();
        self.stack.clear();
    }

    /// 获取栈的调试信息
    pub fn debug_info(&self) -> String {
        let mut info = format!("ExecutionStack (depth: {})\n", self.stack.len());
        for (i, &frame_id) in self.stack.iter().enumerate() {
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
    use crate::executor::node::NodeId;

    #[test]
    fn test_basic_stack_operations() {
        let mut stack = ExecutionStack::new();
        assert!(stack.is_empty());

        let node_id = NodeId::new();
        let frame_id = stack.push_new(node_id, None, None);

        assert_eq!(stack.len(), 1);
        assert!(!stack.is_empty());

        let frame = stack.peek().unwrap();
        assert_eq!(frame.id, frame_id);
        assert_eq!(frame.node_id, node_id);

        let popped = stack.pop().unwrap();
        assert_eq!(popped.id, frame_id);
        assert!(stack.is_empty());
    }

    #[test]
    fn test_parent_child_relationship() {
        let mut stack = ExecutionStack::new();

        let parent_node = NodeId::new();
        let child_node = NodeId::new();

        let parent_id = stack.push_new(parent_node, None, None);
        let child_id = stack.push_new(child_node, None, Some(parent_id));

        assert!(stack.has_active_children(parent_id));

        let child_frame = stack.get_frame(child_id).unwrap();
        assert_eq!(child_frame.parent_frame, Some(parent_id));

        let parent_frame = stack.find_parent(child_id).unwrap();
        assert_eq!(parent_frame.id, parent_id);
    }
}
