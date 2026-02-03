use std::collections::HashMap;
use crate::executor::pin::PinRole;
use crate::executor::value::DataValue;


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