//! 节点处理器类型定义
//!
//! 定义节点执行时使用的处理器函数类型。

use crate::executor::node::NodeData;
use serde_json::Value;

/// 执行上下文 trait
/// 定义处理器需要的上下文接口
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

    /// 根据条件查找节点
    fn find_node_by(&self, predicate: &dyn Fn(&NodeData) -> bool) -> Option<String>;

    /// 打开新窗口
    fn open_window(&mut self, label: String, title: String, url: String) -> Result<(), String>;
}

/// 数据节点处理器
pub type DataProcessor = fn(&mut dyn ExecutionContextTrait, &NodeData, &str) -> Value;

/// 流程节点处理器
pub type FlowProcessor = fn(&mut dyn ExecutionContextTrait, &NodeData) -> Result<String, String>;
