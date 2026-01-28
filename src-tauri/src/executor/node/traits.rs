//! Node 行为抽象

use std::fmt::Debug;
use std::sync::Arc;

use crate::executor::error::{ExecutionResult, NodeResult};
use crate::executor::pin::{InDataPin, OutDataPin, ExecPin};
use super::types::{NodeId, NodeState};

/// 节点接口
pub trait Node: Debug + Send + Sync + 'static {
    /// 获取节点 ID
    fn id(&self) -> NodeId;

    /// 获取节点名称
    fn name(&self) -> &str;

    /// 设置节点名称
    fn set_name(&mut self, name: String);

    /// 获取节点状态
    fn state(&self) -> NodeState;

    /// 执行节点
    fn execute(&mut self) -> ExecutionResult<()>;

    /// 获取所有输入数据 Pin
    fn inputs(&self) -> Vec<Arc<dyn InDataPin>>;

    /// 获取所有输出数据 Pin
    fn outputs(&self) -> Vec<Arc<dyn OutDataPin>>;

    /// 获取所有执行 Pin
    fn exec_pins(&self) -> Vec<Arc<dyn ExecPin>>;

    /// 根据名称查找输入 Pin
    fn get_input(&self, name: &str) -> Option<Arc<dyn InDataPin>>;

    /// 根据名称查找输出 Pin
    fn get_output(&self, name: &str) -> Option<Arc<dyn OutDataPin>>;

    /// 根据名称查找执行 Pin
    fn get_exec_pin(&self, name: &str) -> Option<Arc<dyn ExecPin>>;

    /// 重置节点
    fn reset(&mut self) -> NodeResult<()>;

    /// 销毁节点
    fn dispose(&mut self) -> NodeResult<()>;
}
