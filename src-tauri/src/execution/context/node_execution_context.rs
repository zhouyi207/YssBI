use crate::graph::infer::TypeVarId;
use crate::graph::node::NodeId;
use crate::graph::pin::PinRole;
use crate::graph::value::{DataValue, DataType};

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

    /// 获取类型变量的绑定类型
    ///
    /// 用于在运行时获取类型推断的结果
    /// 返回 None 表示类型变量未绑定
    fn get_bound_type(&self, type_var_id: TypeVarId) -> Option<DataType>;

    /// 通过角色获取 Pin 的推断类型
    ///
    /// 用于在运行时获取 Pin 的实际类型（经过类型推断后）
    fn get_pin_type_by_role(&self, role: &PinRole) -> Result<DataType, String>;

    /// 记录日志
    fn log(&mut self, message: String);

    /// 记录错误
    fn error(&mut self, message: String);
}
