use serde::{Deserialize, Serialize};


/// Node 状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeState {
    /// 空闲
    Idle,
    /// 就绪（所有输入已准备）
    Ready,
    /// 执行中
    Executing,
    /// 已完成
    Completed,
    /// 错误
    Error,
}

impl Default for NodeState {
    fn default() -> Self {
        Self::Idle
    }
}