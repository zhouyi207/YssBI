use crate::execution::WindowDataSource;
use crate::graph::infer::TypeVarId;
use crate::graph::node::NodeInstanceParams;
use crate::graph::pin::{ExecRole, PinRole};
use crate::graph::value::{DataType, DataValue};
use polars::prelude::{DataFrame, Series};
use std::any::Any;
use std::sync::Arc;

/// Node 执行上下文
///
/// 提供语义化的 API 访问输入和输出，而不是通过 PinId/index/name。
pub trait NodeExecutionContextTrait {
    /// 通过角色获取单个输入值
    fn get_input_by_role(&self, role: &PinRole) -> Result<DataValue, String>;

    /// 通过角色获取多个输入值（用于动态组）
    fn get_inputs_by_role(&self, role: &PinRole) -> Result<Vec<DataValue>, String>;

    /// 通过角色家族获取所有输入值（例如获取所有 Operands）。
    /// 无匹配 pin 时返回空 `Vec`（与「未连接任何该族输入」一致）；需要至少一个的节点应自行检查。
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

    /// 获取类型变量的绑定类型
    ///
    /// 用于在运行时获取类型推断的结果
    /// 返回 None 表示类型变量未绑定
    fn get_bound_type(&self, type_var_id: TypeVarId) -> Option<DataType>;

    /// 通过角色获取 Pin 的推断类型
    ///
    /// 用于在运行时获取 Pin 的实际类型（经过类型推断后）
    fn get_pin_type_by_role(&self, role: &PinRole) -> Result<DataType, String>;

    /// 通过角色获取 Pin 的已解析值（含 user_value、default 等）
    ///
    /// 用于常数节点等仅输出、无输入的节点，获取输出 pin 的 user_value 或默认值
    fn get_resolved_value_by_role(&self, role: &PinRole) -> Result<DataValue, String>;

    /// 获取当前节点所有 exec step 输出（如 Sequence 的 Then pins），按步骤索引排序
    fn get_exec_step_outputs(&self) -> Vec<ExecRole>;

    // ====================================================================
    // 节点实例参数
    // ====================================================================

    /// 获取当前节点的实例参数（variable_id、dataframe_id 等）
    fn get_instance_params(&self) -> NodeInstanceParams;

    // ====================================================================
    // 数据缓存操作（DataFrame / Series / 变量）
    // ====================================================================

    /// 按 ID 获取 DataFrame（先查执行缓存，再查原始数据库）
    fn get_dataframe(&mut self, id: &str) -> Result<Arc<DataFrame>, String>;

    /// 列出数据库/中间 DataFrame 的列名
    fn list_database_columns(&mut self, db_id: &str) -> Result<Vec<String>, String>;

    /// 按列加载 Series（DuckDB 列裁剪；结果缓存在执行 store）
    fn load_database_series(&mut self, db_id: &str, column: &str) -> Result<Series, String>;

    /// 存入中间 DataFrame，返回引用 ID
    fn put_dataframe(&mut self, df: DataFrame) -> Result<String, String>;

    /// 按 ID 获取 Series
    fn get_series(&self, id: &str) -> Result<Series, String>;

    /// 存入中间 Series，返回引用 ID
    fn put_series(&mut self, s: Series) -> Result<String, String>;

    /// 读取变量值
    fn get_variable_value(&self, variable_id: &str) -> Result<DataValue, String>;

    /// 写入变量值
    fn set_variable_value(&mut self, variable_id: &str, value: DataValue) -> Result<(), String>;

    // ====================================================================
    // 通用句柄存储（Struct 类型）
    // ====================================================================

    /// 存入不透明对象，返回句柄 ID
    fn put_handle(&mut self, value: Box<dyn Any + Send + Sync>) -> String;

    /// 按 ID 获取句柄（返回 Arc，可安全跨 Mutex 传递）
    fn get_handle(&self, id: &str) -> Result<Arc<dyn Any + Send + Sync>, String>;

    // ====================================================================
    // 窗口操作
    // ====================================================================

    /// 请求前端打开一个展示窗口
    fn open_window(&mut self, window_type: String, data: String);

    /// 打开带后端 source 的窗口（metadata JSON + typed source）。
    fn open_source_window(&mut self, window_type: String, data: String, source: WindowDataSource);

    // ====================================================================
    // 日志
    // ====================================================================

    /// 记录日志
    fn log(&mut self, message: String);

    /// 记录错误
    fn error(&mut self, message: String);
}
