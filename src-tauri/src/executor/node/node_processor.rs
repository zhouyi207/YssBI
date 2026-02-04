//! Node 处理器
//!
//! 处理器通过 Context API 访问 Pin 数据，不直接访问 PinInstance。

use super::NodeExecutionContext;
use crate::executor::pin::{ExecRole, PinRole};
use std::sync::Arc;

pub type NodeProcessor =
    Arc<dyn Fn(&mut dyn NodeExecutionContext) -> Result<PinRole, String> + Send + Sync>;

// Node 处理器类型
// pub enum NodeProcessor {
//     /// 数据处理器（纯函数，无副作用）
//     Data(Box<dyn Fn(&mut NodeExecutionContext) -> Result<(), String> + Send + Sync>),

//     /// 控制流处理器（返回要触发的输出 Exec Pin 的 Role）
//     Flow(Box<dyn Fn(&mut NodeExecutionContext) -> Result<PinRole, String> + Send + Sync>),

//     /// 混合处理器（既处理数据又控制流）
//     Hybrid(Box<dyn Fn(&mut NodeExecutionContext) -> Result<Option<PinRole>, String> + Send + Sync>),
// }

// impl Clone for NodeProcessor {
//     fn clone(&self) -> Self {
//         // 处理器不能真正克隆，这里返回一个占位符
//         // 实际使用时应该从 NodeDefinition 获取
//         match self {
//             NodeProcessor::Data(_) => NodeProcessor::Data(Box::new(|_| Ok(()))),
//             NodeProcessor::Flow(_) => NodeProcessor::Flow(Box::new(|_| Ok(PinRole::Exec(ExecRole::ExecOut)))),
//             NodeProcessor::Hybrid(_) => NodeProcessor::Hybrid(Box::new(|_| Ok(None))),
//         }
//     }
// }

// impl std::fmt::Debug for NodeProcessor {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         match self {
//             NodeProcessor::Data(_) => write!(f, "DataProcessor"),
//             NodeProcessor::Flow(_) => write!(f, "FlowProcessor"),
//             NodeProcessor::Hybrid(_) => write!(f, "HybridProcessor"),
//         }
//     }
// }
