//! Pin 行为抽象

use std::any::Any;
use std::fmt::Debug;
use crate::executor::error::{ExecutionResult, NodeResult};
use crate::executor::types::DataValue;
use crate::executor::node::NodeId;
use super::types::{DataPinEvent, DataPinState, ExecPinState, PinId};

/// 所有 Pin 的基础行为
pub trait BasePin: Debug + Send + Sync + 'static {
    /// 获取 Pin 的唯一 ID
    fn id(&self) -> PinId;

    /// 获取所属节点的 ID
    fn node_id(&self) -> NodeId;

    /// 获取 Pin 的名称
    fn name(&self) -> &str;

    /// 向下转型（用于动态类型判断）
    fn as_any(&self) -> &dyn Any;

    /// 可变向下转型
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// 数据 Pin：负责数据的输入输出
pub trait DataPin: BasePin {
    /// 获取当前数据值
    fn value(&self) -> DataValue;

    /// 设置数据值
    fn set_value(&mut self, value: DataValue) -> NodeResult<()>;

    /// 获取 Pin 状态
    fn state(&self) -> DataPinState;

    /// 设置 Pin 状态
    fn set_state(&mut self, state: DataPinState);

    /// 获取数据类型名称
    fn data_type(&self) -> &str;

    /// 注册事件监听器
    fn subscribe(&mut self, callback: Box<dyn Fn(DataPinEvent) + Send + Sync + 'static>);
}

/// 输入数据 Pin：只能接收数据
pub trait InDataPin: DataPin {
    /// 连接到上游输出 Pin
    fn link_to(&mut self, out_pin_id: PinId) -> NodeResult<()>;

    /// 断开上游连接
    fn unlink(&mut self) -> NodeResult<()>;

    /// 获取上游 Pin ID
    fn upstream(&self) -> Option<PinId>;

    /// 从上游读取数据（如果已连接）
    fn read_from_upstream(&mut self) -> NodeResult<DataValue>;
}

/// 输出数据 Pin：只能输出数据
pub trait OutDataPin: DataPin {
    /// 写入数据并更新状态为 Ready
    fn write(&mut self, data: DataValue) -> NodeResult<()>;

    /// 置为 Error 状态
    fn set_error(&mut self, message: String);

    /// 重置数据和状态
    fn reset(&mut self);

    /// 获取所有下游 Pin ID
    fn downstream(&self) -> Vec<PinId>;

    /// 添加下游连接
    fn add_downstream(&mut self, in_pin_id: PinId) -> NodeResult<()>;

    /// 移除下游连接
    fn remove_downstream(&mut self, in_pin_id: PinId) -> NodeResult<()>;
}

/// 执行 Pin：负责流程控制
pub trait ExecPin: BasePin {
    /// 触发执行
    fn trigger(&mut self) -> ExecutionResult<()>;

    /// 获取执行状态
    fn state(&self) -> ExecPinState;

    /// 设置执行状态
    fn set_state(&mut self, state: ExecPinState);

    /// 添加依赖的数据 Pin
    fn add_dependency(&mut self, pin_id: PinId) -> NodeResult<()>;

    /// 移除依赖
    fn remove_dependency(&mut self, pin_id: PinId) -> NodeResult<()>;

    /// 检查所有依赖是否就绪
    fn check_dependencies_ready(&self) -> bool;

    /// 连接到下游执行 Pin
    fn connect_to(&mut self, next_pin_id: PinId) -> NodeResult<()>;

    /// 获取下游执行 Pin
    fn next(&self) -> Option<PinId>;
}
