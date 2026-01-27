//! 节点处理器类型定义
//!
//! 定义节点执行时使用的处理器函数类型。

use super::types::NodeData;
use serde_json::Value;

// 前向声明，实际类型在 executor 模块中
// 这里使用 trait object 来避免循环依赖

/// 执行上下文 trait
/// 定义处理器需要的上下文接口
///
/// 注意：所有方法必须是 dyn-compatible（不能有泛型参数）
pub trait ExecutionContextTrait {
    /// 获取针脚的值
    fn get_pin_value(&mut self, pin_id: &str) -> Value;

    /// 获取变量值
    fn get_variable(&self, var_id: &str) -> Option<&Value>;

    /// 设置变量值
    fn set_variable(&mut self, var_id: &str, value: Value) -> bool;

    /// 添加日志
    fn log(&mut self, message: String);

    /// 执行子流程
    fn run_flow(&mut self, node_id: &str, output_pin: &str) -> Result<(), String>;

    /// 压入调用栈
    fn push_call_stack(&mut self, node_id: String);

    /// 弹出调用栈
    fn pop_call_stack(&mut self);

    /// 获取调用栈顶部节点 ID
    fn get_call_stack_top(&self) -> Option<&String>;

    /// 根据条件查找节点（使用 trait object 而非泛型以保持 dyn-compatible）
    fn find_node_by(&self, predicate: &dyn Fn(&NodeData) -> bool) -> Option<String>;

    /// 打开新窗口（用于 plot 等可视化节点）
    fn open_window(&mut self, label: String, title: String, url: String) -> Result<(), String>;
}

/// 数据节点处理器
/// 输入: (上下文, 节点数据, 请求的针脚ID) -> 返回值
pub type DataProcessor = fn(&mut dyn ExecutionContextTrait, &NodeData, &str) -> Value;

/// 流程节点处理器
/// 输入: (上下文, 节点数据) -> 返回下一步要执行的输出针脚名称
pub type FlowProcessor = fn(&mut dyn ExecutionContextTrait, &NodeData) -> Result<String, String>;
