//! 执行器错误类型定义
//!
//! 使用 thiserror 定义结构化的错误类型。

use thiserror::Error;
use crate::executor::node::NodeId;
use crate::executor::pin::{PinId, DataPinState, ExecPinState};

/// 节点操作的自定义错误类型
#[derive(Error, Debug)]
pub enum NodeError {
    /// 节点已销毁，无法执行操作
    #[error("节点已销毁，无法执行操作：NodeId={0}")]
    NodeDisposed(NodeId),

    /// Pin 不存在
    #[error("Pin 不存在：PinId={0}")]
    PinNotFound(PinId),

    /// Pin 类型不匹配
    #[error("Pin 类型不匹配：期望 {expected}，实际 {actual}")]
    PinTypeMismatch {
        expected: String,
        actual: String,
    },

    /// 数据 Pin 链接失败：类型不匹配
    #[error("数据 Pin 链接失败：类型不匹配 InPin={in_pin}, OutPin={out_pin}")]
    DataPinLinkTypeMismatch {
        in_pin: PinId,
        out_pin: PinId,
    },

    /// 执行 Pin 触发失败：状态不正确
    #[error("执行 Pin 触发失败：当前状态为 {state:?}，无法执行")]
    ExecPinTriggerFailed {
        state: ExecPinState,
    },

    /// 互斥锁获取失败
    #[error("互斥锁获取失败：{resource}")]
    LockPoisoned {
        resource: &'static str,
    },

    /// 节点不存在
    #[error("节点不存在：NodeId={0}")]
    NodeNotFound(NodeId),

    /// 通用错误
    #[error("{0}")]
    Generic(String),

    /// 连接错误包装
    #[error(transparent)]
    Connection(#[from] ConnectionError),
}

/// 连接错误类型
#[derive(Error, Debug)]
pub enum ConnectionError {
    /// 类型不兼容
    #[error("连接失败：Pin 类型不兼容 (from={from_type}, to={to_type})")]
    TypeMismatch {
        from_type: String,
        to_type: String,
    },

    /// 检测到循环依赖
    #[error("连接失败：检测到循环依赖 (from_node={from_node}, to_node={to_node})")]
    CycleDetected {
        from_node: NodeId,
        to_node: NodeId,
    },

    /// Pin 不存在
    #[error("连接失败：Pin 不存在 (pin_id={0})")]
    PinNotFound(PinId),

    /// 节点不存在
    #[error("连接失败：节点不存在 (node_id={0})")]
    NodeNotFound(NodeId),

    /// 连接已存在
    #[error("连接已存在：from={from_pin}, to={to_pin}")]
    AlreadyConnected {
        from_pin: PinId,
        to_pin: PinId,
    },

    /// 无效的连接方向
    #[error("连接失败：无效的连接方向（不能将输入连接到输入，或输出连接到输出）")]
    InvalidDirection,

    /// 通用错误
    #[error("{0}")]
    Generic(String),
}

/// 执行错误类型
#[derive(Error, Debug)]
pub enum ExecutionError {
    /// 节点执行失败
    #[error("节点执行失败：NodeId={node_id}, 原因：{reason}")]
    NodeExecutionFailed {
        node_id: NodeId,
        reason: String,
    },

    /// 依赖 Pin 未就绪
    #[error("执行失败：依赖 Pin 未就绪 (pin_id={pin_id}, state={state:?})")]
    DependencyNotReady {
        pin_id: PinId,
        state: DataPinState,
    },

    /// 执行超时
    #[error("执行超时：NodeId={0}")]
    Timeout(NodeId),

    /// 检测到循环依赖
    #[error("执行失败：检测到循环依赖，无法执行拓扑排序：{message}")]
    CycleDetected {
        message: String,
    },

    /// 通用错误
    #[error("{0}")]
    Generic(String),
}

/// 简化 Result 类型：节点操作的返回值统一用这个
pub type NodeResult<T = ()> = Result<T, NodeError>;

/// 简化 Result 类型：连接操作的返回值
pub type ConnectionResult<T = ()> = Result<T, ConnectionError>;

/// 简化 Result 类型：执行操作的返回值
pub type ExecutionResult<T = ()> = Result<T, ExecutionError>;
