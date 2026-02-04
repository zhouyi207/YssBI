//! Node 处理器
//!
//! 三层 Processor 模型：
//! 1. FlowProcessor - 控制流决策（返回下一个执行的 ExecRole）
//! 2. DataEvaluator - 数据求值（计算输出数据值）
//! 3. Role → PinId 映射 - 由 Graph 层管理
//!
//! 处理器通过 Context API 访问 Pin 数据，不直接访问 PinInstance。

use super::NodeExecutionContext;
use crate::executor::pin::PinRole;
use std::sync::Arc;

/// 🧱 第一层：控制流处理器
///
/// 职责：
/// - 决定执行流向（返回哪个 ExecRole）
/// - 适用于：Branch、Sequence、Loop 等控制流节点
/// - **不处理数据计算**
///
/// 示例：
/// ```rust
/// // Branch 节点
/// |ctx| {
///     let condition = ctx.get_input_by_role(&PinRole::Data(DataRole::Condition))?;
///     if condition.as_bool()? {
///         Ok(PinRole::Exec(ExecRole::ExecTrue))
///     } else {
///         Ok(PinRole::Exec(ExecRole::ExecFalse))
///     }
/// }
/// ```
pub type FlowProcessor =
    Arc<dyn Fn(&mut dyn NodeExecutionContext) -> Result<PinRole, String> + Send + Sync>;

/// 🧱 第二层：数据求值器
///
/// 职责：
/// - 计算输出数据值
/// - 通过 DataRole 获取输入，通过 DataRole 输出结果
/// - 适用于：纯数据节点（Add、Multiply）、混合节点的数据部分
///
/// 示例：
/// ```rust
/// // Add 节点
/// |ctx| {
///     let operands = ctx.get_inputs_by_family(&PinRole::Data(DataRole::Operands(0)))?;
///     let sum = operands.iter().try_fold(DataValue::Int32(0), |acc, v| {
///         acc.add(v)
///     })?;
///     ctx.emit_output_by_role(&PinRole::Data(DataRole::Result), sum)?;
///     Ok(())
/// }
/// ```
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
