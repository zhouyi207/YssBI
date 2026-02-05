//! Node 处理器
//!
//! 三层 Processor 模型：
//! 1. FlowProcessor - 控制流决策（返回下一个执行的 ExecRole）
//! 2. DataEvaluator - 数据求值（计算输出数据值）
//! 3. Role → PinId 映射 - 由 Graph 层管理
//!
//! 处理器通过 Context API 访问 Pin 数据，不直接访问 PinInstance。

use super::NodeExecutionContext;
use crate::executor::execution::ExecutionEffect;
use std::sync::Arc;

pub type FlowProcessor =
    Arc<dyn Fn(&mut dyn NodeExecutionContext) -> Result<ExecutionEffect, String> + Send + Sync>;

pub type DataEvaluator =
    Arc<dyn Fn(&mut dyn NodeExecutionContext) -> Result<(), String> + Send + Sync>;

/// 节点执行模型
///
/// 根据节点拥有的 processor 类型，自动推断执行模型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeExecutionModel {
    /// 纯数据节点（只有 DataEvaluator）
    /// 例如：Add, Multiply, ToString, GetVariable
    PureData,

    /// 纯控制流节点（只有 FlowProcessor）
    /// 例如：Sequence, Branch（不输出数据）
    PureFlow,

    /// 混合节点（两者都有）
    /// 例如：Branch（输出 condition 值）、Loop（输出 index）
    Hybrid,

    /// 事件节点（都没有，或只有简单的 flow_processor）
    /// 例如：OnStart, OnTick
    Event,
}

impl NodeExecutionModel {
    /// 根据 processor 配置推断执行模型
    pub fn infer(has_flow: bool, has_data: bool) -> Self {
        match (has_flow, has_data) {
            (true, true) => Self::Hybrid,
            (true, false) => Self::PureFlow,
            (false, true) => Self::PureData,
            (false, false) => Self::Event,
        }
    }
}
