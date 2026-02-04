use crate::executor::node::NodeId;
use crate::executor::pin::PinRole;
use crate::executor::value::DataValue;

/// Node 执行上下文
///
/// 提供语义化的 API 访问输入和输出，而不是通过 PinId/index/name。

pub trait NodeExecutionContext {
    /// 通过角色获取单个输入值
    fn get_input_by_role(&self, role: &PinRole) -> Result<DataValue, String>;

    /// 通过角色获取多个输入值（用于动态组）
    fn get_inputs_by_role(&self, role: &PinRole) -> Result<Vec<DataValue>, String>;

    /// 通过角色家族获取所有输入值（例如获取所有 Operands）
    /// pattern: 用于匹配的角色模式，例如 PinRole::Data(DataRole::Operands(0))
    fn get_inputs_by_family(&self, pattern: &PinRole) -> Result<Vec<DataValue>, String>;

    /// 通过角色设置单个输出值
    fn emit_output_by_role(&mut self, role: &PinRole, value: DataValue) -> Result<(), String>;

    /// 通过角色设置多个输出值（用于动态组）
    fn emit_outputs_by_role(
        &mut self,
        role: &PinRole,
        values: Vec<DataValue>,
    ) -> Result<(), String>;

    /// 检查输入是否已连接
    fn is_input_connected(&self, role: &PinRole) -> bool;

    /// 获取当前节点 ID
    fn node_id(&self) -> NodeId;

    /// 记录日志
    fn log(&mut self, message: String);

    /// 记录错误
    fn error(&mut self, message: String);
}

// pub struct NodeExecutionContext {
//     /// 输入值（按 Role 索引）
//     inputs_by_role: HashMap<PinRole, Vec<DataValue>>,

//     /// 输出值（按 Role 索引）
//     outputs_by_role: HashMap<PinRole, DataValue>,
// }

// impl NodeExecutionContext {
//     pub fn new() -> Self {
//         Self {
//             inputs_by_role: HashMap::new(),
//             outputs_by_role: HashMap::new(),
//         }
//     }

//     /// 添加输入值（按 Role）
//     pub fn add_input(&mut self, role: PinRole, value: DataValue) {
//         self.inputs_by_role
//             .entry(role)
//             .or_insert_with(Vec::new)
//             .push(value);
//     }

//     /// 获取单个输入值（按 Role）
//     pub fn get_input_by_role(&self, role: &PinRole) -> Option<&DataValue> {
//         self.inputs_by_role
//             .get(role)
//             .and_then(|values| values.first())
//     }

//     /// 获取多个输入值（按 Role，用于动态 Pin）
//     pub fn get_inputs_by_role(&self, role: &PinRole) -> Vec<&DataValue> {
//         self.inputs_by_role
//             .get(role)
//             .map(|values| values.iter().collect())
//             .unwrap_or_default()
//     }

//     /// 设置输出值（按 Role）
//     pub fn emit_output_by_role(&mut self, role: PinRole, value: DataValue) {
//         self.outputs_by_role.insert(role, value);
//     }

//     /// 获取输出值（按 Role）
//     pub fn get_output_by_role(&self, role: &PinRole) -> Option<&DataValue> {
//         self.outputs_by_role.get(role)
//     }

//     /// 获取所有输出
//     pub fn outputs(&self) -> &HashMap<PinRole, DataValue> {
//         &self.outputs_by_role
//     }
// }

// impl Default for NodeExecutionContext {
//     fn default() -> Self {
//         Self::new()
//     }
// }
