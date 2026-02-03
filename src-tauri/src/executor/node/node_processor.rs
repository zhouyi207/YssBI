//! Node 处理器
//!
//! 处理器通过 Context API 访问 Pin 数据，不直接访问 PinInstance。

use crate::executor::pin::PinRole;
use crate::executor::value::DataValue;
use std::collections::HashMap;

/// Node 执行上下文
///
/// 提供语义化的 API 访问输入和输出，而不是通过 PinId/index/name。
pub struct NodeExecutionContext {
    /// 输入值（按 Role 索引）
    inputs_by_role: HashMap<PinRole, Vec<DataValue>>,
    
    /// 输出值（按 Role 索引）
    outputs_by_role: HashMap<PinRole, DataValue>,
}

impl NodeExecutionContext {
    pub fn new() -> Self {
        Self {
            inputs_by_role: HashMap::new(),
            outputs_by_role: HashMap::new(),
        }
    }

    /// 添加输入值（按 Role）
    pub fn add_input(&mut self, role: PinRole, value: DataValue) {
        self.inputs_by_role
            .entry(role)
            .or_insert_with(Vec::new)
            .push(value);
    }

    /// 获取单个输入值（按 Role）
    pub fn get_input_by_role(&self, role: &PinRole) -> Option<&DataValue> {
        self.inputs_by_role
            .get(role)
            .and_then(|values| values.first())
    }

    /// 获取多个输入值（按 Role，用于动态 Pin）
    pub fn get_inputs_by_role(&self, role: &PinRole) -> Vec<&DataValue> {
        self.inputs_by_role
            .get(role)
            .map(|values| values.iter().collect())
            .unwrap_or_default()
    }

    /// 设置输出值（按 Role）
    pub fn emit_output_by_role(&mut self, role: PinRole, value: DataValue) {
        self.outputs_by_role.insert(role, value);
    }

    /// 获取输出值（按 Role）
    pub fn get_output_by_role(&self, role: &PinRole) -> Option<&DataValue> {
        self.outputs_by_role.get(role)
    }

    /// 获取所有输出
    pub fn outputs(&self) -> &HashMap<PinRole, DataValue> {
        &self.outputs_by_role
    }
}

impl Default for NodeExecutionContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Node 处理器类型
pub enum NodeProcessor {
    /// 数据处理器（纯函数，无副作用）
    Data(Box<dyn Fn(&mut NodeExecutionContext) -> Result<(), String> + Send + Sync>),
    
    /// 控制流处理器（返回要触发的输出 Exec Pin 的 Role）
    Flow(Box<dyn Fn(&mut NodeExecutionContext) -> Result<PinRole, String> + Send + Sync>),
    
    /// 混合处理器（既处理数据又控制流）
    Hybrid(
        Box<
            dyn Fn(&mut NodeExecutionContext) -> Result<Option<PinRole>, String>
                + Send
                + Sync,
        >,
    ),
}

impl Clone for NodeProcessor {
    fn clone(&self) -> Self {
        // 处理器不能真正克隆，这里返回一个占位符
        // 实际使用时应该从 NodeDefinition 获取
        match self {
            NodeProcessor::Data(_) => {
                NodeProcessor::Data(Box::new(|_| Ok(())))
            }
            NodeProcessor::Flow(_) => {
                NodeProcessor::Flow(Box::new(|_| Ok(PinRole::ExecOut)))
            }
            NodeProcessor::Hybrid(_) => {
                NodeProcessor::Hybrid(Box::new(|_| Ok(None)))
            }
        }
    }
}

impl std::fmt::Debug for NodeProcessor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeProcessor::Data(_) => write!(f, "DataProcessor"),
            NodeProcessor::Flow(_) => write!(f, "FlowProcessor"),
            NodeProcessor::Hybrid(_) => write!(f, "HybridProcessor"),
        }
    }
}
